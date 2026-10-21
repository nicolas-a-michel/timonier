//! The `timonier` command line, also installed as `tim`.

mod daemon;
mod hook;
mod init;
mod mcp;
mod measure;
mod telemetry;
mod test;

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
    /// Set the project up for Timonier and its agents: instructions, MCP server, hooks, plan
    Init(init::InitArgs),
    /// Run the tests and measure them: time, CPU, memory, I/O, and each test's duration
    Test(test::TestArgs),
    /// The measuring relay of `timonier test`
    #[command(name = "__measure", hide = true)]
    Measure(measure::MeasureArgs),
    /// Take back what `timonier init` made
    Uninit(init::UninitArgs),
    /// Check the setup `timonier init` made, and repair it
    Doctor(init::DoctorArgs),
    /// What the hooks installed by `timonier init` run
    Hook {
        #[command(subcommand)]
        hook: hook::Hook,
    },
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
    // Logs on stderr, never on stdout (`timonier mcp` speaks MCP there), `RUST_LOG` for more;
    // the daemon in the foreground says what it does. Spans go out when a run asks (ADR 0044).
    let foreground = matches!(
        cli.command,
        Some(Command::Daemon {
            action: daemon::Action::Run(_)
        })
    );
    let _telemetry = if foreground {
        use std::io::IsTerminal;
        telemetry::install("info", std::io::stderr().is_terminal())
    } else {
        telemetry::install("warn", false)
    };
    match cli.command {
        Some(Command::Daemon { action }) => daemon::run(action),
        Some(Command::Mcp(target)) => mcp::run(&target),
        Some(Command::Init(args)) => init::run(&args),
        Some(Command::Test(args)) => test::run(&args),
        Some(Command::Measure(args)) => measure::run(&args),
        Some(Command::Uninit(args)) => init::run_uninit(&args),
        Some(Command::Doctor(args)) => init::run_doctor(&args),
        Some(Command::Hook { hook }) => hook::run(&hook),
        // No command: show the help.
        None => match command.print_help() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        },
    }
}
