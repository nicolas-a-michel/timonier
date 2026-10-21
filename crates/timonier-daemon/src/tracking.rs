//! The project's tracking in `.timonier/` (ADR 0029): the daemon runs the MCP tools of the plan,
//! the todo and the sessions (ADR 0027), and is the only writer of their files.
//!
//! The files are read again at each call: a change made outside the daemon (by hand, by git) is
//! taken as it is, and a file that no longer reads refuses the writes that need it.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use timonier_core::Snapshot;
use timonier_core::drift::Drift;
use timonier_core::intent::{Intent, Policy};
use timonier_core::tools::{self, Mailbox};
use timonier_core::tracking::{
    FORMAT_VERSION, GetSession, MAX_TEXT, NoArguments, Outcome, Plan, PlanAddTask, PlanUpdateTask,
    Refusal, Session, SessionChanges, SessionEnd, SessionSnapshots, SessionStart, SessionStatus,
    Stamp, Todo, TodoAdd, TodoComplete, Untracked, UntrackedGap, id_number, text, utc,
};

/// Where the tracking lives: `.timonier/` at the root of the project.
pub struct Tracker {
    dir: PathBuf,
}

/// What a call needs from the daemon: who calls, the time, and a frozen snapshot.
pub struct Context<'a> {
    /// The MCP client of the connection (`claude-code`…).
    pub agent: &'a str,
    /// Unix time, in milliseconds.
    pub now_ms: i64,
    /// Freezes the head as a `working_tree` snapshot (ADR 0029).
    pub freeze: &'a dyn Fn() -> crate::Result<Snapshot>,
    /// The files changed between two snapshots; from the first snapshot when `None`.
    pub changed_files: &'a dyn Fn(Option<u64>, u64) -> crate::Result<Vec<String>>,
    /// What changed in the code between a session's two snapshots (ADR 0031).
    pub changes: &'a dyn Fn(u64, u64) -> crate::Result<SessionChanges>,
    /// Forgets the contents the sessions' snapshots kept, once a session's diff is written.
    pub forget_contents: &'a dyn Fn() -> crate::Result<()>,
    /// The architecture the head holds, as `actual.json` says it (ADR 0033): for `get_drift`.
    pub actual: &'a dyn Fn() -> crate::Result<timonier_core::actual::Actual>,
}

impl Tracker {
    pub fn new(root: &Path) -> Self {
        Self {
            dir: root.join(".timonier"),
        }
    }

    /// Runs a tool: its result as MCP's `CallToolResult`, the mailbox header first (ADR 0028).
    pub fn call(&self, name: &str, arguments: Option<Value>, context: &Context) -> Value {
        let mailbox = Mailbox::default();
        if name == tools::GET_INTENDED_ARCHITECTURE {
            return match self.intended(arguments) {
                Ok((summary, result, Some(png))) => {
                    tools::with_image(tools::success(mailbox, &summary, result), &png)
                }
                Ok((summary, result, None)) => tools::success(mailbox, &summary, result),
                Err(refusal) => tools::failure(mailbox, &refusal),
            };
        }
        match self.run(name, arguments, context) {
            Ok((summary, result)) => tools::success(mailbox, &summary, result),
            Err(refusal) => tools::failure(mailbox, &refusal),
        }
    }

