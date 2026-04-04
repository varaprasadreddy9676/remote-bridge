use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use crate::config::{find_config, load_config};
use crate::executor::Executor;
use crate::insights::{compare_targets, diagnose_failure};

#[derive(Debug, Deserialize)]
struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

fn send_response(response: Value) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout();
    writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
    stdout.flush()?;
    Ok(())
}

pub fn run_mcp_server() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: McpRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(_) => continue,
        };

        // Notifications have no id — do not send a response
        let is_notification = request.id.is_none();

        match request.method.as_str() {
            "initialize" => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "remote-bridge",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                });
                send_response(response)?;
            }
            "notifications/initialized" => {
                // Notification — no response required
            }
            "tools/list" => {
                if is_notification { continue; }
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.id,
                    "result": {
                        "tools": [
                            {
                                "name": "sync_to_remote",
                                "description": "Syncs a local directory to the remote server via rsync over SSH. If the target has require_confirmation=true in remotebridge.yaml, a dry-run preview is returned and you MUST re-call with confirm=true to proceed. WARNING: when delete=true, remote files not present locally are permanently deleted — always set local_path explicitly.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "target": {
                                            "type": "string",
                                            "description": "Target name from remotebridge.yaml (default: staging)"
                                        },
                                        "local_path": {
                                            "type": "string",
                                            "description": "Local directory to sync (default: current working directory '.'). Set this explicitly to avoid syncing the wrong directory."
                                        },
                                        "dry_run": {
                                            "type": "boolean",
                                            "description": "If true, preview what would be synced without transferring any files (default: false)"
                                        },
                                        "delete": {
                                            "type": "boolean",
                                            "description": "If true, delete remote files that do not exist locally (default: false). DESTRUCTIVE — only set this when you intend a full mirror sync."
                                        },
                                        "confirm": {
                                            "type": "boolean",
                                            "description": "Required when the target has require_confirmation=true. Review the dry-run output first, then re-call with confirm=true to execute."
                                        }
                                    }
                                }
                            },
                            {
                                "name": "run_remote_command",
                                "description": "Executes a shell command on the remote server and returns stdout/stderr. Combined output is truncated to max_lines (default 100) to avoid filling context.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "command": {
                                            "type": "string",
                                            "description": "Shell command to execute on the remote host"
                                        },
                                        "target": {
                                            "type": "string",
                                            "description": "Target name from remotebridge.yaml (default: staging)"
                                        },
                                        "max_lines": {
                                            "type": "integer",
                                            "description": "Maximum number of output lines to return (default: 100, use 0 for unlimited)"
                                        }
                                    },
                                    "required": ["command"]
                                }
                            },
                            {
                                "name": "preflight_check",
                                "description": "Checks the remote environment for OS version and available runtimes",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "target": {
                                            "type": "string",
                                            "description": "Target name from remotebridge.yaml (default: staging)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "fetch_logs",
                                "description": "Fetches recent lines from the remote log files configured for the target",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "target": {
                                            "type": "string",
                                            "description": "Target name from remotebridge.yaml (default: staging)"
                                        },
                                        "lines": {
                                            "type": "integer",
                                            "description": "Number of log lines to fetch (default: 50)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "restart_service",
                                "description": "Restarts the remote service using the restart_cmd defined in remotebridge.yaml",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "target": {
                                            "type": "string",
                                            "description": "Target name from remotebridge.yaml (default: staging)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "deploy",
                                "description": "Full deploy pipeline: sync files, restart service, tail logs on failure",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "target": {
                                            "type": "string",
                                            "description": "Target name from remotebridge.yaml (default: staging)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "diagnose_failure",
                                "description": "Collects a compact, config-aware failure diagnosis bundle for a target: runtime facts, service-manager status, listeners, disk, memory, and recent logs, then summarizes likely causes.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "target": {
                                            "type": "string",
                                            "description": "Target name from remotebridge.yaml (default: staging)"
                                        },
                                        "lines": {
                                            "type": "integer",
                                            "description": "Recent lines to inspect from each configured log file (default: 40, clamped 10-200)"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "compare_targets",
                                "description": "Compares two configured targets using both local config and remote runtime facts, highlighting drift and compatibility risks.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "left": {
                                            "type": "string",
                                            "description": "First target name from remotebridge.yaml"
                                        },
                                        "right": {
                                            "type": "string",
                                            "description": "Second target name from remotebridge.yaml"
                                        }
                                    },
                                    "required": ["left", "right"]
                                }
                            }
                        ]
                    }
                });
                send_response(response)?;
            }
            "tools/call" => {
                if is_notification { continue; }
                if let Some(params) = request.params {
                    let tool_name = params["name"].as_str().unwrap_or("");
                    let tool_args = &params["arguments"];
                    let target = tool_args["target"].as_str().unwrap_or("staging");

                    let result: Result<String, Box<dyn std::error::Error>> = match tool_name {
                        "sync_to_remote" => {
                            let local_path = tool_args["local_path"].as_str().unwrap_or(".");
                            let dry_run = tool_args["dry_run"].as_bool().unwrap_or(false);
                            let delete = tool_args["delete"].as_bool().unwrap_or(false);
                            let confirm = tool_args["confirm"].as_bool().unwrap_or(false);
                            handle_sync(target, local_path, dry_run, delete, confirm)
                        }
                        "run_remote_command" => {
                            let cmd = tool_args["command"].as_str().unwrap_or("");
                            let max_lines = tool_args["max_lines"].as_u64().unwrap_or(100) as usize;
                            if cmd.is_empty() {
                                Err("Missing required argument: command".into())
                            } else {
                                handle_run(target, cmd, max_lines)
                            }
                        }
                        "preflight_check" => handle_preflight(target),
                        "fetch_logs" => {
                            let lines = tool_args["lines"].as_u64().unwrap_or(50) as usize;
                            handle_fetch_logs(target, lines)
                        }
                        "restart_service" => handle_restart(target),
                        "deploy" => handle_deploy(target),
                        "diagnose_failure" => {
                            let lines = tool_args["lines"].as_u64().unwrap_or(40) as usize;
                            handle_diagnose(target, lines)
                        }
                        "compare_targets" => {
                            let left = tool_args["left"].as_str().unwrap_or("");
                            let right = tool_args["right"].as_str().unwrap_or("");
                            if left.is_empty() || right.is_empty() {
                                Err("Missing required arguments: left and right".into())
                            } else {
                                handle_compare(left, right)
                            }
                        }
                        _ => Err(format!("Unknown tool: {}", tool_name).into()),
                    };

                    let response = match result {
                        Ok(output) => json!({
                            "jsonrpc": "2.0",
                            "id": request.id,
                            "result": {
                                "content": [{ "type": "text", "text": output }]
                            }
                        }),
                        Err(e) => json!({
                            "jsonrpc": "2.0",
                            "id": request.id,
                            "error": { "code": -32000, "message": e.to_string() }
                        }),
                    };
                    send_response(response)?;
                }
            }
            _ => {
                if !is_notification {
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": request.id,
                        "error": { "code": -32601, "message": "Method not found" }
                    });
                    send_response(response)?;
                }
            }
        }
    }
    Ok(())
}

