use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use which::which;
use crate::config::TargetConfig;

pub struct Transport {
    target: TargetConfig,
}

impl Transport {
    pub fn new(target: TargetConfig) -> Self {
        Self { target }
    }

    /// Returns extra SSH CLI args for port and identity file.
    fn ssh_extra_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(port) = self.target.port {
            args.push("-p".to_string());
            args.push(port.to_string());
        }
        if let Some(ref key) = self.target.ssh_key {
            args.push("-i".to_string());
            args.push(key.clone());
        }
        args
    }

    /// Builds the rsync `-e` SSH transport string when port or key is set.
    fn rsync_ssh_transport(&self) -> Option<String> {
        if self.target.port.is_none() && self.target.ssh_key.is_none() {
            return None;
        }
        let mut parts = vec!["ssh".to_string(), "-o".to_string(), "LogLevel=ERROR".to_string()];
        if let Some(port) = self.target.port {
            parts.push("-p".to_string());
            parts.push(port.to_string());
        }
        if let Some(ref key) = self.target.ssh_key {
            parts.push("-i".to_string());
            parts.push(key.clone());
        }
        Some(parts.join(" "))
    }

    /// Builds the rsync arg list for unit testing without shelling out.
    pub fn build_rsync_args(
        &self,
        local_path: &str,
        exclude: &[String],
        dry_run: bool,
    ) -> Vec<String> {
        let mut args = vec!["-avz".to_string(), "--delete".to_string()];

        if dry_run {
            args.push("--dry-run".to_string());
            args.push("--itemize-changes".to_string());
        }

        if let Some(transport) = self.rsync_ssh_transport() {
            args.push("-e".to_string());
            args.push(transport);
        }

        for exc in exclude {
            args.push("--exclude".to_string());
            args.push(exc.clone());
        }

        // Apply per-target extra excludes from config
        for exc in &self.target.exclude {
            args.push("--exclude".to_string());
            args.push(exc.clone());
        }

        if Path::new(".gitignore").exists() {
            args.push("--exclude-from".to_string());
            args.push(".gitignore".to_string());
        }

        let src = format!("{}/", local_path.trim_end_matches('/'));
        let dest = format!(
            "{}@{}:{}",
            self.target.user, self.target.host, self.target.remote_path
        );
        args.push(src);
        args.push(dest);
        args
    }

    pub fn sync_files(
        &self,
        local_path: &str,
        exclude: Vec<String>,
        dry_run: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if which("rsync").is_err() {
            return Err("rsync not found. Please install it.".into());
        }

        let args = self.build_rsync_args(local_path, &exclude, dry_run);

        if dry_run {
            println!(
                "Dry run: previewing sync to {} (no files will be changed)...",
                self.target.host
            );
        } else {
            println!("Syncing files to {}...", self.target.host);
        }

        let output = Command::new("rsync").args(&args).output()?;

        if !output.status.success() {
            return Err(
                format!("rsync failed: {}", String::from_utf8_lossy(&output.stderr)).into(),
            );
        }

        if dry_run {
            let out = String::from_utf8_lossy(&output.stdout);
            if out.trim().is_empty() {
                println!("Nothing to sync — remote is already up to date.");
            } else {
                println!("{}", out);
                println!("Dry run complete. No files were changed.");
            }
        } else {
            println!("Sync complete.");
        }

        Ok(())
    }

    pub fn run_remote_command(
        &self,
        command: &str,
    ) -> Result<(i32, String, String), Box<dyn std::error::Error>> {
        if which("ssh").is_err() {
            return Err("ssh not found. Please install it.".into());
        }

        let remote_target = format!("{}@{}", self.target.user, self.target.host);
        let remote_cmd = format!("cd {} && {}", self.target.remote_path, command);

        println!("Executing remote command: {}", command);

        let mut cmd = Command::new("ssh");
        cmd.arg("-o").arg("BatchMode=no");
        cmd.arg("-o").arg("LogLevel=ERROR");
        cmd.args(self.ssh_extra_args());
        cmd.arg(remote_target).arg(remote_cmd);

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok((exit_code, stdout, stderr))
    }

    /// Fetches the last `lines` lines from all configured log paths (one-shot).
    pub fn fetch_logs(&self, lines: usize) -> Result<String, Box<dyn std::error::Error>> {
        if self.target.logs.is_empty() {
            return Ok("No log files configured for this target.".to_string());
        }

        let mut combined = String::new();
        for log_path in &self.target.logs {
            let cmd = format!("tail -n {} {}", lines, log_path);
            let (exit_code, stdout, stderr) = self.run_remote_command(&cmd)?;
            if exit_code == 0 {
                combined.push_str(&format!("--- Log: {} ---\n{}\n", log_path, stdout));
            } else {
                combined.push_str(&format!(
                    "--- Error fetching {}: {} ---\n",
                    log_path, stderr
                ));
            }
        }
        Ok(combined)
    }

    /// Streams remote log files to stdout.
    /// `follow = true` keeps the connection open (tail -f).
    /// `follow = false` fetches the last `lines` lines and exits.
    pub fn tail_logs(&self, lines: usize, follow: bool) -> Result<(), Box<dyn std::error::Error>> {
        if self.target.logs.is_empty() {
            println!("No log files configured for this target.");
            return Ok(());
        }

        if which("ssh").is_err() {
            return Err("ssh not found. Please install it.".into());
        }

        let log_paths = self.target.logs.join(" ");
        let tail_flag = if follow { "-f" } else { "" };
        let remote_cmd = format!(
            "cd {} && tail {} -n {} {}",
            self.target.remote_path, tail_flag, lines, log_paths
        );
        let remote_target = format!("{}@{}", self.target.user, self.target.host);

        let mut cmd = Command::new("ssh");
        cmd.arg("-o").arg("BatchMode=no");
        cmd.arg("-o").arg("LogLevel=ERROR");
        cmd.args(self.ssh_extra_args());
        cmd.arg(remote_target).arg(remote_cmd);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture remote stdout")?;

        for line in BufReader::new(stdout).lines() {
            println!("{}", line?);
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(format!("Remote tail exited with status: {:?}", status.code()).into());
        }
        Ok(())
    }

    /// Returns the `user@host` string for display purposes.
    pub fn remote_host_label(&self) -> String {
        format!("{}@{}", self.target.user, self.target.host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TargetConfig;

    fn make_transport(port: Option<u16>, ssh_key: Option<String>) -> Transport {
        Transport::new(TargetConfig {
            host: "example.com".to_string(),
            user: "deploy".to_string(),
            remote_path: "/var/www/app".to_string(),
            port,
            ssh_key,
            restart_cmd: None,
            logs: vec![],
            require_confirmation: false,
            exclude: vec![],
        })
    }

    // ── ssh_extra_args ────────────────────────────────────────────────────────

    #[test]
    fn test_ssh_extra_args_none() {
        let t = make_transport(None, None);
        assert!(t.ssh_extra_args().is_empty());
    }

    #[test]
    fn test_ssh_extra_args_port_only() {
        let t = make_transport(Some(2222), None);
        assert_eq!(t.ssh_extra_args(), vec!["-p", "2222"]);
    }

    #[test]
    fn test_ssh_extra_args_key_only() {
        let t = make_transport(None, Some("~/.ssh/id_rsa".to_string()));
        assert_eq!(t.ssh_extra_args(), vec!["-i", "~/.ssh/id_rsa"]);
    }

    #[test]
    fn test_ssh_extra_args_port_and_key() {
        let t = make_transport(Some(2222), Some("~/.ssh/deploy.pem".to_string()));
        let args = t.ssh_extra_args();
        assert_eq!(args, vec!["-p", "2222", "-i", "~/.ssh/deploy.pem"]);
    }

    // ── rsync_ssh_transport ───────────────────────────────────────────────────

    #[test]
    fn test_rsync_ssh_transport_none_when_defaults() {
        let t = make_transport(None, None);
        assert!(t.rsync_ssh_transport().is_none());
    }

    #[test]
    fn test_rsync_ssh_transport_with_port() {
        let t = make_transport(Some(2222), None);
        assert_eq!(t.rsync_ssh_transport().unwrap(), "ssh -o LogLevel=ERROR -p 2222");
    }

    #[test]
    fn test_rsync_ssh_transport_with_key() {
        let t = make_transport(None, Some("/keys/prod.pem".to_string()));
        assert_eq!(t.rsync_ssh_transport().unwrap(), "ssh -o LogLevel=ERROR -i /keys/prod.pem");
    }

    #[test]
    fn test_rsync_ssh_transport_with_both() {
        let t = make_transport(Some(22), Some("/keys/prod.pem".to_string()));
        assert_eq!(
            t.rsync_ssh_transport().unwrap(),
            "ssh -o LogLevel=ERROR -p 22 -i /keys/prod.pem"
        );
    }

    // ── build_rsync_args ──────────────────────────────────────────────────────

    #[test]
    fn test_build_rsync_args_basic() {
        let t = make_transport(None, None);
        let args = t.build_rsync_args(".", &[".git/".to_string()], false);
        assert!(args.contains(&"-avz".to_string()));
        assert!(args.contains(&"--delete".to_string()));
        assert!(args.contains(&"--exclude".to_string()));
        assert!(args.contains(&".git/".to_string()));
        assert!(!args.contains(&"--dry-run".to_string()));
    }

    #[test]
    fn test_build_rsync_args_dry_run_flag() {
        let t = make_transport(None, None);
        let args = t.build_rsync_args(".", &[], true);
        assert!(args.contains(&"--dry-run".to_string()));
        assert!(args.contains(&"--itemize-changes".to_string()));
    }

    #[test]
    fn test_build_rsync_args_no_dry_run_flag() {
        let t = make_transport(None, None);
        let args = t.build_rsync_args(".", &[], false);
        assert!(!args.contains(&"--dry-run".to_string()));
    }

    #[test]
    fn test_build_rsync_args_with_ssh_transport() {
        let t = make_transport(Some(2222), None);
        let args = t.build_rsync_args(".", &[], false);
        assert!(args.contains(&"-e".to_string()));
        let e_pos = args.iter().position(|a| a == "-e").unwrap();
        assert_eq!(args[e_pos + 1], "ssh -o LogLevel=ERROR -p 2222");
    }

    #[test]
    fn test_build_rsync_args_dest_format() {
        let t = make_transport(None, None);
        let args = t.build_rsync_args(".", &[], false);
        assert!(args.last().unwrap().contains("deploy@example.com:/var/www/app"));
    }

    #[test]
    fn test_build_rsync_args_includes_target_excludes() {
        let target = TargetConfig {
            host: "example.com".to_string(),
            user: "deploy".to_string(),
            remote_path: "/var/www/app".to_string(),
            port: None,
            ssh_key: None,
            restart_cmd: None,
            logs: vec![],
            require_confirmation: false,
            exclude: vec!["node_modules/".to_string(), "*.log".to_string()],
        };
        let t = Transport::new(target);
        let args = t.build_rsync_args(".", &[], false);
        assert!(args.contains(&"node_modules/".to_string()));
        assert!(args.contains(&"*.log".to_string()));
    }

    // ── fetch_logs ────────────────────────────────────────────────────────────

    #[test]
    fn test_fetch_logs_empty_returns_message() {
        let t = make_transport(None, None);
        let result = t.fetch_logs(10).unwrap();
        assert_eq!(result, "No log files configured for this target.");
    }

    // ── remote_host_label ─────────────────────────────────────────────────────

    #[test]
    fn test_remote_host_label() {
        let t = make_transport(None, None);
        assert_eq!(t.remote_host_label(), "deploy@example.com");
    }
}
