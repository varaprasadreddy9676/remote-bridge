# RemoteBridge 🌉

**A safe, configurable MCP tool for giving AI agents reliable access to remote servers.**

RemoteBridge is a Rust CLI and MCP server for AI-assisted remote workflows. It syncs local code with `rsync`, runs remote commands over SSH, gathers logs and diagnostics, compares environments, and exposes all of that through a compact tool surface your AI can use directly.

Use it when you want an AI agent in Claude Code, Cursor, Windsurf, Codex, or another MCP-enabled tool to work against a remote machine with real operational context, not just a shell prompt.

Instead of making the model guess how your server is set up, RemoteBridge gives it a configured operational interface.

## What RemoteBridge Helps The AI Understand

When an AI is debugging or deploying on a remote machine, it needs more than "run this command".

It needs to understand things like:

- where the app is deployed
- how the service is restarted
- which log files matter
- what runtimes are installed
- whether the service is healthy
- which ports are listening
- whether disk or memory pressure is involved
- how staging differs from production
- which commands are allowed or dangerous

RemoteBridge stores those recurring server facts in `remotebridge.yaml` and exposes the useful workflows as MCP tools.

That lets the AI work with a remote server as a system it can understand, not just a place where commands happen to run.

## What Problem This Actually Solves

Without a structured tool layer, remote work from AI usually turns into a noisy sequence of shell steps:

- find the right host
- find the deploy path
- inspect the runtime
- find the restart command
- find the logs
- tail too much output
- ask more questions to recover context

RemoteBridge compresses that into a smaller set of reusable operations with clearer intent and better defaults.

Your AI can call:

- `sync_to_remote`
- `deploy`
- `preflight_check`
- `fetch_logs`
- `diagnose_failure`
- `compare_targets`

These tools are designed to help the AI inspect and reason about the remote system, not just execute commands on it.

For example, `diagnose_failure` can gather:

- runtime facts
- service-manager state
- recent logs
- listeners
- disk status
- memory state
- likely failure signatures

in one compact response.

## Small Note On Direct SSH

Direct SSH from an AI tool is still useful when:

- You want a fully interactive shell.
- You already know the exact command.
- You want one-off ad hoc exploration.
- You are doing a one-off urgent command and want raw terminal behavior.

RemoteBridge is better when:

- The AI is driving the workflow.
- The same server facts keep getting rediscovered.
- You want deploy, restart, diagnosis, and log workflows to be repeatable.
- You need guardrails around destructive commands.
- You want compact answers instead of full shell transcripts.
- You want config-aware operations like "compare staging and production".

## How RemoteBridge Is Practically Different

RemoteBridge is not better because it hides SSH. It is better because it turns repeated infrastructure reasoning into stable, reusable tool behavior.

Concrete differences from direct SSH:

- The server path is configured once in `remotebridge.yaml` and reused on every call.
- The restart command is configured once instead of being rediscovered or retyped.
- The log files are configured once instead of the model hunting for them.
- The model can call `deploy`, `diagnose_failure`, and `compare_targets` as intent-level operations.
- The output is deliberately bounded so the model receives signal instead of log floods.
- Dangerous commands can be confirmed, blocked, or allowlisted.
- Local code changes and remote execution are part of one workflow instead of two disconnected steps.

This is why RemoteBridge is usually more useful than raw shell access inside an AI agent, even when the AI agent technically supports remote access already.

## Why This Saves Tokens In Practice

RemoteBridge is useful when it removes low-value conversation between the model and the server.

It saves tokens in a few concrete ways:

- `run_remote_command` truncates combined `stdout` and `stderr` to a shared line budget, so the model gets the important tail instead of a full transcript.
- `preflight_check` gathers OS and runtime facts in one remote call instead of making the AI issue several separate shell commands.
- `fetch_logs` pulls configured log files in one structured response instead of the AI repeatedly asking where logs live and tailing each file manually.
- `diagnose_failure` gathers service state, listeners, disk, memory, runtime info, and recent logs in one pass, then summarizes likely causes.
- `compare_targets` turns "SSH into staging, SSH into prod, inspect both, diff mentally" into one semantic tool call.
- `deploy` and `restart_service` remove the need for the AI to restate shell glue like `cd /path && ...` every time.
- Config values such as `remote_path`, `restart_cmd`, `logs`, `allowed_commands`, and `blocked_patterns` are stored once and reused on every call.