fn handle_sync(target: &str, local_path: &str, dry_run: bool, delete: bool, confirm: bool) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config(find_config()?)?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;

    // If the target requires confirmation and the caller hasn't confirmed,
    // run a dry-run and return the preview — refuse to sync.
    if target_cfg.require_confirmation && !confirm {
        let executor = Executor::new(target_cfg.clone());
        let args = executor.get_transport().build_rsync_args(local_path, &[".git/".to_string()], true, delete);
        let output = std::process::Command::new("rsync").args(&args).output()?;
        let preview = String::from_utf8_lossy(&output.stdout);
        let preview = preview.trim();
        let preview = if preview.is_empty() { "(nothing to sync — remote is already up to date)" } else { preview };
        return Err(format!(
            "This target requires confirmation before syncing.\n\
             Dry-run preview:\n{}\n\
             Re-call sync_to_remote with confirm=true to proceed.",
            preview
        ).into());
    }

    let executor = Executor::new(target_cfg.clone());
    executor.get_transport().sync_files(local_path, vec![".git/".to_string()], dry_run, delete)?;
    if dry_run {
        Ok("Dry run complete — no files were transferred.".to_string())
    } else {
        Ok("Sync complete".to_string())
    }
}

fn handle_restart(target: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config(find_config()?)?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    executor.restart()?;
    Ok("Service restarted successfully.".to_string())
}

fn handle_deploy(target: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config(find_config()?)?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    executor.deploy(false)?;
    Ok("Deploy complete.".to_string())
}

fn truncate_to_last_lines(text: &str, keep_lines: usize) -> String {
    if keep_lines == 0 || text.trim().is_empty() {
        return String::new();
    }

    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= keep_lines {
        return text.to_string();
    }

    let kept = &lines[lines.len() - keep_lines..];
    format!(
        "[...truncated, showing last {} of {} lines]\n{}",
        keep_lines,
        lines.len(),
        kept.join("\n")
    )
}

fn allocate_output_budget(stdout: &str, stderr: &str, max_lines: usize, exit_code: i32) -> (usize, usize) {
    let stdout_lines = stdout.lines().count();
    let stderr_lines = stderr.lines().count();
    let total_lines = stdout_lines + stderr_lines;

    if max_lines == 0 || total_lines <= max_lines {
        return (stdout_lines, stderr_lines);
    }

    if stdout_lines == 0 {
        return (0, max_lines.min(stderr_lines));
    }

    if stderr_lines == 0 {
        return (max_lines.min(stdout_lines), 0);
    }

    let stderr_reserved = if exit_code == 0 {
        stderr_lines.min(10).min(max_lines)
    } else {
        (max_lines * 2 / 3).max(1).min(stderr_lines)
    };
    let stdout_reserved = max_lines.saturating_sub(stderr_reserved).min(stdout_lines);

    let mut stdout_budget = stdout_reserved;
    let mut stderr_budget = stderr_reserved;
    let mut remaining = max_lines.saturating_sub(stdout_budget + stderr_budget);

    while remaining > 0 {
        let stdout_missing = stdout_lines.saturating_sub(stdout_budget);
        let stderr_missing = stderr_lines.saturating_sub(stderr_budget);

        if exit_code != 0 && stderr_missing > 0 {
            stderr_budget += 1;
        } else if stdout_missing > 0 {
            stdout_budget += 1;
        } else if stderr_missing > 0 {
            stderr_budget += 1;
        } else {
            break;
        }
        remaining -= 1;
    }

    (stdout_budget, stderr_budget)
}

