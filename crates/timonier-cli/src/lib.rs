//! The `timonier` command line, also installed as `tim`.

use std::process::ExitCode;

use clap::{CommandFactory, Parser};

/// Steer AI coding agents without reading all the code they produce.
#[derive(Debug, Parser)]
#[command(name = "timonier", version = timonier_core::VERSION)]
struct Cli {}

/// Entry point shared by the `timonier` and `tim` binaries.
pub fn run() -> ExitCode {
    let mut command = Cli::command();
    // Handles --help, --version and usage errors, and records the binary name used.
    command.get_matches_mut();
    // No subcommand exists yet: show the help.
    match command.print_help() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
