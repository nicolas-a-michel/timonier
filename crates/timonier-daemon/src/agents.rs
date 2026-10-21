//! The agents' telemetry (ADR 0058): one daemon of the machine holds the agents' port and keeps
//! each request of Claude Code in a spool, one file per Claude Code session, in the machine's
//! data folder; every daemon reads there the requests of the sessions its stop hook names.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use timonier_core::usage::AgentRequest;
use tokio::sync::watch;

/// The machine's port for the agents' telemetry.
pub const PORT: u16 = timonier_otlp::AGENT_PORT;

/// How often a daemon that does not hold the agents' port tries again.
const RETRY: Duration = Duration::from_secs(60);

/// Spool files untouched for longer go.
const KEEP: Duration = Duration::from_secs(90 * 24 * 3600);

/// The machine's data folder, outside any repository: `$TIMONIER_DATA_DIR`, else
/// `$XDG_DATA_HOME/timonier`, else `~/.local/share/timonier` (ADR 0057, 0058).
pub fn data_dir() -> Option<PathBuf> {
    let var = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    var("TIMONIER_DATA_DIR")
        .or_else(|| var("XDG_DATA_HOME").map(|d| d.join("timonier")))
        .or_else(|| var("HOME").map(|h| h.join(".local").join("share").join("timonier")))
}

/// `<data>/agents/projects.json`: the projects whose `init` turned the agents' telemetry on (ADR
/// 0058). Their daemons hold the agents' port; `uninit` takes the settings back with the last.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Projects {
    version: u32,
    projects: Vec<PathBuf>,
}

fn projects_file(data: &Path) -> PathBuf {
    data.join("agents").join("projects.json")
}

fn projects(data: &Path) -> Projects {
    std::fs::read_to_string(projects_file(data))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Projects {
            version: 1,
            projects: Vec::new(),
        })
}

fn write_projects(data: &Path, projects: &Projects) -> std::io::Result<()> {
    let path = projects_file(data);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut text = serde_json::to_string_pretty(projects).map_err(std::io::Error::other)?;
    text.push('\n');
    std::fs::write(path, text)
}

/// `root` measures its agents: its daemon holds the agents' port.
pub fn registered(data: &Path, root: &Path) -> bool {
    projects(data).projects.iter().any(|p| p == root)
}

pub fn register(data: &Path, root: &Path) -> std::io::Result<()> {
    let mut known = projects(data);
    if !known.projects.iter().any(|p| p == root) {
        known.projects.push(root.to_path_buf());
        known.projects.sort();
    }
    write_projects(data, &known)
}

/// Takes `root` out; returns how many projects still measure their agents.
pub fn unregister(data: &Path, root: &Path) -> std::io::Result<usize> {
    let mut known = projects(data);
    known.projects.retain(|p| p != root);
    write_projects(data, &known)?;
    Ok(known.projects.len())
}

/// Where Claude Code's requests are kept: `<data>/agents/claude/<session>.jsonl`.
#[derive(Clone, Debug)]
pub struct Spool {
    dir: PathBuf,
}

impl Spool {
    pub fn new(data: &Path) -> Self {
        Self {
            dir: data.join("agents").join("claude"),
        }
    }

    /// The file of a session; `None` for an id that is not a plain name.
    fn file(&self, session: &str) -> Option<PathBuf> {
        let plain = !session.is_empty()
            && session.len() <= 128
            && session
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
            && !session.starts_with('.');
        plain.then(|| self.dir.join(format!("{session}.jsonl")))
    }

    pub fn append(&self, session: &str, requests: &[AgentRequest]) -> std::io::Result<()> {
        let Some(path) = self.file(session) else {
            return Ok(());
        };
        std::fs::create_dir_all(&self.dir)?;
        let mut lines = String::new();
        for request in requests {
            lines.push_str(&serde_json::to_string(request).map_err(std::io::Error::other)?);
            lines.push('\n');
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        file.write_all(lines.as_bytes())
    }

    /// The requests kept for a session; a line that does not read is skipped.
    pub fn read(&self, session: &str) -> Vec<AgentRequest> {
        let Some(text) = self
            .file(session)
            .and_then(|path| std::fs::read_to_string(path).ok())
        else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    /// Removes the files untouched for 90 days.
    pub fn prune(&self, now: SystemTime) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age > KEEP);
            if old {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

struct Keep(Spool);

impl timonier_otlp::AgentSink for Keep {
    fn requests(&self, requests: Vec<(String, AgentRequest)>) {
        let mut by_session: std::collections::BTreeMap<String, Vec<AgentRequest>> =
            std::collections::BTreeMap::new();
        for (session, request) in requests {
            by_session.entry(session).or_default().push(request);
        }
        for (session, requests) in by_session {
            if let Err(error) = self.0.append(&session, &requests) {
                tracing::warn!(%error, "cannot keep the agent's requests");
            }
        }
    }
}

/// Holds the agents' port while the daemon runs: takes it if it is free, and tries again every
/// minute while another daemon of the machine has it.
pub async fn hold(spool: Spool, port: u16, mut shutdown: watch::Receiver<bool>) {
    spool.prune(SystemTime::now());
    let sink = Arc::new(Keep(spool));
    loop {
        match timonier_otlp::AgentReceiver::start(sink.clone(), port).await {
            Ok(receiver) => {
                let _ = shutdown.wait_for(|stop| *stop).await;
                receiver.stop().await;
                return;
            }
            Err(error) => {
                tracing::debug!(%error, port, "the agents' port is another daemon's");
                tokio::select! {
                    () = tokio::time::sleep(RETRY) => {}
                    _ = shutdown.wait_for(|stop| *stop) => return,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(at_ms: i64) -> AgentRequest {
        AgentRequest {
            at_ms,
            model: "claude-opus-5-5".into(),
            input: 3,
            ..AgentRequest::default()
        }
    }

    #[test]
    fn projects_register_and_the_last_one_out_says_so() {
        let data = tempfile::tempdir().unwrap();
        let (a, b) = (Path::new("/work/a"), Path::new("/work/b"));
        assert!(!registered(data.path(), a));
        register(data.path(), a).unwrap();
        register(data.path(), b).unwrap();
        register(data.path(), a).unwrap();
        assert!(registered(data.path(), a));
        assert_eq!(unregister(data.path(), a).unwrap(), 1);
        assert!(!registered(data.path(), a));
        assert_eq!(unregister(data.path(), b).unwrap(), 0);
    }

    #[test]
    fn the_spool_keeps_each_session_apart_and_refuses_odd_names() {
        let data = tempfile::tempdir().unwrap();
        let spool = Spool::new(data.path());
        spool.append("abc-1", &[request(1), request(2)]).unwrap();
        spool.append("abc-1", &[request(3)]).unwrap();
        spool.append("other", &[request(9)]).unwrap();
        let ats: Vec<i64> = spool.read("abc-1").iter().map(|r| r.at_ms).collect();
        assert_eq!(ats, [1, 2, 3]);
        assert_eq!(spool.read("nobody"), []);
        for odd in ["", "../x", ".hidden", "a/b"] {
            spool.append(odd, &[request(1)]).unwrap();
            assert_eq!(spool.read(odd), [], "{odd}");
        }
        assert!(!data.path().join("x.jsonl").exists());
        // Ninety days later, the files go.
        spool.prune(SystemTime::now() + KEEP + Duration::from_secs(1));
        assert_eq!(spool.read("abc-1"), []);
    }
}
