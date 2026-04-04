use crate::config::TargetConfig;
use crate::transport::Transport;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};

const SECTION_PREFIX: &str = "__RB_SECTION__:";

#[derive(Debug, Clone, PartialEq, Eq)]
enum ServiceManager {
    Systemd { service: String },
    Pm2 { process: String },
    DockerCompose,
    Supervisor { service: String },
    Unknown,
}

impl ServiceManager {
    fn label(&self) -> String {
        match self {
            Self::Systemd { service } => format!("systemd ({})", service),
            Self::Pm2 { process } => format!("pm2 ({})", process),
            Self::DockerCompose => "docker compose".to_string(),
            Self::Supervisor { service } => format!("supervisor ({})", service),
            Self::Unknown => "unknown".to_string(),
        }
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\"'\"'"#))
}

fn tokenize_command(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(|token| token.trim_matches(|c| c == '\'' || c == '"' || c == '`').to_string())
        .filter(|token| !token.is_empty())
        .collect()
}

fn infer_service_manager(restart_cmd: Option<&str>) -> ServiceManager {
    let Some(command) = restart_cmd else {
        return ServiceManager::Unknown;
    };

    let tokens = tokenize_command(command);
    if tokens.is_empty() {
        return ServiceManager::Unknown;
    }

    let mut i = 0;
    if tokens.get(i).map(|t| t.as_str()) == Some("sudo") {
        i += 1;
    }

    match tokens.get(i).map(|t| t.as_str()) {
        Some("systemctl") => {
            for action in ["restart", "reload", "start", "stop", "status"] {
                if let Some(pos) = tokens.iter().position(|token| token == action) {
                    if let Some(service) = tokens.get(pos + 1) {
                        return ServiceManager::Systemd {
                            service: service.trim_end_matches(".service").to_string(),
                        };
                    }
                }
            }
            ServiceManager::Systemd {
                service: "unknown".to_string(),
            }
        }
        Some("pm2") => {
            for action in ["restart", "reload", "start", "stop", "describe"] {
                if let Some(pos) = tokens.iter().position(|token| token == action) {
                    if let Some(process) = tokens.get(pos + 1) {
                        return ServiceManager::Pm2 {
                            process: process.to_string(),
                        };
                    }
                }
            }
            ServiceManager::Pm2 {
                process: "unknown".to_string(),
            }
        }
        Some("supervisorctl") => {
            for action in ["restart", "start", "stop", "status"] {
                if let Some(pos) = tokens.iter().position(|token| token == action) {
                    if let Some(service) = tokens.get(pos + 1) {
                        return ServiceManager::Supervisor {
                            service: service.to_string(),
                        };
                    }
                }
            }
            ServiceManager::Supervisor {
                service: "unknown".to_string(),
            }
        }
        Some("docker") if tokens.get(i + 1).map(|t| t.as_str()) == Some("compose") => {
            ServiceManager::DockerCompose
        }
        Some("docker-compose") => ServiceManager::DockerCompose,
        _ => {
            if command.contains("docker compose") || command.contains("docker-compose") {
                ServiceManager::DockerCompose
            } else {
                ServiceManager::Unknown
            }
        }
    }
}

fn append_section(script: &mut String, name: &str, body: &str) {
    script.push_str(&format!("printf '%s\\n' '{}{}'; ", SECTION_PREFIX, name));
    script.push_str(body);
    script.push(' ');
}

fn build_service_section(manager: &ServiceManager) -> String {
    match manager {
        ServiceManager::Systemd { service } => format!(
            "printf 'manager=systemd\\nservice={}\\n'; \
             systemctl is-active {} 2>&1 || true; \
             systemctl status {} --no-pager -n 20 2>&1 || true;",
            service,
            shell_quote(service),
            shell_quote(service)
        ),
        ServiceManager::Pm2 { process } => format!(
            "printf 'manager=pm2\\nprocess={}\\n'; \
             pm2 describe {} 2>&1 || pm2 status 2>&1 || true;",
            process,
            shell_quote(process)
        ),
        ServiceManager::DockerCompose => {
            "printf 'manager=docker-compose\\n'; \
             docker compose ps --all 2>&1 || docker-compose ps 2>&1 || true;"
                .to_string()
        }
        ServiceManager::Supervisor { service } => format!(
            "printf 'manager=supervisor\\nservice={}\\n'; \
             supervisorctl status {} 2>&1 || true;",
            service,
            shell_quote(service)
        ),
        ServiceManager::Unknown => "printf 'manager=unknown\\n'; \
             printf 'restart_cmd_not_parsed\\n';"
            .to_string(),
    }
}

fn build_logs_section(logs: &[String], lines: usize) -> String {
    if logs.is_empty() {
        return "printf 'no_logs_configured\\n';".to_string();
    }

    let quoted_logs = logs
        .iter()
        .map(|path| shell_quote(path))
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        "for log in {quoted_logs}; do \
            printf 'log=%s\\n' \"$log\"; \
            if [ -r \"$log\" ]; then \
                tail -n {lines} \"$log\"; \
            elif [ -e \"$log\" ]; then \
                printf 'exists_but_not_readable\\n'; \
            else \
                printf 'file_not_found\\n'; \
            fi; \
            printf '\\n'; \
        done;"
    )
}

