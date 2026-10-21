//! The project's tracking (ADR 0029): the plan, the todo and the sessions, the formats of their
//! files in `.timonier/`, and the operations the MCP tools apply to them (ADR 0027). Pure: the
//! daemon reads and writes the files, and gives the time and the session.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The version of the files' formats.
pub const FORMAT_VERSION: u32 = 1;

/// The bounds of what an agent writes.
pub const MAX_TITLE: usize = 200;
pub const MAX_TEXT: usize = 2000;

/// `.timonier/plan.json`: milestones, each with its tasks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub version: u32,
    pub milestones: Vec<Milestone>,
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            milestones: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Milestone {
    /// `M-<n>`, given by the daemon, never reused.
    pub id: String,
    pub title: String,
    pub tasks: Vec<Task>,
}

impl Milestone {
    /// Its progress, computed (ADR 0029): tasks done, over tasks not dropped.
    pub fn progress(&self) -> (u32, u32) {
        let count = |keep: &dyn Fn(TaskStatus) -> bool| {
            u32::try_from(self.tasks.iter().filter(|t| keep(t.status)).count()).unwrap_or(u32::MAX)
        };
        (
            count(&|s| s == TaskStatus::Done),
            count(&|s| s != TaskStatus::Dropped),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// `T-<n>`, given by the daemon, never reused.
    pub id: String,
    pub title: String,
    /// What says the task is done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criterion: Option<String>,
    pub status: TaskStatus,
    /// Components of the whiteboard (`C-…`), from Phase 7 on (H7).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<String>,
    /// The sessions that worked on it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<String>,
    pub created: Stamp,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
    /// Its ticket at the forge, `github#12` (ADR 0056): made by `task_start`, or given by
    /// `link_ticket`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticket: Option<String>,
}

/// The states of a task (ADR 0029).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    Blocked,
    Done,
    Dropped,
}

impl TaskStatus {
    /// Whether a task may go from this state to `to`.
    pub fn may_become(self, to: TaskStatus) -> bool {
        use TaskStatus::{Blocked, Done, Dropped, InProgress, Todo};
        matches!(
            (self, to),
            (Todo, InProgress | Dropped)
                | (InProgress, Blocked | Done | Dropped)
                | (Blocked, InProgress | Dropped)
                | (Done, InProgress)
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Todo => "todo",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Blocked => "blocked",
            TaskStatus::Done => "done",
            TaskStatus::Dropped => "dropped",
        }
    }
}

/// When, and in which session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stamp {
    /// UTC, RFC 3339 (`2026-10-20T08:15:00Z`).
    pub at: String,
    /// The session that wrote it.
    pub session: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub at: String,
    pub session: String,
    pub text: String,
}

/// `.timonier/todo.json`: what the session in progress has to do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Todo {
    pub version: u32,
    /// The session the items belong to; none between sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    pub items: Vec<TodoItem>,
}

impl Default for Todo {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            session: None,
            items: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TodoItem {
    /// `D-<n>`, given by the daemon within the session.
    pub id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    pub status: TodoStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Open,
    Done,
}

/// `.timonier/sessions/<id>.json`: one session of an agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub version: u32,
    /// `S-<date>-<n>` (`S-2026-10-20-1`).
    pub id: String,
    /// The MCP client that opened it (`claude-code`…).
    pub agent: String,
    pub objective: String,
    /// The plan's tasks it worked on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<String>,
    pub status: SessionStatus,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// The snapshots frozen at its start and end (ADR 0029).
    pub snapshots: SessionSnapshots,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Its todo, kept at its end.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub todo: Vec<TodoItem>,
    /// What it changed in the code, written at its end (ADR 0031).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<SessionChanges>,
    /// The whiteboard's budgets the last runs exceeded when it ended, in words (ADR 0052).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub budgets: Vec<String>,
    /// The agent's own sessions that worked in it (Claude Code's `session.id`), as its stop hook
    /// says them (ADR 0058).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agent_sessions: Vec<String>,
    /// The tokens its agent spent while it was open; absent: not measured (ADR 0058).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<crate::usage::TokenUsage>,
}

