//! The commands of `.timonier/tests.json` this machine approved (Q84; audit of 2026-10-21,
//! finding 1): the file is versioned, so a cloned repository could make `timonier test` or
//! `timonier doctor` run anything through `sh -c`. Each approved command is kept, as the hash of
//! the project's path and the command, outside the repository: in `$TIMONIER_DATA_DIR`, else
//! `$XDG_DATA_HOME/timonier`, else `~/.local/share/timonier`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const FILE: &str = "approved-commands.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct Approved {
    version: u32,
    /// BLAKE3 of `<canonical project path>\0<command>`, in hexadecimal.
    commands: BTreeSet<String>,
}

fn folder() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(data) = tests::DATA.with(|d| d.borrow().clone()) {
        return Some(data);
    }
    timonier_daemon::agents::data_dir()
}

fn key(root: &Path, command: &str) -> String {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut hasher = blake3::Hasher::new();
    hasher.update(root.as_os_str().as_encoded_bytes());
    hasher.update(&[0]);
    hasher.update(command.as_bytes());
    hasher.finalize().to_hex().to_string()
}

fn read(path: &Path) -> Approved {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Whether this machine approved `command` for the project at `root`.
pub fn approved(root: &Path, command: &str) -> bool {
    folder().is_some_and(|folder| {
        read(&folder.join(FILE))
            .commands
            .contains(&key(root, command))
    })
}

/// Approves `commands` for the project at `root`: what `init` wrote, or what `--approve` saw.
pub fn approve<'a>(root: &Path, commands: impl IntoIterator<Item = &'a str>) -> Result<()> {
    let folder = folder().context("no home to keep the approved commands in")?;
    let path = folder.join(FILE);
    let mut approved = read(&path);
    approved.version = 1;
    let before = approved.commands.len();
    approved
        .commands
        .extend(commands.into_iter().map(|c| key(root, c)));
    if approved.commands.len() == before && path.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&folder)
        .with_context(|| format!("cannot create {}", folder.display()))?;
    let text = serde_json::to_string_pretty(&approved)?;
    // Written whole, then renamed: a crash leaves the old list.
    let temporary = folder.join(format!("{FILE}.{}", std::process::id()));
    std::fs::write(&temporary, text + "\n")
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    std::fs::rename(&temporary, &path).with_context(|| format!("cannot write {}", path.display()))
}

/// What to say of commands the machine has not approved.
pub fn refusal(commands: &[&str], flag: &str) -> String {
    let listed: Vec<String> = commands.iter().map(|c| format!("  {c}")).collect();
    format!(
        "`.timonier/tests.json` asks to run commands this machine has not approved:\n{}\n\
         It is part of the repository: read them, then run with `{flag}` to approve them once.",
        listed.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_approved_for_its_project_only() {
        let data = tempfile::tempdir().unwrap();
        let one = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        with_data(data.path(), || {
            assert!(!approved(one.path(), "cargo test"));
            approve(one.path(), ["cargo test"]).unwrap();
            assert!(approved(one.path(), "cargo test"));
            assert!(!approved(one.path(), "cargo test; curl evil"));
            assert!(!approved(other.path(), "cargo test"));
            approve(one.path(), ["cargo test"]).unwrap();
            let file: Approved =
                serde_json::from_slice(&std::fs::read(data.path().join(FILE)).unwrap()).unwrap();
            assert_eq!(file.commands.len(), 1);
        });
        assert!(refusal(&["sh x"], "--approve").contains("  sh x"));
    }

    /// Runs `f` with the approvals kept under `data`: the environment stays as it is (the
    /// workspace forbids `unsafe`, which changing it needs).
    fn with_data(data: &Path, f: impl FnOnce()) {
        DATA.with(|d| *d.borrow_mut() = Some(data.to_path_buf()));
        f();
        DATA.with(|d| *d.borrow_mut() = None);
    }

    thread_local! {
        pub(super) static DATA: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
    }
}
