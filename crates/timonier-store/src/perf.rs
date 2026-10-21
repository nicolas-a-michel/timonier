//! The measurements of the runs of tests (ADR 0043): their own database, apart from the graph,
//! with the same single writer and read-only readers (ADR 0011). A run is created when it
//! begins and completed when it ends (ADR 0047).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::{Map, Value};
use timonier_core::gaps::{RunCost, SpanFacts, WithoutCode};
use timonier_core::perf::{
    Accounting, Attachment, CodeLocation, CpuSource, Measured, ReceivedMetric, ReceivedSpan,
    RunRecord, RunStart, RunSummary, SpanStatus, TestCase, TestStatus,
};

use crate::writer::Writer;
use crate::{Error, Result, schema};

/// What each node cost in the runs `older` selects (an `id` query with `?1` = [`KEPT_RUNS`])
/// whose spans are still there, kept before retention drops them (ADR 0043).
fn keep_aggregates(connection: &Connection, older: &str) -> rusqlite::Result<()> {
    let mut runs = connection.prepare(&format!(
        "SELECT id FROM run WHERE id IN ({older})
             AND EXISTS (SELECT 1 FROM span s WHERE s.run = run.id)"
    ))?;
    let ids: Vec<u64> = runs
        .query_map([KEPT_RUNS], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut spans = connection.prepare(
        "SELECT span_id, parent_span_id, node, duration_ns FROM span
         WHERE run = ?1 AND synthesized = 0",
    )?;
    let mut insert = connection.prepare(
        "INSERT OR REPLACE INTO aggregate (run, node, calls, self_ns, entered_ns)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for id in ids {
        let rows: Vec<(String, Option<String>, Option<String>, u64)> = spans
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let called: Vec<timonier_core::perf::Called<'_>> = rows
            .iter()
            .map(
                |(span, parent, node, duration_ns)| timonier_core::perf::Called {
                    id: span,
                    parent: parent.as_deref(),
                    node: node.as_deref(),
                    duration_ns: *duration_ns,
                },
            )
            .collect();
        for node in timonier_core::perf::map_of(&called).0 {
            insert.execute(params![
                id,
                node.node,
                node.calls,
                node.self_ns,
                node.entered_ns
            ])?;
        }
    }
    Ok(())
}

/// Where a run sits in the history: the graph's head snapshot and git's view of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunPlace {
    pub snapshot: u64,
    pub commit: Option<String>,
    pub branch: Option<String>,
    /// The fingerprint of the working tree's changes, `None` when it is the commit (Q77).
    pub worktree: Option<String>,
}

/// A run as kept: `record` and `fingerprint` once it finished.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredRun {
    pub id: u64,
    pub command: Vec<String>,
    pub started_at_ms: u64,
    pub place: RunPlace,
    pub fingerprint: Option<String>,
    pub record: Option<RunRecord>,
    /// Spans counted beyond the cap of a run, not kept.
    pub spans_dropped: u64,
}

/// A span of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    /// The test it belongs to: its place in the JUnit XML, once known.
    pub test: Option<u64>,
    /// The test it named (`timonier.test.name`, `.classname`).
    pub test_name: Option<String>,
    pub test_classname: Option<String>,
    pub name: String,
    /// Nanoseconds since the Unix epoch; `None` when unknown.
    pub start_ns: Option<u64>,
    pub duration_ns: u64,
    pub status: SpanStatus,
    pub synthesized: bool,
    pub code: CodeLocation,
    /// The node of the graph it is attached to, and how (ADR 0048).
    pub node: Option<String>,
    pub attached_by: Option<Attachment>,
    pub attributes: Map<String, Value>,
}

/// A span the receiver took, with the node the daemon found for it.
#[derive(Clone, Debug, PartialEq)]
pub struct KeptSpan {
    pub span: ReceivedSpan,
    pub node: Option<String>,
    pub attached_by: Option<Attachment>,
}

/// What [`PerfStore::add_spans`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpansAdded {
    pub kept: usize,
    /// Beyond the cap of the run: counted in it, not kept.
    pub dropped: usize,
}

/// The spans one run keeps at most (ADR 0043).
pub const SPANS_PER_RUN: usize = 200_000;
/// The runs whose spans and metrics are kept; older ones keep their figures only.
pub const KEPT_RUNS: u64 = 50;
/// How long after its end a run still takes spans: exporters empty their queues on exit.
pub const LATE_MS: u64 = 60_000;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The measurements of one project. Cheap to clone; every clone shares the same writer thread.
#[derive(Clone)]
pub struct PerfStore {
    inner: Arc<Inner>,
}

struct Inner {
    path: PathBuf,
    writer: Writer,
}

