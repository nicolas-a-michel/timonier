//! The development process (ADR 0056): `.timonier/workflow.json`, and the tools that apply it.
//! `task_start` opens the task's ticket and branch, `open_pull_request` (and `session_end`)
//! pushes the branch and opens its pull request, GitHub merging it once the checks pass (Q82).

use serde_json::{Value, json};
use timonier_core::tracking::{Outcome, Refusal, Session, Task, TaskStatus};
use timonier_core::workflow::{
    LinkTicket, PullRequestFacts, TaskStart, TicketProvider, Workflow, branch_name,
    pull_request_body, ticket_number,
};

use crate::tracking::{Context, Tracker, internal, invalid_file, read};

const FILE: &str = "workflow.json";

/// The first line of the CI workflow Timonier writes: one without it is the developer's.
const CI_MARK: &str = "# Written by Timonier from .timonier/workflow.json (ADR 0056).";

fn forge_refusal(error: crate::forge::ForgeError) -> Refusal {
    Refusal::new("forge_failed", error.0)
}

impl Tracker {
    /// The process; Solo rapide (Q83) when the project has not chosen one.
    pub(crate) fn workflow(&self) -> Outcome<Workflow> {
        let path = self.dir.join(FILE);
        if !path.exists() {
            return Ok(Workflow::default_for_init());
        }
        let workflow: Workflow = read(&path)?;
        if workflow.version != timonier_core::workflow::FORMAT_VERSION {
            return Err(invalid_file(&path, "its version is not 1"));
        }
        Ok(workflow)
    }

    /// `workflow/set`: the panel's settings, checked; the preset follows them.
    pub fn set_workflow(&self, mut workflow: Workflow) -> Outcome<Workflow> {
        let problems = workflow.problems();
        if !problems.is_empty() {
            return Err(Refusal::new("invalid_argument", problems.join("; ")));
        }
        workflow.preset = workflow.which_preset();
        self.write(FILE, &workflow)?;
        if workflow.ci.provider == timonier_core::workflow::CiProvider::GithubActions {
            self.write_ci()?;
        }
        Ok(workflow)
    }

    /// `.github/workflows/timonier.yml`: the tests measured, then the budgets and the rules
    /// (`timonier check`), on each pull request and on the main branch. Written when absent or
    /// when it is still Timonier's; one the developer changed is left as it is.
    fn write_ci(&self) -> Outcome<()> {
        let root = self.root();
        let path = root.join(".github").join("workflows").join("timonier.yml");
        if let Ok(current) = std::fs::read_to_string(&path)
            && !current.starts_with(CI_MARK)
        {
            return Ok(());
        }
        let mut steps = String::new();
        if root.join("package-lock.json").is_file() {
            steps.push_str(
                "      - uses: actions/setup-node@v7
        with:
          node-version: 24
      - run: npm ci
",
            );
        }
        let text = format!(
            "{CI_MARK}
# It replays the tests, the whiteboard's budgets and its architecture rules: with GitHub's
# auto-merge, make its job a required check of the main branch.
name: Timonier
on:
  pull_request:
  push:
    branches: [{main}]
permissions:
  contents: read
jobs:
  timonier:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
{steps}      - name: Install Timonier
        run: cargo install --locked --git https://github.com/nicolas-a-michel/timonier timonier-cli
      - name: The tests, measured
        run: timonier test --approve
      - name: The budgets and the architecture rules
        run: timonier check
",
            main = crate::git::main_branch(root),
        );
        let folder = path.parent().unwrap_or(root);
        std::fs::create_dir_all(folder).map_err(internal)?;
        std::fs::write(&path, text).map_err(internal)
    }

    /// `workflow/get`: the process, what it asks of the agent, and what does not hold together.
    pub fn workflow_view(&self) -> Outcome<Value> {
        let workflow = self.workflow()?;
        Ok(json!({
            "workflow": workflow,
            "instructions": workflow.instructions(),
            "problems": workflow.problems(),
        }))
    }

    pub(crate) fn get_workflow(&self) -> Outcome<(String, Value)> {
        let workflow = self.workflow()?;
        let problems = workflow.problems();
        let summary = format!(
            "The process is {}. {}",
            serde_json::to_value(workflow.preset)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            workflow
                .instructions()
                .replace("\n- ", " ")
                .trim_start_matches("- ")
        );
        Ok((
            summary,
            json!({ "workflow": workflow, "instructions": workflow.instructions(), "problems": problems }),
        ))
    }

