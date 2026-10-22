//! `timonier daemon start|stop|status|run` (PLAN.md 4.3), on a temporary project, with the
//! runtime directory in a temporary directory too.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::{cargo_bin, cargo_bin_cmd};

/// A project, its runtime directory, and a daemon stopped when the test ends, even failed.
struct Project {
    repo: tempfile::TempDir,
    runtime: tempfile::TempDir,
}

impl Project {
    fn new() -> Self {
        let repo = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(repo.path().join("src")).expect("src");
        std::fs::write(
            repo.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        std::fs::write(repo.path().join("src/lib.rs"), "pub fn f() {}\n").expect("lib");
        Self {
            repo,
            runtime: tempfile::tempdir().expect("temp dir"),
        }
    }

    fn root(&self) -> PathBuf {
        self.repo.path().canonicalize().expect("canonical root")
    }

    fn run_dir(&self) -> PathBuf {
        self.runtime.path().join("run")
    }

    /// `tim daemon <action> --root <project>`.
    fn tim(&self, action: &str) -> Output {
        self.tim_in(action, self.repo.path(), true)
    }

    fn tim_in(&self, action: &str, dir: &Path, with_root: bool) -> Output {
        let mut command = cargo_bin_cmd!("tim");
        command
            .args(["daemon", action])
            .current_dir(dir)
            .env("TIMONIER_RUNTIME_DIR", self.run_dir());
        if with_root {
            command.arg("--root").arg(self.repo.path());
        }
        command.output().expect("tim runs")
    }

    fn socket(&self) -> Option<PathBuf> {
        let entries = std::fs::read_dir(self.run_dir()).ok()?;
        entries
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "sock"))
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self.tim("stop");
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn status_says_when_no_daemon_runs() {
    let project = Project::new();
    let output = project.tim("status");
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    assert!(
        stdout(&output).contains("No Timonier daemon runs for"),
        "{output:?}"
    );
    // Stopping nothing is not an error.
    let output = project.tim("stop");
    assert!(output.status.success(), "{output:?}");
    assert!(
        stdout(&output).contains("No Timonier daemon runs"),
        "{output:?}"
    );
}

#[test]
fn start_status_then_stop() {
    let project = Project::new();
    let started = project.tim("start");
    assert!(started.status.success(), "{started:?}");
    assert!(
        stdout(&started).contains("The Timonier daemon runs for"),
        "{started:?}"
    );
    assert!(project.root().join(".timonier/graph.sqlite").is_file());

    let again = project.tim("start");
    assert!(again.status.success(), "{again:?}");
    assert!(stdout(&again).contains("already runs"), "{again:?}");

    let status = project.tim("status");
    assert!(status.status.success(), "{status:?}");
    let text = stdout(&status);
    assert!(
        text.contains(&project.root().display().to_string()),
        "{text}"
    );
    assert!(
        text.contains("files") && text.contains("clients      1 (this one included)"),
        "{text}"
    );
    // The log of a daemon started in the background.
    let log = std::fs::read_dir(project.run_dir())
        .unwrap()
        .flatten()
        .find(|e| e.path().extension().is_some_and(|x| x == "log"))
        .expect("a log");
    let log = std::fs::read_to_string(log.path()).unwrap();
    assert!(log.contains("daemon serving"), "{log}");

    let stopped = project.tim("stop");
    assert!(stopped.status.success(), "{stopped:?}");
    assert!(stdout(&stopped).contains("stopped"), "{stopped:?}");
    assert_eq!(project.tim("status").status.code(), Some(3));
    assert!(project.socket().is_none(), "the socket stayed");
}

#[test]
fn the_project_is_found_from_a_subdirectory() {
    let project = Project::new();
    std::fs::create_dir(project.repo.path().join(".timonier")).unwrap();
    let src = project.repo.path().join("src");
    let started = project.tim_in("start", &src, false);
    assert!(started.status.success(), "{started:?}");
    let status = project.tim_in("status", &src, false);
    assert!(status.status.success(), "{status:?}");
    let expected = format!("runs for {}.", project.root().display());
    assert!(stdout(&status).contains(&expected), "{status:?}");
}

/// Waits for `child` to exit, up to `within`.
fn exited(child: &mut Child, within: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("wait") {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
// SIGINT through `kill`: Unix.
#[cfg(unix)]
fn run_serves_in_the_foreground_until_a_signal() {
    let project = Project::new();
    let mut child = Command::new(cargo_bin("tim"))
        .args(["daemon", "run", "--root"])
        .arg(project.repo.path())
        .env("TIMONIER_RUNTIME_DIR", project.run_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tim runs");
    // Once the socket listens, one JSON line says where, and that this process serves.
    let mut line = String::new();
    std::io::BufRead::read_line(
        &mut std::io::BufReader::new(child.stdout.take().unwrap()),
        &mut line,
    )
    .unwrap();
    let announced: serde_json::Value = serde_json::from_str(&line).expect("a JSON line");
    assert_eq!(announced["pid"], child.id());
    let socket = PathBuf::from(announced["socket"].as_str().unwrap());
    assert_eq!(Some(socket.clone()), project.socket());
    assert!(project.tim("status").status.success());
    // A second one sees the first, says where it is, and leaves it alone.
    let second = project.tim("run");
    assert!(second.status.success(), "{second:?}");
    assert!(
        String::from_utf8_lossy(&second.stderr).contains("already runs"),
        "{second:?}"
    );
    let other: serde_json::Value = serde_json::from_slice(&second.stdout).expect("a JSON line");
    assert_eq!(other["pid"], serde_json::Value::Null);
    assert_eq!(other["socket"].as_str(), socket.to_str());

    let kill = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("kill runs");
    assert!(kill.success());
    let status = exited(&mut child, Duration::from_secs(20)).expect("the daemon stopped");
    assert!(status.success(), "{status:?}");
    let mut stderr = String::new();
    std::io::Read::read_to_string(&mut child.stderr.take().unwrap(), &mut stderr).unwrap();
    assert!(
        stderr.contains("signal received") && stderr.contains("daemon stopped"),
        "{stderr}"
    );
    assert_eq!(project.tim("status").status.code(), Some(3));
}

#[test]
fn run_outlives_the_process_that_read_its_output() {
    let project = Project::new();
    let mut child = Command::new(cargo_bin("tim"))
        .args(["daemon", "run", "--root"])
        .arg(project.repo.path())
        .env("TIMONIER_RUNTIME_DIR", project.run_dir())
        // Something to log for each client, to a reader that is gone.
        .env("RUST_LOG", "debug")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tim runs");
    let mut line = String::new();
    std::io::BufRead::read_line(
        &mut std::io::BufReader::new(child.stdout.take().unwrap()),
        &mut line,
    )
    .unwrap();
    // The reader goes, as the extension does when its window closes.
    drop(child.stderr.take());
    for _ in 0..3 {
        assert!(project.tim("status").status.success());
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(child.try_wait().unwrap().is_none(), "the daemon died");
    assert!(project.tim("stop").status.success());
    let status = exited(&mut child, Duration::from_secs(20)).expect("the daemon stopped");
    assert!(status.success(), "{status:?}");
}