fn build_preflight_section() -> String {
    r#"os=$(lsb_release -d 2>/dev/null || grep PRETTY_NAME /etc/os-release 2>/dev/null | cut -d= -f2 | tr -d '"');
printf 'OS: %s\n' "${os:-Unknown}";
if command -v node >/dev/null 2>&1; then printf 'Node.js: %s\n' "$(node -v)"; else printf 'Node.js: Not found\n'; fi;
if command -v python3 >/dev/null 2>&1; then printf 'Python: %s\n' "$(python3 --version 2>&1)"; else printf 'Python: Not found\n'; fi;
if command -v rustc >/dev/null 2>&1; then printf 'Rust: %s\n' "$(rustc --version)"; else printf 'Rust: Not found\n'; fi;
if command -v docker >/dev/null 2>&1; then printf 'Docker: %s\n' "$(docker --version)"; else printf 'Docker: Not found\n'; fi;"#
        .to_string()
}

fn build_diagnose_command(target: &TargetConfig, lines: usize) -> String {
    let manager = infer_service_manager(target.restart_cmd.as_deref());
    let mut script = String::new();

    append_section(
        &mut script,
        "meta",
        "printf 'hostname=%s\\n' \"$(hostname 2>/dev/null || echo unknown)\"; \
         printf 'time=%s\\n' \"$(date -Is 2>/dev/null || date 2>/dev/null || echo unknown)\"; \
         printf 'cwd=%s\\n' \"$PWD\"; \
         printf 'uptime=%s\\n' \"$(uptime 2>/dev/null || echo unavailable)\";",
    );
    append_section(&mut script, "preflight", &build_preflight_section());
    append_section(
        &mut script,
        "disk",
        "df -Pk . 2>/dev/null | tail -1 || echo unavailable;",
    );
    append_section(
        &mut script,
        "memory",
        "free -m 2>/dev/null || vm_stat 2>/dev/null || echo unavailable;",
    );
    append_section(
        &mut script,
        "listeners",
        "(ss -ltnp 2>/dev/null || netstat -ltnp 2>/dev/null || lsof -i -P -n 2>/dev/null) | head -n 20 || true;",
    );
    append_section(&mut script, "service", &build_service_section(&manager));
    append_section(&mut script, "logs", &build_logs_section(&target.logs, lines));
    script
}

fn parse_sections(raw: &str) -> BTreeMap<String, String> {
    let mut sections = BTreeMap::new();
    let mut current_name: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();

    for line in raw.lines() {
        if let Some(name) = line.strip_prefix(SECTION_PREFIX) {
            if let Some(previous) = current_name.replace(name.trim().to_string()) {
                sections.insert(previous, current_lines.join("\n").trim().to_string());
                current_lines.clear();
            }
        } else {
            current_lines.push(line.to_string());
        }
    }

    if let Some(name) = current_name {
        sections.insert(name, current_lines.join("\n").trim().to_string());
    }

    sections
}

fn parse_key_value_lines(text: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            values.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    values
}

fn select_first_meaningful_line(section: Option<&String>) -> Option<String> {
    section?
        .lines()
        .find(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && trimmed != "unavailable"
        })
        .map(|line| line.trim().to_string())
}

fn truncate_line(line: &str, limit: usize) -> String {
    if line.chars().count() <= limit {
        return line.to_string();
    }
    let truncated: String = line.chars().take(limit.saturating_sub(3)).collect();
    format!("{}...", truncated)
}