fn format_command_output(exit_code: i32, stdout: &str, stderr: &str, max_lines: usize) -> String {
    let (stdout_budget, stderr_budget) = allocate_output_budget(stdout, stderr, max_lines, exit_code);
    let stdout_trimmed = truncate_to_last_lines(stdout, stdout_budget);
    let stderr_trimmed = truncate_to_last_lines(stderr, stderr_budget);

    let mut sections = vec![format!("Exit Code: {}", exit_code)];
    if !stdout_trimmed.trim().is_empty() {
        sections.push(format!("STDOUT:\n{}", stdout_trimmed));
    }
    if !stderr_trimmed.trim().is_empty() {
        sections.push(format!("STDERR:\n{}", stderr_trimmed));
    }
    sections.join("\n")
}

fn handle_run(target: &str, command: &str, max_lines: usize) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config(find_config()?)?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    let (code, stdout, stderr) = executor.get_transport().run_remote_command(command)?;
    Ok(format_command_output(code, &stdout, &stderr, max_lines))
}

fn handle_preflight(target: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config(find_config()?)?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    executor.get_transport().preflight_check()
}

fn handle_fetch_logs(target: &str, lines: usize) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config(find_config()?)?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    executor.get_transport().fetch_logs(lines)
}

fn handle_diagnose(target: &str, lines: usize) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config(find_config()?)?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    diagnose_failure(target, target_cfg, executor.get_transport(), lines)
}

fn handle_compare(left: &str, right: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config(find_config()?)?;
    let left_cfg = config.targets.get(left).ok_or(format!("Target {} not found", left))?;
    let right_cfg = config.targets.get(right).ok_or(format!("Target {} not found", right))?;
    compare_targets(left, left_cfg, right, right_cfg)
}

// ── MCP protocol helpers exposed for testing ─────────────────────────────────

pub fn build_initialize_response(id: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "remote-bridge",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

pub fn build_tools_list_response(id: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
                { "name": "sync_to_remote" },
                { "name": "run_remote_command" },
                { "name": "preflight_check" },
                { "name": "fetch_logs" },
                { "name": "restart_service" },
                { "name": "deploy" },
                { "name": "diagnose_failure" },
                { "name": "compare_targets" }
            ]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_initialize_response_protocol_version() {
        let resp = build_initialize_response(Some(json!(1)));
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    }

    #[test]
    fn test_initialize_response_server_info() {
        let resp = build_initialize_response(Some(json!(1)));
        assert_eq!(resp["result"]["serverInfo"]["name"], "remote-bridge");
        assert!(resp["result"]["serverInfo"]["version"].is_string());
    }

    #[test]
    fn test_initialize_response_capabilities() {
        let resp = build_initialize_response(Some(json!(1)));
        assert!(resp["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_tools_list_contains_required_tools() {
        let resp = build_tools_list_response(Some(json!(1)));
        let tools = resp["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"sync_to_remote"));
        assert!(names.contains(&"run_remote_command"));
        assert!(names.contains(&"preflight_check"));
        assert!(names.contains(&"fetch_logs"));
        assert!(names.contains(&"restart_service"));
        assert!(names.contains(&"deploy"));
        assert!(names.contains(&"diagnose_failure"));
        assert!(names.contains(&"compare_targets"));
    }

    #[test]
    fn test_mcp_request_parse_initialize() {
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let req: McpRequest = serde_json::from_str(line).unwrap();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(json!(1)));
    }

    #[test]
    fn test_mcp_request_parse_notification() {
        let line = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let req: McpRequest = serde_json::from_str(line).unwrap();
        assert_eq!(req.method, "notifications/initialized");
        assert!(req.id.is_none());
    }

    #[test]
    fn test_mcp_request_parse_tools_call() {
        let line = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"run_remote_command","arguments":{"command":"ls","target":"staging"}}}"#;
        let req: McpRequest = serde_json::from_str(line).unwrap();
        assert_eq!(req.method, "tools/call");
        let params = req.params.unwrap();
        assert_eq!(params["name"], "run_remote_command");
        assert_eq!(params["arguments"]["command"], "ls");
    }

    #[test]
    fn test_send_response_valid_json() {
        // Ensure serde_json can round-trip an MCP response
        let resp = build_initialize_response(Some(json!(42)));
        let serialized = serde_json::to_string(&resp).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["id"], 42);
        assert_eq!(parsed["jsonrpc"], "2.0");
    }
}