    fn run(
        &self,
        name: &str,
        arguments: Option<Value>,
        context: &Context,
    ) -> Outcome<(String, Value)> {
        match name {
            tools::SESSION_START => self.session_start(&tools::arguments(arguments)?, context),
            tools::SESSION_END => self.session_end(&tools::arguments(arguments)?, context),
            tools::PLAN_ADD_TASK => {
                let input: PlanAddTask = tools::arguments(arguments)?;
                let (mut session, stamp) = self.writing(context)?;
                let mut plan = self.plan()?;
                let task = plan.add_task(&input, stamp)?;
                self.write("plan.json", &plan)?;
                remember(&mut session, &task.id);
                self.write_session(&session)?;
                Ok((
                    format!("Task {} added: {}.", task.id, task.title),
                    json(&task),
                ))
            }
            tools::PLAN_UPDATE_TASK => {
                let input: PlanUpdateTask = tools::arguments(arguments)?;
                let (mut session, stamp) = self.writing(context)?;
                let mut plan = self.plan()?;
                let task = plan.update_task(&input, &stamp)?;
                self.write("plan.json", &plan)?;
                remember(&mut session, &task.id);
                self.write_session(&session)?;
                Ok((
                    format!("Task {} is {}.", task.id, task.status.as_str()),
                    json(&task),
                ))
            }
            tools::TODO_ADD => {
                let input: TodoAdd = tools::arguments(arguments)?;
                self.writing(context)?;
                let (plan, mut todo) = (self.plan()?, self.todo()?);
                let item = todo.add(&input, &plan)?;
                self.write("todo.json", &todo)?;
                Ok((format!("Todo item {} added.", item.id), json(&item)))
            }
            tools::TODO_COMPLETE => {
                let input: TodoComplete = tools::arguments(arguments)?;
                self.writing(context)?;
                let mut todo = self.todo()?;
                let item = todo.complete(&input)?;
                self.write("todo.json", &todo)?;
                Ok((format!("Todo item {} done.", item.id), json(&item)))
            }
            tools::GET_DRIFT => {
                let _: NoArguments = tools::arguments(arguments)?;
                let Some(intent) = self.intent()?.filter(|i| !i.is_empty()) else {
                    return Ok((NOTHING_DRAWN.to_owned(), json(&Drift::default())));
                };
                let actual = (context.actual)().map_err(|e| internal(e.to_string()))?;
                let drift = timonier_core::drift::drift(&intent, &actual);
                Ok((drift_summary(&intent, &drift), json(&drift)))
            }
            tools::GET_PLAN => {
                let _: NoArguments = tools::arguments(arguments)?;
                let plan = self.plan()?;
                let tasks = plan.milestones.iter().map(|m| m.tasks.len()).sum::<usize>();
                Ok((
                    format!("{} milestones, {tasks} tasks.", plan.milestones.len()),
                    with_progress(&plan),
                ))
            }
            tools::GET_TODO => {
                let _: NoArguments = tools::arguments(arguments)?;
                let todo = self.todo()?;
                let open = todo
                    .items
                    .iter()
                    .filter(|i| i.status == timonier_core::tracking::TodoStatus::Open)
                    .count();
                Ok((
                    format!("{} items, {open} open.", todo.items.len()),
                    json(&todo),
                ))
            }
            tools::GET_SESSION => {
                let input: GetSession = tools::arguments(arguments)?;
                let session = match input.session_id {
                    Some(id) => self.session(&id)?,
                    None => self.open_session()?.ok_or_else(no_open_session)?,
                };
                Ok((
                    format!("Session {}: {}.", session.id, session.objective),
                    json(&session),
                ))
            }
            other => Err(Refusal::new(
                "unknown_tool",
                format!("no tool {other}: list the tools again"),
            )),
        }
    }

    fn session_start(&self, input: &SessionStart, context: &Context) -> Outcome<(String, Value)> {
        let objective = text("objective", &input.objective, MAX_TEXT)?;
        if let Some(open) = self.open_session()? {
            return Err(Refusal::new(
                "session_already_open",
                format!(
                    "session {} is open: call session_end to close it first",
                    open.id
                ),
            ));
        }
        let mut plan = self.plan()?;
        if let Some(task) = &input.task
            && plan.task(task).is_none()
        {
            return Err(Refusal::new(
                "unknown_task",
                format!("no task {task}: call get_plan for the tasks"),
            ));
        }
        let start = (context.freeze)().map_err(internal)?;
        let at = utc(context.now_ms);
        let previous = self.last_end()?;
        let session = Session {
            version: FORMAT_VERSION,
            id: self.next_session_id(&at)?,
            agent: context.agent.to_owned(),
            objective,
            tasks: input.task.iter().cloned().collect(),
            status: SessionStatus::Open,
            started_at: at,
            ended_at: None,
            snapshots: SessionSnapshots {
                start: start.seq,
                end: None,
            },
            summary: None,
            todo: Vec::new(),
            changes: None,
        };
        if let Some(task) = &input.task {
            plan.note_session(task, &session.id);
            self.write("plan.json", &plan)?;
        }
        // What changed since the last session ended, no session open: untracked (6.4).
        let files = (context.changed_files)(previous, start.seq).map_err(internal)?;
        if !files.is_empty() {
            let mut untracked = self.untracked()?;
            untracked.gaps.push(UntrackedGap {
                from: previous.unwrap_or(0),
                to: start.seq,
                until: Stamp {
                    at: session.started_at.clone(),
                    session: session.id.clone(),
                },
                files,
            });
            self.write("sessions/untracked.json", &untracked)?;
        }
        self.write_session(&session)?;
        self.write(
            "todo.json",
            &Todo {
                session: Some(session.id.clone()),
                ..Todo::default()
            },
        )?;
        Ok((format!("Session {} open.", session.id), json(&session)))
    }

    fn session_end(&self, input: &SessionEnd, context: &Context) -> Outcome<(String, Value)> {
        let summary = text("summary", &input.summary, MAX_TEXT)?;
        let mut session = self.open_session()?.ok_or_else(no_open_session)?;
        let todo = self.todo()?;
        let end = (context.freeze)().map_err(internal)?;
        session.status = SessionStatus::Ended;
        session.ended_at = Some(utc(context.now_ms));
        session.snapshots.end = Some(end.seq);
        session.summary = Some(summary);
        session.todo = todo.items;
        // The journal keeps what it changed; without it, the session still ends.
        session.changes = match (context.changes)(session.snapshots.start, end.seq) {
            Ok(changes) => Some(changes),
            Err(error) => {
                tracing::warn!(%error, session = %session.id, "cannot compute the session's changes");
                None
            }
        };
        self.write_session(&session)?;
        self.write("todo.json", &Todo::default())?;
        if let Err(error) = (context.forget_contents)() {
            tracing::warn!(%error, "cannot forget the contents of the session's snapshots");
        }
        let left = session
            .todo
            .iter()
            .filter(|i| i.status == timonier_core::tracking::TodoStatus::Open)
            .count();
        Ok((
            format!("Session {} ended, {left} todo items left open.", session.id),
            json(&session),
        ))
    }

