//! The MCP tools (ADR 0027, 0028): their catalogue, as MCP describes a tool, and the shape of
//! their results, the mailbox header first. Pure: the daemon runs them, the MCP server relays.

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::Serialize;
use serde_json::{Value, json};

use crate::budgets::GetPerf;
use crate::gaps::{GetInstrumentationConventions, GetInstrumentationGaps};
use crate::tracking::{
    GetSession, NoArguments, PlanAddTask, PlanUpdateTask, Refusal, SessionEnd, SessionStart,
    TodoAdd, TodoComplete,
};

pub const SESSION_START: &str = "session_start";
pub const SESSION_END: &str = "session_end";
pub const PLAN_ADD_TASK: &str = "plan_add_task";
pub const PLAN_UPDATE_TASK: &str = "plan_update_task";
pub const TODO_ADD: &str = "todo_add";
pub const TODO_COMPLETE: &str = "todo_complete";
pub const GET_PLAN: &str = "get_plan";
pub const GET_TODO: &str = "get_todo";
pub const GET_SESSION: &str = "get_session";
pub const GET_INTENDED_ARCHITECTURE: &str = "get_intended_architecture";
pub const GET_DRIFT: &str = "get_drift";
pub const GET_INSTRUMENTATION_GAPS: &str = "get_instrumentation_gaps";
pub const GET_INSTRUMENTATION_CONVENTIONS: &str = "get_instrumentation_conventions";
pub const GET_BUDGETS: &str = "get_budgets";
pub const GET_BUDGET_VIOLATIONS: &str = "get_budget_violations";
pub const GET_PERF: &str = "get_perf";
pub const GET_REGRESSIONS: &str = "get_regressions";

/// `get_intended_architecture` (ADR 0041): the image of the sketch comes unless declined.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetIntendedArchitecture {
    /// `false`: no image of the sketch, the structure and the places only.
    #[serde(default)]
    pub image: Option<bool>,
}

/// The schema of a tool's input: strict, its subschemas inline, without `$schema`.
fn input_schema<T: JsonSchema>() -> Value {
    let schema = SchemaSettings::draft2020_12()
        .with(|settings| {
            settings.inline_subschemas = true;
            settings.meta_schema = None;
        })
        .into_generator()
        .into_root_schema_for::<T>();
    let mut value = serde_json::to_value(schema).unwrap_or(Value::Null);
    if let Some(object) = value.as_object_mut() {
        object.remove("title");
        object.remove("description");
    }
    value
}

/// One tool, as MCP lists it.
fn tool<T: JsonSchema>(name: &str, description: &str, read_only: bool) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema::<T>(),
        "annotations": {
            "readOnlyHint": read_only,
            "destructiveHint": false,
            "openWorldHint": false,
        },
    })
}

