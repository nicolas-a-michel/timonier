//! The development process (ADR 0056): `.timonier/workflow.json`, its three presets, what the
//! agent's managed block says of it, the name of a task's branch and the description of its
//! pull request. Pure: the daemon runs `gh` and `git`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const FORMAT_VERSION: u32 = 1;

/// The presets of the spec, and `custom` once a setting is changed by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Commits straight on the main branch: a prototype, an exploration.
    SoloFast,
    /// A branch and a pull request per task, merged when the checks pass.
    SoloDisciplined,
    /// Ticket, branch, pull request, human review, merge; the CI blocks.
    Team,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TicketProvider {
    None,
    Github,
    /// Named by the spec; refused until its provider exists.
    Gitlab,
    Jira,
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum CommitConvention {
    Conventional,
    Free,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    Squash,
    Merge,
    Rebase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum CiProvider {
    None,
    GithubActions,
    /// Named by the spec; refused until its generator exists.
    GitlabCi,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Tickets {
    pub provider: TicketProvider,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Branches {
    /// A branch for each task, made by `task_start`.
    pub per_task: bool,
    /// `{ticket}` and `{slug}` are replaced; `{task}` is the task's number.
    #[schemars(length(min = 1, max = 100))]
    pub pattern: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Commits {
    pub convention: CommitConvention,
    /// The ticket's reference in each commit message.
    pub reference_ticket: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PullRequests {
    /// `session_end` opens the pull request of the task's branch.
    pub open_at_session_end: bool,
    pub draft: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Merge {
    pub human_approval: bool,
    /// GitHub merges once the checks pass (Q82).
    pub auto: bool,
    pub strategy: MergeStrategy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Ci {
    pub provider: CiProvider,
}

/// `.timonier/workflow.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    pub version: u32,
    pub preset: Preset,
    pub tickets: Tickets,
    pub branches: Branches,
    pub commits: Commits,
    pub pull_requests: PullRequests,
    pub merge: Merge,
    pub ci: Ci,
}

const PATTERN: &str = "tim/{ticket}-{slug}";

impl Workflow {
    /// A preset as the spec describes it (ADR 0056); `custom` is the default's settings.
    pub fn preset(preset: Preset) -> Self {
        let disciplined = matches!(preset, Preset::SoloDisciplined | Preset::Team);
        Self {
            version: FORMAT_VERSION,
            preset,
            tickets: Tickets {
                provider: if disciplined {
                    TicketProvider::Github
                } else {
                    TicketProvider::None
                },
            },
            branches: Branches {
                per_task: disciplined,
                pattern: PATTERN.to_owned(),
            },
            commits: Commits {
                convention: CommitConvention::Conventional,
                reference_ticket: disciplined,
            },
            pull_requests: PullRequests {
                open_at_session_end: disciplined,
                draft: false,
            },
            merge: Merge {
                human_approval: preset == Preset::Team,
                auto: preset == Preset::SoloDisciplined,
                strategy: MergeStrategy::Squash,
            },
            ci: Ci {
                provider: if disciplined {
                    CiProvider::GithubActions
                } else {
                    CiProvider::None
                },
            },
        }
    }

    /// The default for a new project (Q83).
    pub fn default_for_init() -> Self {
        Self::preset(Preset::SoloFast)
    }

    /// The preset these settings are, or `custom`.
    pub fn which_preset(&self) -> Preset {
        [Preset::SoloFast, Preset::SoloDisciplined, Preset::Team]
            .into_iter()
            .find(|p| {
                let mut same = Self::preset(*p);
                same.branches.pattern.clone_from(&self.branches.pattern);
                same == Self {
                    preset: *p,
                    ..self.clone()
                }
            })
            .unwrap_or(Preset::Custom)
    }

    /// What the tools cannot do yet, or what does not hold together: in words.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.version != FORMAT_VERSION {
            out.push(format!("version {} is not {FORMAT_VERSION}", self.version));
        }
        if matches!(
            self.tickets.provider,
            TicketProvider::Gitlab | TicketProvider::Jira | TicketProvider::Linear
        ) {
            out.push("only GitHub tickets work yet (gh)".to_owned());
        }
        if self.ci.provider == CiProvider::GitlabCi {
            out.push("only GitHub Actions is generated yet".to_owned());
        }
        if self.pull_requests.open_at_session_end && !self.branches.per_task {
            out.push("a pull request at the end of a session needs a branch per task".to_owned());
        }
        if self.merge.auto && self.merge.human_approval {
            out.push("auto-merge and a required human approval exclude each other".to_owned());
        }
        if !self.branches.pattern.contains("{slug}") && !self.branches.pattern.contains("{task}") {
            out.push("the branch pattern needs {slug} or {task}".to_owned());
        }
        out
    }

    /// What the agent's managed block says of the process, derived from these settings.
    pub fn instructions(&self) -> String {
        let mut lines = Vec::new();
        if self.branches.per_task {
            lines.push(
                "- Start each task with `task_start`: it opens its ticket and its branch; work and commit on that branch only."
                    .to_owned(),
            );
        } else {
            lines.push("- Commit on the main branch; no branch per task.".to_owned());
        }
        lines.push(match self.commits.convention {
            CommitConvention::Conventional => {
                if self.commits.reference_ticket {
                    "- Commit messages follow Conventional Commits and end with the task's ticket (`Refs #12`)."
                        .to_owned()
                } else {
                    "- Commit messages follow Conventional Commits.".to_owned()
                }
            }
            CommitConvention::Free => "- Commit messages are free.".to_owned(),
        });
        if self.pull_requests.open_at_session_end {
            lines.push(format!(
                "- `session_end` opens the task's pull request{}; {}.",
                if self.pull_requests.draft {
                    " as a draft"
                } else {
                    ""
                },
                if self.merge.auto {
                    "GitHub merges it once the checks pass"
                } else if self.merge.human_approval {
                    "a person reviews and merges it"
                } else {
                    "it is merged by hand"
                }
            ));
        }
        lines.join("\n")
    }
}

/// A task's branch: the pattern with its ticket number (else the task's), its number and a
/// slug of its title, in ASCII, at most 40 characters of slug.
pub fn branch_name(pattern: &str, task: &str, ticket: Option<u64>, title: &str) -> String {
    let number = task.trim_start_matches("T-");
    let mut slug = String::new();
    for c in title.to_lowercase().chars() {
        let c = match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
        if slug.len() >= 40 {
            break;
        }
    }
    let slug = slug.trim_end_matches('-');
    pattern
        .replace(
            "{ticket}",
            &ticket.map_or_else(|| number.to_owned(), |t| t.to_string()),
        )
        .replace("{task}", number)
        .replace("{slug}", if slug.is_empty() { "task" } else { slug })
}

/// What a pull request's description is made of (spec: the task, the structural diff, the
/// tests, the effect on performance, the violations).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PullRequestFacts {
    pub task: String,
    pub title: String,
    pub criterion: Option<String>,
    pub ticket: Option<u64>,
    /// The session's changes, one line each: components touched, dependencies.
    pub changes: Vec<String>,
    /// The last run of the tests, in a line; `None` without one.
    pub tests: Option<String>,
    /// Regressions and their state, one line each.
    pub performance: Vec<String>,
    /// Budgets exceeded and forbidden edges crossed, one line each.
    pub violations: Vec<String>,
}

/// The pull request's description, in Markdown.
pub fn pull_request_body(facts: &PullRequestFacts) -> String {
    let mut out = format!("{} — {}\n", facts.task, facts.title);
    if let Some(criterion) = &facts.criterion {
        out.push_str(&format!("\nDone when: {criterion}\n"));
    }
    let section = |out: &mut String, heading: &str, lines: &[String], empty: &str| {
        out.push_str(&format!("\n## {heading}\n\n"));
        if lines.is_empty() {
            out.push_str(&format!("{empty}\n"));
        } else {
            for line in lines {
                out.push_str(&format!("- {line}\n"));
            }
        }
    };
    section(
        &mut out,
        "Changes",
        &facts.changes,
        "No change of the architecture.",
    );
    out.push_str("\n## Tests\n\n");
    out.push_str(&format!(
        "{}\n",
        facts
            .tests
            .as_deref()
            .unwrap_or("No run of `timonier test` yet.")
    ));
    section(
        &mut out,
        "Performance",
        &facts.performance,
        "No regression found.",
    );
    section(&mut out, "Violations", &facts.violations, "None.");
    if let Some(ticket) = facts.ticket {
        out.push_str(&format!("\nCloses #{ticket}\n"));
    }
    out.push_str("\n<sub>Written by Timonier.</sub>\n");
    out
}

/// `workflow/get`, and what the settings panel shows (ADR 0056).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct WorkflowView {
    pub workflow: Workflow,
    /// What the process asks of the agent, in the managed block's words.
    pub instructions: String,
    /// What does not hold together, or what the tools cannot do yet.
    pub problems: Vec<String>,
}

/// `task_start`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskStart {
    #[schemars(regex(pattern = r"^T-[1-9][0-9]*$"))]
    pub task_id: String,
}

/// `open_pull_request`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenPullRequest {
    /// A draft, whatever the process says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<bool>,
}

/// `link_ticket`: a ticket that already exists, `github#12` or `12`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkTicket {
    #[schemars(regex(pattern = r"^T-[1-9][0-9]*$"))]
    pub task_id: String,
    #[schemars(regex(pattern = r"^(github#)?[1-9][0-9]{0,9}$"))]
    pub ticket: String,
}