    /// A write needs an open session: it is attributed to it.
    fn writing(&self, context: &Context) -> Outcome<(Session, Stamp)> {
        let session = self.open_session()?.ok_or_else(no_open_session)?;
        let stamp = Stamp {
            at: utc(context.now_ms),
            session: session.id.clone(),
        };
        Ok((session, stamp))
    }

    /// Why an agent that wants to stop should be held once (ADR 0033, Q49): an open session whose
    /// todo is empty or not done, or code changed since the last session with none open.
    /// `changed` gives the files changed since a snapshot (the first when `None`) to the head.
    pub fn stop_reason(
        &self,
        changed: &dyn Fn(Option<u64>) -> crate::Result<Vec<String>>,
    ) -> Outcome<Option<String>> {
        if let Some(session) = self.open_session()? {
            let todo = self.todo()?;
            let open = todo
                .items
                .iter()
                .filter(|i| i.status == timonier_core::tracking::TodoStatus::Open)
                .count();
            return Ok(if todo.items.is_empty() {
                Some(format!(
                    "Timonier session {} is open and its todo is empty: split the work with \
                     todo_add and complete it with todo_complete, or call session_end with a \
                     summary if the work is done.",
                    session.id
                ))
            } else if open > 0 {
                Some(format!(
                    "Timonier session {} has {open} todo item(s) not done: complete them with \
                     todo_complete, or call session_end with a summary that says what is left.",
                    session.id
                ))
            } else {
                None
            });
        }
        let files = changed(self.last_end()?).map_err(internal)?;
        if files.is_empty() {
            return Ok(None);
        }
        let shown: Vec<&str> = files.iter().take(5).map(String::as_str).collect();
        let more = files.len().saturating_sub(shown.len());
        Ok(Some(format!(
            "Code changed with no Timonier session open ({}{}): call session_start with the \
             objective of this work, then session_end with a summary.",
            shown.join(", "),
            if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            }
        )))
    }

    /// The open session: the one the todo belongs to, if its file says it is open.
    pub fn open_session(&self) -> Outcome<Option<Session>> {
        let Some(id) = self.todo()?.session else {
            return Ok(None);
        };
        let session = self.session(&id)?;
        Ok((session.status == SessionStatus::Open).then_some(session))
    }

    fn session(&self, id: &str) -> Outcome<Session> {
        if !is_session_id(id) {
            return Err(Refusal::new(
                "invalid_argument",
                format!("{id} is no session id"),
            ));
        }
        let path = self.dir.join("sessions").join(format!("{id}.json"));
        if !path.exists() {
            return Err(Refusal::new(
                "unknown_session",
                format!("no session {id} in the journal"),
            ));
        }
        let session: Session = read(&path)?;
        if session.version != FORMAT_VERSION || session.id != id {
            return Err(invalid_file(&path, "its version or its id is wrong"));
        }
        Ok(session)
    }

    /// The snapshot the last ended session froze at its end.
    fn last_end(&self) -> Outcome<Option<u64>> {
        let mut last = None;
        for entry in std::fs::read_dir(self.dir.join("sessions"))
            .into_iter()
            .flatten()
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(id) = name.strip_suffix(".json").filter(|id| is_session_id(id)) else {
                continue;
            };
            let session = self.session(id)?;
            last = last.max(session.snapshots.end);
        }
        Ok(last)
    }

    /// Every session of the journal that reads, by id; one that does not is left out.
    pub fn sessions(&self) -> Vec<Session> {
        self.journal().0
    }

    /// The sessions that read, by id, and the files of `sessions/` that do not, with why.
    pub fn journal(&self) -> (Vec<Session>, Vec<(String, String)>) {
        let mut problems = Vec::new();
        let mut sessions: Vec<Session> = std::fs::read_dir(self.dir.join("sessions"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let id = name.strip_suffix(".json").filter(|id| is_session_id(id))?;
                match self.session(id) {
                    Ok(session) => Some(session),
                    Err(refusal) => {
                        problems.push((format!("sessions/{name}"), refusal.message));
                        None
                    }
                }
            })
            .collect();
        sessions.sort_by(|a, b| a.id.cmp(&b.id));
        problems.sort();
        (sessions, problems)
    }

    /// The gaps between sessions, as the journal keeps them.
    pub fn untracked(&self) -> Outcome<Untracked> {
        let path = self.dir.join("sessions/untracked.json");
        if !path.exists() {
            return Ok(Untracked::default());
        }
        let untracked: Untracked = read(&path)?;
        if untracked.version != FORMAT_VERSION {
            return Err(invalid_file(&path, "its version is wrong"));
        }
        Ok(untracked)
    }

    fn next_session_id(&self, at: &str) -> Outcome<String> {
        let day = &at[..10];
        let prefix = format!("S-{day}");
        let used = std::fs::read_dir(self.dir.join("sessions"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                id_number(&prefix, name.strip_suffix(".json")?)
            })
            .max()
            .unwrap_or(0);
        Ok(format!("{prefix}-{}", used + 1))
    }

    pub fn plan(&self) -> Outcome<Plan> {
        let path = self.dir.join("plan.json");
        if !path.exists() {
            return Ok(Plan::default());
        }
        let plan: Plan = read(&path)?;
        plan.check()?;
        Ok(plan)
    }

    pub fn todo(&self) -> Outcome<Todo> {
        let path = self.dir.join("todo.json");
        if !path.exists() {
            return Ok(Todo::default());
        }
        let todo: Todo = read(&path)?;
        todo.check()?;
        Ok(todo)
    }

    /// The whiteboard, `None` before anything is drawn; refused when the file does not read.
    fn intent(&self) -> Outcome<Option<Intent>> {
        match std::fs::read_to_string(self.dir.join("intent.json")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(io_refusal(&self.dir.join("intent.json"), &error)),
            Ok(text) => timonier_core::intent::parse(&text).map(|i| Some(i.normalized())),
        }
    }

    /// `get_intended_architecture` (ADR 0041): the structure, where things are drawn, and the
    /// image of the sketch (Q67) unless declined.
    fn intended(&self, arguments: Option<Value>) -> Outcome<(String, Value, Option<Vec<u8>>)> {
        let input: tools::GetIntendedArchitecture = tools::arguments(arguments)?;
        let Some(intent) = self.intent()?.filter(|i| !i.is_empty()) else {
            return Ok((
                NOTHING_DRAWN.to_owned(),
                serde_json::json!({ "drawn": false }),
                None,
            ));
        };
        let shapes: std::collections::BTreeMap<&String, _> = intent
            .sketch
            .shapes
            .keys()
            .filter_map(|id| Some((id, intent.bounds_of(id)?)))
            .collect();
        let image = if input.image == Some(false) {
            None
        } else {
            crate::sketch::render(&intent)
        };
        let result = serde_json::json!({
            "drawn": true,
            "layers": intent.layers,
            "components": intent.components,
            "edges": intent.edges,
            "notes": intent.notes,
            "budgets": intent.budgets,
            "places": { "shapes": shapes, "strokes": intent.stroke_places() },
            "image": image.is_some(),
        });
        Ok((intended_summary(&intent, image.is_some()), result, image))
    }

    fn write_session(&self, session: &Session) -> Outcome<()> {
        self.write(&format!("sessions/{}.json", session.id), session)
    }

    /// A whole file at once: a temporary file beside it, then a rename.
    pub(crate) fn write<T: Serialize>(&self, name: &str, value: &T) -> Outcome<()> {
        let path = self.dir.join(name);
        let folder = path.parent().unwrap_or(&self.dir);
        std::fs::create_dir_all(folder).map_err(|e| io_refusal(&path, &e))?;
        let mut text = serde_json::to_string_pretty(value).map_err(|e| internal(e.to_string()))?;
        text.push('\n');
        let temporary = folder.join(format!(
            ".{}.tmp",
            path.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        ));
        std::fs::write(&temporary, text).map_err(|e| io_refusal(&temporary, &e))?;
        std::fs::rename(&temporary, &path).map_err(|e| io_refusal(&path, &e))
    }
}

