//! `timonier uninit` (PLAN.md 6.9, ADR 0033): takes back what `init` made, from its record. A
//! file it created goes, unless the developer added to it since; a file it changed loses
//! Timonier's part, and comes back exactly as it was when nothing else changed in it.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::detect::Agent;
use super::edits::{self, HASH_BEGIN, HASH_END, MD_BEGIN, MD_END};
use super::plan::{Change, Planned, RECORD, Record, stop_command};

/// What Timonier added to `path`, taken out of `text`; `None` when nothing else is left.
fn without(path: &Path, text: &str) -> Result<Option<String>, String> {
    let name = path.display().to_string();
    let hooks = |text: &str| -> Result<Option<String>, String> {
        let mut text = Some(text.to_owned());
        for agent in Agent::ALL {
            let Some(current) = &text else { break };
            text = edits::json_without_hook(current, "Stop", &stop_command(agent))?;
        }
        Ok(text)
    };
    match name.as_str() {
        "AGENTS.md" | "CLAUDE.md" => Ok(edits::without_block(text, MD_BEGIN, MD_END)),
        ".mcp.json" | ".cursor/mcp.json" => {
            edits::json_without_entry(text, "mcpServers", "timonier")
        }
        ".vscode/mcp.json" => edits::json_without_entry(text, "servers", "timonier"),
        ".claude/settings.json" | ".codex/hooks.json" | ".github/hooks/timonier.json" => {
            hooks(text)
        }
        ".codex/config.toml" | ".config/nextest.toml" => {
            Ok(edits::without_block(text, HASH_BEGIN, HASH_END))
        }
        // The OpenTelemetry SDK (ADR 0050): Timonier's blocks only.
        "Cargo.toml" => Ok(edits::cargo_without_sdk(text)),
        _ if name.ends_with("pre-commit") => {
            // A hook of Timonier's own making: the interpreter line alone is left.
            Ok(edits::without_block(text, HASH_BEGIN, HASH_END)
                .filter(|rest| rest.trim() != "#!/bin/sh"))
        }
        // The record, the ignore file: Timonier's.
        _ => Ok(None),
    }
}

/// The packages of the OpenTelemetry SDK the npm packs add (ADR 0050).
fn sdk_packages() -> Vec<&'static str> {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES
        .get_or_init(|| {
            timonier_daemon::Packs::builtin()
                .map(|packs| {
                    packs
                        .iter()
                        .filter(|p| p.manifest.ecosystem.as_str() == "npm")
                        .flat_map(|p| p.manifest.instrumentation.iter())
                        .flat_map(|i| i.dependencies.iter().map(|d| d.name.clone()))
                        .collect()
                })
                .unwrap_or_default()
        })
        .iter()
        .map(String::as_str)
        .collect()
}

/// The same content, whatever the layout: JSON compared as values.
fn same(path: &Path, a: &str, b: &str) -> bool {
    if path.extension().is_some_and(|e| e == "json") {
        let parse = |t: &str| serde_json::from_str::<Value>(t).ok();
        return parse(a).is_some() && parse(a) == parse(b);
    }
    a == b
}

pub struct Undone {
    pub planned: Planned,
    /// The tracking goes (`init` alone wrote it, or `--purge` asked for it, Q54), and with it
    /// `.timonier/` whole: `init` made the folder (the daemon is stopped first).
    pub remove_folder: bool,
    /// What goes of the tracking when the folder stays, relative to the root.
    pub tracking_paths: Vec<PathBuf>,
}

/// Whether the tracking holds only what `init` wrote: its first session, its milestone, an
/// empty todo (Q54).
pub fn only_init_tracking(root: &Path) -> bool {
    let dir = root.join(".timonier");
    let json = |path: PathBuf| -> Option<serde_json::Value> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    };
    let sessions_by_init = std::fs::read_dir(dir.join("sessions"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("S-"))
        .all(|e| json(e.path()).is_some_and(|s| s["agent"] == super::AGENT));
    let plan_by_init = json(dir.join("plan.json")).is_none_or(|plan| {
        plan["milestones"]
            .as_array()
            .is_none_or(|m| m.iter().all(|m| m["title"] == "Initialisation"))
    });
    let todo_empty = json(dir.join("todo.json"))
        .is_none_or(|todo| todo["items"].as_array().is_none_or(Vec::is_empty));
    // The empty whiteboard `init` wrote is init's; one drawn on is the developer's.
    // Larger than the empty one, it was drawn on: not read (the audit's finding 3).
    let empty = timonier_core::intent::Intent::empty().to_text();
    let path = dir.join("intent.json");
    let no_intent = match std::fs::metadata(&path) {
        Err(_) => true,
        Ok(meta) if meta.len() > empty.len() as u64 => false,
        Ok(_) => std::fs::read_to_string(&path).is_ok_and(|text| text == empty),
    };
    sessions_by_init && plan_by_init && todo_empty && no_intent
}

