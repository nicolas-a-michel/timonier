//! Speed 3 (ADR 0073): the SCIP indexers, run after each commit on a thread of their own, one
//! at a time, at low priority, cancelled by a newer commit. What an indexer writes goes back to
//! the pipeline's thread, which reads it into the graph; what happened to each language is kept
//! in `.timonier/scip/status.json`, for `doctor`.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use timonier_index::pack::{Environment, ScipRule};
use timonier_index::scip::{Indexed, MAX_INDEX_BYTES};

/// The folder of the indexes, the logs and the status, under `.timonier/` (ignored by git).
pub const DIR: &str = ".timonier/scip";

/// The folder of the indexes, made with its `.gitignore`: the statuses, the logs, the indexes
/// and the worktree of the commit are the machine's, never versioned (ADR 0073).
fn folder(root: &Path) -> std::io::Result<PathBuf> {
    let dir = root.join(DIR);
    std::fs::create_dir_all(&dir)?;
    let ignore = dir.join(".gitignore");
    if !ignore.exists() {
        std::fs::write(
            &ignore,
            "# Written by Timonier: the SCIP indexes and their logs stay out of git.\n*\n",
        )?;
    }
    Ok(dir)
}

/// An indexer that runs longer is stopped (ADR 0073); `TIMONIER_SCIP_TIMEOUT` in seconds.
const TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// How often a running indexer is looked at: its end, a newer commit, its time.
const POLL: Duration = Duration::from_millis(200);

/// One language of a commit to index.
#[derive(Clone, Debug)]
pub struct LanguageJob {
    pub language: String,
    pub rule: ScipRule,
    /// The language's files of the model, with their content when the job was made.
    pub files: HashMap<String, Indexed>,
}

/// A commit to index, language by language.
#[derive(Clone, Debug)]
pub struct Job {
    pub sha: String,
    pub root: PathBuf,
    /// The project's name, for the indexers that want one (`scip-python --project-name`).
    pub project: String,
    pub languages: Vec<LanguageJob>,
}

/// An index an indexer wrote, for the pipeline to read.
#[derive(Debug)]
pub struct Produced {
    pub sha: String,
    pub language: String,
    pub indexer: String,
    /// The indexer's wall time.
    pub seconds: f64,
    pub bytes: Vec<u8>,
    pub files: HashMap<String, Indexed>,
}

/// What happened to a language's index, last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Running,
    /// Read into the graph.
    Applied,
    /// The program is not found: `detail` says how to install it.
    Missing,
    /// None of the files it needs is there.
    NotReady,
    /// It confirmed too few of tree-sitter's links: a project not ready, a build that failed.
    Incomplete,
    Failed,
    TimedOut,
    TooLarge,
    /// A newer commit came first.
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LanguageStatus {
    pub indexer: String,
    pub state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The indexer's wall time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
    /// The share of tree-sitter's links the index confirmed (ADR 0073).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed: Option<f64>,
    pub at_ms: u64,
}

/// `.timonier/scip/status.json`: the commit last indexed, and each language's outcome.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    #[serde(default)]
    pub languages: BTreeMap<String, LanguageStatus>,
}

impl Status {
    pub fn path(root: &Path) -> PathBuf {
        root.join(DIR).join("status.json")
    }

    pub fn read(root: &Path) -> Self {
        std::fs::read_to_string(Self::path(root))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }
}

/// The status, shared by the indexers' thread and the pipeline's, written at each change.
#[derive(Clone)]
pub struct Statuses {
    root: PathBuf,
    status: Arc<Mutex<Status>>,
}