fn compute_disk_signal(section: Option<&String>) -> Option<String> {
    let line = section?.lines().last()?.trim();
    let regex = Regex::new(r"(\d+)%").ok()?;
    let usage = regex
        .captures(line)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<usize>().ok())?;

    if usage >= 95 {
        Some(format!("Disk is critically full ({}% used)", usage))
    } else if usage >= 85 {
        Some(format!("Disk is getting tight ({}% used)", usage))
    } else {
        None
    }
}

fn known_signal_patterns() -> [(&'static str, &'static [&'static str], &'static str); 8] {
    [
        (
            "Port conflict",
            &["eaddrinuse", "address already in use", "port is already allocated"],
            "Free the occupied port or change the app port before restarting.",
        ),
        (
            "Permissions issue",
            &["permission denied", "eacces", "operation not permitted"],
            "Check file ownership, executable bits, and service user permissions.",
        ),
        (
            "Disk pressure",
            &["no space left on device", "disk full"],
            "Clear space or move logs/artifacts before redeploying.",
        ),
        (
            "Memory pressure",
            &["out of memory", "oom", "cannot allocate memory", "killed process"],
            "Reduce memory usage or increase available memory/swap.",
        ),
        (
            "Missing dependency or file",
            &["cannot find module", "module not found", "no such file or directory"],
            "Verify build artifacts, install steps, and file paths on the server.",
        ),
        (
            "Network dependency failure",
            &["connection refused", "timed out", "timeout", "host unreachable", "network is unreachable"],
            "Check upstream services, firewall rules, and bound interfaces.",
        ),
        (
            "Auth or credentials failure",
            &["authentication failed", "access denied", "password authentication failed", "permission denied for user"],
            "Verify secrets, tokens, and database credentials.",
        ),
        (
            "Crash signature",
            &["panic", "traceback", "exception", "segmentation fault", "fatal"],
            "Inspect the stack trace and last successful deploy change set.",
        ),
    ]
}

fn collect_signal_hits(text: &str) -> Vec<(String, String, String)> {
    let mut hits = Vec::new();
    let mut seen = BTreeSet::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        for (label, needles, suggestion) in known_signal_patterns() {
            if needles.iter().any(|needle| lower.contains(needle)) && seen.insert(label) {
                hits.push((
                    label.to_string(),
                    truncate_line(trimmed, 160),
                    suggestion.to_string(),
                ));
            }
        }
    }

    hits
}

fn manager_health_line(manager: &ServiceManager, service_section: Option<&String>) -> Option<String> {
    let text = service_section?;
    let lower = text.to_lowercase();

    match manager {
        ServiceManager::Systemd { .. } => {
            if lower.contains("failed") {
                Some("Service manager reports failed".to_string())
            } else if lower.contains("inactive") {
                Some("Service manager reports inactive".to_string())
            } else if lower.contains("active") {
                Some("Service manager reports active".to_string())
            } else {
                None
            }
        }
        ServiceManager::Pm2 { .. } => {
            if lower.contains("errored") {
                Some("PM2 reports errored".to_string())
            } else if lower.contains("stopped") {
                Some("PM2 reports stopped".to_string())
            } else if lower.contains("online") {
                Some("PM2 reports online".to_string())
            } else {
                None
            }
        }
        ServiceManager::DockerCompose => {
            if lower.contains("exited") {
                Some("docker compose reports exited containers".to_string())
            } else if lower.contains("up") {
                Some("docker compose reports running containers".to_string())
            } else {
                None
            }
        }
        ServiceManager::Supervisor { .. } => {
            if lower.contains("fatal") {
                Some("supervisor reports fatal".to_string())
            } else if lower.contains("running") {
                Some("supervisor reports running".to_string())
            } else if lower.contains("stopped") {
                Some("supervisor reports stopped".to_string())
            } else {
                None
            }
        }
        ServiceManager::Unknown => None,
    }
}

