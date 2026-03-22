use clap::{Parser, Subcommand};
use remote_bridge::config::{load_config, create_default_config};
use remote_bridge::executor::Executor;
use remote_bridge::parser::{parse_markdown, ShellCommand};
use std::io::{self, Read};

#[derive(Parser)]
#[command(name = "remote-bridge")]
#[command(about = "Bridge the gap between AI-generated code and remote servers", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initializes a new RemoteBridge configuration
    Init {
        #[arg(short, long)]
        name: String,
        #[arg(short = 'H', long)]
        host: String,
        #[arg(short, long)]
        user: String,
        #[arg(short, long)]
        path: String,
    },
    /// Manually syncs local files to the remote target
    Sync {
        #[arg(short, long, default_value = "staging")]
        target: String,
    },
    /// Executes a command on the remote target
    Run {
        command_str: String,
        #[arg(short, long, default_value = "staging")]
        target: String,
    },
    /// Checks the remote environment for OS and runtimes
    Preflight {
        #[arg(short, long, default_value = "staging")]
        target: String,
    },
    /// Applies file changes and runs commands from Markdown piped to STDIN
    Apply {
        #[arg(short, long, default_value = "staging")]
        target: String,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name, host, user, path } => {
            create_default_config("remotebridge.yaml", &name, &host, &user, &path)?;
            println!("remotebridge.yaml created successfully!");
        }
        Commands::Sync { target } => {
            let config = load_config("remotebridge.yaml")?;
            let target_cfg = config.targets.get(&target).ok_or(format!("Target {} not found", target))?;
            let executor = Executor::new(target_cfg.clone());
            executor.get_transport().sync_files(".", vec![".git/".to_string(), ".remote_bridge/".to_string()])?;
        }
        Commands::Run { command_str, target } => {
            let config = load_config("remotebridge.yaml")?;
            let target_cfg = config.targets.get(&target).ok_or(format!("Target {} not found", target))?;
            let executor = Executor::new(target_cfg.clone());
            executor.run_commands(&[ShellCommand { command: command_str, lang: "bash".to_string() }])?;
        }
        Commands::Preflight { target } => {
            let config = load_config("remotebridge.yaml")?;
            let target_cfg = config.targets.get(&target).ok_or(format!("Target {} not found", target))?;
            let executor = Executor::new(target_cfg.clone());
            let transport = executor.get_transport();
            
            println!("Running pre-flight check on {}...", target);
            
            let (_, os, _) = transport.run_remote_command("lsb_release -d || cat /etc/os-release | grep PRETTY_NAME")?;
            println!("OS: {}", os.trim());
            
            let (code, node, _) = transport.run_remote_command("node -v")?;
            if code == 0 { println!("Node.js: {}", node.trim()); } else { println!("Node.js: Not found"); }
            
            let (code, python, _) = transport.run_remote_command("python3 --version")?;
            if code == 0 { println!("Python: {}", python.trim()); } else { println!("Python: Not found"); }
        }
        Commands::Apply { target } => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            
            if buffer.is_empty() {
                println!("No input received.");
                return Ok(());
            }

            let (file_changes, shell_commands) = parse_markdown(&buffer);
            
            if file_changes.is_empty() && shell_commands.is_empty() {
                println!("No file changes or shell commands found.");
                return Ok(());
            }

            let config = load_config("remotebridge.yaml")?;
            let target_cfg = config.targets.get(&target).ok_or(format!("Target {} not found", target))?;
            let executor = Executor::new(target_cfg.clone());
            
            if !file_changes.is_empty() {
                println!("Found {} file changes.", file_changes.len());
                executor.apply_file_changes(&file_changes)?;
            }
            
            if !shell_commands.is_empty() {
                println!("Found {} shell commands.", shell_commands.len());
                executor.run_commands(&shell_commands)?;
            }
        }
    }

    Ok(())
}