impl Statuses {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            status: Arc::new(Mutex::new(Status::read(root))),
        }
    }

    pub fn set(&self, sha: &str, language: &str, status: LanguageStatus) {
        let Ok(mut current) = self.status.lock() else {
            return;
        };
        if current.sha.as_deref() != Some(sha) {
            current.sha = Some(sha.to_owned());
        }
        current.languages.insert(language.to_owned(), status);
        let text = serde_json::to_string_pretty(&*current).unwrap_or_default();
        let path = Status::path(&self.root);
        let _ = folder(&self.root);
        if let Err(error) = std::fs::write(&path, text + "\n") {
            tracing::warn!(%error, "SCIP status not written");
        }
    }

    /// Whether `sha` was indexed to the end, whatever came of it: a daemon stopped while an
    /// indexer ran leaves it `running`, and it runs again.
    pub fn done(&self, sha: &str) -> bool {
        self.status.lock().ok().is_some_and(|s| {
            s.sha.as_deref() == Some(sha) && s.languages.values().all(|l| l.state != State::Running)
        })
    }
}

pub fn status(indexer: &str, state: State, detail: Option<String>) -> LanguageStatus {
    LanguageStatus {
        indexer: indexer.to_owned(),
        state,
        detail,
        seconds: None,
        confirmed: None,
        at_ms: now_ms(),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// SCIP is off: `TIMONIER_SCIP=off` (ADR 0073, « laisser couper SCIP par un réglage »).
pub fn disabled() -> bool {
    std::env::var("TIMONIER_SCIP").is_ok_and(|v| v == "off" || v == "0")
}

/// Starts the indexers' thread: jobs come on the returned sender, indexes go to `produced`.
pub fn spawn(
    statuses: Statuses,
    produced: impl Fn(Produced) + Send + 'static,
) -> std::io::Result<Sender<Job>> {
    let (send, receive) = std::sync::mpsc::channel::<Job>();
    std::thread::Builder::new()
        .name("timonier-scip".into())
        .spawn(move || run(&receive, &statuses, &produced))?;
    Ok(send)
}

fn run(jobs: &Receiver<Job>, statuses: &Statuses, produced: &impl Fn(Produced)) {
    let mut next = jobs.recv().ok();
    while let Some(job) = next.take() {
        // Two languages of one indexer, with the same arguments (C and C++): it runs once,
        // and each language reads its files from the same index (review of Phase 11, m9).
        let mut ran: HashMap<Run, Ran> = HashMap::new();
        for language in &job.languages {
            let name = &language.rule.name;
            statuses.set(
                &job.sha,
                &language.language,
                status(name, State::Running, None),
            );
            let key = (
                language.rule.program.clone(),
                language.rule.args.clone(),
                language.rule.in_place,
            );
            let (outcome, seconds) = if let Some((outcome, seconds)) = ran.get(&key) {
                (outcome.clone(), *seconds)
            } else {
                let start = Instant::now();
                let outcome = index(&job, language, || match jobs.try_recv() {
                    Ok(newer) => {
                        next = Some(newer);
                        true
                    }
                    Err(TryRecvError::Empty) => false,
                    Err(TryRecvError::Disconnected) => true,
                });
                let seconds = start.elapsed().as_secs_f64();
                ran.insert(key, (outcome.clone(), seconds));
                (outcome, seconds)
            };
            match outcome {
                Ok(bytes) => {
                    tracing::info!(language = %language.language, indexer = %name, seconds, bytes = bytes.len(), "SCIP index written");
                    produced(Produced {
                        sha: job.sha.clone(),
                        language: language.language.clone(),
                        indexer: name.clone(),
                        seconds,
                        bytes,
                        files: language.files.clone(),
                    });
                }
                Err((state, detail)) => {
                    tracing::info!(language = %language.language, indexer = %name, ?state, detail = detail.as_deref().unwrap_or(""), "no SCIP index");
                    let mut s = status(name, state, detail);
                    s.seconds = Some(seconds);
                    statuses.set(&job.sha, &language.language, s);
                }
            }
            if next.is_some() {
                break;
            }
        }
        if next.is_none() {
            next = jobs.recv().ok();
        }
    }
}

type Failure = (State, Option<String>);

/// An indexer's run, by what it runs: its program, its arguments, and where.
type Run = (String, Vec<String>, bool);

/// What a run gave, and how long it took.
type Ran = (Result<Vec<u8>, Failure>, f64);

/// Runs one language's indexer: the index's bytes, or why there is none.
fn index(
    job: &Job,
    language: &LanguageJob,
    mut cancelled: impl FnMut() -> bool,
) -> Result<Vec<u8>, Failure> {
    let rule = &language.rule;
    let Some(program) = usable(&rule.program) else {
        let detail = rule
            .install
            .as_ref()
            .map(|c| format!("install it: `{c}`"))
            .or_else(|| rule.note.clone());
        return Err((State::Missing, detail));
    };
    let needs = if rule.needs.is_empty() {
        None
    } else {
        match rule.needs.iter().find(|n| job.root.join(n).is_file()) {
            Some(found) => Some(found.clone()),
            None => return Err((State::NotReady, rule.needs_note.clone())),
        }
    };
    let variant = rule
        .variants
        .iter()
        .find(|v| job.root.join(&v.when).is_file())
        .map(|v| v.value.clone());
    let dir = folder(&job.root).map_err(|e| (State::Failed, Some(e.to_string())))?;
    let output = dir.join(format!("{}.scip", language.language));
    let _ = std::fs::remove_file(&output);
    let worktree = if rule.in_place {
        None
    } else {
        Some(Worktree::add(&job.root, &dir.join("worktree"), &job.sha)?)
    };
    let cwd = worktree
        .as_ref()
        .map_or(job.root.as_path(), |w| w.project.as_path());
    let writes_output = rule.args.iter().any(|a| a.contains("{output}"));
    let args: Vec<String> = rule
        .args
        .iter()
        .map(|a| {
            a.replace("{output}", &output.to_string_lossy())
                .replace("{project}", &job.project)
                .replace("{needs}", needs.as_deref().unwrap_or(""))
                .replace("{variant}", variant.as_deref().unwrap_or(""))
        })
        .collect();
    let log = std::fs::File::create(dir.join(format!("{}.log", language.language)))
        .map_err(|e| (State::Failed, Some(e.to_string())))?;
    let mut command = low_priority(&program);
    command
        .args(&args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(
            log.try_clone()
                .map_err(|e| (State::Failed, Some(e.to_string())))?,
        )
        .stderr(log);
    if let Some(environment) = &rule.environment {
        activate(&mut command, cwd, environment);
    }
    // What the indexer may leave in the project goes again, unless it was there before.
    let _cleanup = Cleanup {
        paths: rule
            .writes
            .iter()
            .map(|w| cwd.join(w))
            .filter(|p| !p.exists())
            .collect(),
    };
    // An indexer that writes `index.scip` where it runs: a file of that name of the project's
    // own is put aside meanwhile, and back after (review of Phase 11, m9).
    let _aside = (!writes_output)
        .then(|| Aside::new(&cwd.join("index.scip"), &dir.join("index.scip.kept")))
        .flatten();
    let mut child = command
        .spawn()
        .map_err(|e| (State::Failed, Some(format!("{}: {e}", rule.program))))?;
    let limit = std::env::var("TIMONIER_SCIP_TIMEOUT")
        .ok()
        .and_then(|s| s.parse().ok())
        .map_or(TIMEOUT, Duration::from_secs);
    wait(&mut child, limit, &mut cancelled)?;
    let written = if writes_output {
        output
    } else {
        cwd.join("index.scip")
    };
    let size = std::fs::metadata(&written)
        .map_err(|_| (State::Failed, Some(format!("{} wrote no index", rule.name))))?
        .len();
    if size > MAX_INDEX_BYTES {
        let _ = std::fs::remove_file(&written);
        return Err((State::TooLarge, Some(format!("{} MB", size / 1_000_000))));
    }
    let bytes = std::fs::read(&written).map_err(|e| (State::Failed, Some(e.to_string())))?;
    let _ = std::fs::remove_file(&written);
    Ok(bytes)
}

/// Waits for the indexer, stopped at the time limit or when a newer commit comes.
fn wait(
    child: &mut Child,
    limit: Duration,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), Failure> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(exit)) if exit.success() => return Ok(()),
            Ok(Some(exit)) => {
                return Err((
                    State::Failed,
                    Some(format!("{exit}; see its log in {DIR}/")),
                ));
            }
            Ok(None) => {}
            Err(error) => return Err((State::Failed, Some(error.to_string()))),
        }
        if cancelled() {
            stop(child);
            return Err((State::Cancelled, None));
        }
        if start.elapsed() > limit {
            stop(child);
            return Err((State::TimedOut, Some(format!("over {} s", limit.as_secs()))));
        }
        std::thread::sleep(POLL);
    }
}

