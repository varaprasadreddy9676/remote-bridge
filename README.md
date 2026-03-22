# RemoteBridge 🌉

**Bridge the gap between AI-generated code and remote servers.**

RemoteBridge is a high-performance CLI tool written in Rust that acts as a stateful proxy between your local AI coding environment (Claude Code, Gemini CLI, Aider, etc.) and your remote infrastructure. It parses Markdown output from AI tools, syncs file changes via `rsync`, executes remote commands over SSH, and pipes logs back to your terminal — so the AI can "see" remote errors and fix them.

---

## 💬 Just Talk To Your AI

Once installed, you don't run commands manually. You just tell your AI what you want — in plain English — and RemoteBridge handles it.

```
You:  "Sync my project files to the staging server"
You:  "Run npm install on the remote server"
You:  "Deploy my latest changes to ubuntu@your-server.com"
You:  "Check what OS and runtimes are installed on the server"
You:  "Tail the remote logs and show me what's failing"
You:  "Something broke after deploy — fetch the logs and fix it"
You:  "Push my code and restart the app"
```

Your AI calls the right tool, syncs the right files, runs the right commands — and if something fails, it reads the remote logs and fixes the code automatically.

**No manual SSH. No piping. No copy-pasting errors.**

---

## 🚀 The Problem & Solution

**The Problem:** AI coding tools operate locally. Developers without CI/CD must manually FTP files and SSH into servers to test changes. The AI never sees remote runtime errors, creating a "context gap."

**The Solution:** RemoteBridge automates the **Sync → Run → Feedback** loop. It parses AI output, applies file changes locally, syncs them to a remote target, runs commands, and tails remote logs if something fails.

---

## ✨ Features

| Feature | Description |
| :--- | :--- |
| **Markdown Interception** | Extracts ` ```bash ` and ` ```lang filename ` blocks from STDIN automatically |
| **Shadow Syncing** | Ultra-fast delta-based file transfer via `rsync` over SSH |
| **`--dry-run`** | Preview exactly what rsync would transfer before touching anything |
| **Watch Mode** | Polls local files and auto-syncs on any change — live deploy loop |
| **Full Deploy Pipeline** | One command: sync → restart → tail logs on failure |
| **Permission Gate** | Pauses for confirmation on `sudo`, `rm`, database commands, and 20+ dangerous patterns |
| **Hard Block** | User-defined `blocked_patterns` are always rejected — no AI override possible |
| **Allowlist** | `allowed_commands` lets you restrict exactly which commands can ever run |
| **Audit Log** | Every command logged with exit code to `~/.remote-bridge-audit.log` |
| **Pre-flight Check** | Detects remote OS, Node.js, Python, Rust, Docker versions |
| **Log Backfeed** | Auto-tails remote logs when a command fails — AI reads and fixes |
| **MCP Server** | Native Model Context Protocol server for Claude Desktop & other AI IDEs |
| **SSH Key + Port** | Per-target SSH identity file and custom port support |

---

## 🛠 Get Started in 3 Steps

### Step 1 — Install

