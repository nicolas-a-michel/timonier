//! The `timonier` command line, also installed as `tim`.

mod daemon;
mod mcp;

use std::process::ExitCode;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

/// Steer AI coding agents without reading all the code they produce.
#[derive(Debug, Parser)]
#[command(name = "timonier", version = timonier_core::VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Manage this project's daemon, which keeps its code graph
    Daemon {
        #[command(subcommand)]
        action: daemon::Action,
    },
    /// Serve the MCP tools to an agent on stdio, starting the daemon if needed
    Mcp(daemon::Target),
}

/// Entry point shared by the `timonier` and `tim` binaries.
pub fn run() -> ExitCode {
    let mut command = Cli::command();
    // Handles --help, --version and usage errors, and records the binary name used.
    let matches = command.get_matches_mut();
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(error) => error.exit(),
    };
    match cli.command {
        Some(Command::Daemon { action }) => daemon::run(action),
        Some(Command::Mcp(target)) => mcp::run(&target),
        // No command: show the help.
        None => match command.print_help() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        },
    }
}
