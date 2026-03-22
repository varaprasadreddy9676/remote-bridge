use crate::config::TargetConfig;
use crate::parser::{FileChange, ShellCommand};
use crate::transport::Transport;
use dialoguer::Confirm;
use std::fs;
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
        )?;
        Ok(())
    }

    pub fn run_commands(&self, commands: &[ShellCommand]) -> Result<(), Box<dyn std::error::Error>> {
        if commands.is_empty() {
            return Ok(());
        }

        for cmd in commands {
            if self.should_confirm(&cmd.command) {
                if !Confirm::new()
                    .with_prompt(format!(
                        "Command contains potentially dangerous keywords. Execute on {}?",
                        self.target.host
                    ))
                    .interact()?
                {
                    println!("Skipping command: {}", cmd.command);
                    continue;
                }
            }

            let (exit_code, stdout, stderr) = self.transport.run_remote_command(&cmd.command)?;

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

    pub fn should_confirm(&self, command: &str) -> bool {
        let dangerous = ["rm ", "sudo ", "db ", "database", "drop ", "delete "];
        let cmd_lower = command.to_lowercase();
        dangerous.iter().any(|&k| cmd_lower.contains(k)) || self.target.require_confirmation
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
}