pub fn diagnose_failure(
    target_name: &str,
    target: &TargetConfig,
    transport: &Transport,
    lines: usize,
) -> Result<String, Box<dyn std::error::Error>> {
    let capped_lines = lines.clamp(10, 200);
    let manager = infer_service_manager(target.restart_cmd.as_deref());
    let command = build_diagnose_command(target, capped_lines);
    let (exit_code, stdout, stderr) = transport.run_remote_command(&command)?;
    if exit_code != 0 {
        return Err(format!("Failed to gather diagnostics: {}", stderr.trim()).into());
    }

    let sections = parse_sections(&stdout);
    let combined = sections.values().cloned().collect::<Vec<_>>().join("\n");
    let mut hits = collect_signal_hits(&combined);
    if let Some(disk_signal) = compute_disk_signal(sections.get("disk")) {
        hits.push((
            "Disk signal".to_string(),
            disk_signal.clone(),
            "Trim logs, caches, or old releases to recover space.".to_string(),
        ));
    }

    let preflight = parse_key_value_lines(sections.get("preflight").map(|s| s.as_str()).unwrap_or(""));
    let mut out = Vec::new();
    out.push(format!(
        "Diagnosis for {} ({}@{}:{})",
        target_name, target.user, target.host, target.remote_path
    ));
    out.push(format!("Service manager: {}", manager.label()));
    if let Some(line) = manager_health_line(&manager, sections.get("service")) {
        out.push(format!("Service health: {}", line));
    }
    if let Some(host_line) = select_first_meaningful_line(sections.get("meta")) {
        out.push(format!("Snapshot: {}", truncate_line(&host_line, 120)));
    }
    for key in ["OS", "Node.js", "Python", "Rust", "Docker"] {
        if let Some(value) = preflight.get(key) {
            out.push(format!("{}: {}", key, value));
        }
    }

    if hits.is_empty() {
        out.push("Likely issues: no known failure signature matched; review service and log excerpts.".to_string());
    } else {
        out.push("Likely issues:".to_string());
        for (label, evidence, suggestion) in hits.iter().take(4) {
            out.push(format!("- {}: {}", label, evidence));
            out.push(format!("  Next step: {}", suggestion));
        }
    }

    if let Some(line) = select_first_meaningful_line(sections.get("disk")) {
        out.push(format!("Disk: {}", truncate_line(&line, 140)));
    }
    if let Some(line) = select_first_meaningful_line(sections.get("memory")) {
        out.push(format!("Memory: {}", truncate_line(&line, 140)));
    }

    if let Some(section) = sections.get("service") {
        let excerpts = section
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(5)
            .map(|line| format!("- {}", truncate_line(line.trim(), 160)))
            .collect::<Vec<_>>();
        if !excerpts.is_empty() {
            out.push("Service excerpt:".to_string());
            out.extend(excerpts);
        }
    }

    if let Some(section) = sections.get("logs") {
        let excerpts = section
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && !trimmed.starts_with("log=")
            })
            .take(8)
            .map(|line| format!("- {}", truncate_line(line.trim(), 160)))
            .collect::<Vec<_>>();
        if !excerpts.is_empty() {
            out.push(format!("Recent log excerpt ({} lines requested):", capped_lines));
            out.extend(excerpts);
        }
    }

    Ok(out.join("\n"))
}

fn compare_optional<T: PartialEq + ToString>(label: &str, left: Option<T>, right: Option<T>) -> Option<String> {
    if left == right {
        None
    } else {
        Some(format!(
            "{}: {} vs {}",
            label,
            left.map(|value| value.to_string()).unwrap_or_else(|| "unset".to_string()),
            right.map(|value| value.to_string()).unwrap_or_else(|| "unset".to_string())
        ))
    }
}

