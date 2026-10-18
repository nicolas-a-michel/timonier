//! The few git questions the consolidation asks (ADR 0015), through git's command line. A
//! failure of git is `None`, never mistaken for an answer.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn git(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn text(bytes: Vec<u8>) -> Option<String> {
    let text = String::from_utf8(bytes).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// NUL-separated paths.
fn paths(bytes: &[u8]) -> impl Iterator<Item = &str> {
    bytes
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .filter_map(|p| std::str::from_utf8(p).ok())
}

/// The commit HEAD points at; `None` outside a repository or before the first commit.
pub(crate) fn head(root: &Path) -> Option<String> {
    git(root, &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]).and_then(text)
}

/// Where git keeps what the watcher follows, as absolute paths (ADR 0015).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Places {
    /// The git directory of this working tree: HEAD and its log move there.
    pub(crate) dir: PathBuf,
    /// The git directory worktrees share, the same one outside a worktree: its
    /// `info/exclude` says what the walk skips.
    pub(crate) common: PathBuf,
    /// The top of the working tree: its folders down to the root hold ignore files too.
    pub(crate) top: PathBuf,
}

/// Where git keeps what the watcher follows; `None` outside a repository. Each answer is asked
/// on its own: a git that cannot give the shared directory or the top gives the git directory
/// and the root in their place.
pub(crate) fn places(root: &Path) -> Option<Places> {
    let dir = PathBuf::from(git(root, &["rev-parse", "--absolute-git-dir"]).and_then(text)?);
    // Relative to the root, or absolute.
    let common = git(root, &["rev-parse", "--git-common-dir"])
        .and_then(text)
        .and_then(|common| std::fs::canonicalize(root.join(common)).ok())
        .unwrap_or_else(|| dir.clone());
    let top = git(root, &["rev-parse", "--show-toplevel"])
        .and_then(text)
        .map_or_else(|| root.to_path_buf(), PathBuf::from);
    Some(Places { dir, common, top })
}

/// Paths, relative to the root, whose working-tree content differs from HEAD: modified,
/// staged, deleted or untracked. Ignored files are not listed. `None` if git fails.
pub(crate) fn dirty(root: &Path) -> Option<Vec<String>> {
    let prefix = String::from_utf8(git(root, &["rev-parse", "--show-prefix"])?).ok()?;
    let prefix = prefix.trim();
    let status = git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--no-renames",
            ".",
        ],
    )?;
    Some(
        status
            .split(|b| *b == 0)
            .filter(|entry| entry.len() > 3)
            .filter_map(|entry| std::str::from_utf8(&entry[3..]).ok())
            // Paths are relative to the top of the repository; the root may be a subdirectory.
            .filter_map(|path| path.strip_prefix(prefix).map(str::to_owned))
            .collect(),
    )
}

/// Paths, relative to the root, that differ between two commits. `None` if git fails.
pub(crate) fn changed_between(root: &Path, from: &str, to: &str) -> Option<Vec<String>> {
    let listed = git(
        root,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "--relative",
            "-z",
            from,
            to,
            "--",
        ],
    )?;
    Some(paths(&listed).map(str::to_owned).collect())
}

/// The content of each path in a commit, `None` for a path the commit does not have. `None`
/// as a whole if git fails: an unknown content must not pass for an absent file.
pub(crate) fn committed(
    root: &Path,
    commit: &str,
    wanted: &[String],
) -> Option<BTreeMap<String, Option<Vec<u8>>>> {
    if wanted.is_empty() {
        return Some(BTreeMap::new());
    }
    let mut args = vec!["ls-tree", "-r", "-z", "--name-only", commit, "--"];
    args.extend(wanted.iter().map(String::as_str));
    let listed = git(root, &args)?;
    let present: BTreeSet<&str> = paths(&listed).collect();
    let mut contents = BTreeMap::new();
    for path in wanted {
        let content = if present.contains(path.as_str()) {
            Some(git(root, &["show", &format!("{commit}:./{path}")])?)
        } else {
            None
        };
        contents.insert(path.clone(), content);
    }
    Some(contents)
}

