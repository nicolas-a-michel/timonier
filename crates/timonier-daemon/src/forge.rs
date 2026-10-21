//! GitHub through `gh`, and the branches through `git` (ADR 0056, Q81): the daemon runs them in
//! the repository and keeps no token; `gh` uses its own login. The program is settable
//! (`Options::gh`), for the tests' fake one.

use std::path::{Path, PathBuf};
use std::process::Command;

use timonier_core::workflow::MergeStrategy;

/// What a forge command said when it failed, for the agent to act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeError(pub String);

impl std::fmt::Display for ForgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub type Result<T> = std::result::Result<T, ForgeError>;

#[derive(Debug, Clone)]
pub struct Forge {
    root: PathBuf,
    gh: PathBuf,
}

impl Forge {
    pub fn new(root: &Path, gh: Option<&Path>) -> Self {
        Self {
            root: root.to_path_buf(),
            gh: gh.map_or_else(|| PathBuf::from("gh"), Path::to_path_buf),
        }
    }

    fn run(&self, program: &Path, args: &[&str]) -> Result<String> {
        let output = Command::new(program)
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|error| {
                ForgeError(if program == self.gh {
                    format!(
                        "cannot run `gh` ({error}): install the GitHub CLI and run `gh auth login`"
                    )
                } else {
                    format!("cannot run `{}`: {error}", program.display())
                })
            })?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
        }
        let said = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let login = said.contains("gh auth login") || said.contains("not logged");
        Err(ForgeError(if program == self.gh && login {
            "`gh` is not logged in: run `gh auth login`".to_owned()
        } else {
            format!(
                "`{} {}` failed: {said}",
                program
                    .file_name()
                    .map_or_else(|| "?".into(), |n| n.to_string_lossy()),
                args.first().copied().unwrap_or_default()
            )
        }))
    }

    fn gh(&self, args: &[&str]) -> Result<String> {
        self.run(&self.gh, args)
    }

    fn git(&self, args: &[&str]) -> Result<String> {
        self.run(Path::new("git"), args)
    }

    /// A new issue; its number, read in the URL `gh` prints.
    pub fn create_issue(&self, title: &str, body: &str) -> Result<u64> {
        let url = self.gh(&["issue", "create", "--title", title, "--body", body])?;
        url.lines()
            .last()
            .and_then(|line| line.rsplit('/').next())
            .and_then(|n| n.trim().parse().ok())
            .ok_or_else(|| ForgeError(format!("`gh issue create` gave no issue: {url}")))
    }

    pub fn close_issue(&self, number: u64) -> Result<()> {
        self.gh(&["issue", "close", &number.to_string()]).map(drop)
    }

    /// The branch checked out, `None` when the head is detached.
    pub fn current_branch(&self) -> Option<String> {
        self.git(&["symbolic-ref", "--quiet", "--short", "HEAD"])
            .ok()
    }

    /// Takes the task's branch: made from `base` the first time, checked out after.
    pub fn take_branch(&self, name: &str, base: &str) -> Result<()> {
        if self
            .git(&[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/heads/{name}"),
            ])
            .is_ok()
        {
            return self.git(&["switch", name]).map(drop);
        }
        self.git(&["switch", "-c", name, base]).map(drop)
    }

    /// Pushes the branch, then opens its pull request against `base`; its URL.
    pub fn open_pull_request(
        &self,
        branch: &str,
        base: &str,
        title: &str,
        body: &str,
        draft: bool,
    ) -> Result<String> {
        self.git(&["push", "--set-upstream", "origin", branch])?;
        let mut args = vec![
            "pr", "create", "--head", branch, "--base", base, "--title", title, "--body", body,
        ];
        if draft {
            args.push("--draft");
        }
        self.gh(&args)
            .map(|out| out.lines().last().unwrap_or_default().to_owned())
    }

    /// GitHub merges the branch's pull request once its checks pass (Q82).
    pub fn auto_merge(&self, branch: &str, strategy: MergeStrategy) -> Result<()> {
        let how = match strategy {
            MergeStrategy::Squash => "--squash",
            MergeStrategy::Merge => "--merge",
            MergeStrategy::Rebase => "--rebase",
        };
        self.gh(&["pr", "merge", branch, "--auto", how]).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `gh` that records its arguments and answers as GitHub would.
    fn fake(dir: &Path, fails: bool) -> PathBuf {
        let program = dir.join("gh");
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> \"{}/calls\"\n{}case \"$1 $2\" in\n  \"issue create\") echo https://github.com/me/demo/issues/42 ;;\n  \"pr create\") echo https://github.com/me/demo/pull/7 ;;\nesac\n",
            dir.display(),
            if fails {
                "echo 'To get started with GitHub CLI, please run:  gh auth login' >&2; exit 4\n"
            } else {
                ""
            }
        );
        std::fs::write(&program, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        program
    }

    #[test]
    fn an_issue_is_made_and_closed_through_gh() {
        let dir = tempfile::tempdir().unwrap();
        let forge = Forge::new(dir.path(), Some(&fake(dir.path(), false)));
        assert_eq!(forge.create_issue("T-7 — Cache", "Done when: …"), Ok(42));
        forge.close_issue(42).unwrap();
        forge
            .auto_merge("tim/42-cache", MergeStrategy::Squash)
            .unwrap();
        let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
        assert_eq!(
            calls,
            "issue create --title T-7 — Cache --body Done when: …\nissue close 42\npr merge tim/42-cache --auto --squash\n"
        );
    }

    #[test]
    fn gh_not_logged_in_says_what_to_do() {
        let dir = tempfile::tempdir().unwrap();
        let forge = Forge::new(dir.path(), Some(&fake(dir.path(), true)));
        assert_eq!(
            forge.create_issue("x", "y"),
            Err(ForgeError(
                "`gh` is not logged in: run `gh auth login`".into()
            ))
        );
        let missing = Forge::new(dir.path(), Some(Path::new("/nowhere/gh")));
        assert!(
            missing
                .close_issue(1)
                .unwrap_err()
                .0
                .contains("install the GitHub CLI")
        );
    }
}
