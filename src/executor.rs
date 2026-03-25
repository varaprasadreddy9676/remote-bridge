use crate::config::TargetConfig;
use crate::parser::{FileChange, ShellCommand};
use crate::transport::Transport;
use dialoguer::Confirm;
use std::fs;
use std::io::Write;
use std::path::Path;

pub struct Executor {
    target: TargetConfig,
    transport: Transport,
}

impl Executor {
    pub fn new(target: TargetConfig) -> Self {
        let transport = Transport::new(target.clone());
        Self { target, transport }
    }

    pub fn apply_file_changes(&self, changes: &[FileChange]) -> Result<(), Box<dyn std::error::Error>> {
        if changes.is_empty() {
            return Ok(());
        }

        for change in changes {
            let path = Path::new(&change.path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            println!("Applying file change: {}", change.path);
            fs::write(path, &change.content)?;
        }

        self.transport.sync_files(
            ".",
            vec![".git/".to_string(), ".remote_bridge/".to_string()],
            false,
            true,
        )?;
        Ok(())
    }

    pub fn run_commands(&self, commands: &[ShellCommand]) -> Result<(), Box<dyn std::error::Error>> {
        if commands.is_empty() {
            return Ok(());
        }

        for cmd in commands {
            // Hard block — user-defined patterns, no override
            if self.is_blocked(&cmd.command) {
                println!("BLOCKED: '{}' matches a blocked pattern and will not run.", cmd.command);
                self.audit(&cmd.command, -2);
                continue;
            }

            // Allowlist — when set, only matching prefixes are permitted
            if !self.is_allowed(&cmd.command) {
                println!("NOT ALLOWED: '{}' is not in allowed_commands.", cmd.command);
                self.audit(&cmd.command, -3);
                continue;
            }

            if self.should_confirm(&cmd.command) {
                if !Confirm::new()
                    .with_prompt(format!(
                        "Command contains potentially dangerous keywords. Execute on {}?",
                        self.target.host
                    ))
                    .interact()?
                {
                    println!("Skipping command: {}", cmd.command);
                    self.audit(&cmd.command, -1);
                    continue;
                }
            }

            let (exit_code, stdout, stderr) = self.transport.run_remote_command(&cmd.command)?;
            self.audit(&cmd.command, exit_code);

            if !stdout.is_empty() {
                println!("STDOUT:\n{}", stdout);
            }

            if exit_code != 0 {
                println!("Command failed with exit code {}", exit_code);
                if !stderr.is_empty() {
                    println!("STDERR:\n{}", stderr);
                }
                if !self.target.logs.is_empty() {
                    println!("Fetching remote logs for context...");
                    let logs = self.transport.fetch_logs(20)?;
                    println!("REMOTE LOGS:\n{}", logs);
                }
            }
        }

        Ok(())
    }

    /// Restarts the remote service using the configured `restart_cmd`.
    pub fn restart(&self) -> Result<(), Box<dyn std::error::Error>> {
        let cmd = self.target.restart_cmd.as_ref().ok_or_else(|| {
            format!(
                "No restart_cmd configured for target '{}'. Add it to remotebridge.yaml.",
                self.target.host
            )
        })?;

        println!("Restarting service on {}...", self.target.host);
        let (exit_code, stdout, stderr) = self.transport.run_remote_command(cmd)?;

        if !stdout.is_empty() {
            println!("{}", stdout);
        }

        if exit_code != 0 {
            if !stderr.is_empty() {
                println!("STDERR:\n{}", stderr);
            }
            return Err(format!("Restart command failed with exit code {}", exit_code).into());
        }

        println!("Service restarted successfully.");
        Ok(())
    }

    /// Full deploy pipeline: sync → restart → tail logs on failure.
    pub fn deploy(&self, follow_logs: bool) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "Deploying to {} ({})...",
            self.target.host, self.target.remote_path
        );

        self.transport.sync_files(
            ".",
            vec![".git/".to_string(), ".remote_bridge/".to_string()],
            false,
            true,
        )?;

        if self.target.restart_cmd.is_some() {
            if let Err(e) = self.restart() {
                println!("Deploy failed: {}", e);
                if !self.target.logs.is_empty() {
                    println!("Fetching logs for context...");
                    let _ = self.transport.tail_logs(50, false);
                }
                return Err(e);
            }
        }

        println!("Deploy complete.");

        if follow_logs {
            println!("Following remote logs (Ctrl-C to stop)...");
            self.transport.tail_logs(20, true)?;
        }