The important point is not just "fewer SSH commands." It is:

- less repeated reasoning around the same infrastructure facts
- less output the model has to read
- fewer follow-up prompts caused by noisy shell transcripts

## Why This Is Better Than Letting AI Freestyle Shell

Raw SSH gives the AI a lot of power, but very little structure.

RemoteBridge adds:

- Config awareness: target host, deploy path, restart command, and log paths live in `remotebridge.yaml`.
- Operational semantics: tools like `deploy`, `diagnose_failure`, and `compare_targets` describe intent, not just shell syntax.
- Safer defaults: dangerous commands can require confirmation, be blocked entirely, or be limited to allowlisted prefixes.
- Better output discipline: the MCP surface returns compressed, useful output instead of dumping everything.
- Local + remote coordination: syncing local files to a remote server is part of the same workflow instead of a separate manual step.

That last point matters. A remote shell alone cannot see your local uncommitted code. RemoteBridge can sync exactly what the AI changed locally, then execute the remote step that depends on it.

## Core Value

RemoteBridge is not trying to beat SSH at being a shell.

It is trying to give AI agents a better abstraction than:

1. guess the host
2. guess the path
3. guess the service manager
4. guess the logs
5. dump too much output
6. try again

## Why We Built This In Rust

RemoteBridge could have been written in Node.js or Python. We chose Rust for practical reasons tied to the job this tool does.

Rust helps here because:

- it produces a single native binary with fast startup, which is useful for CLI and stdio MCP usage
- it avoids requiring users to manage a Python runtime or a larger JS runtime stack at execution time
- it is a good fit for process-heavy, I/O-heavy tooling such as `ssh`, `rsync`, log streaming, and MCP stdio handling
- it gives strong compile-time guarantees around error handling and data flow, which matters for deployment and remote execution tooling
- it tends to have predictable performance and low overhead for repeated tool calls

For this project specifically, that means:

- faster startup for MCP clients launching the server
- less packaging friction for a CLI that people want to install and use immediately
- a native binary that is well suited to wrapping system tools like `ssh` and `rsync`
- a smaller risk surface for runtime dependency issues compared with large script-runtime stacks

Node.js or Python would have been workable. Rust is a better fit for a tool whose job is to be a reliable systems bridge between an AI client and remote infrastructure.

## Features

| Feature | Why it matters |
| :--- | :--- |
| **Rsync Sync** | Push local code to the server without re-uploading everything |
| **SSH Multiplexing** | Reuses OpenSSH control connections across repeated sync and command calls |
| **Structured MCP Tools** | Lets the AI call semantic actions instead of building shell strings each time |
| **Output Truncation** | Keeps command responses small enough to stay useful in model context |
| **Preflight Check** | Captures runtime facts in one step |
| **Deploy Pipeline** | Encodes sync → restart → failure-log flow as one operation |
| **Failure Diagnosis** | Pulls service state, runtime facts, listeners, disk, memory, and logs into one compact summary |
| **Target Comparison** | Surfaces config drift and runtime drift between environments |
| **Safety Gates** | Confirmation, hard blocks, and allowlists reduce destructive mistakes |
| **Audit Log** | Records what actually ran and how it exited |
| **Watch Mode** | Keeps a remote target in sync during an edit/test loop |
| **Per-target SSH Config** | Supports host/user/path/port/key per environment |

## A Practical Example

Without RemoteBridge, an AI debugging a broken deploy often does something like this:

1. SSH in.
2. Ask for `pwd`.
3. Ask for `ls`.
4. Try to find the app path.
5. Guess how to restart the service.
6. Ask for logs.
7. Pull too much output.
8. Ask follow-up questions because the output was noisy.

With RemoteBridge, the same request can be:

**You:** "The staging deploy is broken. Diagnose it."

The AI can call `diagnose_failure` and get back:

- target identity
- inferred service manager
- service health
- runtime versions
- disk or memory pressure signals
- relevant log excerpts
- likely causes
- next-step suggestions

That is a much better use of context than a multi-turn shell transcript.

