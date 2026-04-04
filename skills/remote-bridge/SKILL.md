# remote-bridge

Deploy and manage remote servers directly from Codex using SSH and rsync.

## Description

This skill enables Codex to sync files, run commands, tail logs, restart services, diagnose failures, compare environments, and run full deploy pipelines on remote servers — all via the remote-bridge MCP server.

## Prerequisites

Install the remote-bridge CLI (includes the MCP server):

```bash
npm install -g remote-bridge-cli
```

Create a `remotebridge.yaml` in your project root:

```yaml
project_name: "my-app"
targets:
  staging:
    host: "your-server.com"
    user: "ubuntu"
    remote_path: "/var/www/app"
    ssh_key: "~/.ssh/id_rsa"
    restart_cmd: "pm2 restart app"
    logs:
      - "/var/www/app/logs/error.log"
  production:
    host: "prod.example.com"
    user: "ubuntu"
    remote_path: "/var/www/app"
    ssh_key: "~/.ssh/id_rsa"
    restart_cmd: "pm2 restart app"
    require_confirmation: true
    logs:
      - "/var/www/app/logs/error.log"
```

Add to your Codex MCP config:

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

## Tools

| Tool | Description |
|---|---|
| `sync_to_remote` | Sync a local directory to the remote server via rsync |
| `run_remote_command` | Execute a shell command on the remote server |
| `preflight_check` | Check remote OS and available runtimes |
| `fetch_logs` | Tail recent lines from configured log files |
| `restart_service` | Restart the remote service via `restart_cmd` |
| `deploy` | Full pipeline: sync → restart → tail logs on failure |
| `diagnose_failure` | Collect a compact diagnosis bundle and summarize likely causes from service state and logs |
| `compare_targets` | Compare two configured targets for config and runtime drift |

## Safety

- `--delete` is **off by default** on `sync_to_remote`. Pass `delete=true` only for intentional full-mirror syncs.
- Set `require_confirmation: true` on production targets. Codex will show a dry-run preview and require `confirm=true` before syncing.
- Always set `local_path` explicitly on `sync_to_remote` to avoid syncing the wrong directory.

## Usage Examples

**Deploy to staging:**
> "Deploy the current project to staging"

**Sync specific folder:**
> "Sync the ./dist folder to production"

**Check what changed before syncing:**
> "Show me a dry run of what would sync to production"

**Run a remote command:**
> "Run `npm install` on the staging server"

**Tail logs after a deploy:**
> "Show me the last 100 lines of logs on staging"

## Source

GitHub: https://github.com/varaprasadreddy9676/remote-bridge
npm: `remote-bridge-cli`
