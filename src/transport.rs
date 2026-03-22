use std::process::Command;
use std::path::Path;
use which::which;
use crate::config::TargetConfig;

pub struct Transport {
    target: TargetConfig,
}

impl Transport {
    pub fn new(target: TargetConfig) -> Self {
        Self { target }
    }

    pub fn sync_files(&self, local_path: &str, exclude: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        if which("rsync").is_err() {
            return Err("rsync not found. Please install it.".into());
        }

        let remote_dest = format!("{}@{}:{}", self.target.user, self.target.host, self.target.remote_path);
        
        let mut cmd = Command::new("rsync");
        cmd.args(["-avz", "--delete"]);
        
        for exc in exclude {
            cmd.arg("--exclude").arg(exc);
        }
        
        if Path::new(".gitignore").exists() {
            cmd.arg("--exclude-from").arg(".gitignore");
        }
        
        cmd.arg(format!("{}/", local_path.trim_end_matches('/')));
        cmd.arg(remote_dest);
        
        println!("Syncing files to {}...", self.target.host);
        let output = cmd.output()?;
        
        if !output.status.success() {
            return Err(format!("rsync failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }
        
        println!("Sync complete.");
        Ok(())
    }

    pub fn run_remote_command(&self, command: &str) -> Result<(i32, String, String), Box<dyn std::error::Error>> {
        if which("ssh").is_err() {
            return Err("ssh not found. Please install it.".into());
        }

        let remote_target = format!("{}@{}", self.target.user, self.target.host);
        let remote_cmd = format!("cd {} && {}", self.target.remote_path, command);
        
        println!("Executing remote command: {}", command);
        
        let output = Command::new("ssh")
            .arg("-o")
            .arg("BatchMode=no")
            .arg(remote_target)
            .arg(remote_cmd)
            .output()?;
            
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);
        
        Ok((exit_code, stdout, stderr))
    }

    pub fn fetch_logs(&self, lines: usize) -> Result<String, Box<dyn std::error::Error>> {
        let mut combined = String::new();
        
        for log_path in &self.target.logs {
            let cmd = format!("tail -n {} {}", lines, log_path);
            let (exit_code, stdout, stderr) = self.run_remote_command(&cmd)?;
            
            if exit_code == 0 {
                combined.push_str(&format!("--- Log: {} ---\n{}\n", log_path, stdout));
            } else {
                combined.push_str(&format!("--- Error fetching log: {} ---\n{}\n", log_path, stderr));
            }
        }
        
        Ok(combined)
    }
}