/// What a session changed in the code, between its snapshots, as the review diff tells it
/// (ADR 0024, 0031). The boxes added, modified or removed are the components it touched. One
/// word per field: the journal and the protocol spell them the same.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SessionChanges {
    /// Files added, removed or changed.
    pub files: u32,
    pub added: Vec<ChangedBox>,
    pub modified: Vec<ChangedBox>,
    pub removed: Vec<ChangedBox>,
    /// Dependencies between boxes that the session brought.
    pub linked: Vec<ChangedLink>,
    /// Dependencies that it took away.
    pub unlinked: Vec<ChangedLink>,
}

impl SessionChanges {
    pub fn is_empty(&self) -> bool {
        self.files == 0
            && self.added.is_empty()
            && self.modified.is_empty()
            && self.removed.is_empty()
            && self.linked.is_empty()
            && self.unlinked.is_empty()
    }
}

/// A package or a component, as the journal names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ChangedBox {
    /// Its id in the graph (`pkg:cargo/demo`, `component:dir/src`).
    pub id: String,
    pub name: String,
    /// The name of the box that holds it; `null` at the top.
    pub within: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ChangedLink {
    pub from: ChangedBox,
    pub to: ChangedBox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Open,
    Ended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionSnapshots {
    pub start: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<u64>,
}

/// `.timonier/sessions/untracked.json`: what changed while no session was open (ADR 0029,
/// PLAN.md 6.4), one gap per session that followed it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Untracked {
    pub version: u32,
    pub gaps: Vec<UntrackedGap>,
}

impl Default for Untracked {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            gaps: Vec::new(),
        }
    }
}

/// The files changed between two snapshots with no session open: the end of a session (or the
/// first snapshot) and the start of the next.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UntrackedGap {
    pub from: u64,
    pub to: u64,
    /// The session that came after: when it started, and its id.
    pub until: Stamp,
    /// Paths relative to the root, sorted.
    pub files: Vec<String>,
}

/// Why an operation was refused: a stable code, and a message that says what to do.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
}

impl Refusal {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub type Outcome<T> = std::result::Result<T, Refusal>;

/// A text an agent wrote: trimmed, not empty, not longer than `max` characters.
pub fn text(field: &str, value: &str, max: usize) -> Outcome<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(Refusal::new(
            "invalid_argument",
            format!("{field} is empty: give it a few words"),
        ));
    }
    if value.chars().count() > max {
        return Err(Refusal::new(
            "invalid_argument",
            format!("{field} is longer than {max} characters: shorten it"),
        ));
    }
    Ok(value.to_owned())
}

