use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead};
use crate::config::load_config;
use crate::executor::Executor;
use crate::parser::ShellCommand;

#[derive(Debug, Deserialize)]
struct McpRequest {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

pub fn run_mcp_server() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let request: McpRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(_) => continue,
        };

        match request.method.as_str() {
            "initialize" => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.id,
                    "result": {
                        "protocolVersion": "0.1.0",
                        "capabilities": {
                            "tools": {}
                        }
                    }
                });
                println!("{}", serde_json::to_string(&response)?);
            }
            "listTools" => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.id,
                    "result": {
                        "tools": [
                            {
                                "name": "sync_to_remote",
                                "description": "Syncs local files to the remote server",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "target": { "type": "string", "default": "staging" }
                                    }
                                }
                            },
                            {
                                "name": "run_remote_command",
                                "description": "Executes a shell command on the remote server",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "command": { "type": "string" },
                                        "target": { "type": "string", "default": "staging" }
                                    },
                                    "required": ["command"]
                                }
                            }
                        ]
                    }
                });
                println!("{}", serde_json::to_string(&response)?);
            }
            "callTool" => {
                if let Some(params) = request.params {
                    let tool_name = params["name"].as_str().unwrap_or("");
                    let tool_args = &params["arguments"];
                    let target = tool_args["target"].as_str().unwrap_or("staging");

                    let result = match tool_name {
                        "sync_to_remote" => handle_sync(target),
                        "run_remote_command" => {
                            let cmd = tool_args["command"].as_str().unwrap_or("");
                            handle_run(target, cmd)
                        }
                        _ => Err("Unknown tool".into()),
                    };

                    let response = match result {
                        Ok(output) => json!({
                            "jsonrpc": "2.0",
                            "id": request.id,
                            "result": { "content": [{ "type": "text", "text": output }] }
                        }),
                        Err(e) => json!({
                            "jsonrpc": "2.0",
                            "id": request.id,
                            "error": { "code": -32000, "message": e.to_string() }
                        }),
                    };
                    println!("{}", serde_json::to_string(&response)?);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn handle_sync(target: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config("remotebridge.yaml")?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    executor.get_transport().sync_files(".", vec![".git/".to_string()])?;
    Ok("Sync complete".to_string())
}

fn handle_run(target: &str, command: &str) -> Result<String, Box<dyn std::error::Error>> {
    let config = load_config("remotebridge.yaml")?;
    let target_cfg = config.targets.get(target).ok_or(format!("Target {} not found", target))?;
    let executor = Executor::new(target_cfg.clone());
    let (code, stdout, stderr) = executor.get_transport().run_remote_command(command)?;
    Ok(format!("Exit Code: {}\nSTDOUT: {}\nSTDERR: {}", code, stdout, stderr))
}