/// Which paths `.gitignore` excludes, whether they exist or not. `None` if git fails.
pub(crate) fn ignored(root: &Path, candidates: &[String]) -> Option<BTreeSet<String>> {
    if candidates.is_empty() {
        return Some(BTreeSet::new());
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["check-ignore", "--no-index", "-z", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    {
        let mut stdin = child.stdin.take()?;
        for path in candidates {
            stdin.write_all(path.as_bytes()).ok()?;
            stdin.write_all(&[0]).ok()?;
        }
    }
    let output = child.wait_with_output().ok()?;
    // 0: some are ignored; 1: none is; anything else is a failure.
    match output.status.code() {
        Some(0 | 1) => Some(paths(&output.stdout).map(str::to_owned).collect()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args([
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            status.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn a_repository_without_commit_has_no_head() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(head(dir.path()), None);
        run(dir.path(), &["init", "--quiet"]);
        assert_eq!(head(dir.path()), None);
        assert!(places(dir.path()).is_some());
    }

    #[test]
    fn paths_are_relative_to_a_root_in_a_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path();
        write(top, "app/src/a.rs", "fn a() {}\n");
        write(top, "app/src/b.rs", "fn b() {}\n");
        write(top, "other/c.rs", "fn c() {}\n");
        write(top, ".gitignore", "*.gen.rs\n");
        run(top, &["init", "--quiet"]);
        run(top, &["add", "."]);
        run(top, &["commit", "--quiet", "-m", "one"]);
        let root = top.join("app");
        let sha = head(&root).unwrap();
        write(top, "app/src/a.rs", "fn a() { 1; }\n");
        write(top, "app/src/new.rs", "fn n() {}\n");
        write(top, "other/c.rs", "fn c() { 2; }\n");
        std::fs::remove_file(top.join("app/src/b.rs")).unwrap();
        let mut changed = dirty(&root).unwrap();
        changed.sort();
        assert_eq!(changed, ["src/a.rs", "src/b.rs", "src/new.rs"]);
        let contents = committed(&root, &sha, &changed).unwrap();
        assert_eq!(contents["src/a.rs"].as_deref(), Some(&b"fn a() {}\n"[..]));
        assert_eq!(contents["src/b.rs"].as_deref(), Some(&b"fn b() {}\n"[..]));
        assert_eq!(contents["src/new.rs"], None);
        let ignored = ignored(&root, &["src/x.gen.rs".into(), "src/a.rs".into()]).unwrap();
        assert_eq!(ignored, BTreeSet::from(["src/x.gen.rs".to_owned()]));
        // An unknown commit is a failure, not an empty tree.
        assert!(committed(&root, "0000000000000000000000000000000000000000", &changed).is_none());
    }

    #[test]
    fn a_worktree_has_its_git_directory_elsewhere() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main");
        std::fs::create_dir_all(&main).unwrap();
        write(&main, "a.rs", "fn a() {}\n");
        run(&main, &["init", "--quiet"]);
        run(&main, &["add", "."]);
        run(&main, &["commit", "--quiet", "-m", "one"]);
        let worktree = dir.path().join("wt");
        run(
            &main,
            &["worktree", "add", "--quiet", worktree.to_str().unwrap()],
        );
        let places = places(&worktree).unwrap();
        assert!(!places.dir.starts_with(&worktree), "{places:?}");
        // `info/exclude` is the main repository's (ADR 0023).
        let real = |p: &Path| p.canonicalize().unwrap();
        assert_eq!(real(&places.common), real(&main).join(".git"));
        assert_eq!(real(&places.top), real(&worktree));
        assert_eq!(head(&worktree), head(&main));
    }

    #[test]
    fn a_root_in_a_subdirectory_has_its_top_above() {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(top.join("app")).unwrap();
        run(&top, &["init", "--quiet"]);
        let places = places(&top.join("app")).unwrap();
        let real = |p: &Path| p.canonicalize().unwrap();
        assert_eq!(real(&places.dir), top.join(".git"));
        assert_eq!(real(&places.common), top.join(".git"));
        assert_eq!(real(&places.top), top);
    }
}
