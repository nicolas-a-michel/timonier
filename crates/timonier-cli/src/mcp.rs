//! `timonier mcp`: the MCP server an agent starts in the project (ADR 0027). Standard output
//! carries MCP: everything else goes to standard error.

use std::process::ExitCode;

use crate::daemon::{self, Target};

pub fn run(target: &Target) -> ExitCode {
    // Logs on stderr, never on stdout: `RUST_LOG=timonier_mcp=debug` for more.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .try_init();
    let outcome = daemon::resolve(target)
        .and_then(|paths| daemon::launch(&paths).map(|_| paths))
        .and_then(|paths| timonier_mcp::run(&paths.socket).map_err(anyhow::Error::from));
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("timonier mcp: {error:#}");
            ExitCode::FAILURE
        }
    }
}