    /// `task_start`: the task in progress, its ticket made and its branch taken when the
    /// process asks for them.
    pub(crate) fn task_start(
        &self,
        input: &TaskStart,
        context: &Context,
    ) -> Outcome<(String, Value)> {
        let workflow = self.workflow()?;
        let (session, stamp) = self.writing(context)?;
        let mut plan = self.plan()?;
        let task = plan.task(&input.task_id).cloned().ok_or_else(|| {
            Refusal::new(
                "unknown_task",
                format!("no task {} in the plan", input.task_id),
            )
        })?;
        if matches!(task.status, TaskStatus::Done | TaskStatus::Dropped) {
            return Err(Refusal::new(
                "invalid_transition",
                format!(
                    "{} is {}: reopen it with plan_update_task first",
                    task.id,
                    task.status.as_str()
                ),
            ));
        }
        let mut said = Vec::new();
        if task.status != TaskStatus::InProgress {
            let update = timonier_core::tracking::PlanUpdateTask {
                task_id: task.id.clone(),
                status: Some(TaskStatus::InProgress),
                title: None,
                criterion: None,
                note: None,
            };
            plan.update_task(&update, &stamp)?;
            said.push(format!("{} is in progress", task.id));
        }
        let mut ticket = task.ticket.as_deref().and_then(ticket_number);
        if workflow.tickets.provider == TicketProvider::Github && ticket.is_none() {
            let body = format!(
                "{}\n\nTask {} of the plan, kept by Timonier.",
                task.criterion
                    .as_deref()
                    .map_or_else(String::new, |c| format!("Done when: {c}")),
                task.id
            );
            let number = context
                .forge
                .create_issue(&format!("{} — {}", task.id, task.title), body.trim())
                .map_err(forge_refusal)?;
            plan.set_ticket(&task.id, &format!("github#{number}"));
            said.push(format!("issue #{number} opened"));
            ticket = Some(number);
        }
        self.write("plan.json", &plan)?;
        let mut branch = None;
        if workflow.branches.per_task {
            let name = branch_name(&workflow.branches.pattern, &task.id, ticket, &task.title);
            let base = crate::git::main_branch(self.root());
            context
                .forge
                .take_branch(&name, &base)
                .map_err(forge_refusal)?;
            said.push(format!("on branch {name}"));
            branch = Some(name);
        }
        self.remember_task(session, &task.id)?;
        let summary = if said.is_empty() {
            format!("{} goes on as it is.", task.id)
        } else {
            format!("{}.", said.join(", "))
        };
        Ok((
            summary,
            json!({ "task": plan.task(&task.id), "branch": branch, "ticket": ticket }),
        ))
    }

    /// `link_ticket`: an issue that already exists, for the task.
    pub(crate) fn link_ticket(
        &self,
        input: &LinkTicket,
        context: &Context,
    ) -> Outcome<(String, Value)> {
        let (session, _) = self.writing(context)?;
        let number = ticket_number(&input.ticket)
            .ok_or_else(|| Refusal::new("invalid_argument", "a ticket is `github#12` or `12`"))?;
        let mut plan = self.plan()?;
        let task = plan
            .set_ticket(&input.task_id, &format!("github#{number}"))
            .ok_or_else(|| {
                Refusal::new(
                    "unknown_task",
                    format!("no task {} in the plan", input.task_id),
                )
            })?;
        self.write("plan.json", &plan)?;
        self.remember_task(session, &task.id)?;
        Ok((
            format!("{} is linked to issue #{number}.", task.id),
            crate::tracking::json(&task),
        ))
    }