impl PerfStore {
    /// Opens (or creates) the database at `path` and brings its schema up to date.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let opened =
            schema::open_writer_with(&path, &schema::PERF_MIGRATIONS).and_then(Writer::spawn);
        let writer = opened.map_err(|source| Error::Open {
            path: path.clone(),
            source: Box::new(source),
        })?;
        Ok(Self {
            inner: Arc::new(Inner { path, writer }),
        })
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    /// A run begins; returns its number.
    #[tracing::instrument(skip_all)]
    pub fn start(&self, start: RunStart, place: RunPlace) -> Result<u64> {
        self.inner.writer.run(move |connection| {
            connection.execute(
                "INSERT INTO run (started_at_ms, command, snapshot, commit_sha, branch, traced, batch,
                     worktree, smoke)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    start.started_at_ms,
                    json(&start.command),
                    place.snapshot,
                    place.commit,
                    place.branch,
                    start.traced,
                    start.batch,
                    place.worktree,
                    start.smoke
                ],
            )?;
            let id = u64::try_from(connection.last_insert_rowid()).unwrap_or_default();
            // Older runs keep their figures and what each node cost, not their spans and
            // metrics (ADR 0043).
            let older = "SELECT id FROM run ORDER BY id DESC LIMIT -1 OFFSET ?1";
            keep_aggregates(connection, older)?;
            connection.execute(
                &format!("DELETE FROM span WHERE run IN ({older})"),
                [KEPT_RUNS],
            )?;
            connection.execute(
                &format!("DELETE FROM metric WHERE run IN ({older})"),
                [KEPT_RUNS],
            )?;
            Ok(id)
        })
    }

    /// What each node cost in run `id` once its spans are gone (ADR 0043); empty while they are
    /// kept: [`Self::spans`] says it then.
    pub fn aggregates(&self, id: u64) -> Result<Vec<timonier_core::perf::NodePerf>> {
        let connection = schema::open_reader(&self.inner.path)?;
        let mut statement = connection.prepare(
            "SELECT node, calls, self_ns, entered_ns FROM aggregate WHERE run = ?1 ORDER BY node",
        )?;
        let rows = statement.query_map([id], |r| {
            Ok(timonier_core::perf::NodePerf {
                node: r.get(0)?,
                calls: r.get(1)?,
                self_ns: r.get(2)?,
                entered_ns: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Whether run `id` takes spans now: begun with traces, and not ended more than [`LATE_MS`]
    /// ago.
    pub fn accepts(&self, id: u64) -> Result<bool> {
        let connection = schema::open_reader(&self.inner.path)?;
        // A run without traces takes none: its cost is the reference of the overhead.
        let row: Option<(bool, Option<u64>)> = connection
            .query_row(
                "SELECT finished, finished_at_ms FROM run WHERE id = ?1 AND traced = 1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            None => false,
            Some((false, _)) => true,
            Some((true, at)) => at.is_some_and(|at| now_ms() <= at.saturating_add(LATE_MS)),
        })
    }

    /// Keeps the spans the receiver took for run `id`, up to the cap of the run; links them to
    /// their test if the run has ended (ADR 0048).
    #[tracing::instrument(skip_all)]
    pub fn add_spans(&self, id: u64, spans: Vec<KeptSpan>) -> Result<SpansAdded> {
        self.inner
            .writer
            .run(move |connection| add_spans(connection, id, spans, SPANS_PER_RUN))
    }

    /// Keeps the metric points the receiver took for run `id`.
    #[tracing::instrument(skip_all)]
    pub fn add_metrics(&self, id: u64, metrics: Vec<ReceivedMetric>) -> Result<usize> {
        self.inner.writer.run(move |connection| {
            let tx = connection.transaction()?;
            {
                let mut insert = tx.prepare(
                    "INSERT INTO metric (run, name, kind, time_ns, value, count, sum, attributes)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )?;
                for m in &metrics {
                    insert.execute(params![
                        id,
                        m.name,
                        word(&m.kind),
                        m.time_ns,
                        m.value,
                        m.count,
                        m.sum,
                        json(&m.attributes)
                    ])?;
                }
            }
            tx.commit()?;
            Ok(metrics.len())
        })
    }

    /// What was measured of run `id`, its tests, and the root span of each test (ADR 0047), in
    /// one transaction. Returns the number of root spans made.
    #[tracing::instrument(skip_all)]
    pub fn finish(&self, id: u64, record: RunRecord) -> Result<usize> {
        self.inner
            .writer
            .run(move |connection| finish(connection, id, &record))
    }

    /// A run, or `None` if there is no such run.
    pub fn run(&self, id: u64) -> Result<Option<StoredRun>> {
        let connection = schema::open_reader(&self.inner.path)?;
        load(&connection, id)
    }

    /// The last `limit` runs, the newest first, as the views list them (ADR 0051).
    pub fn summaries(&self, limit: u32) -> Result<Vec<RunSummary>> {
        let connection = schema::open_reader(&self.inner.path)?;
        let mut statement = connection.prepare(
            "SELECT r.id, r.started_at_ms, r.command, r.finished, r.traced, r.snapshot,
                 r.commit_sha, r.branch, r.exit_code, r.wall_us,
                 r.user_cpu_us + r.system_cpu_us, r.instructions, r.cpu_source,
                 coalesce(r.tree_peak_bytes, r.largest_process_bytes),
                 r.tree_peak_bytes IS NOT NULL,
                 (SELECT count(*) FROM test t WHERE t.run = r.id),
                 (SELECT count(*) FROM test t WHERE t.run = r.id
                    AND t.status IN ('failed', 'error')),
                 (SELECT count(*) FROM span s WHERE s.run = r.id AND s.synthesized = 0),
                 r.batch, r.fingerprint, r.worktree, r.smoke
             FROM run r ORDER BY r.id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |r| {
            Ok((
                RunSummary {
                    run: format!("R-{}", r.get::<_, u64>(0)?),
                    started_at_ms: r.get(1)?,
                    command: Vec::new(),
                    finished: r.get(3)?,
                    traced: r.get(4)?,
                    snapshot: r.get(5)?,
                    commit: r.get(6)?,
                    branch: r.get(7)?,
                    exit_code: r.get(8)?,
                    wall_us: r.get(9)?,
                    cpu_us: r.get(10)?,
                    instructions: r.get(11)?,
                    cpu_source: None,
                    memory_bytes: r.get(13)?,
                    memory_whole_tree: r.get(14)?,
                    tests: r.get(15)?,
                    failed: r.get(16)?,
                    spans: r.get(17)?,
                    batch: r.get(18)?,
                    fingerprint: r.get(19)?,
                    worktree: r.get(20)?,
                    smoke: r.get(21)?,
                },
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(12)?,
            ))
        })?;
        let mut runs = Vec::new();
        for row in rows {
            let (mut summary, command, source) = row?;
            summary.command = parse("run.command", &command, "a JSON array of words")?;
            summary.cpu_source = source
                .map(|s| parse("run.cpu_source", &format!("\"{s}\""), "a CPU source"))
                .transpose()?;
            runs.push(summary);
        }
        Ok(runs)
    }

    /// The last run with traces that finished, if any, the smoke tests aside: what the gaps judge
    /// (ADR 0049) and the views show by default.
    pub fn latest_traced(&self) -> Result<Option<u64>> {
        let connection = schema::open_reader(&self.inner.path)?;
        Ok(connection.query_row(
            "SELECT max(id) FROM run WHERE finished = 1 AND traced = 1 AND smoke = 0",
            [],
            |r| r.get::<_, Option<u64>>(0),
        )?)
    }

    /// What the spans of run `id` say of its instrumentation (ADR 0049).
    pub fn span_facts(&self, id: u64) -> Result<SpanFacts> {
        let connection = schema::open_reader(&self.inner.path)?;
        let (spans, unattached): (u64, u64) = connection.query_row(
            "SELECT count(*), count(*) FILTER (WHERE node IS NULL) FROM span
             WHERE run = ?1 AND synthesized = 0",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let dropped: u64 = connection
            .query_row("SELECT spans_dropped FROM run WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?
            .unwrap_or(0);
        let mut seen = std::collections::BTreeSet::new();
        let mut statement = connection.prepare(
            "SELECT DISTINCT node FROM span
             WHERE run = ?1 AND node IS NOT NULL AND attached_by IN ('id', 'name', 'line')",
        )?;
        for node in statement.query_map([id], |r| r.get::<_, String>(0))? {
            seen.insert(node?);
        }
        let mut without_code = Vec::new();
        let mut statement = connection.prepare(
            "SELECT name, code_function IS NULL, code_file IS NULL, code_line IS NULL, count(*)
             FROM span
             WHERE run = ?1 AND synthesized = 0
               AND (code_function IS NULL OR code_file IS NULL OR code_line IS NULL)
             GROUP BY 1, 2, 3, 4 ORDER BY 5 DESC, 1",
        )?;
        let rows = statement.query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                [
                    r.get::<_, bool>(1)?,
                    r.get::<_, bool>(2)?,
                    r.get::<_, bool>(3)?,
                ],
                r.get::<_, u64>(4)?,
            ))
        })?;
        for row in rows {
            let (name, missing, spans) = row?;
            let names = ["code.function.name", "code.file.path", "code.line.number"];
            without_code.push(WithoutCode {
                name,
                spans,
                missing: names
                    .iter()
                    .zip(missing)
                    .filter(|(_, missing)| *missing)
                    .map(|(name, _)| (*name).to_owned())
                    .collect(),
            });
        }
        Ok(SpanFacts {
            spans,
            seen,
            without_code,
            unattached,
            dropped,
        })
    }

    /// The finished runs comparable with run `id` (same commit, command and machine
    /// fingerprint, so the same CPU source), and what each cost (ADR 0049).
    pub fn comparable_costs(&self, id: u64) -> Result<Option<(CpuSource, Vec<RunCost>)>> {
        let connection = schema::open_reader(&self.inner.path)?;
        let source: Option<Option<String>> = connection
            .query_row("SELECT cpu_source FROM run WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        let Some(Some(source)) = source else {
            return Ok(None);
        };
        let source: CpuSource = parse("run.cpu_source", &format!("\"{source}\""), "a CPU source")?;
        let mut statement = connection.prepare(
            "SELECT o.id, o.traced, o.instructions, o.user_cpu_us + o.system_cpu_us
             FROM run r JOIN run o
               ON o.commit_sha IS r.commit_sha AND o.command = r.command
              AND o.fingerprint = r.fingerprint AND o.finished = 1
             WHERE r.id = ?1 ORDER BY o.id",
        )?;
        let rows = statement.query_map([id], |r| {
            Ok((
                r.get::<_, u64>(0)?,
                r.get::<_, bool>(1)?,
                r.get::<_, Option<u64>>(2)?,
                r.get::<_, u64>(3)?,
            ))
        })?;
        let mut costs = Vec::new();
        for row in rows {
            let (other, traced, instructions, cpu_us) = row?;
            #[allow(clippy::cast_precision_loss)]
            let cost = match source {
                CpuSource::Instructions => instructions.unwrap_or(0) as f64,
                CpuSource::CpuTime => cpu_us as f64,
            };
            costs.push(RunCost {
                run: format!("R-{other}"),
                traced,
                cost,
            });
        }
        Ok(Some((source, costs)))
    }

    /// The spans of run `id`, by trace then span.
    pub fn spans(&self, id: u64) -> Result<Vec<StoredSpan>> {
        let connection = schema::open_reader(&self.inner.path)?;
        let mut statement = connection.prepare(
            "SELECT trace_id, span_id, parent_span_id, test, test_name, test_classname, name,
                 start_ns, duration_ns, status, synthesized, code_function, code_function_derived,
                 code_file, code_line, node, attached_by, attributes
             FROM span WHERE run = ?1 ORDER BY trace_id, span_id",
        )?;
        let rows = statement.query_map([id], |r| {
            Ok((
                StoredSpan {
                    trace_id: r.get(0)?,
                    span_id: r.get(1)?,
                    parent_span_id: r.get(2)?,
                    test: r.get(3)?,
                    test_name: r.get(4)?,
                    test_classname: r.get(5)?,
                    name: r.get(6)?,
                    start_ns: r.get(7)?,
                    duration_ns: r.get(8)?,
                    status: SpanStatus::Unset,
                    synthesized: r.get(10)?,
                    code: CodeLocation {
                        function: r.get(11)?,
                        function_derived: r.get(12)?,
                        file: r.get(13)?,
                        line: r.get(14)?,
                    },
                    node: r.get(15)?,
                    attached_by: None,
                    attributes: Map::new(),
                },
                r.get::<_, String>(9)?,
                r.get::<_, Option<String>>(16)?,
                r.get::<_, String>(17)?,
            ))
        })?;
        let mut spans = Vec::new();
        for row in rows {
            let (mut span, status, attached_by, attributes) = row?;
            span.status = parse("span.status", &format!("\"{status}\""), "a span status")?;
            span.attached_by = attached_by
                .map(|a| parse("span.attached_by", &format!("\"{a}\""), "an attachment"))
                .transpose()?;
            span.attributes = parse("span.attributes", &attributes, "a JSON object")?;
            spans.push(span);
        }
        Ok(spans)
    }
}

fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// The name a closed set of values takes in its column, its serde name.
fn word<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(word)) => word,
        _ => String::new(),
    }
}