impl PlanSessions for Plan {
    fn note_session(&mut self, task: &str, session: &str) {
        if let Some(task) = self
            .milestones
            .iter_mut()
            .flat_map(|m| &mut m.tasks)
            .find(|t| t.id == task)
            && !task.sessions.iter().any(|s| s == session)
        {
            task.sessions.push(session.to_owned());
        }
    }
}

/// The session a task of the plan is worked on in.
trait PlanSessions {
    fn note_session(&mut self, task: &str, session: &str);
}

/// The files a change touched between two snapshots, sorted: those of the nodes and edges the
/// diff holds (ADR 0010), each found in the snapshot where it was. From the first snapshot when
/// `from` is `None`.
pub fn changed_files(
    reader: &mut timonier_store::Reader,
    from: Option<u64>,
    to: u64,
) -> crate::Result<Vec<String>> {
    let from = match from {
        Some(from) => from,
        None => match reader.snapshots()?.first() {
            Some(first) => first.seq,
            None => return Ok(Vec::new()),
        },
    };
    if from >= to {
        return Ok(Vec::new());
    }
    let diff = reader.diff(from, to)?;
    if diff.is_empty() {
        return Ok(Vec::new());
    }
    let before = timonier_core::graph::Graph::new(reader.nodes_at(from)?, Vec::new());
    let after = timonier_core::graph::Graph::new(reader.nodes_at(to)?, Vec::new());
    let mut files = std::collections::BTreeSet::new();
    let mut note = |graph: &timonier_core::graph::Graph, id: &timonier_core::NodeId| {
        if let Some(timonier_core::IdForm::File { path }) = graph.file_of(id).map(|f| f.form()) {
            files.insert(path.to_owned());
        }
    };
    for change in &diff.nodes {
        use timonier_core::diff::NodeChange;
        match change {
            NodeChange::Added { node } => note(&after, &node.id),
            NodeChange::Removed { node } => note(&before, &node.id),
            NodeChange::Modified {
                before: old,
                after: new,
            }
            | NodeChange::Moved {
                before: old,
                after: new,
                ..
            } => {
                note(&before, &old.id);
                note(&after, &new.id);
            }
        }
    }
    for change in &diff.edges {
        use timonier_core::diff::EdgeChange;
        match change {
            EdgeChange::Added { edge } => note(&after, &edge.src),
            EdgeChange::Removed { edge } => note(&before, &edge.src),
            EdgeChange::Modified { after: edge, .. } => note(&after, &edge.src),
        }
    }
    Ok(files.into_iter().collect())
}

