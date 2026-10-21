//! `timonier daemon start|stop|status|run`: one daemon per project (ADR 0019).

use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use serde_json::Value;
use timonier_core::protocol::{StatusResult, methods};
use timonier_daemon::{Client, Daemon, Lock, Options, Paths};

/// Exit status of `status` when no daemon runs, as for LSB init scripts.
pub const NOT_RUNNING: u8 = 3;

/// How long `start` waits for the daemon to answer, and `stop` for it to let go of its lock.
const WAIT: Duration = Duration::from_secs(15);

/// How the CLI names itself to the daemon.
const CLIENT: &str = concat!("timonier-cli ", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Subcommand)]
pub enum Action {
    /// Start this project's daemon in the background
    Start(Target),
    /// Stop this project's daemon
    Stop(Target),
    /// Show whether this project's daemon runs, and what it does
    Status(Target),
    /// Run this project's daemon in the foreground, logging to stderr
    Run(Target),
}

#[derive(Debug, Args)]
pub struct Target {
    /// The project's root [default: the nearest directory, from the current one up, that holds
    /// `.timonier/` or `.git`; else the current directory]
    #[arg(long, value_name = "DIR")]
    root: Option<PathBuf>,
}

pub fn run(action: Action) -> ExitCode {
    let outcome = match action {
        Action::Start(target) => resolve(&target).and_then(|paths| start(&paths)),
        Action::Stop(target) => resolve(&target).and_then(|paths| stop(&paths)),
        Action::Status(target) => resolve(&target).and_then(|paths| status(&paths)),
        Action::Run(target) => resolve(&target).and_then(|paths| foreground(&paths)),
    };
    match outcome {
        Ok(code) => code,
        Err(error) => {
            eprintln!("timonier: {error:#}");
            ExitCode::FAILURE
        }
    }
}

impl Target {
    /// The project at `root`, or found from the current directory.
    pub fn new(root: Option<PathBuf>) -> Self {
        Self { root }
    }
}

pub fn resolve(target: &Target) -> Result<Paths> {
    let root = match &target.root {
        Some(root) => root.clone(),
        None => {
            let here = std::env::current_dir().context("cannot read the current directory")?;
            project_root(&here)
        }
    };
    Paths::new(&root).with_context(|| format!("no project at `{}`", root.display()))
}

/// The nearest directory, from `here` up, that holds `.timonier/` or `.git`; else `here`. The
/// home folder and `/` are never one: a `.timonier/` left there would take every folder under
/// it (T-119).
fn project_root(here: &Path) -> PathBuf {
    here.ancestors()
        .filter(|dir| !timonier_daemon::runtime::not_a_project(dir))
        .find(|dir| dir.join(".timonier").is_dir() || dir.join(".git").exists())
        .unwrap_or(here)
        .to_path_buf()
}

/// A client of the daemon, if one answers.
fn connect(paths: &Paths) -> Option<Client> {
    connect_as(paths, CLIENT)
}

/// A daemon answers on the socket but refuses this client: another version (the program was
/// installed again while it ran). Said, with what to do, rather than « does not start ».
fn refused(paths: &Paths) -> Result<()> {
    let Ok(mut client) = Client::connect(&paths.socket) else {
        return Ok(());
    };
    client.set_timeout(Duration::from_secs(5));
    match client.initialize(CLIENT) {
        Err(timonier_daemon::ClientError::Rpc(error))
            if error.code == timonier_core::protocol::errors::VERSION_REFUSED =>
        {
            bail!(
                "a Timonier daemon of another version runs for {} ({}): stop it with `timonier \
                 daemon stop`, then start again; a daemon older than this version cannot be \
                 stopped this way: find its process with `pgrep -af \"daemon run --root\"` and \
                 end it with `kill` and its number",
                paths.root.display(),
                error.message
            )
        }
        Err(error) => bail!(
            "a Timonier daemon answers for {} but not to this client: {error}",
            paths.root.display()
        ),
        Ok(_) => Ok(()),
    }
}