/// Stops an indexer and what it started (Maven, `cargo check`, a build script): on Unix its
/// process group, which it leads (`low_priority`); on Windows its tree (review of Phase 11, I5).
fn stop(child: &mut Child) {
    let pid = child.id().to_string();
    let tree = if cfg!(unix) {
        Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    } else {
        Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    };
    if !tree.is_ok_and(|s| s.success()) {
        let _ = child.kill();
    }
    let _ = child.wait();
}

/// The program at low priority: `nice -n 10` where there is one (Linux, macOS), leading a
/// process group of its own, so that `stop` reaches what it starts.
fn low_priority(program: &Path) -> Command {
    let mut command = if cfg!(unix)
        && let Some(nice) = find_program("nice")
    {
        let mut command = Command::new(nice);
        command.arg("-n").arg("10").arg(program);
        command
    } else {
        Command::new(program)
    };
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command
}

/// The project's environment, if one of its folders is there, as its activation would.
fn activate(command: &mut Command, cwd: &Path, environment: &Environment) {
    let Some(env) = environment
        .folders
        .iter()
        .map(|c| cwd.join(c))
        .find(|d| d.join(&environment.marker).is_file())
    else {
        return;
    };
    let bin = env.join(if cfg!(windows) {
        &environment.bin_windows
    } else {
        &environment.bin
    });
    let path = std::env::var_os("PATH").unwrap_or_default();
    let joined = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path)))
        .unwrap_or(path);
    command.env("PATH", joined).env(&environment.variable, &env);
}