fn parse<T: serde::de::DeserializeOwned>(
    column: &'static str,
    text: &str,
    expected: &'static str,
) -> Result<T> {
    serde_json::from_str(text).map_err(|_| Error::Corrupt {
        column,
        value: text.to_owned(),
        expected,
    })
}

/// The root span of test `ord` of run `run` (ADR 0047): ids that no exporter makes (a trace id
/// of the run's number and the test's, a span id of the test's).
fn test_root(run: u64, ord: u64, case: &TestCase) -> StoredSpan {
    let mut attributes = Map::new();
    attributes.insert("timonier.test.status".into(), word(&case.status).into());
    attributes.insert("timonier.synthesized".into(), true.into());
    StoredSpan {
        trace_id: format!("{run:016x}{:016x}", ord + 1),
        span_id: format!("{:016x}", ord + 1),
        parent_span_id: None,
        test: Some(ord),
        test_name: Some(case.name.clone()),
        test_classname: case.classname.clone(),
        name: case.name.clone(),
        start_ns: case.started_at_us.map(|us| us.saturating_mul(1000)),
        duration_ns: case.duration_us.saturating_mul(1000),
        status: match case.status {
            TestStatus::Failed | TestStatus::Error => SpanStatus::Error,
            TestStatus::Passed | TestStatus::Skipped => SpanStatus::Unset,
        },
        synthesized: true,
        code: CodeLocation {
            function: None,
            function_derived: false,
            file: case.file.clone(),
            line: case.line,
        },
        node: None,
        attached_by: None,
        attributes,
    }
}