**Prerequisites:** Rust ([rustup.rs](https://rustup.rs/)), `rsync`, `ssh` in PATH

```bash
npm install -g remote-bridge-cli
```
> Builds a native binary optimized for your machine.

### Step 2 — Add to your AI IDE

```bash
# Claude Code (available in every project automatically)
claude mcp add remote-bridge --scope user -- remote-bridge mcp
```

For other tools see the [MCP configuration section](#-mcp-support--works-with-every-ai-ide) below.

### Step 3 — Point it at your server

Run this once per project:
```bash
remote-bridge init --name my-app -H your-server.com --user ubuntu --path /var/www/app
```

That's it. Now open your AI and just talk to it.

---

### Build from Source
```bash
git clone https://github.com/varaprasadreddy9676/remote-bridge.git
cd remote-bridge
cargo build --release
cp target/release/remote-bridge /usr/local/bin/
```

---

## ⚙️ Configuration (`remotebridge.yaml`)

Initialize a project:
```bash
remote-bridge init --name my-app -H 13.234.xx.xx --user ubuntu --path /var/www/html/app
```

This creates a `remotebridge.yaml`:
```yaml
project_name: "my-app"
targets:
  staging:
    host: "13.234.xx.xx"
    user: "ubuntu"
    remote_path: "/var/www/html/app"
    port: 22                          # optional, default: 22
    ssh_key: "~/.ssh/id_rsa"          # optional, uses default SSH key if omitted
    restart_cmd: "pm2 restart app"    # optional, used by restart & deploy commands
    logs:
      - "/var/www/html/app/logs/error.log"
      - "/var/log/nginx/error.log"
    require_confirmation: false        # set true to confirm every command
    exclude:                           # extra rsync exclusions beyond .gitignore
      - "node_modules/"
      - "*.log"
    blocked_patterns:                  # always rejected — no AI override
      - "rm -rf"
      - "drop table"
    allowed_commands:                  # when set, only these prefixes can run
      - "npm"
      - "pm2"
    audit_log: "~/.remote-bridge-staging.log"  # default: ~/.remote-bridge-audit.log
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

---

## 📖 Commands

### Core Workflow

| Command | Description |
| :--- | :--- |
| `init` | Create a new `remotebridge.yaml` config |
| `preflight` | Check remote OS, Node.js, Python, Rust, Docker versions |
| `sync` | Sync local files to the remote server |
| `sync --dry-run` | Preview what would be synced without transferring anything |
| `run <cmd>` | Execute a single command on the remote server |
| `apply` | **(Core)** Parse STDIN Markdown and apply file changes + commands |

### Service Management

| Command | Description |
| :--- | :--- |
| `restart` | Restart the remote service using `restart_cmd` from config |
| `deploy` | **Full pipeline:** sync → restart → tail logs on failure |
| `deploy --follow` | Same as above, then `tail -f` logs after success |

### Monitoring

| Command | Description |
| :--- | :--- |
| `logs` | Fetch the last 50 lines from configured log files |
| `logs -n 200` | Fetch the last 200 lines |
| `logs --follow` | Stream logs live (`tail -f`) |

### Developer Loop

| Command | Description |
| :--- | :--- |
| `watch` | Poll local files every 2s, auto-sync on change |
| `watch --interval 5` | Custom polling interval in seconds |

---

## 🤖 Integrating with AI CLIs

RemoteBridge is pipe-friendly. Pipe any AI tool's output directly into `remote-bridge apply`.

### With Claude Code
```bash
claude "Fix the database connection in src/db.ts and restart the app" --non-interactive \
  | remote-bridge apply --target staging
```

### With Gemini CLI
```bash
gemini "Add rate limiting to the Express API" \
  | remote-bridge apply --target staging
```

### With Aider
```bash
aider --message "Refactor the login logic" --apply \
  | remote-bridge apply --target staging
```

### The Full AI Loop (Recommended)
```bash
# 1. Start watching in one terminal — changes sync automatically
remote-bridge watch --target staging

# 2. Run AI in another terminal
claude "Optimize the database queries in src/db.ts"

# 3. Deploy when ready
remote-bridge deploy --target staging --follow
```

---

## 🔌 MCP Support — Works With Every AI IDE

RemoteBridge is a native **Model Context Protocol (MCP)** server. Any MCP-compatible AI IDE can use it as a tool directly — no piping required. The config is almost identical across all tools.

### Available MCP Tools

| Tool | Description |
| :--- | :--- |
| `sync_to_remote` | Push local code to the remote server |
| `run_remote_command` | Execute any shell command on the remote host |
| `preflight_check` | Check remote OS and runtime versions |
| `fetch_logs` | Retrieve recent log lines |
| `restart_service` | Restart the configured remote service |
| `deploy` | Full sync + restart + log-tail pipeline |

---

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

---

### Cursor

File: `~/.cursor/mcp.json` (global) or `.cursor/mcp.json` (per-project)
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
Or go to **Cursor Settings → MCP → Add Server**.

---

### VS Code (GitHub Copilot / Continue / Cline)

File: `.vscode/mcp.json` in your project root
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
For the **Cline** extension, add via the Cline sidebar → MCP Servers → Configure.

---

### Windsurf (Codeium)

File: `~/.codeium/windsurf/mcp_config.json`
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
Or go to **Windsurf Settings → Cascade → MCP Servers → Add**.

---

### Zed

File: `~/.config/zed/settings.json`
```json
{
  "context_servers": {
    "remote-bridge": {
      "command": {
        "path": "remote-bridge",
        "args": ["mcp"]
      }
    }
  }
}
```

---

### OpenAI Codex CLI

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
Or pass inline: `codex --mcp-server "remote-bridge mcp" "Deploy my changes"`

---

### Continue.dev

File: `~/.continue/config.json`
```json
{
  "mcpServers": [
    {
      "name": "remote-bridge",
      "command": "remote-bridge",
      "args": ["mcp"]
    }
  ]
}
```

---

### Claude Code (CLI)

File: `.claude/mcp.json` in your project, or `~/.claude/mcp.json` globally
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
Or add via CLI: `claude mcp add remote-bridge -- remote-bridge mcp`

---

### Any MCP-compatible tool

The pattern is always the same — `stdio` transport, command `remote-bridge`, arg `mcp`:
```json
{
  "command": "remote-bridge",
  "args": ["mcp"],
  "transport": "stdio"
}
```

### What it actually looks like

Once configured, you just talk to your AI naturally inside Claude Code, Cursor, Windsurf, or any MCP-enabled IDE:

---

**You:** *"Sync my project files to the staging server"*
> RemoteBridge calls `sync_to_remote` → rsync transfers only changed files

**You:** *"Run npm install on the remote server"*
> RemoteBridge calls `run_remote_command` with `npm install` → streams output back

**You:** *"Deploy my latest changes to ubuntu@your-server.com"*
> RemoteBridge calls `deploy` → syncs files, restarts service, tails logs if it fails

**You:** *"Check what OS and runtimes are installed on the server"*
> RemoteBridge calls `preflight_check` → returns Ubuntu version, Node, Python, Docker

**You:** *"Tail the remote logs and show me what's failing"*
> RemoteBridge calls `fetch_logs` → AI reads the error and fixes your code

**You:** *"Something broke after deploy — fetch the logs and fix it"*
> RemoteBridge fetches logs → AI sees the stack trace → writes the fix → deploys again

---

The AI decides which tool to call. You just describe what you want.

---

## 🔒 SSH Authentication & Security

RemoteBridge uses your existing SSH key authentication — no passwords stored.

### Setup

1. **Copy your SSH key to the server:**
   ```bash
   ssh-copy-id -i ~/.ssh/id_rsa.pub ubuntu@your-server.com
   ```

2. **Use a specific key or non-standard port in config:**
   ```yaml
   targets:
     staging:
       host: "13.234.xx.xx"
       user: "ubuntu"
       port: 2222
       ssh_key: "~/keys/staging.pem"
   ```

3. **Or use `~/.ssh/config` for aliases:**
   ```
   Host staging-server
       HostName 13.234.xx.xx
       User ubuntu
       IdentityFile ~/keys/my-key.pem
       Port 22
   ```
   Then in `remotebridge.yaml`:
   ```yaml
   host: "staging-server"
   user: "ubuntu"
   ```

---

## 🛡 Safety Features

RemoteBridge has layered defenses so a hallucinating AI can never destroy your server.

### Confirmation Gate (built-in)
Commands containing `sudo`, `rm`, `drop`, `delete`, `database`, `shutdown`, `reboot`, `killall`, `curl | bash`, `wget | sh`, and 20+ other dangerous patterns **always pause for confirmation** before running on the remote host.

### Hard Block (user-defined)
Patterns you add to `blocked_patterns` are **always rejected — no confirmation, no override**:
```yaml
targets:
  staging:
    blocked_patterns:
      - "rm -rf"
      - "drop table"
      - "truncate"
```

### Allowlist (user-defined)
When `allowed_commands` is set, **only commands matching those prefixes can run**. Anything else is silently blocked:
```yaml
targets:
  production:
    allowed_commands:
      - "npm"
      - "pm2"
      - "systemctl restart myapp"
```

### Audit Log
Every command execution is logged automatically to `~/.remote-bridge-audit.log` (or a custom path):
```
[1742660591] host=your-server.com path=/var/www/app exit=0 cmd=npm install
[1742660612] host=your-server.com path=/var/www/app exit=-2 cmd=rm -rf /var/data
```
Exit codes: `0+` = actual exit code, `-1` = skipped by user, `-2` = hard blocked, `-3` = not in allowlist.

Custom path:
```yaml
targets:
  staging:
    audit_log: "/var/log/remote-bridge-staging.log"
```

### Other Protections
- **`require_confirmation: true`** — confirm every command, no exceptions
- **`--dry-run`** on sync — preview rsync diff without touching anything
- **No password storage** — SSH key auth only
- **No secrets in config** — host/user/path only
- **MCP output truncation** — `run_remote_command` returns at most 100 lines by default, preventing log floods from filling AI context

---

## 🤝 Contributing

Contributions, issues, and feature requests are welcome at [GitHub Issues](https://github.com/varaprasadreddy9676/remote-bridge/issues).

## 📜 License

MIT License.