/// The number of an id `<prefix>-<n>`, `n` from 1, without leading zeros.
pub fn id_number(prefix: &str, id: &str) -> Option<u32> {
    let digits = id.strip_prefix(prefix)?.strip_prefix('-')?;
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

impl Plan {
    pub fn task(&self, id: &str) -> Option<&Task> {
        self.milestones
            .iter()
            .flat_map(|m| &m.tasks)
            .find(|t| t.id == id)
    }

    /// Gives task `id` its ticket (ADR 0056); `None` when there is no such task.
    pub fn set_ticket(&mut self, id: &str, ticket: &str) -> Option<Task> {
        let task = self.task_mut(id)?;
        task.ticket = Some(ticket.to_owned());
        Some(task.clone())
    }

    fn task_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.milestones
            .iter_mut()
            .flat_map(|m| &mut m.tasks)
            .find(|t| t.id == id)
    }

    fn next(&self, prefix: &str) -> String {
        let used = self
            .milestones
            .iter()
            .map(|m| &m.id)
            .chain(
                self.milestones
                    .iter()
                    .flat_map(|m| m.tasks.iter().map(|t| &t.id)),
            )
            .filter_map(|id| id_number(prefix, id))
            .max()
            .unwrap_or(0);
        format!("{prefix}-{}", used + 1)
    }

    /// Checks what a file holds: well-formed ids, unique, and the texts within bounds.
    pub fn check(&self) -> Outcome<()> {
        if self.version != FORMAT_VERSION {
            return Err(Refusal::new(
                "invalid_file",
                format!(
                    "plan.json has version {}, not {FORMAT_VERSION}",
                    self.version
                ),
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for milestone in &self.milestones {
            if id_number("M", &milestone.id).is_none() || !seen.insert(&milestone.id) {
                return Err(Refusal::new(
                    "invalid_file",
                    format!(
                        "plan.json: milestone id {:?} is malformed or repeated",
                        milestone.id
                    ),
                ));
            }
            for task in &milestone.tasks {
                if id_number("T", &task.id).is_none() || !seen.insert(&task.id) {
                    return Err(Refusal::new(
                        "invalid_file",
                        format!("plan.json: task id {:?} is malformed or repeated", task.id),
                    ));
                }
            }
        }
        Ok(())
    }

    /// `plan_add_task`: a task in an existing milestone, or in a new one.
    pub fn add_task(&mut self, input: &PlanAddTask, stamp: Stamp) -> Outcome<Task> {
        let title = text("title", &input.title, MAX_TITLE)?;
        let criterion = input
            .criterion
            .as_deref()
            .map(|c| text("criterion", c, MAX_TEXT))
            .transpose()?;
        let index = match (&input.milestone, &input.new_milestone) {
            (Some(id), None) => self
                .milestones
                .iter()
                .position(|m| &m.id == id)
                .ok_or_else(|| {
                    Refusal::new(
                        "unknown_milestone",
                        format!("no milestone {id}: call get_plan for the milestones"),
                    )
                })?,
            (None, Some(new)) => {
                let title = text("new_milestone", new, MAX_TITLE)?;
                let id = self.next("M");
                self.milestones.push(Milestone {
                    id,
                    title,
                    tasks: Vec::new(),
                });
                self.milestones.len() - 1
            }
            _ => {
                return Err(Refusal::new(
                    "invalid_argument",
                    "give either milestone (an existing M-… id) or new_milestone (a title)",
                ));
            }
        };
        let task = Task {
            id: self.next("T"),
            title,
            criterion,
            status: TaskStatus::Todo,
            components: Vec::new(),
            sessions: vec![stamp.session.clone()],
            created: stamp,
            notes: Vec::new(),
            ticket: None,
        };
        self.milestones[index].tasks.push(task.clone());
        Ok(task)
    }

    /// `plan_update_task`: a new state, title, criterion or note. The same state again changes
    /// nothing; a forbidden transition is refused.
    pub fn update_task(&mut self, input: &PlanUpdateTask, stamp: &Stamp) -> Outcome<Task> {
        let title = input
            .title
            .as_deref()
            .map(|t| text("title", t, MAX_TITLE))
            .transpose()?;
        let criterion = input
            .criterion
            .as_deref()
            .map(|c| text("criterion", c, MAX_TEXT))
            .transpose()?;
        let note = input
            .note
            .as_deref()
            .map(|n| text("note", n, MAX_TEXT))
            .transpose()?;
        if input.status.is_none() && title.is_none() && criterion.is_none() && note.is_none() {
            return Err(Refusal::new(
                "invalid_argument",
                "nothing to update: give status, title, criterion or note",
            ));
        }
        let task = self.task_mut(&input.task_id).ok_or_else(|| {
            Refusal::new(
                "unknown_task",
                format!("no task {}: call get_plan for the tasks", input.task_id),
            )
        })?;
        if let Some(status) = input.status
            && status != task.status
        {
            if !task.status.may_become(status) {
                return Err(Refusal::new(
                    "forbidden_transition",
                    format!(
                        "task {} is {}: it cannot become {}",
                        task.id,
                        task.status.as_str(),
                        status.as_str()
                    ),
                ));
            }
            if task.status == TaskStatus::Done && note.is_none() {
                return Err(Refusal::new(
                    "invalid_argument",
                    format!("reopening task {} needs a note that says why", task.id),
                ));
            }
            task.status = status;
        }
        if let Some(title) = title {
            task.title = title;
        }
        if let Some(criterion) = criterion {
            task.criterion = Some(criterion);
        }
        if let Some(text) = note {
            task.notes.push(Note {
                at: stamp.at.clone(),
                session: stamp.session.clone(),
                text,
            });
        }
        if !task.sessions.contains(&stamp.session) {
            task.sessions.push(stamp.session.clone());
        }
        Ok(task.clone())
    }
}

impl Todo {
    pub fn check(&self) -> Outcome<()> {
        if self.version != FORMAT_VERSION {
            return Err(Refusal::new(
                "invalid_file",
                format!(
                    "todo.json has version {}, not {FORMAT_VERSION}",
                    self.version
                ),
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for item in &self.items {
            if id_number("D", &item.id).is_none() || !seen.insert(&item.id) {
                return Err(Refusal::new(
                    "invalid_file",
                    format!("todo.json: item id {:?} is malformed or repeated", item.id),
                ));
            }
        }
        Ok(())
    }

    /// `todo_add`: an item of the session in progress, maybe for a task of the plan.
    pub fn add(&mut self, input: &TodoAdd, plan: &Plan) -> Outcome<TodoItem> {
        let text = text("text", &input.text, MAX_TEXT)?;
        if let Some(task) = &input.task
            && plan.task(task).is_none()
        {
            return Err(Refusal::new(
                "unknown_task",
                format!("no task {task}: call get_plan for the tasks"),
            ));
        }
        let next = self
            .items
            .iter()
            .filter_map(|i| id_number("D", &i.id))
            .max()
            .unwrap_or(0)
            + 1;
        let item = TodoItem {
            id: format!("D-{next}"),
            text,
            task: input.task.clone(),
            status: TodoStatus::Open,
        };
        self.items.push(item.clone());
        Ok(item)
    }

    /// `todo_complete`: done; done again changes nothing.
    pub fn complete(&mut self, input: &TodoComplete) -> Outcome<TodoItem> {
        let item = self
            .items
            .iter_mut()
            .find(|i| i.id == input.item_id)
            .ok_or_else(|| {
                Refusal::new(
                    "unknown_item",
                    format!(
                        "no todo item {}: call get_todo for the items",
                        input.item_id
                    ),
                )
            })?;
        item.status = TodoStatus::Done;
        Ok(item.clone())
    }
}

/// UTC time of a Unix time in milliseconds, RFC 3339 to the second.
pub fn utc(millis: i64) -> String {
    let seconds = millis.div_euclid(1000);
    let (days, time) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Days to civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time / 3600,
        time % 3600 / 60,
        time % 60
    )
}

// The inputs of the tools (ADR 0027): strict, bounded, one source for their JSON schemas.

/// `session_start`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionStart {
    /// What the session is for, in a sentence.
    #[schemars(length(min = 1, max = 2000))]
    pub objective: String,
    /// The task of the plan it works on (`T-…`), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(pattern(r"^T-[1-9][0-9]*$"))]
    pub task: Option<String>,
}

/// `session_end`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionEnd {
    /// What was done, and what is left, in a few sentences.
    #[schemars(length(min = 1, max = 2000))]
    pub summary: String,
}

/// `plan_add_task`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanAddTask {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    /// What says the task is done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 2000))]
    pub criterion: Option<String>,
    /// An existing milestone (`M-…`); or else `new_milestone`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(pattern(r"^M-[1-9][0-9]*$"))]
    pub milestone: Option<String>,
    /// The title of a new milestone for the task; or else `milestone`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 200))]
    pub new_milestone: Option<String>,
}

/// `plan_update_task`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanUpdateTask {
    #[schemars(pattern(r"^T-[1-9][0-9]*$"))]
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 200))]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 2000))]
    pub criterion: Option<String>,
    /// Why; required to reopen a task that was done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 2000))]
    pub note: Option<String>,
}

