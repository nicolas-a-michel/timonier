//! `timonier mcp`: the MCP server an agent starts in the project (ADR 0027). Standard output
//! carries MCP: everything else goes to standard error.

use std::process::ExitCode;

use crate::daemon::{self, Target};

pub fn run(target: &Target) -> ExitCode {
    let outcome = daemon::resolve(target)
        .and_then(|paths| daemon::launch(&paths).map(|_| paths))
        .and_then(|paths| {
            // A daemon that goes (stopped, upgraded) is started again for the next call (T-110).
            let again = paths.clone();
            let relaunch: timonier_mcp::Relaunch = std::sync::Arc::new(move || {
                daemon::launch(&again)
                    .map(|_| ())
                    .map_err(|e| format!("{e:#}"))
            });
            timonier_mcp::run(&paths.socket, relaunch).map_err(anyhow::Error::from)
        });
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("timonier mcp: {error:#}");
            ExitCode::FAILURE
        }
    }
}