/// A program found that answers `--version`: rustup's proxy for a component not installed is
/// on the `PATH` and fails.
pub fn usable(program: &str) -> Option<PathBuf> {
    let found = find_program(program)?;
    Command::new(&found)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .filter(std::process::ExitStatus::success)
        .map(|_| found)
}

/// A program on the `PATH`, or in the folders the installers put it in, which the daemon's
/// `PATH`, inherited from the editor, may lack.
pub fn find_program(program: &str) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        vec![
            format!("{program}.exe"),
            format!("{program}.cmd"),
            program.to_owned(),
        ]
    } else {
        vec![program.to_owned()]
    };
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let extra = home.into_iter().flat_map(|h| {
        crate::packs::bin_dirs()
            .into_iter()
            .map(move |dir| h.join(dir))
    });
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .chain(extra)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|candidate| candidate.is_file())
}

/// Files an indexer created in the project, removed when dropped, whatever the outcome.
struct Cleanup {
    paths: Vec<PathBuf>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// A file of the project put aside while an indexer runs, put back when dropped.
struct Aside {
    path: PathBuf,
    kept: PathBuf,
}

impl Aside {
    fn new(path: &Path, kept: &Path) -> Option<Self> {
        std::fs::rename(path, kept).ok()?;
        Some(Self {
            path: path.to_path_buf(),
            kept: kept.to_path_buf(),
        })
    }
}

impl Drop for Aside {
    fn drop(&mut self) {
        let _ = std::fs::rename(&self.kept, &self.path);
    }
}

/// A git worktree of the commit, removed when dropped: where an indexer that cleans the build
/// outputs runs (ADR 0073).
struct Worktree {
    root: PathBuf,
    path: PathBuf,
    /// The project in it: the worktree holds the whole repository, and the project may be one
    /// of its folders (review of Phase 11, I-a).
    project: PathBuf,
}

impl Worktree {
    fn add(root: &Path, path: &Path, sha: &str) -> Result<Self, Failure> {
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
        };
        let _ = git(&["worktree", "remove", "--force", &path.to_string_lossy()]);
        let _ = std::fs::remove_dir_all(path);
        let prefix = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["rev-parse", "--show-prefix"])
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default();
        match git(&["worktree", "add", "--detach", &path.to_string_lossy(), sha]) {
            Ok(status) if status.success() => Ok(Self {
                root: root.to_path_buf(),
                path: path.to_path_buf(),
                project: path.join(prefix),
            }),
            _ => Err((State::Failed, Some("git worktree add failed".into()))),
        }
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        let _ = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(["worktree", "remove", "--force"])
            .arg(&self.path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Waits for an index, for the tests: `RecvTimeoutError` when none comes in time.
#[cfg(test)]
fn next(
    receive: &Receiver<Produced>,
    within: Duration,
) -> Result<Produced, std::sync::mpsc::RecvTimeoutError> {
    receive.recv_timeout(within)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake indexer: `bash`, which answers `--version` (`sh` does not, on Debian).
    fn rule(program: &str, args: &[&str]) -> ScipRule {
        toml::from_str(&format!(
            "name = \"fake\"\nprogram = \"{program}\"\nargs = [{}]\n",
            args.iter()
                .map(|a| format!("\"{a}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .expect("rule")
    }

    fn job(root: &Path, rule: ScipRule) -> Job {
        Job {
            sha: "c0ffee".into(),
            root: root.to_path_buf(),
            project: "demo".into(),
            languages: vec![LanguageJob {
                language: "rust".into(),
                rule,
                files: HashMap::new(),
            }],
        }
    }

    /// An indexer writes its index where `{output}` says; the thread hands its bytes over and
    /// removes the file.
    #[cfg(unix)]
    #[test]
    fn an_indexer_s_index_goes_to_the_pipeline() {
        let dir = tempfile::tempdir().unwrap();
        let statuses = Statuses::new(dir.path());
        let (send, receive) = std::sync::mpsc::channel();
        let jobs = spawn(statuses.clone(), move |p| send.send(p).unwrap()).unwrap();
        jobs.send(job(
            dir.path(),
            rule("bash", &["-c", "printf scip > \\\"$0\\\"", "{output}"]),
        ))
        .unwrap();
        let produced = next(&receive, Duration::from_secs(20)).unwrap();
        assert_eq!(
            (produced.sha.as_str(), produced.language.as_str()),
            ("c0ffee", "rust")
        );
        assert_eq!(produced.bytes, b"scip");
        assert!(!dir.path().join(DIR).join("rust.scip").exists());
        // The folder stays out of git, whatever the project's ignore files say.
        let ignore = std::fs::read_to_string(dir.path().join(DIR).join(".gitignore")).unwrap();
        assert!(ignore.lines().any(|l| l == "*"), "{ignore}");
    }

    /// A missing program, missing files, a failure: no index, and the status says why.
    #[cfg(unix)]
    #[test]
    fn the_status_says_why_there_is_no_index() {
        let dir = tempfile::tempdir().unwrap();
        let statuses = Statuses::new(dir.path());
        let (send, _receive) = std::sync::mpsc::channel();
        let jobs = spawn(statuses.clone(), move |p| send.send(p).unwrap()).unwrap();
        // One commit at a time: a newer one would cancel the one before.
        let outcome = |sha: &str, rule: ScipRule| {
            let mut j = job(dir.path(), rule);
            j.sha = sha.into();
            jobs.send(j).unwrap();
            let start = Instant::now();
            loop {
                let status = Status::read(dir.path());
                if status.sha.as_deref() == Some(sha)
                    && status
                        .languages
                        .get("rust")
                        .is_some_and(|l| l.state != State::Running)
                {
                    return status.languages["rust"].clone();
                }
                assert!(start.elapsed() < Duration::from_secs(20), "{status:?}");
                std::thread::sleep(Duration::from_millis(50));
            }
        };
        let mut missing = rule("timonier-no-such-indexer", &[]);
        missing.install = Some("cargo install it".into());
        let missing = outcome("c0ffee", missing);
        assert_eq!(missing.state, State::Missing);
        assert_eq!(
            missing.detail.as_deref(),
            Some("install it: `cargo install it`")
        );
        let mut not_ready = rule("bash", &["-c", "true"]);
        not_ready.needs = vec!["compile_commands.json".into()];
        not_ready.needs_note = Some("build it once".into());
        let not_ready = outcome("d00d", not_ready);
        assert_eq!(
            (not_ready.state, not_ready.detail.as_deref()),
            (State::NotReady, Some("build it once"))
        );
        let failed = outcome("beef", rule("bash", &["-c", "exit 3"]));
        assert_eq!(failed.state, State::Failed);
        assert!(failed.detail.unwrap().starts_with("exit status: 3"));
    }

    /// A newer commit stops the indexer that runs for an older one.
    #[cfg(unix)]
    #[test]
    fn a_newer_commit_cancels_the_running_indexer() {
        let dir = tempfile::tempdir().unwrap();
        let statuses = Statuses::new(dir.path());
        let (send, receive) = std::sync::mpsc::channel();
        let jobs = spawn(statuses.clone(), move |p| send.send(p).unwrap()).unwrap();
        jobs.send(job(dir.path(), rule("bash", &["-c", "sleep 30"])))
            .unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let mut newer = job(
            dir.path(),
            rule("bash", &["-c", "printf new > \\\"$0\\\"", "{output}"]),
        );
        newer.sha = "f00d".into();
        let start = Instant::now();
        jobs.send(newer).unwrap();
        let produced = next(&receive, Duration::from_secs(20)).unwrap();
        assert_eq!(
            (produced.sha.as_str(), produced.bytes.as_slice()),
            ("f00d", &b"new"[..])
        );
        assert!(start.elapsed() < Duration::from_secs(10));
    }

    /// Cancelled, the indexer goes with what it started: a build it launched does not run on
    /// (review of Phase 11, I5).
    #[cfg(unix)]
    #[test]
    fn a_cancelled_indexer_takes_what_it_started_along() {
        let dir = tempfile::tempdir().unwrap();
        let statuses = Statuses::new(dir.path());
        let (send, receive) = std::sync::mpsc::channel();
        let jobs = spawn(statuses.clone(), move |p| send.send(p).unwrap()).unwrap();
        jobs.send(job(
            dir.path(),
            rule("bash", &["-c", "(sleep 2; touch started-build) & sleep 30"]),
        ))
        .unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let mut newer = job(
            dir.path(),
            rule("bash", &["-c", "printf new > \\\"$0\\\"", "{output}"]),
        );
        newer.sha = "f00d".into();
        jobs.send(newer).unwrap();
        next(&receive, Duration::from_secs(20)).unwrap();
        std::thread::sleep(Duration::from_secs(3));
        assert!(!dir.path().join("started-build").exists());
    }

    /// An indexer that reads the commit in a worktree runs in the project's folder of it, when
    /// the project is a folder of its repository (review of Phase 11, I-a).
    #[cfg(unix)]
    #[test]
    fn a_worktree_indexer_runs_in_the_project_s_folder() {
        let repo = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let done = Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(["-c", "user.name=T", "-c", "user.email=t@example.com"])
                .args(["-c", "commit.gpgsign=false"])
                .args(args)
                .output()
                .unwrap();
            assert!(done.status.success(), "git {args:?}");
            String::from_utf8(done.stdout).unwrap().trim().to_owned()
        };
        std::fs::create_dir_all(repo.path().join("app")).unwrap();
        std::fs::write(repo.path().join("app/pom.xml"), "<project/>\n").unwrap();
        git(&["init", "--quiet", "--initial-branch=main"]);
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "first"]);
        let root = repo.path().join("app");
        let statuses = Statuses::new(&root);
        let (send, receive) = std::sync::mpsc::channel();
        let jobs = spawn(statuses, move |p| send.send(p).unwrap()).unwrap();
        let mut in_worktree = rule("bash", &["-c", "ls > \\\"$0\\\"", "{output}"]);
        in_worktree.in_place = false;
        let mut sub = job(&root, in_worktree);
        sub.sha = git(&["rev-parse", "HEAD"]);
        jobs.send(sub).unwrap();
        let produced = next(&receive, Duration::from_secs(20)).unwrap();
        assert_eq!(String::from_utf8(produced.bytes).unwrap(), "pom.xml\n");
    }

    /// One indexer for two languages runs once, and each gets the index (review of Phase 11,
    /// m9); an `index.scip` of the project's own is back after an indexer that writes its own.
    #[cfg(unix)]
    #[test]
    fn a_shared_indexer_runs_once_and_the_project_s_index_stays() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.scip"), "mine").unwrap();
        let statuses = Statuses::new(dir.path());
        let (send, receive) = std::sync::mpsc::channel();
        let jobs = spawn(statuses.clone(), move |p| send.send(p).unwrap()).unwrap();
        let shared = rule(
            "bash",
            &["-c", "echo run >> runs.log; printf theirs > index.scip"],
        );
        let mut both = job(dir.path(), shared.clone());
        both.languages.push(LanguageJob {
            language: "javascript".into(),
            rule: shared,
            files: HashMap::new(),
        });
        jobs.send(both).unwrap();
        let first = next(&receive, Duration::from_secs(20)).unwrap();
        let second = next(&receive, Duration::from_secs(20)).unwrap();
        assert_eq!(
            (first.bytes.as_slice(), second.bytes.as_slice()),
            (&b"theirs"[..], &b"theirs"[..])
        );
        assert_eq!(
            (first.language.as_str(), second.language.as_str()),
            ("rust", "javascript")
        );
        let runs = std::fs::read_to_string(dir.path().join("runs.log")).unwrap();
        assert_eq!(runs.lines().count(), 1, "{runs}");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("index.scip")).unwrap(),
            "mine"
        );
    }