pub fn compare_targets(
    left_name: &str,
    left: &TargetConfig,
    right_name: &str,
    right: &TargetConfig,
) -> Result<String, Box<dyn std::error::Error>> {
    let left_transport = Transport::new(left.clone());
    let right_transport = Transport::new(right.clone());

    let left_preflight = parse_key_value_lines(&left_transport.preflight_check()?);
    let right_preflight = parse_key_value_lines(&right_transport.preflight_check()?);
    let left_manager = infer_service_manager(left.restart_cmd.as_deref());
    let right_manager = infer_service_manager(right.restart_cmd.as_deref());

    let mut out = Vec::new();
    out.push(format!("Target comparison: {} vs {}", left_name, right_name));
    out.push(format!(
        "{} => {}@{}:{}",
        left_name, left.user, left.host, left.remote_path
    ));
    out.push(format!(
        "{} => {}@{}:{}",
        right_name, right.user, right.host, right.remote_path
    ));

    let mut config_diffs = Vec::new();
    if left.host != right.host {
        config_diffs.push(format!("host: {} vs {}", left.host, right.host));
    }
    if left.user != right.user {
        config_diffs.push(format!("user: {} vs {}", left.user, right.user));
    }
    if left.remote_path != right.remote_path {
        config_diffs.push(format!("remote_path: {} vs {}", left.remote_path, right.remote_path));
    }
    if let Some(diff) = compare_optional("port", left.port, right.port) {
        config_diffs.push(diff);
    }
    if left.require_confirmation != right.require_confirmation {
        config_diffs.push(format!(
            "require_confirmation: {} vs {}",
            left.require_confirmation, right.require_confirmation
        ));
    }
    if left.logs != right.logs {
        config_diffs.push(format!(
            "logs: {} configured vs {} configured",
            left.logs.len(),
            right.logs.len()
        ));
    }
    if left.allowed_commands != right.allowed_commands {
        config_diffs.push(format!(
            "allowed_commands: {} entries vs {} entries",
            left.allowed_commands.len(),
            right.allowed_commands.len()
        ));
    }
    if left.blocked_patterns != right.blocked_patterns {
        config_diffs.push(format!(
            "blocked_patterns: {} entries vs {} entries",
            left.blocked_patterns.len(),
            right.blocked_patterns.len()
        ));
    }
    if left_manager != right_manager {
        config_diffs.push(format!(
            "service manager: {} vs {}",
            left_manager.label(),
            right_manager.label()
        ));
    }

    if config_diffs.is_empty() {
        out.push("Config drift: none".to_string());
    } else {
        out.push("Config drift:".to_string());
        for diff in config_diffs {
            out.push(format!("- {}", diff));
        }
    }

    let mut runtime_diffs = Vec::new();
    for key in ["OS", "Node.js", "Python", "Rust", "Docker"] {
        let left_value = left_preflight.get(key).cloned().unwrap_or_else(|| "unknown".to_string());
        let right_value = right_preflight.get(key).cloned().unwrap_or_else(|| "unknown".to_string());
        if left_value != right_value {
            runtime_diffs.push(format!("{}: {} vs {}", key, left_value, right_value));
        }
    }

    if runtime_diffs.is_empty() {
        out.push("Runtime drift: none".to_string());
    } else {
        out.push("Runtime drift:".to_string());
        for diff in runtime_diffs {
            out.push(format!("- {}", diff));
        }
    }

    let mut risks = Vec::new();
    if left.require_confirmation != right.require_confirmation {
        risks.push("Different confirmation policy means automation safety differs by target.".to_string());
    }
    if left.remote_path != right.remote_path {
        risks.push("Different deploy roots increase path-sensitive bug risk.".to_string());
    }
    if left_manager != right_manager {
        risks.push("Different service managers require different restart/debug assumptions.".to_string());
    }
    if left_preflight.get("Node.js") != right_preflight.get("Node.js") {
        risks.push("Node.js versions differ; build or runtime behavior may diverge.".to_string());
    }

    if risks.is_empty() {
        out.push("Compatibility risks: none detected from current config and runtime facts.".to_string());
    } else {
        out.push("Compatibility risks:".to_string());
        for risk in risks {
            out.push(format!("- {}", risk));
        }
    }

    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_service_manager_systemd() {
        let manager = infer_service_manager(Some("sudo systemctl restart myapp.service"));
        assert_eq!(
            manager,
            ServiceManager::Systemd {
                service: "myapp".to_string()
            }
        );
    }

    #[test]
    fn test_infer_service_manager_pm2() {
        let manager = infer_service_manager(Some("pm2 restart web --update-env"));
        assert_eq!(
            manager,
            ServiceManager::Pm2 {
                process: "web".to_string()
            }
        );
    }

    #[test]
    fn test_parse_sections_groups_content() {
        let raw = "__RB_SECTION__:meta\none\n__RB_SECTION__:logs\ntwo\nthree\n";
        let sections = parse_sections(raw);
        assert_eq!(sections["meta"], "one");
        assert_eq!(sections["logs"], "two\nthree");
    }

    #[test]
    fn test_collect_signal_hits_detects_known_failure() {
        let hits = collect_signal_hits("Error: listen EADDRINUSE: address already in use 0.0.0.0:3000");
        assert_eq!(hits[0].0, "Port conflict");
    }
}