## Quick Start

### 1. Install

Prerequisites: Rust, `rsync`, and `ssh` available in `PATH`.

```bash
npm install -g remote-bridge-cli
```

### 2. Add the MCP server to your AI tool

```bash
claude mcp add remote-bridge --scope user -- remote-bridge mcp
```

See [MCP Support](#mcp-support) below for other tools.

### 3. Initialize project config

```bash
remote-bridge init --name my-app -H your-server.com --user ubuntu --path /var/www/app
```

That creates `remotebridge.yaml`, which becomes the shared source of truth for your AI workflows.

## Example Configuration

```yaml
project_name: "my-app"
targets:
  staging:
    host: "13.234.xx.xx"
    user: "ubuntu"
    remote_path: "/var/www/html/app"
    port: 22
    ssh_key: "~/.ssh/id_rsa"
    restart_cmd: "pm2 restart app"
    logs:
      - "/var/www/html/app/logs/error.log"
      - "/var/log/nginx/error.log"
    require_confirmation: false
    exclude:
      - "node_modules/"
      - "*.log"
    blocked_patterns:
      - "rm -rf"
      - "drop table"
    allowed_commands:
      - "npm"
      - "pm2"
    audit_log: "~/.remote-bridge-staging.log"
  production:
    host: "prod.example.com"
    user: "ubuntu"
    remote_path: "/opt/app"
    ssh_key: "~/keys/prod.pem"
    restart_cmd: "systemctl restart myapp"
    logs:
      - "/opt/app/logs/error.log"
    require_confirmation: true
```

## Why `remotebridge.yaml` Matters

This file is the reason the MCP tool is more useful than raw SSH.

It stores facts the AI should not have to rediscover:

- where the app lives
- how it restarts
- which logs matter
- which commands are safe
- which environment needs stronger confirmation

Once that information is encoded once, every future tool call gets simpler and cheaper.

## CLI Commands

| Command | Description |
| :--- | :--- |
| `init` | Create a new `remotebridge.yaml` |
| `sync` | Sync local files to the remote server |
| `sync --dry-run` | Preview rsync changes without touching the server |
| `run <cmd>` | Execute one remote command |
| `preflight` | Collect remote OS and runtime versions |
| `logs` | Fetch recent configured logs |
| `logs --follow` | Stream configured logs live |
| `restart` | Restart the configured service |
| `deploy` | Sync, restart, and fetch failure context if restart fails |
| `watch` | Poll local files and auto-sync on change |
| `apply` | Parse Markdown from AI output and apply file changes plus shell commands |

## Integrating With AI CLIs

RemoteBridge is also pipe-friendly.

### Claude Code

```bash
claude "Fix the database connection in src/db.ts and restart the app" --non-interactive \
  | remote-bridge apply --target staging
```

### Gemini CLI

```bash
gemini "Add rate limiting to the Express API" \
  | remote-bridge apply --target staging
```

### Aider

```bash
aider --message "Refactor the login logic" --apply \
  | remote-bridge apply --target staging
```

## MCP Support

RemoteBridge is a native MCP server. Any MCP-compatible AI IDE can use it.

### Available MCP Tools

| Tool | Practical value |
| :--- | :--- |
| `sync_to_remote` | Push local code to the server from the same AI session |
| `run_remote_command` | Run remote shell commands with bounded output |
| `preflight_check` | Get runtime facts in one compact response |
| `fetch_logs` | Pull configured logs without rediscovering paths |
| `restart_service` | Reuse the configured restart command safely |
| `deploy` | Run the standard remote deploy flow as one action |
| `diagnose_failure` | Collect and summarize failure context instead of streaming raw shell debugging |
| `compare_targets` | Compare environments using both config and live runtime facts |

### What These Tools Let AI Do Better Than Raw SSH

`diagnose_failure` is the clearest example.

A raw SSH agent has to decide:

- which service manager exists
- how to inspect it
- which logs to read
- how many lines to tail
- whether disk, memory, or port state might be relevant
- which lines are likely signal versus noise

`diagnose_failure` bakes that investigation into one operation.

`compare_targets` is another example. SSH can inspect one server at a time. This tool compares:

- host/path/config differences
- confirmation-policy drift
- service-manager differences
- runtime version differences
- high-level compatibility risks

That is not impossible with SSH. It is just expensive and repetitive for AI.

### Example Requests

Inside an MCP-enabled IDE, you can say:

- "Sync my current project to staging."
- "Deploy the latest local changes to staging."
- "Check what runtimes are installed on production."
- "The app failed after deploy. Diagnose staging."
- "Compare staging and production and tell me if runtime drift could explain the bug."
- "Show me the last 100 log lines for production."

## MCP Configuration Examples

### Claude Desktop

File: `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "remote-bridge": {
      "command": "remote-bridge",
      "args": ["mcp"]
    }
  }
}
```

### Cursor

File: `~/.cursor/mcp.json` or `.cursor/mcp.json`

```json
{
  "mcpServers": {
    "remote-bridge": {
      "command": "remote-bridge",
      "args": ["mcp"]
    }
  }
}
```

### VS Code / Cline / Continue / Copilot-compatible MCP clients

```json
{
  "servers": {
    "remote-bridge": {
      "type": "stdio",
      "command": "remote-bridge",
      "args": ["mcp"]
    }
  }
}
```

### Codex CLI

File: `~/.codex/config.json`

```json
{
  "mcpServers": {
    "remote-bridge": {
      "command": "remote-bridge",
      "args": ["mcp"]
    }
  }
}
```

Or inline:

```bash
codex --mcp-server "remote-bridge mcp" "Diagnose the staging deploy failure"
```

### Generic MCP Pattern

```json
{
  "command": "remote-bridge",
  "args": ["mcp"],
  "transport": "stdio"
}
```

## SSH Authentication

RemoteBridge uses your existing SSH setup.

1. Copy your key to the server:

   ```bash
   ssh-copy-id -i ~/.ssh/id_rsa.pub ubuntu@your-server.com
   ```

2. Or specify key and port in config:

   ```yaml
   targets:
     staging:
       host: "13.234.xx.xx"
       user: "ubuntu"
       port: 2222
       ssh_key: "~/keys/staging.pem"
   ```

3. Or use `~/.ssh/config` aliases:

   ```text
   Host staging-server
       HostName 13.234.xx.xx
       User ubuntu
       IdentityFile ~/keys/my-key.pem
       Port 22
   ```

   Then point `host` at the alias:

   ```yaml
   targets:
     staging:
       host: "staging-server"
       user: "ubuntu"
   ```

## Safety Model

RemoteBridge is designed for AI-driven execution, so safety is not optional.

### Confirmation Gate

Commands containing risky patterns such as `sudo`, `rm`, `drop`, `delete`, `shutdown`, `reboot`, `killall`, `curl | bash`, and similar destructive sequences require confirmation before execution.

### Hard Block

Anything in `blocked_patterns` is always rejected:

```yaml
targets:
  production:
    blocked_patterns:
      - "rm -rf"
      - "drop table"
      - "truncate"
```

### Allowlist

If `allowed_commands` is set, only matching prefixes are allowed:

```yaml
targets:
  production:
    allowed_commands:
      - "npm"
      - "pm2"
      - "systemctl restart myapp"
```

### Audit Log

Every command is logged automatically:

```text
[1742660591] host=your-server.com path=/var/www/app exit=0 cmd=npm install
[1742660612] host=your-server.com path=/var/www/app exit=-2 cmd=rm -rf /var/data
```

Exit codes:

- `0+` actual process exit code
- `-1` skipped by user
- `-2` hard blocked
- `-3` not allowlisted

### Other Protections

- `require_confirmation: true` can force confirmation on every command for sensitive targets.
- `sync --dry-run` previews what will change before syncing.
- No passwords are stored by RemoteBridge.
- No secrets need to live in `remotebridge.yaml`.
- MCP output stays bounded so one noisy command does not flood model context.

## Honest Tradeoff

If you want a raw terminal, use SSH.

If you want an AI to reliably work with remote infrastructure without repeatedly wasting tokens on rediscovery, shell glue, and noisy output, use RemoteBridge.

That is the value proposition.

## Contributing

Contributions and issues are welcome at [GitHub Issues](https://github.com/varaprasadreddy9676/remote-bridge/issues).

## License

MIT License.
