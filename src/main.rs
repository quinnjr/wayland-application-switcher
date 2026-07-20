mod backend;
mod commands;
mod picker;
mod resolve;
mod timeout;

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
        Command::Notify {
            query,
            summary,
            body,
            icon,
        } => commands::run_notify(&query, &summary, body.as_deref(), icon.as_deref()),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("was: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_requires_a_query() {
        let cli = Cli::try_parse_from(["was", "switch", "konsole"]).unwrap();
        assert!(matches!(cli.command, Command::Switch { query } if query == "konsole"));
        assert!(Cli::try_parse_from(["was", "switch"]).is_err());
    }

    #[test]
    fn list_query_is_optional() {
        let cli = Cli::try_parse_from(["was", "list"]).unwrap();
        assert!(matches!(cli.command, Command::List { query: None }));

        let cli = Cli::try_parse_from(["was", "list", "konsole"]).unwrap();
        assert!(matches!(cli.command, Command::List { query: Some(q) } if q == "konsole"));
    }

    #[test]
    fn notify_requires_query_and_summary_but_not_body_or_icon() {
        let cli = Cli::try_parse_from(["was", "notify", "--query", "konsole", "--summary", "hi"])
            .unwrap();
        assert!(matches!(
            cli.command,
            Command::Notify { query, summary, body: None, icon: None }
                if query == "konsole" && summary == "hi"
        ));
        assert!(Cli::try_parse_from(["was", "notify", "--summary", "hi"]).is_err());
        assert!(Cli::try_parse_from(["was", "notify", "--query", "konsole"]).is_err());
    }
}