const INSERT_SPAN: &str = "INSERT OR IGNORE INTO span (run, trace_id, span_id, parent_span_id,
        test, test_name, test_classname, name, start_ns, duration_ns, status, synthesized,
        code_function, code_function_derived, code_file, code_line, node, attached_by,
        attributes)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)";

/// Inserts `span` unless the run has it already (an exporter that sent it twice); `true` if
/// it was new.
fn insert_span(
    statement: &mut rusqlite::Statement<'_>,
    run: u64,
    span: &StoredSpan,
) -> Result<bool> {
    let inserted = statement.execute(params![
        run,
        span.trace_id,
        span.span_id,
        span.parent_span_id,
        span.test,
        span.test_name,
        span.test_classname,
        span.name,
        span.start_ns,
        span.duration_ns,
        word(&span.status),
        span.synthesized,
        span.code.function,
        span.code.function_derived,
        span.code.file,
        span.code.line,
        span.node,
        span.attached_by.as_ref().map(word),
        json(&span.attributes),
    ])?;
    Ok(inserted > 0)
}

/// The test of each span that names one, by its name and class (ADR 0048), once the run's tests
/// are known.
fn link(connection: &Connection, run: u64) -> Result<()> {
    connection.execute(
        "UPDATE span SET test = (
             SELECT t.ord FROM test t
             WHERE t.run = span.run AND t.name = span.test_name
               AND (span.test_classname IS NULL OR t.classname IS NULL
                    OR t.classname = span.test_classname)
             ORDER BY t.ord LIMIT 1)
         WHERE run = ?1 AND synthesized = 0 AND test IS NULL AND test_name IS NOT NULL",
        [run],
    )?;
    Ok(())
}

fn add_spans(
    connection: &mut Connection,
    id: u64,
    spans: Vec<KeptSpan>,
    cap: usize,
) -> Result<SpansAdded> {
    let tx = connection.transaction()?;
    let finished: Option<bool> = tx
        .query_row("SELECT finished FROM run WHERE id = ?1", [id], |r| r.get(0))
        .optional()?;
    let Some(finished) = finished else {
        return Err(Error::UnknownRun(id));
    };
    let held: usize = tx.query_row(
        "SELECT count(*) FROM span WHERE run = ?1 AND synthesized = 0",
        [id],
        |r| r.get(0),
    )?;
    let mut added = SpansAdded::default();
    {
        let mut insert = tx.prepare(INSERT_SPAN)?;
        for kept in spans {
            if held + added.kept >= cap {
                added.dropped += 1;
                continue;
            }
            let span = kept.span;
            let stored = StoredSpan {
                trace_id: span.trace_id,
                span_id: span.span_id,
                parent_span_id: span.parent_span_id,
                test: None,
                test_name: span.test_name,
                test_classname: span.test_classname,
                name: span.name,
                start_ns: Some(span.start_ns),
                duration_ns: span.end_ns.saturating_sub(span.start_ns),
                status: span.status,
                synthesized: false,
                code: span.code,
                node: kept.node,
                attached_by: kept.attached_by,
                attributes: span.attributes,
            };
            if insert_span(&mut insert, id, &stored)? {
                added.kept += 1;
            }
        }
    }
    if added.dropped > 0 {
        tx.execute(
            "UPDATE run SET spans_dropped = spans_dropped + ?2 WHERE id = ?1",
            params![id, added.dropped],
        )?;
    }
    if finished {
        link(&tx, id)?;
    }
    tx.commit()?;
    Ok(added)
}