/// `todo_add`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TodoAdd {
    #[schemars(length(min = 1, max = 2000))]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(pattern(r"^T-[1-9][0-9]*$"))]
    pub task: Option<String>,
}

/// `todo_complete`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TodoComplete {
    #[schemars(pattern(r"^D-[1-9][0-9]*$"))]
    pub item_id: String,
}

/// `get_plan`, `get_todo`: no argument.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArguments {}

/// `get_session`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetSession {
    /// A session (`S-…`); the open one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(pattern(r"^S-[0-9]{4}-[0-9]{2}-[0-9]{2}-[1-9][0-9]*$"))]
    pub session_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(session: &str) -> Stamp {
        Stamp {
            at: "2026-10-20T08:00:00Z".into(),
            session: session.into(),
        }
    }

    fn add(
        plan: &mut Plan,
        title: &str,
        milestone: Option<&str>,
        new: Option<&str>,
    ) -> Outcome<Task> {
        plan.add_task(
            &PlanAddTask {
                title: title.into(),
                criterion: None,
                milestone: milestone.map(Into::into),
                new_milestone: new.map(Into::into),
            },
            stamp("S-2026-10-20-1"),
        )
    }

    fn update(
        plan: &mut Plan,
        task: &str,
        status: Option<TaskStatus>,
        note: Option<&str>,
    ) -> Outcome<Task> {
        plan.update_task(
            &PlanUpdateTask {
                task_id: task.into(),
                status,
                title: None,
                criterion: None,
                note: note.map(Into::into),
            },
            &stamp("S-2026-10-20-2"),
        )
    }

    #[test]
    fn tasks_get_ids_that_are_never_reused() {
        let mut plan = Plan::default();
        let first = add(&mut plan, "Index Rust", None, Some("Indexing")).unwrap();
        let second = add(&mut plan, "Index TS", Some("M-1"), None).unwrap();
        assert_eq!((first.id.as_str(), second.id.as_str()), ("T-1", "T-2"));
        assert_eq!(plan.milestones[0].id, "M-1");
        update(&mut plan, "T-1", Some(TaskStatus::Dropped), None).unwrap();
        // A dropped task stays: its id is not given again.
        assert_eq!(
            add(&mut plan, "Other", Some("M-1"), None).unwrap().id,
            "T-3"
        );
        assert_eq!(plan.check(), Ok(()));
    }

    #[test]
    fn a_task_needs_one_milestone_and_a_title() {
        let mut plan = Plan::default();
        let refused = |r: Outcome<Task>| r.unwrap_err().code;
        assert_eq!(refused(add(&mut plan, "x", None, None)), "invalid_argument");
        assert_eq!(
            refused(add(&mut plan, "x", Some("M-1"), Some("y"))),
            "invalid_argument"
        );
        assert_eq!(
            refused(add(&mut plan, "x", Some("M-9"), None)),
            "unknown_milestone"
        );
        assert_eq!(
            refused(add(&mut plan, "   ", None, Some("m"))),
            "invalid_argument"
        );
        assert_eq!(
            refused(add(&mut plan, &"x".repeat(MAX_TITLE + 1), None, Some("m"))),
            "invalid_argument"
        );
    }

    #[test]
    fn transitions_follow_the_states_of_a_task() {
        let mut plan = Plan::default();
        add(&mut plan, "Task", None, Some("M")).unwrap();
        let code = |r: Outcome<Task>| r.map(|t| t.status).map_err(|e| e.code);
        assert_eq!(
            code(update(&mut plan, "T-1", Some(TaskStatus::Done), None)),
            Err("forbidden_transition")
        );
        assert_eq!(
            code(update(&mut plan, "T-1", Some(TaskStatus::InProgress), None)),
            Ok(TaskStatus::InProgress)
        );
        // The same state again: nothing changes, nothing refused.
        assert_eq!(
            code(update(&mut plan, "T-1", Some(TaskStatus::InProgress), None)),
            Ok(TaskStatus::InProgress)
        );
        assert_eq!(
            code(update(&mut plan, "T-1", Some(TaskStatus::Done), None)),
            Ok(TaskStatus::Done)
        );
        // Reopened only with a reason.
        assert_eq!(
            code(update(&mut plan, "T-1", Some(TaskStatus::InProgress), None)),
            Err("invalid_argument")
        );
        assert_eq!(
            code(update(
                &mut plan,
                "T-1",
                Some(TaskStatus::InProgress),
                Some("the test fails")
            )),
            Ok(TaskStatus::InProgress)
        );
        assert_eq!(
            code(update(&mut plan, "T-1", None, None)),
            Err("invalid_argument")
        );
        assert_eq!(
            code(update(&mut plan, "T-9", None, Some("x"))),
            Err("unknown_task")
        );
        let task = plan.task("T-1").unwrap();
        assert_eq!(task.notes.len(), 1);
        assert_eq!(task.sessions, ["S-2026-10-20-1", "S-2026-10-20-2"]);
    }

    #[test]
    fn every_transition_is_listed() {
        use TaskStatus::*;
        let all = [Todo, InProgress, Blocked, Done, Dropped];
        let allowed: Vec<(TaskStatus, TaskStatus)> = all
            .iter()
            .flat_map(|a| all.iter().map(move |b| (*a, *b)))
            .filter(|(a, b)| a.may_become(*b))
            .collect();
        assert_eq!(
            allowed,
            [
                (Todo, InProgress),
                (Todo, Dropped),
                (InProgress, Blocked),
                (InProgress, Done),
                (InProgress, Dropped),
                (Blocked, InProgress),
                (Blocked, Dropped),
                (Done, InProgress),
            ]
        );
    }

    #[test]
    fn todo_items_are_added_and_completed() {
        let mut plan = Plan::default();
        add(&mut plan, "Task", None, Some("M")).unwrap();
        let mut todo = Todo::default();
        let item = todo
            .add(
                &TodoAdd {
                    text: "Write the test".into(),
                    task: Some("T-1".into()),
                },
                &plan,
            )
            .unwrap();
        assert_eq!(item.id, "D-1");
        let refused = todo
            .add(
                &TodoAdd {
                    text: "x".into(),
                    task: Some("T-7".into()),
                },
                &plan,
            )
            .unwrap_err();
        assert_eq!(refused.code, "unknown_task");
        let done = |todo: &mut Todo| {
            todo.complete(&TodoComplete {
                item_id: "D-1".into(),
            })
        };
        assert_eq!(done(&mut todo).unwrap().status, TodoStatus::Done);
        // Done again: nothing changes.
        assert_eq!(done(&mut todo).unwrap().status, TodoStatus::Done);
        assert_eq!(
            todo.complete(&TodoComplete {
                item_id: "D-5".into()
            })
            .unwrap_err()
            .code,
            "unknown_item"
        );
        assert_eq!(todo.check(), Ok(()));
    }

    #[test]
    fn files_with_bad_ids_or_versions_are_refused() {
        let mut plan = Plan::default();
        add(&mut plan, "Task", None, Some("M")).unwrap();
        let mut repeated = plan.clone();
        let copy = repeated.milestones[0].tasks[0].clone();
        repeated.milestones[0].tasks.push(copy);
        assert_eq!(repeated.check().unwrap_err().code, "invalid_file");
        let mut versioned = plan.clone();
        versioned.version = 2;
        assert_eq!(versioned.check().unwrap_err().code, "invalid_file");
        for bad in ["T-0", "T-01", "T-", "T1", "X-1", "T-1a"] {
            assert_eq!(id_number("T", bad), None, "{bad}");
        }
        assert_eq!(id_number("T", "T-12"), Some(12));
    }

    #[test]
    fn files_round_trip_and_refuse_unknown_fields() {
        let mut plan = Plan::default();
        add(&mut plan, "Task", None, Some("M")).unwrap();
        let json = serde_json::to_string_pretty(&plan).unwrap();
        assert_eq!(serde_json::from_str::<Plan>(&json).unwrap(), plan);
        let extra = json.replacen("\"version\": 1", "\"version\": 1, \"owner\": \"x\"", 1);
        assert!(serde_json::from_str::<Plan>(&extra).is_err());
        assert!(
            serde_json::from_value::<TodoAdd>(serde_json::json!({"text": "x", "due": 1})).is_err()
        );
    }

    #[test]
    fn times_are_utc_rfc_3339() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_792_484_100_000), "2026-10-20T08:15:00Z");
        assert_eq!(utc(951_782_400_000), "2000-02-29T00:00:00Z");
    }

    /// The schemas of the files, for whoever reads `.timonier/` (ADR 0029). Update with
    /// `TIMONIER_UPDATE_TOOLS=1 cargo test -p timonier-core`.
    #[test]
    fn the_file_schemas_are_up_to_date() {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schemas");
        let schemas = [
            ("plan.schema.json", schemars::schema_for!(Plan)),
            ("todo.schema.json", schemars::schema_for!(Todo)),
            ("session.schema.json", schemars::schema_for!(Session)),
            ("untracked.schema.json", schemars::schema_for!(Untracked)),
            (
                "actual.schema.json",
                schemars::schema_for!(crate::actual::Actual),
            ),
            (
                "intent.schema.json",
                schemars::schema_for!(crate::intent::Intent),
            ),
        ];
        let update = std::env::var_os("TIMONIER_UPDATE_TOOLS").is_some();
        for (name, schema) in schemas {
            let current = serde_json::to_string_pretty(&schema).unwrap() + "\n";
            let path = folder.join(name);
            if update {
                std::fs::create_dir_all(&folder).unwrap();
                std::fs::write(&path, &current).unwrap();
            }
            let kept = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(
                kept == current,
                "{name} is stale: TIMONIER_UPDATE_TOOLS=1 cargo test -p timonier-core"
            );
        }
    }

    #[test]
    fn input_schemas_are_strict() {
        let schema = serde_json::to_value(schemars::schema_for!(PlanAddTask)).unwrap();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], serde_json::json!(["title"]));
        assert_eq!(schema["properties"]["title"]["maxLength"], 200);
        assert_eq!(
            schema["properties"]["milestone"]["pattern"],
            "^M-[1-9][0-9]*$"
        );
    }
}
