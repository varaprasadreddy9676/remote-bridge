use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TargetConfig {
    pub host: String,
    pub user: String,
    pub remote_path: String,
    pub restart_cmd: Option<String>,
    pub logs: Vec<String>,
    #[serde(default)]
    pub require_confirmation: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoteBridgeConfig {
    pub project_name: String,
    pub targets: HashMap<String, TargetConfig>,
}

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<RemoteBridgeConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: RemoteBridgeConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

pub fn create_default_config<P: AsRef<Path>>(path: P, name: &str, host: &str, user: &str, remote_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut targets = HashMap::new();
    targets.insert("staging".to_string(), TargetConfig {
        host: host.to_string(),
        user: user.to_string(),
        remote_path: remote_path.to_string(),
        restart_cmd: None,
        logs: vec![format!("{}/logs/error.log", remote_path)],
        require_confirmation: false,
    });
    
    let config = RemoteBridgeConfig {
        project_name: name.to_string(),
        targets,
    };
    
    let yaml = serde_yaml::to_string(&config)?;
    fs::write(path, yaml)?;
    Ok(())
}