fn finish(connection: &mut Connection, id: u64, record: &RunRecord) -> Result<usize> {
    let m = &record.measured;
    let tx = connection.transaction()?;
    let finished: Option<bool> = tx
        .query_row("SELECT finished FROM run WHERE id = ?1", [id], |r| r.get(0))
        .optional()?;
    match finished {
        None => return Err(Error::UnknownRun(id)),
        Some(true) => return Err(Error::RunFinished(id)),
        Some(false) => {}
    }
    tx.execute(
        "UPDATE run SET finished = 1, finished_at_ms = ?21, exit_code = ?2, fingerprint = ?3, machine = ?4,
             cpu_source = ?5, accounting = ?6, wall_us = ?7, user_cpu_us = ?8, system_cpu_us = ?9,
             largest_process_bytes = ?10, tree_peak_bytes = ?11, block_reads = ?12,
             block_writes = ?13, io_read_bytes = ?14, io_write_bytes = ?15, instructions = ?16,
             counter_refused = ?17, samples = ?18, junit = ?19, junit_problem = ?20
         WHERE id = ?1",
        params![
            id,
            m.exit_code,
            record.machine.fingerprint(m.cpu_source()),
            json(&record.machine),
            word(&m.cpu_source()),
            word(&m.accounting),
            m.wall_us,
            m.user_cpu_us,
            m.system_cpu_us,
            m.largest_process_bytes,
            m.tree_peak_bytes,
            m.block_reads,
            m.block_writes,
            m.io_read_bytes,
            m.io_write_bytes,
            m.instructions,
            m.counter_refused,
            json(&m.samples),
            record.junit,
            record.junit_problem,
            now_ms(),
        ],
    )?;
    {
        let mut test = tx.prepare(
            "INSERT INTO test (run, ord, name, classname, file, line, status, duration_us,
                 started_at_us)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        let mut span = tx.prepare(INSERT_SPAN)?;
        for (ord, case) in (0_u64..).zip(&record.tests) {
            test.execute(params![
                id,
                ord,
                case.name,
                case.classname,
                case.file,
                case.line,
                word(&case.status),
                case.duration_us,
                case.started_at_us,
            ])?;
            insert_span(&mut span, id, &test_root(id, ord, case))?;
        }
    }
    link(&tx, id)?;
    tx.commit()?;
    Ok(record.tests.len())
}

fn tests_of(connection: &Connection, id: u64) -> Result<Vec<TestCase>> {
    let mut statement = connection.prepare(
        "SELECT name, classname, file, line, status, duration_us, started_at_us FROM test
         WHERE run = ?1 ORDER BY ord",
    )?;
    let rows = statement.query_map([id], |r| {
        Ok((
            TestCase {
                name: r.get(0)?,
                classname: r.get(1)?,
                file: r.get(2)?,
                line: r.get(3)?,
                status: TestStatus::Passed,
                duration_us: r.get(5)?,
                started_at_us: r.get(6)?,
            },
            r.get::<_, String>(4)?,
        ))
    })?;
    let mut tests = Vec::new();
    for row in rows {
        let (mut case, status) = row?;
        case.status = parse("test.status", &format!("\"{status}\""), "a test status")?;
        tests.push(case);
    }
    Ok(tests)
}

/// The figures of a finished run, read from its row.
fn measured(r: &Row<'_>) -> rusqlite::Result<(Measured, String, String)> {
    Ok((
        Measured {
            wall_us: r.get("wall_us")?,
            exit_code: r.get("exit_code")?,
            // Read below from its column, as a word.
            accounting: Accounting::Rusage,
            user_cpu_us: r.get("user_cpu_us")?,
            system_cpu_us: r.get("system_cpu_us")?,
            largest_process_bytes: r.get("largest_process_bytes")?,
            tree_peak_bytes: r.get("tree_peak_bytes")?,
            block_reads: r.get("block_reads")?,
            block_writes: r.get("block_writes")?,
            io_read_bytes: r.get("io_read_bytes")?,
            io_write_bytes: r.get("io_write_bytes")?,
            instructions: r.get("instructions")?,
            counter_refused: r.get("counter_refused")?,
            samples: Vec::new(),
            started_at_ms: 0,
        },
        r.get("accounting")?,
        r.get("samples")?,
    ))
}

fn load(connection: &Connection, id: u64) -> Result<Option<StoredRun>> {
    type Head = (
        u64,
        String,
        RunPlace,
        bool,
        Option<String>,
        Option<String>,
        u64,
    );
    let head: Option<Head> = connection
        .query_row(
            "SELECT started_at_ms, command, snapshot, commit_sha, branch, finished, fingerprint,
                 machine, spans_dropped, worktree
             FROM run WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    RunPlace {
                        snapshot: r.get(2)?,
                        commit: r.get(3)?,
                        branch: r.get(4)?,
                        worktree: r.get(9)?,
                    },
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                ))
            },
        )
        .optional()?;
    let Some((started_at_ms, command, place, finished, fingerprint, machine, spans_dropped)) = head
    else {
        return Ok(None);
    };
    let command: Vec<String> = parse("run.command", &command, "a JSON array of words")?;
    let record = if finished {
        let (mut measured, accounting, samples) =
            connection.query_row("SELECT * FROM run WHERE id = ?1", [id], measured)?;
        measured.accounting = parse(
            "run.accounting",
            &format!("\"{accounting}\""),
            "an accounting",
        )?;
        measured.samples = parse("run.samples", &samples, "a JSON array of samples")?;
        let junit: (Option<String>, Option<String>) = connection.query_row(
            "SELECT junit, junit_problem FROM run WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Some(RunRecord {
            command: command.clone(),
            started_at_ms,
            measured,
            junit: junit.0,
            junit_problem: junit.1,
            tests: tests_of(connection, id)?,
            machine: parse(
                "run.machine",
                machine.as_deref().unwrap_or_default(),
                "a JSON machine",
            )?,
        })
    } else {
        None
    };
    Ok(Some(StoredRun {
        id,
        spans_dropped,
        command,
        started_at_ms,
        place,
        fingerprint,
        record,
    }))
}