/// Every tool, sorted by name: what `tools/list` answers.
pub fn catalogue() -> Vec<Value> {
    let mut tools = vec![
        tool::<SessionStart>(
            SESSION_START,
            "Open a work session before changing the code: give its objective, and the plan's \
             task it serves if any. Writes to the plan and the todo need an open session. Returns \
             the session.",
            false,
        ),
        tool::<SessionEnd>(
            SESSION_END,
            "Close the open session at the end of the work, with a summary of what was done and \
             what is left. Returns the session as kept in the journal.",
            false,
        ),
        tool::<PlanAddTask>(
            PLAN_ADD_TASK,
            "Add a task to the plan, in an existing milestone or a new one, with its done \
             criterion. Returns the task and its id (T-…).",
            false,
        ),
        tool::<PlanUpdateTask>(
            PLAN_UPDATE_TASK,
            "Change a task of the plan: its status (todo, in_progress, blocked, done, dropped; \
             only allowed transitions), title, criterion, or add a note. Returns the task.",
            false,
        ),
        tool::<TodoAdd>(
            TODO_ADD,
            "Add an item to the todo of the open session, when splitting the work; link it to \
             a task of the plan if any. Returns the item and its id (D-…).",
            false,
        ),
        tool::<TodoComplete>(
            TODO_COMPLETE,
            "Mark an item of the todo as done, as soon as it is. Returns the item.",
            false,
        ),
        tool::<NoArguments>(
            GET_PLAN,
            "Read the plan: milestones, their tasks, statuses and progress.",
            true,
        ),
        tool::<NoArguments>(GET_TODO, "Read the todo of the open session.", true),
        tool::<GetSession>(
            GET_SESSION,
            "Read a session of the journal: the open one, or the one given.",
            true,
        ),
        tool::<GetIntendedArchitecture>(
            GET_INTENDED_ARCHITECTURE,
            "Read the architecture the developer wants, from their whiteboard: layers, intended \
             components (name, layer, role), edges with their policy (allowed, forbidden, \
             required; between layers they hold for all their content), notes and performance \
             budgets, where each shape and freehand stroke is drawn, and an image of the sketch \
             for the freehand drawing. Read it before designing or changing the structure.",
            true,
        ),
        tool::<NoArguments>(
            GET_DRIFT,
            "Compare the code with the developer's whiteboard: each drawn component and the real \
             package or component it stands for (by name, then place), the drawn components the \
             code lacks, the forbidden edges the code crosses with the dependencies that cross \
             them, and the required edges it lacks. Check it after changing the structure.",
            true,
        ),
        tool::<GetInstrumentationGaps>(
            GET_INSTRUMENTATION_GAPS,
            "Check the instrumentation after a run of `timonier test`: the public functions no \
             span was seen in (paged), the spans that lack code.function.name, code.file.path or \
             code.line.number, those attached to no node of the graph, and the overhead of the \
             traces against runs without them. The last finished run with traces unless one is \
             given, `timonier doctor`'s smoke tests aside.",
            true,
        ),
        tool::<GetInstrumentationConventions>(
            GET_INSTRUMENTATION_CONVENTIONS,
            "Read how this project instruments a language with OpenTelemetry: the SDK packages, \
             the initialisation module, one span per public function with the code.* \
             attributes, no span in hot loops. Read it before instrumenting code.",
            true,
        ),
        tool::<NoArguments>(
            GET_BUDGETS,
            "Read the performance budgets the developer set on the whiteboard (latency p50 and p95 \
             of each incoming call, peak memory, allocations, calls), each checked against the \
             median of the last runs of `timonier test`: ok, exceeded, unmeasured, or attached to \
             nothing the code has.",
            true,
        ),
        tool::<NoArguments>(
            GET_BUDGET_VIOLATIONS,
            "List the whiteboard's performance budgets the last runs exceed, with the limit and \
             what was measured. Check it after a change, before saying the work is done.",
            true,
        ),
        tool::<crate::perf::RegressionParams>(
            GET_REGRESSIONS,
            "Read the performance regressions of the last batch of runs (`timonier test --runs \
             auto`) against the main branch: each measure (the run's CPU work, a test's duration, \
             a function's own time) slower beyond doubt (Mann-Whitney p < 0.01) and by at least \
             max(5 %, 3 MAD), probable when seen once, confirmed when the next batch sees it too.",
            true,
        ),
        tool::<GetPerf>(
            GET_PERF,
            "Read what a component cost in a run of `timonier test`: its incoming calls and their \
             latency (p50, p95), the peak memory during them, the time spent in it, what inside \
             took the most time, the tests that reached it, and its budgets.",
            true,
        ),
    ];
    tools.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    tools
}

/// How urgent the most urgent unread message is (ADR 0028).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Normal,
    High,
    Blocking,
}

/// The mailbox header every result carries (ADR 0028). Empty until the messages of v2 (H10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Mailbox {
    pub unread: u32,
    pub priority: Option<Priority>,
}

impl Mailbox {
    /// Its line, first in the text of every result.
    pub fn line(&self) -> String {
        match self.priority {
            Some(priority) if self.unread > 0 => format!(
                "Mailbox: {} unread, highest priority {}.",
                self.unread,
                serde_json::to_value(priority)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default()
            ),
            _ => format!("Mailbox: {} unread.", self.unread),
        }
    }
}

/// A result, as MCP's `CallToolResult`: the header and the summary in the text, the header and
/// the result in the structured content.
pub fn success(mailbox: Mailbox, summary: &str, result: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": format!("{}\n{summary}", mailbox.line()) }],
        "structuredContent": { "mailbox": mailbox, "result": result },
        "isError": false,
    })
}