fn remember(session: &mut Session, task: &str) {
    if !session.tasks.iter().any(|t| t == task) {
        session.tasks.push(task.to_owned());
    }
}

/// The plan, each milestone with its progress: tasks done over tasks not dropped.
fn with_progress(plan: &Plan) -> Value {
    let mut value = json(plan);
    if let Some(milestones) = value["milestones"].as_array_mut() {
        for (milestone, kept) in milestones.iter_mut().zip(&plan.milestones) {
            let (done, of) = kept.progress();
            milestone["progress"] = json!({ "done": done, "of": of });
        }
    }
    value
}

fn is_session_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("S-") else {
        return false;
    };
    rest.len() > 11
        && rest.as_bytes()[..10].iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
        && id_number(&id[..12], id).is_some()
}

fn read<T: DeserializeOwned>(path: &Path) -> Outcome<T> {
    let text = std::fs::read_to_string(path).map_err(|e| io_refusal(path, &e))?;
    serde_json::from_str(&text).map_err(|e| invalid_file(path, &e.to_string()))
}

fn json<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn no_open_session() -> Refusal {
    Refusal::new(
        "no_open_session",
        "no open session: call session_start first",
    )
}

fn invalid_file(path: &Path, why: &str) -> Refusal {
    Refusal::new(
        "invalid_file",
        format!(
            "{} does not read ({why}): fix it or take it back from git; the daemon changes nothing until then",
            path.display()
        ),
    )
}

fn io_refusal(path: &Path, error: &std::io::Error) -> Refusal {
    Refusal::new("io_error", format!("{}: {error}", path.display()))
}

fn internal(error: impl ToString) -> Refusal {
    Refusal::new("internal", error.to_string())
}

/// What the whiteboard tools say before anything is drawn.
const NOTHING_DRAWN: &str = "Nothing is drawn yet: the developer draws the architecture they want on \
     Timonier's whiteboard (« Timonier: Open the Whiteboard »).";

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn intended_summary(intent: &Intent, image: bool) -> String {
    let forbidden = intent
        .edges
        .iter()
        .filter(|e| e.policy == Policy::Forbidden)
        .count();
    let required = intent
        .edges
        .iter()
        .filter(|e| e.policy == Policy::Required)
        .count();
    format!(
        "{}, {}, {} ({forbidden} forbidden, {required} required), {}, {}, {}.{}",
        count(intent.layers.len(), "layer", "layers"),
        count(intent.components.len(), "component", "components"),
        count(intent.edges.len(), "edge", "edges"),
        count(intent.notes.len(), "note", "notes"),
        count(intent.budgets.len(), "budget", "budgets"),
        count(
            intent.sketch.strokes.len(),
            "freehand stroke",
            "freehand strokes"
        ),
        if image {
            " The sketch is attached as an image; ids are written on it."
        } else {
            ""
        }
    )
}