#[cfg(test)]
mod tests {
    use timonier_core::perf::{Machine, Sample};

    use crate::Error;

    use super::*;

    fn record() -> RunRecord {
        RunRecord {
            command: vec!["sh".to_owned(), "-c".to_owned(), "true".to_owned()],
            started_at_ms: 1_760_000_000_000,
            measured: Measured {
                wall_us: 1_500,
                exit_code: Some(3),
                accounting: Accounting::SystemdScope,
                user_cpu_us: 700,
                system_cpu_us: 300,
                largest_process_bytes: 8 << 20,
                tree_peak_bytes: Some(12 << 20),
                block_reads: 1,
                block_writes: 2,
                io_read_bytes: None,
                io_write_bytes: None,
                instructions: Some(u64::from(u32::MAX) * 4),
                counter_refused: None,
                samples: vec![Sample {
                    at_ms: 100,
                    memory_bytes: 4 << 20,
                    cpu_us: 50,
                }],
                started_at_ms: 0,
            },
            junit: Some("report.xml".to_owned()),
            junit_problem: None,
            tests: vec![
                TestCase {
                    name: "parses".to_owned(),
                    classname: Some("parser".to_owned()),
                    file: Some("src/parser.rs".to_owned()),
                    line: Some(12),
                    status: TestStatus::Passed,
                    duration_us: 1_200,
                    started_at_us: Some(1_760_000_000_000_500),
                },
                TestCase {
                    name: "fails".to_owned(),
                    classname: None,
                    file: None,
                    line: None,
                    status: TestStatus::Failed,
                    duration_us: 30,
                    started_at_us: None,
                },
            ],
            machine: Machine {
                cpu: "cpu".to_owned(),
                cores: 4,
                memory_bytes: 1 << 34,
                os: "linux".to_owned(),
                kernel: "7.0".to_owned(),
            },
        }
    }

    fn start() -> RunStart {
        RunStart {
            command: record().command,
            started_at_ms: record().started_at_ms,
            traced: true,
            batch: None,
            smoke: false,
        }
    }

    fn place() -> RunPlace {
        RunPlace {
            snapshot: 7,
            commit: Some("abc".to_owned()),
            branch: Some("main".to_owned()),
            worktree: None,
        }
    }

    #[test]
    fn a_run_begins_then_reads_back_as_finished() {
        let dir = tempfile::tempdir().unwrap();
        let store = PerfStore::open(dir.path().join("perf.sqlite")).unwrap();
        let first = store.start(start(), place()).unwrap();
        let second = store.start(start(), place()).unwrap();
        assert_eq!((first, second), (1, 2));
        let begun = store.run(first).unwrap().unwrap();
        assert_eq!((begun.record, begun.fingerprint), (None, None));
        assert_eq!(begun.place, place());

        assert_eq!(store.finish(first, record()).unwrap(), 2);
        let stored = store.run(first).unwrap().unwrap();
        assert_eq!(stored.record, Some(record()));
        assert_eq!(
            stored.fingerprint,
            Some(record().machine.fingerprint(CpuSource::Instructions))
        );
        // The other run stays begun: interrupted, if it never ends.
        assert_eq!(store.run(second).unwrap().unwrap().record, None);
        assert!(store.run(3).unwrap().is_none());
    }

    #[test]
    fn a_run_finishes_once_and_only_if_it_began() {
        let dir = tempfile::tempdir().unwrap();
        let store = PerfStore::open(dir.path().join("perf.sqlite")).unwrap();
        assert!(matches!(
            store.finish(1, record()),
            Err(Error::UnknownRun(1))
        ));
        let id = store.start(start(), place()).unwrap();
        store.finish(id, record()).unwrap();
        assert!(matches!(
            store.finish(id, record()),
            Err(Error::RunFinished(1))
        ));
        assert_eq!(
            store.run(id).unwrap().unwrap().record.unwrap().tests.len(),
            2
        );
    }

    /// ADR 0047: each test is a root span of the run, named as the test.
    #[test]
    fn each_test_becomes_a_root_span() {
        let dir = tempfile::tempdir().unwrap();
        let store = PerfStore::open(dir.path().join("perf.sqlite")).unwrap();
        let id = store.start(start(), place()).unwrap();
        store.finish(id, record()).unwrap();
        let spans = store.spans(id).unwrap();
        assert_eq!(spans.len(), 2);
        let parses = &spans[0];
        assert_eq!(parses.name, "parses");
        assert_eq!(parses.parent_span_id, None);
        assert_eq!(parses.test, Some(0));
        assert!(parses.synthesized);
        assert_eq!(parses.duration_ns, 1_200_000);
        assert_eq!(parses.start_ns, Some(1_760_000_000_000_500_000));
        assert_eq!(parses.trace_id.len(), 32);
        assert_eq!(parses.span_id.len(), 16);
        assert_eq!(parses.test_name.as_deref(), Some("parses"));
        assert_eq!(parses.test_classname.as_deref(), Some("parser"));
        assert_eq!(parses.code.file.as_deref(), Some("src/parser.rs"));
        assert_eq!(parses.code.line, Some(12));
        assert_eq!(parses.status, SpanStatus::Unset);
        let fails = &spans[1];
        assert_eq!((fails.start_ns, fails.test), (None, Some(1)));
        assert_eq!(fails.status, SpanStatus::Error);
        assert_eq!(fails.attributes["timonier.test.status"], "failed");
        assert_ne!(parses.trace_id, fails.trace_id);
    }