/// A result with an image after its text (an MCP image content), for the agent's vision.
pub fn with_image(mut result: Value, png: &[u8]) -> Value {
    if let Some(content) = result["content"].as_array_mut() {
        content.push(json!({ "type": "image", "data": base64(png), "mimeType": "image/png" }));
    }
    result
}

/// Standard base64 with padding (RFC 4648, § 4), as MCP carries images.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> (18 - 6 * i)) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A refused call: the header, a stable code and what to do.
pub fn failure(mailbox: Mailbox, refusal: &Refusal) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": format!("{}\nError ({}): {}", mailbox.line(), refusal.code, refusal.message),
        }],
        "structuredContent": {
            "mailbox": mailbox,
            "error": { "code": refusal.code, "message": refusal.message },
        },
        "isError": true,
    })
}

/// The arguments of a call, as the tool's input type; refused with what is wrong.
pub fn arguments<T: serde::de::DeserializeOwned>(arguments: Option<Value>) -> Result<T, Refusal> {
    serde_json::from_value(arguments.unwrap_or_else(|| json!({}))).map_err(|error| {
        Refusal::new(
            "invalid_argument",
            format!("the arguments do not match the tool's schema: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The contract with the agents: a change of a name, a description or a schema shows here
    /// (ADR 0027). Update with `TIMONIER_UPDATE_TOOLS=1 cargo test -p timonier-core`.
    #[test]
    fn the_catalogue_is_the_contract() {
        let current = serde_json::to_string_pretty(&catalogue()).unwrap() + "\n";
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tools.json");
        if std::env::var_os("TIMONIER_UPDATE_TOOLS").is_some() {
            std::fs::write(&path, &current).unwrap();
        }
        let kept = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            kept == current,
            "the tools changed: check the change, then TIMONIER_UPDATE_TOOLS=1 cargo test -p timonier-core"
        );
    }

    #[test]
    fn every_schema_is_strict_and_inline() {
        for tool in catalogue() {
            let schema = &tool["inputSchema"];
            assert_eq!(schema["type"], "object", "{}", tool["name"]);
            assert_eq!(schema["additionalProperties"], false, "{}", tool["name"]);
            let text = schema.to_string();
            assert!(
                !text.contains("$ref") && !text.contains("$schema"),
                "{text}"
            );
        }
    }

    #[test]
    fn every_result_carries_the_header() {
        let mailbox = Mailbox::default();
        let ok = success(mailbox, "Task T-1 added.", json!({"id": "T-1"}));
        assert_eq!(
            ok["structuredContent"]["mailbox"],
            json!({"unread": 0, "priority": null})
        );
        assert_eq!(
            ok["content"][0]["text"],
            "Mailbox: 0 unread.\nTask T-1 added."
        );
        let refused = failure(
            mailbox,
            &Refusal::new("no_open_session", "call session_start"),
        );
        assert_eq!(refused["isError"], true);
        assert_eq!(
            refused["structuredContent"]["error"]["code"],
            "no_open_session"
        );
        assert!(
            refused["content"][0]["text"]
                .as_str()
                .unwrap()
                .starts_with("Mailbox: 0 unread.\n")
        );
        let urgent = Mailbox {
            unread: 2,
            priority: Some(Priority::High),
        };
        assert_eq!(urgent.line(), "Mailbox: 2 unread, highest priority high.");
    }

    #[test]
    fn images_go_after_the_text_in_base64() {
        // RFC 4648, § 10.
        for (bytes, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(bytes.as_bytes()), encoded);
        }
        let result = with_image(
            success(Mailbox::default(), "Drawn.", json!({})),
            &[0xff, 0xd8],
        );
        assert_eq!(
            result["content"][1],
            json!({"type": "image", "data": "/9g=", "mimeType": "image/png"})
        );
        assert_eq!(result["content"][0]["type"], "text");
    }

    #[test]
    fn wrong_arguments_are_refused_with_the_reason() {
        let refused = arguments::<TodoAdd>(Some(json!({"text": "x", "due": 3}))).unwrap_err();
        assert_eq!(refused.code, "invalid_argument");
        assert!(refused.message.contains("due"), "{}", refused.message);
        assert!(arguments::<NoArguments>(None).is_ok());
    }
}