    /// The pull request of the branch checked out, for the session's task whose branch it is.
    pub(crate) fn pull_request(
        &self,
        session: &Session,
        draft: Option<bool>,
        context: &Context,
    ) -> Outcome<(String, Value)> {
        let workflow = self.workflow()?;
        let branch = context.forge.current_branch().ok_or_else(|| {
            Refusal::new(
                "no_branch",
                "the head is detached: no branch to open a pull request for",
            )
        })?;
        let base = crate::git::main_branch(self.root());
        if branch == base {
            return Err(Refusal::new(
                "no_branch",
                format!("on `{base}`, the main branch: start the task with task_start first"),
            ));
        }
        let plan = self.plan()?;
        let task: Task = session
            .tasks
            .iter()
            .rev()
            .filter_map(|id| plan.task(id))
            .find(|t| {
                branch_name(
                    &workflow.branches.pattern,
                    &t.id,
                    t.ticket.as_deref().and_then(ticket_number),
                    &t.title,
                ) == branch
            })
            .cloned()
            .ok_or_else(|| {
                Refusal::new(
                    "no_task",
                    format!("the branch `{branch}` is no task's of this session: start one with task_start"),
                )
            })?;
        let frozen = (context.freeze)().map_err(internal)?;
        let changes = (context.changes)(session.snapshots.start, frozen.seq).ok();
        let mut lines = Vec::new();
        if let Some(changes) = &changes {
            for (what, boxes) in [
                ("new", &changes.added),
                ("modified", &changes.modified),
                ("removed", &changes.removed),
            ] {
                for b in boxes {
                    lines.push(format!("{} {what}", b.name));
                }
            }
            for link in &changes.linked {
                lines.push(format!(
                    "{} → {}: dependency added",
                    link.from.name, link.to.name
                ));
            }
            for link in &changes.unlinked {
                lines.push(format!(
                    "{} → {}: dependency removed",
                    link.from.name, link.to.name
                ));
            }
        }
        let mut violations: Vec<String> = (context.budgets)()
            .ok()
            .flatten()
            .map(|v| {
                v.checks
                    .iter()
                    .filter(|c| c.state == timonier_core::budgets::BudgetState::Exceeded)
                    .map(crate::tracking::budget_words)
                    .collect()
            })
            .unwrap_or_default();
        if let (Ok(Some(intent)), Ok(actual)) = (self.intent(), (context.actual)()) {
            let drift = timonier_core::drift::drift(&intent, &actual);
            violations.extend(
                drift
                    .forbidden
                    .iter()
                    .map(|f| format!("forbidden edge {} crossed", f.edge)),
            );
        }
        let performance: Vec<String> = (context.regressions)(None)
            .map(|report| {
                report
                    .regressions
                    .iter()
                    .map(|r| format!("{} {:+.1} %", r.label, r.verdict.change * 100.0))
                    .collect()
            })
            .unwrap_or_default();
        let facts = PullRequestFacts {
            task: task.id.clone(),
            title: task.title.clone(),
            criterion: task.criterion.clone(),
            ticket: task.ticket.as_deref().and_then(ticket_number),
            changes: lines,
            tests: (context.last_run)(),
            performance,
            violations,
        };
        let draft = draft.unwrap_or(workflow.pull_requests.draft);
        let url = context
            .forge
            .open_pull_request(
                &branch,
                &base,
                &format!("{} — {}", task.id, task.title),
                &pull_request_body(&facts),
                draft,
            )
            .map_err(forge_refusal)?;
        let mut summary = format!("Pull request opened: {url}");
        if workflow.merge.auto {
            match context.forge.auto_merge(&branch, workflow.merge.strategy) {
                Ok(()) => summary.push_str("; GitHub merges it once the checks pass"),
                Err(error) => summary.push_str(&format!("; auto-merge not set: {error}")),
            }
        }
        summary.push('.');
        Ok((
            summary,
            json!({ "url": url, "branch": branch, "task": task.id }),
        ))
    }

    /// A ticket closed with its task (ADR 0056): what to say of it, if anything.
    pub(crate) fn close_ticket(&self, task: &Task, context: &Context) -> Option<String> {
        if !matches!(task.status, TaskStatus::Done | TaskStatus::Dropped) {
            return None;
        }
        let number = task.ticket.as_deref().and_then(ticket_number)?;
        if self.workflow().ok()?.tickets.provider != TicketProvider::Github {
            return None;
        }
        Some(match context.forge.close_issue(number) {
            Ok(()) => format!(" Issue #{number} closed."),
            Err(error) => format!(" Issue #{number} not closed: {error}."),
        })
    }
}
