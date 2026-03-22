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

        self.transport.sync_files(".", vec![".git/".to_string(), ".remote_bridge/".to_string()])?;
        Ok(())
    }

    pub fn run_commands(&self, commands: &[ShellCommand]) -> Result<(), Box<dyn std::error::Error>> {
        if commands.is_empty() {
            return Ok(());
        }

        for cmd in commands {
            if self.should_confirm(&cmd.command) {
                if !Confirm::new()
                    .with_prompt(format!("Command contains potentially dangerous keywords. Execute on {}?", self.target.host))
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

    fn should_confirm(&self, command: &str) -> bool {
        let dangerous = ["rm ", "sudo ", "db ", "database", "drop ", "delete "];
        let cmd_lower = command.to_lowercase();
        dangerous.iter().any(|&k| cmd_lower.contains(k)) || self.target.require_confirmation
    }

    pub fn get_transport(&self) -> &Transport {
        &self.transport
    }
}
