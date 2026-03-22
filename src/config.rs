use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TargetConfig {
    pub host: String,
    pub user: String,
    pub remote_path: String,
    /// SSH port (default: 22)
    #[serde(default)]
    pub port: Option<u16>,
    /// Path to SSH identity file (e.g. ~/.ssh/id_rsa or ~/keys/prod.pem)
    #[serde(default)]
    pub ssh_key: Option<String>,
    pub restart_cmd: Option<String>,
    pub logs: Vec<String>,
    #[serde(default)]
    pub require_confirmation: bool,
    /// Additional rsync exclude patterns beyond the defaults
    #[serde(default)]
    pub exclude: Vec<String>,
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
        port: None,
        ssh_key: None,
        restart_cmd: None,
        logs: vec![format!("{}/logs/error.log", remote_path)],
        require_confirmation: false,
        exclude: vec![],
    });

    let config = RemoteBridgeConfig {
        project_name: name.to_string(),
        targets,
    };

    let yaml = serde_yaml::to_string(&config)?;
    fs::write(path, yaml)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_yaml(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_load_config_valid() {
        let yaml = r#"
project_name: test-app
targets:
  staging:
    host: "192.168.1.1"
    user: "deploy"
    remote_path: "/var/www/app"
    restart_cmd: null
    logs: []
    require_confirmation: false
"#;
        let file = write_temp_yaml(yaml);
        let config = load_config(file.path()).unwrap();
        assert_eq!(config.project_name, "test-app");
        assert!(config.targets.contains_key("staging"));
        let target = &config.targets["staging"];
        assert_eq!(target.host, "192.168.1.1");
        assert_eq!(target.user, "deploy");
        assert_eq!(target.remote_path, "/var/www/app");
    }

    #[test]
    fn test_load_config_require_confirmation_default_false() {
        let yaml = r#"
project_name: test-app
targets:
  staging:
    host: "localhost"
    user: "user"
    remote_path: "/tmp/app"
    logs: []
"#;
        let file = write_temp_yaml(yaml);
        let config = load_config(file.path()).unwrap();
        assert!(!config.targets["staging"].require_confirmation);
    }

    #[test]
    fn test_load_config_with_logs() {
        let yaml = r#"
project_name: test-app
targets:
  prod:
    host: "prod.example.com"
    user: "ubuntu"
    remote_path: "/opt/app"
    logs:
      - "/opt/app/logs/error.log"
      - "/var/log/nginx/error.log"
    require_confirmation: true
"#;
        let file = write_temp_yaml(yaml);
        let config = load_config(file.path()).unwrap();
        let target = &config.targets["prod"];
        assert_eq!(target.logs.len(), 2);
        assert!(target.require_confirmation);
    }

    #[test]
    fn test_load_config_missing_file() {
        let result = load_config("/nonexistent/path/config.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_invalid_yaml() {
        let file = write_temp_yaml("not: valid: yaml: ::::");
        let result = load_config(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_create_default_config() {
        let file = NamedTempFile::new().unwrap();
        create_default_config(file.path(), "my-app", "1.2.3.4", "ubuntu", "/var/www/app").unwrap();
        let config = load_config(file.path()).unwrap();
        assert_eq!(config.project_name, "my-app");
        let target = &config.targets["staging"];
        assert_eq!(target.host, "1.2.3.4");
        assert_eq!(target.user, "ubuntu");
        assert_eq!(target.remote_path, "/var/www/app");
        assert_eq!(target.logs, vec!["/var/www/app/logs/error.log"]);
        assert!(!target.require_confirmation);
        assert!(target.port.is_none());
        assert!(target.ssh_key.is_none());
        assert!(target.exclude.is_empty());
    }

    #[test]
    fn test_load_config_with_port_and_ssh_key() {
        let yaml = r#"
project_name: test-app
targets:
  staging:
    host: "192.168.1.1"
    user: "deploy"
    remote_path: "/var/www/app"
    port: 2222
    ssh_key: "~/.ssh/deploy_key"
    logs: []
"#;
        let file = write_temp_yaml(yaml);
        let config = load_config(file.path()).unwrap();
        let target = &config.targets["staging"];
        assert_eq!(target.port, Some(2222));
        assert_eq!(target.ssh_key.as_deref(), Some("~/.ssh/deploy_key"));
    }

    #[test]
    fn test_load_config_port_key_absent_defaults_to_none() {
        let yaml = r#"
project_name: test-app
targets:
  staging:
    host: "localhost"
    user: "user"
    remote_path: "/tmp/app"
    logs: []
"#;
        let file = write_temp_yaml(yaml);
        let config = load_config(file.path()).unwrap();
        let target = &config.targets["staging"];
        assert!(target.port.is_none());
        assert!(target.ssh_key.is_none());
        assert!(target.exclude.is_empty());
    }

    #[test]
    fn test_load_config_with_extra_excludes() {
        let yaml = r#"
project_name: test-app
targets:
  staging:
    host: "localhost"
    user: "user"
    remote_path: "/tmp/app"
    logs: []
    exclude:
      - "node_modules/"
      - "*.log"
"#;
        let file = write_temp_yaml(yaml);
        let config = load_config(file.path()).unwrap();
        let target = &config.targets["staging"];
        assert_eq!(target.exclude, vec!["node_modules/", "*.log"]);
    }

    #[test]
    fn test_create_default_config_roundtrip() {
        let file = NamedTempFile::new().unwrap();
        create_default_config(file.path(), "roundtrip", "host", "user", "/path").unwrap();
        let config = load_config(file.path()).unwrap();
        assert_eq!(config.project_name, "roundtrip");
    }

    #[test]
    fn test_multiple_targets() {
        let yaml = r#"
project_name: multi-target-app
targets:
  staging:
    host: "staging.example.com"
    user: "deploy"
    remote_path: "/opt/staging"
    logs: []
    require_confirmation: false
  production:
    host: "prod.example.com"
    user: "deploy"
    remote_path: "/opt/production"
    logs: []
    require_confirmation: true
"#;
        let file = write_temp_yaml(yaml);
        let config = load_config(file.path()).unwrap();
        assert_eq!(config.targets.len(), 2);
        assert!(config.targets.contains_key("staging"));
        assert!(config.targets.contains_key("production"));
    }
}
