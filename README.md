# RemoteBridge 🌉

**Bridge the gap between AI-generated code and remote servers.**

RemoteBridge is a high-performance CLI tool written in Rust that acts as a stateful proxy between your local AI coding environment (Claude Code, Gemini CLI, Aider, etc.) and your remote infrastructure. It parses Markdown output from AI tools, syncs file changes via `rsync`, executes remote commands over SSH, and pipes logs back to your terminal — so the AI can "see" remote errors and fix them.

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
| **Permission Gate** | Pauses for confirmation on `sudo`, `rm`, or database commands |
| **Pre-flight Check** | Detects remote OS, Node.js, Python, Rust, Docker versions |
| **Log Backfeed** | Auto-tails remote logs when a command fails — AI reads and fixes |
| **MCP Server** | Native Model Context Protocol server for Claude Desktop & other AI IDEs |
| **SSH Key + Port** | Per-target SSH identity file and custom port support |

---

## 🛠 Installation

### Prerequisites
- **Rust:** Install via [rustup.rs](https://rustup.rs/)
- **System Tools:** `rsync` and `ssh` must be in your PATH

### Install via NPM (Recommended)
```bash
npm install -g remote-bridge-cli
```
> This triggers a local `cargo build --release` optimized for your architecture.

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

## 🔌 Model Context Protocol (MCP) Support

RemoteBridge is a native MCP server. AI IDEs like Claude Desktop can use it as a tool directly — no manual piping required.

### Claude Desktop Configuration

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:
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

### Available MCP Tools

| Tool | Description |
| :--- | :--- |
| `sync_to_remote` | Push local code to the remote server |
| `run_remote_command` | Execute any shell command on the remote host |
| `preflight_check` | Check remote OS and runtime versions |
| `fetch_logs` | Retrieve recent log lines |
| `restart_service` | Restart the configured remote service |
| `deploy` | Full sync + restart + log-tail pipeline |

Once configured, Claude can:
```
"Push my latest changes and restart the app"
→ calls sync_to_remote then restart_service automatically
```

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

- **Permission Gate:** Prompts before any command containing `sudo`, `rm`, `drop`, `delete`, or `database`
- **`require_confirmation: true`** in config forces confirmation for every command
- **`--dry-run`** on sync shows a diff without touching files
- **No password storage** — SSH key auth only
- **No secrets in config** — host/user/path only

---

## 🤝 Contributing

Contributions, issues, and feature requests are welcome at [GitHub Issues](https://github.com/varaprasadreddy9676/remote-bridge/issues).

## 📜 License

MIT License.
