# RemoteBridge 🌉

**Bridge the gap between AI-generated code and remote server execution.**

RemoteBridge is a high-performance CLI tool written in Rust, designed to act as a stateful proxy between your local AI coding environment (like Gemini CLI, Claude Code, or Aider) and your remote infrastructure. It intercepts AI output, syncs file changes via `rsync`, executes remote commands via `ssh`, and pipes logs back to your terminal so the AI can "see" remote errors.

## 🚀 The Problem & Solution

**The Problem:** AI coding tools operate locally. In environments without robust CI/CD, developers must manually FTP files and SSH into servers to test changes. The AI doesn't see remote runtime errors, leading to a "context gap."

**The Solution:** RemoteBridge automates the "Sync -> Run -> Feedback" loop. It parses Markdown output from any AI CLI, applies file changes locally, syncs them to a remote target, runs the requested commands, and fetches remote logs if something fails.

---

## ✨ Key Features

- **Markdown Interception:** Automatically extracts ```bash``` and ```file path``` blocks from STDIN.
- **Shadow Syncing:** Uses `rsync` over SSH for ultra-fast, delta-based file transfers.
- **Permission Gate:** Pauses for confirmation if a command contains `sudo`, `rm`, or database keywords.
- **Remote Awareness (Pre-flight):** Detects remote OS, Node.js, and Python versions to inform the AI.
- **Log Backfeed:** If a remote command fails, it automatically tails configured log files and prints them to STDOUT for the AI to "read" and fix.

---

## 🛠 Installation

### Prerequisites
- **Rust:** Install via [rustup.rs](https://rustup.rs/).
- **System Tools:** `rsync` and `ssh` must be available in your PATH.

### Build from Source
```bash
git clone https://github.com/your-repo/remote-bridge.git
cd remote-bridge
cargo build --release
cp target/release/remote-bridge /usr/local/bin/
```

---

## ⚙️ Configuration (`remotebridge.yaml`)

Initialize your project with:
```bash
remote-bridge init --name my-app -H 13.234.xx.xx --user ubuntu --path /var/www/html/app
```

This creates a `remotebridge.yaml` in your root:
```yaml
project_name: "my-app"
targets:
  staging:
    host: "13.234.xx.xx"
    user: "ubuntu"
    remote_path: "/var/www/html/app"
    logs:
      - "/var/www/html/app/logs/error.log"
      - "/var/log/nginx/error.log"
    require_confirmation: false
```

---

## 🤖 Integrating with AI CLIs

RemoteBridge is designed to be "Pipe-Friendly." You can pipe the output of any AI tool directly into `remote-bridge apply`.

### 1. With Gemini CLI
```bash
gemini "Fix the database connection in src/db.ts and restart the app" | remote-bridge apply --target staging
```

### 2. With Claude Code CLI
```bash
claude "Add a new endpoint to the express app" --non-interactive | remote-bridge apply --target staging
```

### 3. With Aider
```bash
aider --message "Refactor the login logic" --apply | remote-bridge apply --target staging
```

### 4. Direct Piping (Any Tool)
Any tool that outputs Markdown to STDOUT can be bridged:
```bash
cat fix_instructions.md | remote-bridge apply
```

---

## 📖 Commands

| Command | Description |
| :--- | :--- |
| `init` | Create a new `remotebridge.yaml` config. |
| `preflight` | Check remote OS, Node.js, and Python versions. |
| `sync` | Manually sync local changes to the remote server. |
| `run` | Execute a single command on the remote server. |
| `apply` | **(Core)** Parse STDIN for files/commands and execute them. |

---

## 🛡 Safety First

RemoteBridge includes a **Permission Gate**. By default, it will prompt you before running:
- `sudo` commands
- `rm` operations
- Database operations (`db`, `drop`, `delete`)

You can also force confirmation for all commands by setting `require_confirmation: true` in your target config.

---

## 🤝 Contributing

This is an open-source project designed to make remote development with AI seamless. Contributions, issues, and feature requests are welcome!

## 📜 License

MIT License. See `LICENSE` for details.
