mod backend;
mod commands;
mod picker;
mod resolve;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "was", about = "Wayland application switcher")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Activate the window matching QUERY (or pick among several matches).
    Switch { query: String },
    /// List windows matching QUERY (all windows if omitted).
    List { query: Option<String> },
    /// Send a notification; clicking it activates the window matching QUERY.
    Notify {
        #[arg(long)]
        query: String,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        icon: Option<String>,
    },
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Switch { query } => commands::run_switch(&query),
        Command::List { query } => commands::run_list(query.as_deref()),
        Command::Notify { query, summary, body, icon } => {
            commands::run_notify(&query, &summary, body.as_deref(), icon.as_deref())
        }
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("was: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