    /// A file the indexer creates in the project goes again; one that was there stays.
    #[cfg(unix)]
    #[test]
    fn what_an_indexer_writes_in_the_project_goes_again() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("kept.json"), "{}").unwrap();
        let statuses = Statuses::new(dir.path());
        let (send, receive) = std::sync::mpsc::channel();
        let jobs = spawn(statuses, move |p| send.send(p).unwrap()).unwrap();
        let mut writing = rule(
            "bash",
            &[
                "-c",
                "echo x > tsconfig.json; echo y > kept.json; printf i > $0",
                "{output}",
            ],
        );
        writing.writes = vec!["tsconfig.json".into(), "kept.json".into()];
        jobs.send(job(dir.path(), writing)).unwrap();
        next(&receive, Duration::from_secs(20)).unwrap();
        assert!(!dir.path().join("tsconfig.json").exists());
        assert!(dir.path().join("kept.json").exists());
    }

    /// A virtual environment of the project comes first on the indexer's `PATH`.
    #[cfg(unix)]
    #[test]
    fn the_project_s_virtual_environment_is_activated() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".venv/bin")).unwrap();
        std::fs::write(dir.path().join(".venv/pyvenv.cfg"), "home = /usr\n").unwrap();
        let mut command = Command::new("sh");
        let environment = Environment {
            folders: vec![".venv".into()],
            marker: "pyvenv.cfg".into(),
            bin: "bin".into(),
            bin_windows: "Scripts".into(),
            variable: "VIRTUAL_ENV".into(),
        };
        activate(&mut command, dir.path(), &environment);
        let envs: HashMap<_, _> = command
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert!(
            envs["PATH"]
                .as_deref()
                .unwrap()
                .starts_with(&dir.path().join(".venv/bin").to_string_lossy().into_owned())
        );
        assert_eq!(
            envs["VIRTUAL_ENV"].as_deref(),
            Some(dir.path().join(".venv").to_string_lossy().as_ref())
        );
    }
}