/// The project's own files in `.timonier/`: what `uninit` keeps unless `--purge` (Q54).
pub const TRACKING: [&str; 4] = ["plan.json", "todo.json", "sessions/", "intent.json"];

pub fn plan(root: &Path, purge: bool) -> Result<Undone, String> {
    let record: Record = std::fs::read_to_string(root.join(RECORD))
        .map_err(|_| {
            "Timonier was not set up here by `timonier init`: nothing to take back".to_owned()
        })
        .and_then(|t| {
            serde_json::from_str(&t).map_err(|e| format!("{RECORD} does not read: {e}"))
        })?;
    let remove_tracking = purge || (!record.tracked_before && only_init_tracking(root));
    let remove_folder = remove_tracking && record.fresh;
    let mut changes = Vec::new();
    let mut notes = Vec::new();
    let read = |path: &Path| std::fs::read_to_string(root.join(path)).ok();
    for path in record.created.iter().map(PathBuf::from) {
        if remove_folder && path.starts_with(".timonier") {
            continue;
        }
        // The tracking stays: so does what keeps its database out of git.
        if !remove_tracking && path == Path::new(".timonier/.gitignore") {
            continue;
        }
        let Some(current) = read(&path) else { continue };
        // The whiteboard `init` wrote goes while still empty; drawn on, it is the tracking's to
        // keep or purge (Q54).
        if path == Path::new(super::plan::INTENT)
            && current != timonier_core::intent::Intent::empty().to_text()
        {
            continue;
        }
        match without(&path, &current) {
            Ok(after) => changes.push(Change {
                path,
                before: Some(current),
                after,
                executable: false,
            }),
            Err(why) => notes.push(format!("{} was left as it is: {why}.", path.display())),
        }
    }
    // `actual.json`, written by the git hook Timonier installed, goes with the setup.
    let actual = Path::new(".timonier/actual.json");
    if !remove_folder && let Some(current) = read(actual) {
        changes.push(Change {
            path: actual.to_path_buf(),
            before: Some(current),
            after: None,
            executable: false,
        });
    }
    let tracking_paths: Vec<PathBuf> = if remove_tracking && !remove_folder {
        TRACKING
            .iter()
            .filter(|p| purge || **p != "intent.json")
            .map(|p| Path::new(".timonier").join(p.trim_end_matches('/')))
            .filter(|p| root.join(p).exists())
            // The empty whiteboard `init` wrote already goes with the setup.
            .filter(|p| !changes.iter().any(|c: &Change| &c.path == p))
            .collect()
    } else {
        Vec::new()
    };
    for modified in &record.modified {
        let path = PathBuf::from(&modified.path);
        let Some(current) = read(&path) else { continue };
        let undone = if path == Path::new("package.json") {
            // The SDK's packages that were not there before `init` (ADR 0050).
            edits::npm_without_sdk(&current, &modified.original, &sdk_packages())
        } else {
            without(&path, &current)
        };
        let after = match undone {
            Ok(Some(rest)) if same(&path, &rest, &modified.original) => {
                Some(modified.original.clone())
            }
            Ok(rest) => rest,
            Err(why) => {
                notes.push(format!("{} was left as it is: {why}.", path.display()));
                continue;
            }
        };
        if after.as_deref() != Some(current.as_str()) {
            changes.push(Change {
                path,
                before: Some(current),
                after,
                executable: false,
            });
        }
    }
    let mut info = Vec::new();
    if !remove_tracking {
        info.push(
            if record.tracked_before {
                "The plan, the todo, the journal and the whiteboard in .timonier/ stay: they were \
             there before `timonier init`. `timonier uninit --purge` removes them too."
            } else {
                "The plan, the todo, the journal and the whiteboard in .timonier/ stay: work was \
             tracked there after `timonier init`. `timonier uninit --purge` removes them too."
            }
            .to_owned(),
        );
    }
    Ok(Undone {
        planned: Planned {
            changes,
            notes,
            info,
        },
        remove_folder,
        tracking_paths,
    })
}