    fn received(span_id: &str, test: Option<(&str, Option<&str>)>) -> KeptSpan {
        KeptSpan {
            span: ReceivedSpan {
                run: "R-1".to_owned(),
                trace_id: "ab".repeat(16),
                span_id: span_id.to_owned(),
                parent_span_id: None,
                name: format!("span {span_id}"),
                start_ns: 100,
                end_ns: 350,
                status: SpanStatus::Ok,
                test_name: test.map(|(name, _)| name.to_owned()),
                test_classname: test.and_then(|(_, class)| class.map(str::to_owned)),
                code: CodeLocation {
                    function: Some("demo::parser::parse".to_owned()),
                    function_derived: true,
                    file: Some("src/parser.rs".to_owned()),
                    line: Some(12),
                },
                language: Some("rust".to_owned()),
                attributes: Map::new(),
            },
            node: Some("rust:demo::parser::parse".to_owned()),
            attached_by: Some(Attachment::Id),
        }
    }

    /// ADR 0048: a received span keeps its code and node, and finds its test by name and class,
    /// whether it comes before the run ends or after.
    #[test]
    fn received_spans_find_their_test_before_or_after_the_end() {
        let dir = tempfile::tempdir().unwrap();
        let store = PerfStore::open(dir.path().join("perf.sqlite")).unwrap();
        let id = store.start(start(), place()).unwrap();
        assert!(store.accepts(id).unwrap());
        let early = store
            .add_spans(
                id,
                vec![
                    received("01", Some(("parses", Some("parser")))),
                    received("02", Some(("parses", Some("elsewhere")))),
                    received("03", None),
                    // Sent twice by its exporter: kept once.
                    received("03", None),
                ],
            )
            .unwrap();
        assert_eq!(
            early,
            SpansAdded {
                kept: 3,
                dropped: 0
            }
        );
        store.finish(id, record()).unwrap();
        assert!(store.accepts(id).unwrap(), "late spans still come");
        store
            .add_spans(id, vec![received("04", Some(("fails", None)))])
            .unwrap();
        let spans = store.spans(id).unwrap();
        let of = |span_id: &str| spans.iter().find(|s| s.span_id == span_id).unwrap();
        assert_eq!(of("01").test, Some(0));
        assert_eq!(of("02").test, None, "another class");
        assert_eq!(of("03").test, None);
        assert_eq!(of("04").test, Some(1));
        let first = of("01");
        assert_eq!((first.start_ns, first.duration_ns), (Some(100), 250));
        assert_eq!(first.node.as_deref(), Some("rust:demo::parser::parse"));
        assert_eq!(first.attached_by, Some(Attachment::Id));
        assert!(first.code.function_derived);
        assert!(!first.synthesized);
        assert!(matches!(
            store.add_spans(9, vec![]),
            Err(Error::UnknownRun(9))
        ));
        assert!(!store.accepts(9).unwrap());
    }

    #[test]
    fn beyond_the_cap_spans_are_counted_not_kept() {
        let dir = tempfile::tempdir().unwrap();
        let store = PerfStore::open(dir.path().join("perf.sqlite")).unwrap();
        let id = store.start(start(), place()).unwrap();
        let spans = vec![
            received("01", None),
            received("02", None),
            received("03", None),
        ];
        let added = store
            .inner
            .writer
            .run(move |c| add_spans(c, id, spans, 2))
            .unwrap();
        assert_eq!(
            added,
            SpansAdded {
                kept: 2,
                dropped: 1
            }
        );
        assert_eq!(store.run(id).unwrap().unwrap().spans_dropped, 1);
    }

    #[test]
    fn metric_points_are_kept_with_their_run() {
        let dir = tempfile::tempdir().unwrap();
        let store = PerfStore::open(dir.path().join("perf.sqlite")).unwrap();
        let id = store.start(start(), place()).unwrap();
        let point = ReceivedMetric {
            run: "R-1".to_owned(),
            name: "timonier.rows".to_owned(),
            kind: timonier_core::perf::MetricKind::Sum,
            time_ns: 5,
            value: Some(7.0),
            count: None,
            sum: None,
            attributes: Map::new(),
        };
        assert_eq!(
            store.add_metrics(id, vec![point.clone(), point]).unwrap(),
            2
        );
    }

    /// ADR 0043: the spans of the runs older than the last 50 go; the runs and what each node cost
    /// stay.
    #[test]
    fn older_runs_lose_their_spans_not_their_figures() {
        let dir = tempfile::tempdir().unwrap();
        let store = PerfStore::open(dir.path().join("perf.sqlite")).unwrap();
        let first = store
            .start(
                start(),
                RunPlace {
                    worktree: Some("f00d".to_owned()),
                    ..place()
                },
            )
            .unwrap();
        store.finish(first, record()).unwrap();
        store
            .add_spans(first, vec![received("01", Some(("parses", None)))])
            .unwrap();
        assert!(
            store.aggregates(first).unwrap().is_empty(),
            "the spans say it"
        );
        for _ in 0..KEPT_RUNS - 1 {
            store.start(start(), place()).unwrap();
        }
        assert_eq!(
            store.spans(first).unwrap().len(),
            3,
            "still among the last 50: two roots and a span"
        );
        store.start(start(), place()).unwrap();
        assert!(store.spans(first).unwrap().is_empty());
        // What each node cost stays, for the reference and the evolution (relecture n° 1, I4).
        assert_eq!(
            store.aggregates(first).unwrap(),
            [timonier_core::perf::NodePerf {
                node: "rust:demo::parser::parse".to_owned(),
                calls: 1,
                self_ns: 250,
                entered_ns: 250,
            }]
        );
        // Q77: the run on a modified tree says so.
        let summary = store
            .summaries(100)
            .unwrap()
            .into_iter()
            .find(|r| r.run == format!("R-{first}"));
        assert_eq!(summary.unwrap().worktree.as_deref(), Some("f00d"));
        assert_eq!(
            store
                .run(first)
                .unwrap()
                .unwrap()
                .record
                .unwrap()
                .tests
                .len(),
            2
        );
    }

