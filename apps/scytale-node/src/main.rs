use clap::{Parser, Subcommand};
use scytale_consensus::INITIAL_REWARD;
use scytale_core::QUANTA_PER_SCY;

#[derive(Parser, Debug)]
#[command(
    name = "scytale-node",
    author,
    version,
    about = "Scytale Blockchain Engine CLI & Node Daemon"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to data directory
    #[arg(short, long, default_value = ".scytale")]
    data_dir: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the Scytale full node daemon
    Start {
        /// P2P listening port
        #[arg(short, long, default_value_t = 8333)]
        port: u16,
        /// Enable autonomous Proof-of-Work mining
        #[arg(short, long, default_value_t = false)]
        mine: bool,
    },
    /// Inspect blockchain status
    Status,
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Start { port, mine }) => {
            println!(
                "Starting Scytale Node Daemon on port {} (data dir: {}, mining: {})...",
                port, cli.data_dir, mine
            );
            println!(
                "Protocol Baseline: Initial Subsidy = {} quanta ({} SCY)",
                INITIAL_REWARD,
                INITIAL_REWARD / QUANTA_PER_SCY
            );
        }
        Some(Commands::Status) => {
            println!(
                "Scytale Node Status: Operational (data dir: {})",
                cli.data_dir
            );
        }
        None => {
            println!("No subcommand specified. Run with `--help` for available commands.");
        }
    }
}