fn drift_summary(intent: &Intent, drift: &Drift) -> String {
    let name = |id: &str| {
        intent
            .layers
            .iter()
            .map(|l| (&l.id, &l.name))
            .chain(intent.components.iter().map(|c| (&c.id, &c.name)))
            .find(|(i, _)| *i == id)
            .map_or_else(|| id.to_owned(), |(_, n)| format!("{id} {n}"))
    };
    let mut said = vec![format!(
        "{}: {} found in the code, {} missing{}.",
        count(drift.anchors.len(), "drawn component", "drawn components"),
        drift.anchors.len() - drift.missing.len(),
        drift.missing.len(),
        if drift.missing.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                drift
                    .missing
                    .iter()
                    .map(|m| name(m))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    )];
    for crossing in &drift.forbidden {
        said.push(format!(
            "Forbidden edge {} crossed: {} → {}, {}.",
            crossing.edge,
            name(&crossing.src),
            name(&crossing.dst),
            count(crossing.dependencies.len(), "dependency", "dependencies")
        ));
    }
    for crossing in &drift.required {
        said.push(format!(
            "Required edge {} missing: {} → {}.",
            crossing.edge,
            name(&crossing.src),
            name(&crossing.dst)
        ));
    }
    if !drift.unchecked.is_empty() {
        said.push(format!(
            "{} not checked: an end stands for nothing in the code.",
            count(drift.unchecked.len(), "edge", "edges")
        ));
    }
    if drift.forbidden.is_empty() && drift.required.is_empty() && drift.missing.is_empty() {
        said.push("The code follows the whiteboard.".to_owned());
    }
    said.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(
        freeze: &'a dyn Fn() -> crate::Result<Snapshot>,
        changed_files: &'a dyn Fn(Option<u64>, u64) -> crate::Result<Vec<String>>,
        now_ms: i64,
    ) -> Context<'a> {
        Context {
            agent: "test-agent",
            now_ms,
            freeze,
            changed_files,
            // One file changed, whatever the snapshots.
            changes: &|_, _| {
                Ok(SessionChanges {
                    files: 1,
                    ..SessionChanges::default()
                })
            },
            forget_contents: &|| Ok(()),
            actual: &|| {
                Ok(timonier_core::actual::Actual {
                    version: 1,
                    boxes: Vec::new(),
                    dependencies: Vec::new(),
                })
            },
        }
    }

    /// PLAN.md 7.5, ADR 0041: the whiteboard read for the agent, with its image, and compared
    /// with the code.
    #[test]
    fn the_agent_reads_the_whiteboard_and_its_drift() {
        use timonier_core::actual::{Actual, ActualBox, ActualDependency};
        let root = tempfile::tempdir().unwrap();
        let tracker = Tracker::new(root.path());
        let actual = || {
            Ok(Actual {
                version: 1,
                boxes: ["a", "b"]
                    .iter()
                    .map(|n| ActualBox {
                        id: format!("pkg:cargo/{n}"),
                        kind: "package".into(),
                        name: (*n).into(),
                        role: None,
                        parent: None,
                    })
                    .collect(),
                dependencies: vec![ActualDependency {
                    from: "pkg:cargo/a".into(),
                    to: "pkg:cargo/b".into(),
                    kinds: vec!["calls".into()],
                    weight: 3,
                }],
            })
        };
        let freeze = || Ok(snapshot(1));
        let changed = |_: Option<u64>, _: u64| Ok(Vec::new());
        let context = Context {
            actual: &actual,
            ..context(&freeze, &changed, 0)
        };
        let call = |name: &str, arguments: Value| tracker.call(name, Some(arguments), &context);

        // Nothing drawn yet, or the empty whiteboard `init` puts: said so, no image, no drift.
        std::fs::create_dir_all(root.path().join(".timonier")).unwrap();
        std::fs::write(
            root.path().join(".timonier/intent.json"),
            Intent::empty().to_text(),
        )
        .unwrap();
        let nothing = call(tools::GET_INTENDED_ARCHITECTURE, json!({}));
        assert_eq!(nothing["structuredContent"]["result"]["drawn"], false);
        assert!(
            nothing["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Nothing is drawn yet")
        );
        assert_eq!(nothing["content"].as_array().unwrap().len(), 1);

        std::fs::create_dir_all(root.path().join(".timonier")).unwrap();
        std::fs::write(
            root.path().join(".timonier/intent.json"),
            timonier_core::intent::parse(
                r#"{"version": 1,
                    "components": [{"id": "C-1", "name": "A", "layer": null, "role": "Front"},
                                   {"id": "C-2", "name": "B", "layer": null, "role": ""},
                                   {"id": "C-3", "name": "Web", "layer": null, "role": ""}],
                    "edges": [{"id": "E-1", "src": "C-1", "dst": "C-2", "kind": "depends_on", "policy": "forbidden"}],
                    "sketch": {"shapes": {"C-1": {"x": 0, "y": 0, "width": 180, "height": 80},
                                          "C-2": {"x": 300, "y": 0, "width": 180, "height": 80}},
                               "strokes": [{"id": "S-1", "x": 10, "y": 100, "points": [[0, 0, 0.5], [50, 5, 0.5]]}]}}"#,
            )
            .unwrap()
            .to_text(),
        )
        .unwrap();
        let read = call(tools::GET_INTENDED_ARCHITECTURE, json!({}));
        let result = &read["structuredContent"]["result"];
        assert_eq!(result["components"][0]["role"], "Front");
        assert_eq!(result["places"]["strokes"][0]["near"], "C-1");
        assert_eq!(result["places"]["shapes"]["C-2"]["x"], 300.0);
        assert!(
            result.get("sketch").is_none(),
            "the points stay out: the image says them"
        );
        let text = read["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("3 components, 1 edge (1 forbidden, 0 required)"),
            "{text}"
        );
        assert_eq!(read["content"][1]["mimeType"], "image/png");
        assert!(
            read["content"][1]["data"]
                .as_str()
                .unwrap()
                .starts_with("iVBORw0KGgo"),
            "a PNG"
        );
        let plain = call(tools::GET_INTENDED_ARCHITECTURE, json!({"image": false}));
        assert_eq!(plain["content"].as_array().unwrap().len(), 1);
        assert_eq!(plain["structuredContent"]["result"]["image"], false);

        let drift = call(tools::GET_DRIFT, json!({}));
        let text = drift["content"][0]["text"].as_str().unwrap();
        assert_eq!(
            text,
            "Mailbox: 0 unread.\n3 drawn components: 2 found in the code, 1 missing (C-3 Web). \
             Forbidden edge E-1 crossed: C-1 A → C-2 B, 1 dependency."
        );
        let result = &drift["structuredContent"]["result"];
        assert_eq!(result["anchors"][0]["actual"], "pkg:cargo/a");
        assert_eq!(result["forbidden"][0]["dependencies"][0]["weight"], 3);

        // A file that does not read: refused, with the reason.
        std::fs::write(root.path().join(".timonier/intent.json"), "{").unwrap();
        let refused = call(tools::GET_DRIFT, json!({}));
        assert_eq!(refused["isError"], true);
        assert_eq!(
            refused["structuredContent"]["error"]["code"],
            "invalid_file"
        );
    }

    fn snapshot(seq: u64) -> Snapshot {
        Snapshot {
            seq,
            commit_sha: None,
            timestamp_ms: 0,
            kind: timonier_core::SnapshotKind::WorkingTree,
            frozen: true,
        }
    }

    /// The structured content of a result, and whether it is an error.
    fn call(tracker: &Tracker, name: &str, arguments: Value) -> (Value, bool) {
        let seq = std::cell::Cell::new(0);
        let freeze = || {
            seq.set(seq.get() + 1);
            Ok(snapshot(seq.get()))
        };
        // Between two snapshots, one file changed: `src/lib.rs`, said with its snapshots.
        let changed = |from: Option<u64>, to: u64| -> crate::Result<Vec<String>> {
            Ok(vec![format!("src/lib.rs ({}..{to})", from.unwrap_or(0))])
        };
        let result = tracker.call(
            name,
            Some(arguments),
            &context(&freeze, &changed, 1_792_484_100_000),
        );
        assert_eq!(
            result["structuredContent"]["mailbox"]["unread"], 0,
            "{result}"
        );
        (
            result["structuredContent"].clone(),
            result["isError"].as_bool().unwrap_or(false),
        )
    }

    fn code(tracker: &Tracker, name: &str, arguments: Value) -> String {
        let (content, error) = call(tracker, name, arguments);
        assert!(error, "{content}");
        content["error"]["code"].as_str().unwrap().to_owned()
    }

    #[test]
    fn a_session_keeps_the_plan_and_the_todo() {
        let root = tempfile::tempdir().unwrap();
        let tracker = Tracker::new(root.path());
        // Nothing written without a session.
        assert_eq!(
            code(
                &tracker,
                tools::PLAN_ADD_TASK,
                json!({"title": "x", "new_milestone": "M"})
            ),
            "no_open_session"
        );
        let (session, _) = call(
            &tracker,
            tools::SESSION_START,
            json!({"objective": "Add MCP"}),
        );
        assert_eq!(session["result"]["id"], "S-2026-10-20-1");
        assert_eq!(session["result"]["agent"], "test-agent");
        assert_eq!(
            code(
                &tracker,
                tools::SESSION_START,
                json!({"objective": "Again"})
            ),
            "session_already_open"
        );
        let (task, _) = call(
            &tracker,
            tools::PLAN_ADD_TASK,
            json!({"title": "Write the proxy", "new_milestone": "MCP"}),
        );
        assert_eq!(task["result"]["id"], "T-1");
        call(
            &tracker,
            tools::PLAN_UPDATE_TASK,
            json!({"task_id": "T-1", "status": "in_progress"}),
        );
        assert_eq!(
            code(
                &tracker,
                tools::PLAN_UPDATE_TASK,
                json!({"task_id": "T-1", "status": "todo"})
            ),
            "forbidden_transition"
        );
        call(
            &tracker,
            tools::TODO_ADD,
            json!({"text": "Map tools/list", "task": "T-1"}),
        );
        call(&tracker, tools::TODO_COMPLETE, json!({"item_id": "D-1"}));
        call(&tracker, tools::TODO_ADD, json!({"text": "Map tools/call"}));
        let (plan, _) = call(&tracker, tools::GET_PLAN, json!({}));
        assert_eq!(
            plan["result"]["milestones"][0]["progress"],
            json!({"done": 0, "of": 1})
        );
        let (ended, _) = call(
            &tracker,
            tools::SESSION_END,
            json!({"summary": "Proxy half done."}),
        );
        assert_eq!(ended["result"]["status"], "ended");
        assert_eq!(ended["result"]["tasks"], json!(["T-1"]));
        assert_eq!(ended["result"]["todo"].as_array().unwrap().len(), 2);
        assert!(ended["result"]["snapshots"]["end"].is_u64());
        // ADR 0031: what it changed in the code, in its journal.
        assert_eq!(ended["result"]["changes"]["files"], 1);
        // The files: versioned, readable, the todo empty between sessions.
        let todo: Todo = read(&root.path().join(".timonier/todo.json")).unwrap();
        assert_eq!(todo, Todo::default());
        let journal = root.path().join(".timonier/sessions/S-2026-10-20-1.json");
        assert!(
            std::fs::read_to_string(journal)
                .unwrap()
                .contains("\"summary\": \"Proxy half done.\"")
        );
        // The next session of the day gets the next number.
        let (next, _) = call(
            &tracker,
            tools::SESSION_START,
            json!({"objective": "Finish"}),
        );
        assert_eq!(next["result"]["id"], "S-2026-10-20-2");
        // What changed before each session, none open, is kept as untracked (6.4): from the
        // first snapshot, then from the end of the session before.
        let untracked = tracker.untracked().unwrap();
        assert_eq!(untracked.gaps.len(), 2);
        assert_eq!(untracked.gaps[0].files, ["src/lib.rs (0..1)"]);
        assert_eq!(untracked.gaps[0].until.session, "S-2026-10-20-1");
        assert_eq!(untracked.gaps[1].from, 1);
        assert_eq!(untracked.gaps[1].until.session, "S-2026-10-20-2");
    }

    #[test]
    fn invalid_calls_are_refused_with_what_to_do() {
        let root = tempfile::tempdir().unwrap();
        let tracker = Tracker::new(root.path());
        assert_eq!(code(&tracker, "rm_rf", json!({})), "unknown_tool");
        assert_eq!(
            code(
                &tracker,
                tools::SESSION_START,
                json!({"objective": "x", "why": 1})
            ),
            "invalid_argument"
        );
        assert_eq!(
            code(&tracker, tools::SESSION_START, json!({"objective": "  "})),
            "invalid_argument"
        );
        assert_eq!(
            code(
                &tracker,
                tools::SESSION_START,
                json!({"objective": "x", "task": "T-4"})
            ),
            "unknown_task"
        );
        assert_eq!(
            code(&tracker, tools::SESSION_END, json!({"summary": "x"})),
            "no_open_session"
        );
        assert_eq!(
            code(&tracker, tools::GET_SESSION, json!({})),
            "no_open_session"
        );
        assert_eq!(
            code(
                &tracker,
                tools::GET_SESSION,
                json!({"session_id": "S-2026-10-20-9"})
            ),
            "unknown_session"
        );
    }

    #[test]
    fn a_file_broken_outside_refuses_the_writes_and_says_so() {
        let root = tempfile::tempdir().unwrap();
        let tracker = Tracker::new(root.path());
        call(&tracker, tools::SESSION_START, json!({"objective": "x"}));
        std::fs::write(root.path().join(".timonier/plan.json"), "{ not json").unwrap();
        assert_eq!(
            code(
                &tracker,
                tools::PLAN_ADD_TASK,
                json!({"title": "x", "new_milestone": "M"})
            ),
            "invalid_file"
        );
        // A valid change by hand is taken as it is.
        let mut plan = Plan::default();
        plan.milestones.push(timonier_core::tracking::Milestone {
            id: "M-4".into(),
            title: "By hand".into(),
            tasks: Vec::new(),
        });
        std::fs::write(
            root.path().join(".timonier/plan.json"),
            serde_json::to_string(&plan).unwrap(),
        )
        .unwrap();
        let (task, _) = call(
            &tracker,
            tools::PLAN_ADD_TASK,
            json!({"title": "x", "milestone": "M-4"}),
        );
        assert_eq!(task["result"]["id"], "T-1");
    }

    #[test]
    fn session_ids_are_checked() {
        assert!(is_session_id("S-2026-10-20-1"));
        assert!(is_session_id("S-2026-10-20-12"));
        for bad in [
            "S-2026-10-20-0",
            "S-2026-10-20",
            "S-26-10-20-1",
            "T-1",
            "S-2026-10-20-1/..",
            "S-2026-1x-20-1",
        ] {
            assert!(!is_session_id(bad), "{bad}");
        }
    }
}