    /// ADR 0049: what the spans of a run say of its instrumentation.
    #[test]
    fn the_facts_of_a_run_s_spans() {
        let dir = tempfile::tempdir().unwrap();
        let store = PerfStore::open(dir.path().join("perf.sqlite")).unwrap();
        let id = store.start(start(), place()).unwrap();
        let mut bare = received("02", None);
        bare.span.name = "bare".to_owned();
        bare.span.code = CodeLocation::default();
        bare.node = None;
        bare.attached_by = None;
        let mut by_file = received("03", None);
        by_file.span.code.line = None;
        by_file.node = Some("file:src/parser.rs".to_owned());
        by_file.attached_by = Some(Attachment::File);
        store
            .add_spans(id, vec![received("01", None), bare, by_file])
            .unwrap();
        store.finish(id, record()).unwrap();
        let facts = store.span_facts(id).unwrap();
        assert_eq!((facts.spans, facts.unattached, facts.dropped), (3, 1, 0));
        // Attached to the file alone: not seen.
        assert_eq!(
            facts.seen.iter().collect::<Vec<_>>(),
            ["rust:demo::parser::parse"]
        );
        let missing: Vec<_> = facts
            .without_code
            .iter()
            .map(|w| (w.name.as_str(), w.spans, w.missing.len()))
            .collect();
        assert_eq!(missing, [("bare", 1, 3), ("span 03", 1, 1)]);
        assert_eq!(store.latest_traced().unwrap(), Some(id));
        // Relecture n° 2, I-1: a smoke test is not what the views show by default.
        let smoke = store
            .start(
                RunStart {
                    smoke: true,
                    ..start()
                },
                place(),
            )
            .unwrap();
        store.finish(smoke, record()).unwrap();
        assert_eq!(store.latest_traced().unwrap(), Some(id));
        let listed = store.summaries(10).unwrap();
        assert!(
            listed
                .iter()
                .any(|r| r.run == format!("R-{smoke}") && r.smoke)
        );
    }

    #[test]
    fn runs_with_and_without_traces_of_the_same_commit_compare() {
        let dir = tempfile::tempdir().unwrap();
        let store = PerfStore::open(dir.path().join("perf.sqlite")).unwrap();
        let traced = store.start(start(), place()).unwrap();
        store.finish(traced, record()).unwrap();
        let untraced = store
            .start(
                RunStart {
                    traced: false,
                    ..start()
                },
                place(),
            )
            .unwrap();
        assert!(
            !store.accepts(untraced).unwrap(),
            "a run without traces takes none"
        );
        let mut cheaper = record();
        cheaper.measured.instructions = Some(1_000);
        store.finish(untraced, cheaper).unwrap();
        let elsewhere = store
            .start(
                start(),
                RunPlace {
                    commit: Some("other".to_owned()),
                    ..place()
                },
            )
            .unwrap();
        store.finish(elsewhere, record()).unwrap();
        let (source, costs) = store.comparable_costs(traced).unwrap().unwrap();
        assert_eq!(source, CpuSource::Instructions);
        let shown: Vec<_> = costs.iter().map(|c| (c.run.as_str(), c.traced)).collect();
        assert_eq!(shown, [("R-1", true), ("R-2", false)]);
        assert_eq!(costs[1].cost, 1_000.0);
        assert_eq!(store.comparable_costs(9).unwrap(), None);
    }

    #[test]
    fn the_views_list_the_runs_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let store = PerfStore::open(dir.path().join("perf.sqlite")).unwrap();
        let first = store.start(start(), place()).unwrap();
        store.add_spans(first, vec![received("01", None)]).unwrap();
        store.finish(first, record()).unwrap();
        store.start(start(), place()).unwrap();
        let runs = store.summaries(10).unwrap();
        assert_eq!(
            runs.iter().map(|r| r.run.as_str()).collect::<Vec<_>>(),
            ["R-2", "R-1"]
        );
        let done = &runs[1];
        assert!(done.finished && !runs[0].finished);
        assert_eq!((done.tests, done.failed, done.spans), (2, 1, 1));
        assert_eq!(done.wall_us, Some(1_500));
        assert_eq!(done.cpu_us, Some(1_000));
        assert_eq!(done.memory_bytes, Some(12 << 20));
        assert!(done.memory_whole_tree);
        assert_eq!(done.cpu_source, Some(CpuSource::Instructions));
        assert_eq!(done.command, record().command);
        assert_eq!(store.summaries(1).unwrap().len(), 1);
    }

    #[test]
    fn runs_stay_when_the_store_opens_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("perf.sqlite");
        let store = PerfStore::open(&path).unwrap();
        let id = store.start(start(), place()).unwrap();
        store.finish(id, record()).unwrap();
        drop(store);
        let store = PerfStore::open(&path).unwrap();
        assert_eq!(
            store.run(id).unwrap().unwrap().record.unwrap().tests.len(),
            2
        );
    }
}