/// Applies it: the files, then the folders left empty that held what went, then `.timonier/`.
pub fn apply(root: &Path, undone: &Undone) -> std::io::Result<()> {
    super::plan::apply(root, &undone.planned)?;
    for change in undone.planned.changes.iter().filter(|c| c.after.is_none()) {
        let mut folder = root.join(&change.path);
        while folder.pop() && folder.starts_with(root) && folder != root {
            if std::fs::remove_dir(&folder).is_err() {
                break;
            }
        }
    }
    for path in &undone.tracking_paths {
        let path = root.join(path);
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }
    if undone.remove_folder {
        std::fs::remove_dir_all(root.join(".timonier"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init::detect::Git;
    use crate::init::plan::{self, Choices};

    fn set_up(root: &Path) {
        let planned = plan::plan(
            root,
            &Choices {
                agents: Agent::ALL.to_vec(),
                git: Git::Repository {
                    hooks: root.join(".git/hooks"),
                    hooks_path: None,
                },
                runners: Vec::new(),
                path: Vec::new(),
            },
        );
        plan::apply(root, &planned).unwrap();
    }

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn each_file_loses_only_timonier_s_part() {
        let root = tempfile::tempdir().unwrap();
        set_up(root.path());
        let read = |p: &str| std::fs::read_to_string(root.path().join(p)).unwrap();
        for path in [
            "AGENTS.md",
            ".mcp.json",
            ".vscode/mcp.json",
            ".cursor/mcp.json",
            ".codex/config.toml",
            ".claude/settings.json",
            ".github/hooks/timonier.json",
            ".codex/hooks.json",
            ".git/hooks/pre-commit",
        ] {
            assert_eq!(without(Path::new(path), &read(path)), Ok(None), "{path}");
        }
        assert_eq!(without(Path::new(".timonier/tests.json"), "{}"), Ok(None));
    }

    #[test]
    fn the_tracking_goes_only_when_init_alone_wrote_it_or_when_purged() {
        let root = tempfile::tempdir().unwrap();
        set_up(root.path());
        // What init writes through the daemon.
        write(
            root.path(),
            ".timonier/plan.json",
            r#"{"milestones": [{"title": "Initialisation"}]}"#,
        );
        write(root.path(), ".timonier/todo.json", r#"{"items": []}"#);
        write(
            root.path(),
            ".timonier/sessions/S-2026-10-20-1.json",
            r#"{"agent": "timonier-init"}"#,
        );
        assert!(only_init_tracking(root.path()));
        assert!(plan(root.path(), false).unwrap().remove_folder);
        // An agent's session since: the tracking is the project's.
        write(
            root.path(),
            ".timonier/sessions/S-2026-10-20-2.json",
            r#"{"agent": "claude-code"}"#,
        );
        write(root.path(), ".timonier/actual.json", "{}");
        assert!(!only_init_tracking(root.path()));
        let kept = plan(root.path(), false).unwrap();
        assert!(!kept.remove_folder);
        assert!(kept.planned.info.iter().any(|n| n.contains("--purge")));
        // Timonier's own files of .timonier/ go; what keeps the database out of git stays.
        let gone: Vec<String> = kept
            .planned
            .changes
            .iter()
            .filter(|c| c.after.is_none())
            .map(|c| c.path.display().to_string())
            .collect();
        assert!(gone.contains(&".timonier/init.json".to_owned()), "{gone:?}");
        // What the git hook wrote goes with it (review n° 3 of Phase 6, m2).
        assert!(
            gone.contains(&".timonier/actual.json".to_owned()),
            "{gone:?}"
        );
        assert!(
            !gone.contains(&".timonier/.gitignore".to_owned()),
            "{gone:?}"
        );
        assert!(plan(root.path(), true).unwrap().remove_folder);
        // The whiteboard is the developer's too.
        std::fs::remove_file(root.path().join(".timonier/sessions/S-2026-10-20-2.json")).unwrap();
        write(root.path(), ".timonier/intent.json", "{}");
        assert!(!only_init_tracking(root.path()));
    }
}
