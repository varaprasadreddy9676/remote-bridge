use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use crate::config::load_config;
use crate::executor::Executor;

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
                                "description": "Syncs local files to the remote server via rsync over SSH",
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
                                "name": "run_remote_command",
                                "description": "Executes a shell command on the remote server and returns stdout/stderr",
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
                        "sync_to_remote" => handle_sync(target),
                        "run_remote_command" => {
                            let cmd = tool_args["command"].as_str().unwrap_or("");
                            if cmd.is_empty() {
                                Err("Missing required argument: command".into())
                            } else {
                                handle_run(target, cmd)
                            }
                        }
                        "preflight_check" => handle_preflight(target),
                        "fetch_logs" => {
                            let lines = tool_args["lines"].as_u64().unwrap_or(50) as usize;
                            handle_fetch_logs(target, lines)
                        }
                        "restart_service" => handle_restart(target),
                        "deploy" => handle_deploy(target),
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

fn handle_sync(target: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config("remotebridge.yaml")?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    executor.get_transport().sync_files(".", vec![".git/".to_string()], false)?;
    Ok("Sync complete".to_string())
}

fn handle_restart(target: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config("remotebridge.yaml")?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    executor.restart()?;
    Ok("Service restarted successfully.".to_string())
}

fn handle_deploy(target: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config("remotebridge.yaml")?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    executor.deploy(false)?;
    Ok("Deploy complete.".to_string())
}

fn handle_run(target: &str, command: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config("remotebridge.yaml")?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    let (code, stdout, stderr) = executor.get_transport().run_remote_command(command)?;
    Ok(format!("Exit Code: {}\nSTDOUT: {}\nSTDERR: {}", code, stdout, stderr))
}

fn handle_preflight(target: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config("remotebridge.yaml")?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    let transport = executor.get_transport();

    let mut output = String::new();

    let (_, os, _) = transport.run_remote_command(
        "lsb_release -d 2>/dev/null || grep PRETTY_NAME /etc/os-release 2>/dev/null | cut -d= -f2 | tr -d '\"'",
    )?;
    output.push_str(&format!("OS: {}\n", os.trim()));

    let (code, node, _) = transport.run_remote_command("node -v")?;
    if code == 0 {
        output.push_str(&format!("Node.js: {}\n", node.trim()));
    } else {
        output.push_str("Node.js: Not found\n");
    }

    let (code, python, _) = transport.run_remote_command("python3 --version")?;
    if code == 0 {
        output.push_str(&format!("Python: {}\n", python.trim()));
    } else {
        output.push_str("Python: Not found\n");
    }

    Ok(output)
}

fn handle_fetch_logs(target: &str, lines: usize) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config("remotebridge.yaml")?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    executor.get_transport().fetch_logs(lines)
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
                { "name": "deploy" }
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