/// A client of the daemon that names itself `name` (the agent of the sessions it opens), if
/// one answers.
pub fn connect_as(paths: &Paths, name: &str) -> Option<Client> {
    let mut client = Client::connect(&paths.socket).ok()?;
    client.set_timeout(Duration::from_secs(30));
    client.initialize(name).ok()?;
    Some(client)
}

fn start(paths: &Paths) -> Result<ExitCode> {
    match launch(paths)? {
        Launched::Running => println!(
            "The Timonier daemon already runs for {}.",
            paths.root.display()
        ),
        Launched::Started(process) => {
            println!(
                "The Timonier daemon runs for {} (process {process}).",
                paths.root.display(),
            );
            println!("Its log: {}", paths.log.display());
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// What [`launch`] found or did.
pub enum Launched {
    Running,
    Started(u32),
}

/// Starts this project's daemon in the background unless one answers, and waits until it
/// does. Prints nothing: `timonier mcp` speaks MCP on its standard output.
pub fn launch(paths: &Paths) -> Result<Launched> {
    if connect(paths).is_some() {
        return Ok(Launched::Running);
    }
    refused(paths)?;
    // The runtime directory, closed to others, before the log goes in it.
    timonier_daemon::runtime::prepare_runtime(paths)?;
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(&paths.log)
        .with_context(|| format!("cannot open the log `{}`", paths.log.display()))?;
    let program = std::env::current_exe().context("cannot find the timonier program")?;
    let mut child = Command::new(program)
        .args(["daemon", "run", "--root"])
        .arg(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        // Its own process group: a terminal closed or a Ctrl-C here does not reach it.
        .process_group(0)
        .spawn()
        .context("cannot start the daemon")?;
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        if connect(paths).is_some() {
            return Ok(Launched::Started(child.id()));
        }
        if let Some(exit) = child.try_wait()? {
            // Another start may have won the lock meanwhile.
            if connect(paths).is_some() {
                return Ok(Launched::Running);
            }
            bail!(
                "the daemon stopped ({exit}); its log: {}",
                paths.log.display()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    bail!(
        "the daemon did not answer within {} s; its log: {}",
        WAIT.as_secs(),
        paths.log.display()
    )
}

pub fn stop(paths: &Paths) -> Result<ExitCode> {
    // A daemon of another version refuses the handshake, but stops when asked.
    let client = connect(paths).or_else(|| {
        let mut client = Client::connect(&paths.socket).ok()?;
        client.set_timeout(Duration::from_secs(30));
        Some(client)
    });
    let Some(mut client) = client.filter(|_| Lock::is_held(paths).unwrap_or(true)) else {
        println!("No Timonier daemon runs for {}.", paths.root.display());
        return Ok(ExitCode::SUCCESS);
    };
    let _: Value = client
        .request(methods::SHUTDOWN, &Value::Null)
        .context("the daemon refused to stop")?;
    // It stops once the pipeline has finished its step: the lock says when.
    let deadline = Instant::now() + WAIT;
    while Lock::is_held(paths)? {
        if Instant::now() >= deadline {
            bail!("the daemon still runs after {} s", WAIT.as_secs());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("The Timonier daemon for {} stopped.", paths.root.display());
    Ok(ExitCode::SUCCESS)
}

fn status(paths: &Paths) -> Result<ExitCode> {
    let Some(mut client) = connect(paths) else {
        println!("No Timonier daemon runs for {}.", paths.root.display());
        return Ok(ExitCode::from(NOT_RUNNING));
    };
    let status: StatusResult = client
        .request(methods::STATUS, &Value::Null)
        .context("the daemon did not give its status")?;
    println!("The Timonier daemon runs for {}.", status.root);
    println!("  version      {}", status.daemon_version);
    println!("  files        {}", status.files);
    println!("  snapshot     {}", status.head);
    println!(
        "  refinements  {} waiting, {} held until the editor's files are saved",
        status.refinements_waiting, status.refinements_held
    );
    println!("  clients      {} (this one included)", status.clients);
    println!("  socket       {}", paths.socket.display());
    Ok(ExitCode::SUCCESS)
}

/// The agents' telemetry, for a project whose `init` turned it on (ADR 0058).
/// `TIMONIER_AGENT_PORT` gives another port than the machine's, for tests.
fn agents_options(root: &std::path::Path) -> Option<timonier_daemon::server::AgentsOptions> {
    let data = timonier_daemon::agents::data_dir()?;
    if !timonier_daemon::agents::registered(&data, root) {
        return None;
    }
    let port = std::env::var("TIMONIER_AGENT_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(timonier_daemon::agents::PORT);
    Some(timonier_daemon::server::AgentsOptions { data, port })
}

fn foreground(paths: &Paths) -> Result<ExitCode> {
    let mut options = Options::new(&paths.root);
    options.runtime = Some(paths.runtime.clone());
    options.agents = agents_options(&paths.root);
    let Some(daemon) = Daemon::bind(options)? else {
        eprintln!(
            "A Timonier daemon already runs for {}.",
            paths.root.display()
        );
        announce(paths, None)?;
        return Ok(ExitCode::SUCCESS);
    };
    announce(paths, Some(std::process::id()))?;
    daemon.run()?;
    Ok(ExitCode::SUCCESS)
}

/// One JSON line on stdout, once the socket listens: where the daemon is, and whether this
/// process is it (`pid`) or another one serves (`null`). The extension reads it (ADR 0019).
fn announce(paths: &Paths, pid: Option<u32>) -> Result<()> {
    use std::io::Write;
    let line = serde_json::json!({ "socket": paths.socket, "pid": pid });
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{line}").context("cannot write to stdout")?;
    stdout.flush().context("cannot write to stdout")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_project_is_the_nearest_marked_directory() {
        let temp = tempfile::tempdir().unwrap();
        let outer = temp.path().join("outer");
        let inner = outer.join("inner");
        let deep = inner.join("src/deep");
        std::fs::create_dir_all(&deep).unwrap();
        // Nothing marked: the current directory.
        assert_eq!(project_root(&deep), deep);
        std::fs::create_dir(outer.join(".git")).unwrap();
        assert_eq!(project_root(&deep), outer);
        std::fs::create_dir(inner.join(".timonier")).unwrap();
        assert_eq!(project_root(&deep), inner);
        assert_eq!(project_root(&outer), outer);
    }

    /// Review n° 4 of Phase 7, m-4: a daemon that refuses this version is said as such, with
    /// how to stop it; another refusal is not taken for a version.
    #[test]
    fn a_daemon_of_another_version_is_said_as_such() {
        use std::io::{BufRead, BufReader, Read, Write};
        let refused_with = |code: i64| -> String {
            let root = tempfile::tempdir().unwrap();
            let runtime = tempfile::tempdir().unwrap();
            let paths = Paths::in_runtime(root.path(), runtime.path()).unwrap();
            let listener = std::os::unix::net::UnixListener::bind(&paths.socket).unwrap();
            // A daemon that answers every request with this error.
            let fake = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if let Some(n) = line.strip_prefix("Content-Length: ") {
                        length = n.trim().parse().unwrap();
                    }
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let request: Value = serde_json::from_slice(&body).unwrap();
                let answer = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request["id"],
                    "error": {"code": code, "message": "this daemon speaks protocol 8, the client 9"},
                });
                (&stream)
                    .write_all(&timonier_core::rpc::frame(answer.to_string().as_bytes()))
                    .unwrap();
            });
            let said = refused(&paths).unwrap_err().to_string();
            fake.join().unwrap();
            said
        };
        let version = refused_with(timonier_core::protocol::errors::VERSION_REFUSED);
        assert!(version.contains("another version"), "{version}");
        assert!(
            version.contains("timonier daemon stop") && version.contains("pgrep"),
            "{version}"
        );
        let other = refused_with(-32603);
        assert!(
            other.contains("not to this client") && !other.contains("another version"),
            "{other}"
        );
    }
}