/// The number of a ticket as `link_ticket` takes it, or as a task keeps it.
pub fn ticket_number(ticket: &str) -> Option<u64> {
    ticket
        .strip_prefix("github#")
        .unwrap_or(ticket)
        .parse()
        .ok()
        .filter(|n| *n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_are_those_of_the_spec() {
        let fast = Workflow::preset(Preset::SoloFast);
        assert!(!fast.branches.per_task && !fast.pull_requests.open_at_session_end);
        assert_eq!(fast.tickets.provider, TicketProvider::None);
        let disciplined = Workflow::preset(Preset::SoloDisciplined);
        assert!(disciplined.branches.per_task && disciplined.merge.auto);
        assert!(!disciplined.merge.human_approval);
        assert_eq!(disciplined.ci.provider, CiProvider::GithubActions);
        let team = Workflow::preset(Preset::Team);
        assert!(team.merge.human_approval && !team.merge.auto);
        for preset in [Preset::SoloFast, Preset::SoloDisciplined, Preset::Team] {
            let workflow = Workflow::preset(preset);
            assert!(
                workflow.problems().is_empty(),
                "{preset:?}: {:?}",
                workflow.problems()
            );
            assert_eq!(workflow.which_preset(), preset);
        }
        assert_eq!(Workflow::default_for_init().preset, Preset::SoloFast);
        // One setting changed: custom.
        let mut changed = Workflow::preset(Preset::SoloDisciplined);
        changed.pull_requests.draft = true;
        assert_eq!(changed.which_preset(), Preset::Custom);
    }

    #[test]
    fn settings_that_do_not_hold_together_are_said() {
        let mut odd = Workflow::preset(Preset::SoloDisciplined);
        odd.branches.per_task = false;
        odd.merge.human_approval = true;
        odd.tickets.provider = TicketProvider::Jira;
        let problems = odd.problems();
        assert_eq!(problems.len(), 3, "{problems:?}");
    }

    #[test]
    fn the_managed_block_follows_the_process() {
        assert!(
            Workflow::preset(Preset::SoloFast)
                .instructions()
                .contains("Commit on the main branch")
        );
        let disciplined = Workflow::preset(Preset::SoloDisciplined).instructions();
        assert!(disciplined.contains("`task_start`"), "{disciplined}");
        assert!(disciplined.contains("GitHub merges it once the checks pass"));
        assert!(
            Workflow::preset(Preset::Team)
                .instructions()
                .contains("a person reviews and merges it")
        );
    }

    #[test]
    fn a_branch_is_named_by_its_ticket_and_title() {
        assert_eq!(
            branch_name(PATTERN, "T-7", Some(42), "Écrire le cache d'extraction !"),
            "tim/42-ecrire-le-cache-d-extraction"
        );
        assert_eq!(branch_name(PATTERN, "T-7", None, "???"), "tim/7-task");
        assert_eq!(
            branch_name("feature/{task}", "T-12", Some(3), "x"),
            "feature/12"
        );
        assert_eq!(ticket_number("github#12"), Some(12));
        assert_eq!(ticket_number("0"), None);
    }

    #[test]
    fn a_pull_request_says_the_task_its_changes_tests_and_violations() {
        let body = pull_request_body(&PullRequestFacts {
            task: "T-7".into(),
            title: "Extraction cache".into(),
            criterion: Some("a second indexing parses nothing".into()),
            ticket: Some(42),
            changes: vec!["timonier-store modified".into()],
            tests: Some("R-3: 544 tests, 0 failed, 11.3 s".into()),
            performance: vec![],
            violations: vec!["budget N-1 exceeded on Daemon: p95 12 ms > 5 ms".into()],
        });
        assert!(body.starts_with("T-7 — Extraction cache\n"));
        assert!(body.contains("Done when: a second indexing parses nothing"));
        assert!(body.contains("- timonier-store modified"));
        assert!(body.contains("R-3: 544 tests"));
        assert!(body.contains("No regression found."));
        assert!(body.contains("- budget N-1 exceeded"));
        assert!(body.contains("Closes #42"));
    }
}