        Ok(())
    }

    /// Returns true if the command must be confirmed before running.
    pub fn should_confirm(&self, command: &str) -> bool {
        if self.target.require_confirmation {
            return true;
        }
        let cmd = command.to_lowercase();
        // Built-in dangerous patterns — expanded to catch common destructive commands
        let dangerous = [
            "rm ",   "rm\t",                 // file deletion
            "sudo ", "sudo\t",               // privilege escalation
            "db ",   "database",             // database operations
            "drop ", "delete ",              // data destruction
            "mkfs",  "dd ",                  // disk operations
            "chmod 777", "chmod -r",         // permission changes
            "curl | ", "curl |",             // piped remote execution
            "wget | ", "wget |",
            "| bash", "| sh",
            "| python", "| node",
            "> /dev/", "truncate",           // device/file overwrite
            "shutdown", "reboot", "halt",    // system control
            "killall", "pkill",              // process killing
            "passwd", "chpasswd",            // password changes
            ":(){",                          // fork bomb
            "format ",                       // formatting
        ];
        dangerous.iter().any(|&k| cmd.contains(k))
    }

    /// Returns true if the command matches a user-defined blocked pattern.
    /// Blocked commands are ALWAYS rejected — no confirmation prompt, no override.
    pub fn is_blocked(&self, command: &str) -> bool {
        if self.target.blocked_patterns.is_empty() {
            return false;
        }
        let cmd = command.to_lowercase();
        self.target.blocked_patterns.iter().any(|pat| cmd.contains(&pat.to_lowercase()))
    }

    /// Returns true if the command is permitted by the allowlist.
    /// When `allowed_commands` is empty, all commands are permitted (subject to other checks).
    /// When set, ONLY commands that start with an entry in the list are permitted.
    pub fn is_allowed(&self, command: &str) -> bool {
        if self.target.allowed_commands.is_empty() {
            return true;
        }
        let cmd = command.trim().to_lowercase();
        self.target.allowed_commands.iter().any(|allowed| {
            cmd.starts_with(&allowed.to_lowercase())
        })
    }

    /// Appends an audit log entry for every executed command.
    fn audit(&self, command: &str, exit_code: i32) {
        let log_path = self.target.audit_log.clone().unwrap_or_else(|| {
            dirs_next::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".remote-bridge-audit.log")
                .to_string_lossy()
                .to_string()
        });

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let entry = format!(
            "[{}] host={} path={} exit={} cmd={}\n",
            now, self.target.host, self.target.remote_path, exit_code, command
        );

        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&log_path) {
            let _ = f.write_all(entry.as_bytes());
        }
    }

    pub fn get_transport(&self) -> &Transport {
        &self.transport
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TargetConfig;

    fn make_target(require_confirmation: bool) -> TargetConfig {
        TargetConfig {
            host: "localhost".to_string(),
            user: "testuser".to_string(),
            remote_path: "/tmp/test".to_string(),
            port: None,
            ssh_key: None,
            restart_cmd: None,
            logs: vec![],
            require_confirmation,
            exclude: vec![],
            allowed_commands: vec![],
            blocked_patterns: vec![],
            audit_log: None,
        }
    }

    fn make_target_with_restart(cmd: &str) -> TargetConfig {
        TargetConfig {
            restart_cmd: Some(cmd.to_string()),
            ..make_target(false)
        }
    }

    // ── should_confirm ────────────────────────────────────────────────────────

    #[test]
    fn test_should_confirm_safe_command() {
        let executor = Executor::new(make_target(false));
        assert!(!executor.should_confirm("npm install"));
    }

    #[test]
    fn test_should_confirm_rm_command() {
        let executor = Executor::new(make_target(false));
        assert!(executor.should_confirm("rm -rf /tmp/old"));
    }

    #[test]
    fn test_should_confirm_sudo_command() {
        let executor = Executor::new(make_target(false));
        assert!(executor.should_confirm("sudo systemctl restart app"));
    }

    #[test]
    fn test_should_confirm_database_keyword() {
        let executor = Executor::new(make_target(false));
        assert!(executor.should_confirm("database reset"));
        assert!(executor.should_confirm("db migrate"));
        assert!(executor.should_confirm("drop table users"));
    }

    #[test]
    fn test_should_confirm_delete_command() {
        let executor = Executor::new(make_target(false));
        assert!(executor.should_confirm("delete from users"));
    }

    #[test]
    fn test_should_confirm_require_confirmation_flag() {
        let executor = Executor::new(make_target(true));
        assert!(executor.should_confirm("ls -la"));
    }

    #[test]
    fn test_should_confirm_case_insensitive() {
        let executor = Executor::new(make_target(false));
        assert!(executor.should_confirm("SUDO apt install vim"));
        assert!(executor.should_confirm("DROP TABLE users"));
    }

    // ── empty-slice guards ────────────────────────────────────────────────────

    #[test]
    fn test_apply_file_changes_empty() {
        let executor = Executor::new(make_target(false));
        assert!(executor.apply_file_changes(&[]).is_ok());
    }

    #[test]
    fn test_run_commands_empty() {
        let executor = Executor::new(make_target(false));
        assert!(executor.run_commands(&[]).is_ok());
    }

    // ── restart ───────────────────────────────────────────────────────────────

    #[test]
    fn test_restart_no_restart_cmd_returns_err() {
        let executor = Executor::new(make_target(false));
        let err = executor.restart().unwrap_err();
        assert!(err.to_string().contains("No restart_cmd configured"));
    }

    #[test]
    fn test_restart_error_message_includes_host() {
        let executor = Executor::new(make_target(false));
        let err = executor.restart().unwrap_err();
        assert!(err.to_string().contains("localhost"));
    }

    #[test]
    fn test_restart_with_cmd_does_not_early_return_on_guard() {
        // Verify the method doesn't error on the guard check when restart_cmd is set.
        // (Actual SSH call will fail since there's no server, but guard logic passes.)
        let executor = Executor::new(make_target_with_restart("systemctl restart app"));
        // The guard check passes; the SSH call will fail, which is expected in unit tests.
        let result = executor.restart();
        // Should not be the "No restart_cmd" error — any other error is acceptable
        if let Err(e) = result {
            assert!(!e.to_string().contains("No restart_cmd configured"));
        }
    }

    // ── deploy ────────────────────────────────────────────────────────────────

    #[test]
    fn test_deploy_no_restart_cmd_skips_restart_step() {
        // When no restart_cmd, deploy should not error on the restart guard.
        // It will fail at rsync since localhost SSH isn't set up, but that's fine —
        // the important assertion is that there's no "No restart_cmd" error.
        let executor = Executor::new(make_target(false));
        let result = executor.deploy(false);
        if let Err(e) = &result {
            assert!(!e.to_string().contains("No restart_cmd configured"));
        }
    }

    // ── is_blocked ────────────────────────────────────────────────────────────

    #[test]
    fn test_is_blocked_no_patterns_returns_false() {
        let executor = Executor::new(make_target(false));
        assert!(!executor.is_blocked("rm -rf /"));
    }

    #[test]
    fn test_is_blocked_matching_pattern() {
        let mut target = make_target(false);
        target.blocked_patterns = vec!["rm -rf".to_string()];
        let executor = Executor::new(target);
        assert!(executor.is_blocked("rm -rf /var/data"));
    }

    #[test]
    fn test_is_blocked_non_matching_pattern() {
        let mut target = make_target(false);
        target.blocked_patterns = vec!["drop table".to_string()];
        let executor = Executor::new(target);
        assert!(!executor.is_blocked("npm run build"));
    }

    #[test]
    fn test_is_blocked_case_insensitive() {
        let mut target = make_target(false);
        target.blocked_patterns = vec!["drop table".to_string()];
        let executor = Executor::new(target);
        assert!(executor.is_blocked("DROP TABLE users"));
    }

    #[test]
    fn test_is_blocked_multiple_patterns_any_match() {
        let mut target = make_target(false);
        target.blocked_patterns = vec!["rm -rf".to_string(), "shutdown".to_string()];
        let executor = Executor::new(target);
        assert!(executor.is_blocked("shutdown -h now"));
        assert!(!executor.is_blocked("npm install"));
    }

    // ── is_allowed ────────────────────────────────────────────────────────────

    #[test]
    fn test_is_allowed_empty_list_permits_all() {
        let executor = Executor::new(make_target(false));
        assert!(executor.is_allowed("rm -rf /"));
        assert!(executor.is_allowed("anything goes"));
    }

    #[test]
    fn test_is_allowed_matching_prefix() {
        let mut target = make_target(false);
        target.allowed_commands = vec!["npm".to_string(), "cargo".to_string()];
        let executor = Executor::new(target);
        assert!(executor.is_allowed("npm install"));
        assert!(executor.is_allowed("cargo build --release"));
    }

    #[test]
    fn test_is_allowed_non_matching_prefix_rejected() {
        let mut target = make_target(false);
        target.allowed_commands = vec!["npm".to_string()];
        let executor = Executor::new(target);
        assert!(!executor.is_allowed("rm -rf /tmp"));
        assert!(!executor.is_allowed("sudo systemctl restart"));
    }

    #[test]
    fn test_is_allowed_case_insensitive() {
        let mut target = make_target(false);
        target.allowed_commands = vec!["npm".to_string()];
        let executor = Executor::new(target);
        assert!(executor.is_allowed("NPM install"));
    }
}
