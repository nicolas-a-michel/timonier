//! What a run of tests measures (ADR 0043, 0045, 0046): level 1, measured around the command by
//! `timonier test`, and the duration of each test, read in its JUnit XML. Pure types: the CLI
//! measures, the daemon records.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What the CPU work of a run is counted in (ADR 0046): instructions when the hardware counters
/// are allowed, CPU time otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum CpuSource {
    Instructions,
    CpuTime,
}

/// Where the memory and CPU of the whole tree come from (ADR 0045).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accounting {
    /// A transient systemd user scope: its cgroup holds the whole tree.
    SystemdScope,
    /// `getrusage` of the relay's children only: the memory is that of the largest process.
    Rusage,
}

/// One sample of the scope's resources, every 100 ms, for the curve under the timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    /// Since the start of the run (`RunStart::started_at_ms`), as its spans are.
    pub at_ms: u64,
    pub memory_bytes: u64,
    pub cpu_us: u64,
}

/// Level 1: what the measuring relay saw of the command and all its descendants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measured {
    pub wall_us: u64,
    /// `None`: ended by a signal, or not started.
    pub exit_code: Option<i32>,
    pub accounting: Accounting,
    pub user_cpu_us: u64,
    pub system_cpu_us: u64,
    /// The largest single process of the tree (`getrusage`).
    pub largest_process_bytes: u64,
    /// The whole tree at its peak (the scope's `memory.peak`).
    pub tree_peak_bytes: Option<u64>,
    pub block_reads: u64,
    pub block_writes: u64,
    /// Bytes from the scope's `io.stat`, when the io controller is delegated.
    pub io_read_bytes: Option<u64>,
    pub io_write_bytes: Option<u64>,
    /// User-space instructions of the whole tree (ADR 0046).
    pub instructions: Option<u64>,
    /// Why there are no instructions, when there are none.
    pub counter_refused: Option<String>,
    pub samples: Vec<Sample>,
    /// When the relay started the command, wall clock in milliseconds: its samples count from
    /// it, and `timonier test` moves them to the run's start (relecture n° 1, I3).
    #[serde(default)]
    pub started_at_ms: u64,
}

impl Measured {
    /// The samples counted from `origin_ms` instead of the command's start: the run starts
    /// before the relay, which the daemon's start may delay by seconds.
    pub fn rebase_samples(&mut self, origin_ms: u64) {
        let offset = self.started_at_ms.saturating_sub(origin_ms);
        for sample in &mut self.samples {
            sample.at_ms = sample.at_ms.saturating_add(offset);
        }
        self.started_at_ms = origin_ms;
    }
    pub fn cpu_source(&self) -> CpuSource {
        if self.instructions.is_some() {
            CpuSource::Instructions
        } else {
            CpuSource::CpuTime
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TestStatus {
    Passed,
    Failed,
    Error,
    Skipped,
}

/// One test case of the JUnit XML.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCase {
    pub name: String,
    pub classname: Option<String>,
    pub file: Option<String>,
    pub line: Option<u64>,
    pub status: TestStatus,
    pub duration_us: u64,
    /// When it started, in microseconds since the Unix epoch, when the runner says it.
    pub started_at_us: Option<u64>,
}

/// What the machine is, for its fingerprint: a change resets the reference of the regressions
/// (spec, ADR 0043).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    pub cpu: String,
    pub cores: u32,
    pub memory_bytes: u64,
    pub os: String,
    pub kernel: String,
}

impl Machine {
    /// A stable hash of the machine and of what counts its CPU work (ADR 0046): 16 hex digits.
    pub fn fingerprint(&self, source: CpuSource) -> String {
        let source = match source {
            CpuSource::Instructions => "instructions",
            CpuSource::CpuTime => "cpu_time",
        };
        let text = format!(
            "{}\n{}\n{}\n{}\n{}\n{source}",
            self.cpu, self.cores, self.memory_bytes, self.os, self.kernel
        );
        // FNV-1a, 64 bits: stable across versions and platforms, unlike `DefaultHasher`.
        let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
        format!("{hash:016x}")
    }
}

/// `run/start`: a run of tests begins (ADR 0047).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStart {
    pub command: Vec<String>,
    /// Milliseconds since the Unix epoch.
    pub started_at_ms: u64,
    /// `false`: the run turns the instrumentation off, to measure its overhead (ADR 0049).
    #[serde(default = "traced")]
    pub traced: bool,
    /// The runs of one `timonier test --runs N` share it (ADR 0053).
    #[serde(default)]
    pub batch: Option<String>,
    /// `timonier doctor`'s smoke test (ADR 0050): it says nothing of the suite, so neither the
    /// budgets nor the views take it by default (relecture n° 2, I-1).
    #[serde(default)]
    pub smoke: bool,
}

fn traced() -> bool {
    true
}

/// What `run/start` answers: the run, where it sits, and where its traces go.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStarted {
    /// `R-<n>`.
    pub run: String,
    /// The head snapshot of the graph when the run began.
    pub snapshot: u64,
    pub commit: Option<String>,
    pub branch: Option<String>,
    /// For `OTEL_EXPORTER_OTLP_ENDPOINT`; `None` when the receiver could not listen.
    pub endpoint: Option<String>,
    /// What the run's senders put in the `x-timonier-token` header (`OTEL_EXPORTER_OTLP_HEADERS`):
    /// the receiver takes a run's spans with it only (audit of 2026-10-21, finding 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl RunStarted {
    /// What the run adds to `OTEL_RESOURCE_ATTRIBUTES` (spec « Wiring »).
    pub fn resource_attributes(&self) -> [(&'static str, String); 2] {
        [
            ("timonier.run_id", self.run.clone()),
            ("timonier.snapshot", self.snapshot.to_string()),
        ]
    }
}

/// `OTEL_RESOURCE_ATTRIBUTES` with `added` after what `existing` holds; a key already there is
/// replaced. The format is `key=value` pairs separated by commas (OpenTelemetry).
pub fn merged_resource_attributes(existing: Option<&str>, added: &[(&str, String)]) -> String {
    let mut pairs: Vec<String> = existing
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|pair| !pair.is_empty())
        .filter(|pair| {
            let key = pair.split_once('=').map_or(*pair, |(key, _)| key).trim();
            !added.iter().any(|(k, _)| *k == key)
        })
        .map(str::to_owned)
        .collect();
    pairs.extend(added.iter().map(|(key, value)| format!("{key}={value}")));
    pairs.join(",")
}

/// A run of tests, as `timonier test` measured it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub command: Vec<String>,
    /// Milliseconds since the Unix epoch.
    pub started_at_ms: u64,
    pub measured: Measured,
    /// The JUnit XML read, relative to the root when inside it.
    pub junit: Option<String>,
    /// Why no test was read from it: missing, older than the run, unreadable.
    pub junit_problem: Option<String>,
    pub tests: Vec<TestCase>,
    pub machine: Machine,
}

/// `run/finish`: what was measured of a run that `run/start` began.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunFinish {
    pub run: String,
    pub record: RunRecord,
}

/// What `run/finish` answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunFinished {
    pub run: String,
    pub fingerprint: String,
    /// The root spans made for the tests (ADR 0047).
    pub test_spans: usize,
}

/// The number of a run from its name, `R-<n>`.
pub fn run_number(run: &str) -> Option<u64> {
    run.strip_prefix("R-")?.parse().ok().filter(|n| *n > 0)
}

/// A span's status, as OTLP gives it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    #[default]
    Unset,
    Ok,
    Error,
}

/// Where a span's code is (the `code.*` attributes, ADR 0044).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLocation {
    pub function: Option<String>,
    /// `function` was derived from `code.module.name` and the span's name (ADR 0044).
    pub function_derived: bool,
    pub file: Option<String>,
    pub line: Option<u64>,
}

/// A span the receiver took for a run (ADR 0048).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedSpan {
    /// `timonier.run_id`.
    pub run: String,
    /// Lowercase hex: 32 digits for the trace, 16 for spans.
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub start_ns: u64,
    pub end_ns: u64,
    pub status: SpanStatus,
    /// `timonier.test.name` and `timonier.test.classname`, of the span or of its resource.
    pub test_name: Option<String>,
    pub test_classname: Option<String>,
    pub code: CodeLocation,
    /// `telemetry.sdk.language` of the resource.
    pub language: Option<String>,
    /// The other attributes of the span, bounded (ADR 0043).
    pub attributes: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    Sum,
    Gauge,
    Histogram,
}

/// A point of a metric the receiver took for a run: a sum or a gauge has its value, a
/// histogram its count and sum.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedMetric {
    pub run: String,
    pub name: String,
    pub kind: MetricKind,
    pub time_ns: u64,
    pub value: Option<f64>,
    pub count: Option<u64>,
    pub sum: Option<f64>,
    pub attributes: serde_json::Map<String, serde_json::Value>,
}

/// How a span found its node of the graph (ADR 0048), in the order tried.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attachment {
    /// `code.function.name` is an ID of the graph.
    Id,
    /// A function of the file whose ID ends with the function's last name.
    Name,
    /// The innermost symbol of the file holding the line.
    Line,
    File,
}

/// A run, as the views list it (ADR 0051): what it is, and its figures once finished.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    /// `R-<n>`.
    pub run: String,
    #[ts(type = "number")]
    pub started_at_ms: u64,
    pub command: Vec<String>,
    pub finished: bool,
    pub traced: bool,
    #[ts(type = "number")]
    pub snapshot: u64,
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub exit_code: Option<i32>,
    #[ts(type = "number | null")]
    pub wall_us: Option<u64>,
    /// User and system CPU time.
    #[ts(type = "number | null")]
    pub cpu_us: Option<u64>,
    #[ts(type = "number | null")]
    pub instructions: Option<u64>,
    pub cpu_source: Option<CpuSource>,
    /// The whole tree's peak when a scope measured it, else the largest process's.
    #[ts(type = "number | null")]
    pub memory_bytes: Option<u64>,
    pub memory_whole_tree: bool,
    pub tests: u32,
    pub failed: u32,
    #[ts(type = "number")]
    pub spans: u64,
    pub batch: Option<String>,
    /// The machine's fingerprint, once finished (ADR 0043).
    pub fingerprint: Option<String>,
    /// The fingerprint of the working tree's changes when it was not the commit (Q77): such a
    /// run never enters the reference of the main branch.
    pub worktree: Option<String>,
    /// `timonier doctor`'s smoke test (relecture n° 2, I-1).
    pub smoke: bool,
}

/// `perf/runs`: the last runs, the newest first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PerfRuns {
    pub runs: Vec<RunSummary>,
}

/// `perf/run` and `perf/map`: a run; for the map, the last finished one with traces when absent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PerfRunParams {
    pub run: Option<String>,
}

/// A test of a run, for the views.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TestView {
    /// Its place in the JUnit XML.
    pub ord: u32,
    pub name: String,
    pub classname: Option<String>,
    pub status: TestStatus,
    #[ts(type = "number")]
    pub duration_us: u64,
    pub file: Option<String>,
    #[ts(type = "number | null")]
    pub line: Option<u64>,
}

/// A span of a run, its times from the run's start (nanoseconds fit in a JavaScript number
/// that way).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpanView {
    pub trace: String,
    pub id: String,
    pub parent: Option<String>,
    /// The test it belongs to (`TestView::ord`).
    pub test: Option<u32>,
    pub name: String,
    /// `None`: when it started is not known (a test the JUnit does not date).
    #[ts(type = "number | null")]
    pub start_ns: Option<u64>,
    #[ts(type = "number")]
    pub duration_ns: u64,
    pub status: SpanStatus,
    /// A root Timonier made for a test (ADR 0047).
    pub synthesized: bool,
    pub node: Option<String>,
    pub function: Option<String>,
    pub file: Option<String>,
    #[ts(type = "number | null")]
    pub line: Option<u64>,
}

/// `perf/run`: one run, for the timeline and the call tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PerfRun {
    pub summary: RunSummary,
    pub tests: Vec<TestView>,
    pub spans: Vec<SpanView>,
    /// The run has more spans than the views take (ADR 0051).
    #[ts(type = "number")]
    pub spans_left_out: u64,
    pub samples: Vec<Sample>,
    /// The name of each node a span is attached to.
    #[ts(type = "Record<string, string>")]
    pub nodes: BTreeMap<String, String>,
}

/// What a node of the graph cost in a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NodePerf {
    pub node: String,
    #[ts(type = "number")]
    pub calls: u64,
    /// Its spans' time less that of the spans they called: added up, a box's spans give the
    /// time spent in it, never counted twice.
    #[ts(type = "number")]
    pub self_ns: u64,
    /// The time of its spans that no span of a node called: the tests', or a process's own
    /// start. With the links' time, the cumulative time of a box (Q76).
    #[ts(type = "number")]
    pub entered_ns: u64,
}

/// Calls from the spans of one node to those of another: the measured traffic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkPerf {
    pub from: String,
    pub to: String,
    #[ts(type = "number")]
    pub calls: u64,
    /// The time of the callee's spans so called, nested calls to itself counted once (Q76).
    #[ts(type = "number")]
    pub ns: u64,
}

/// `perf/map`: the map's layer (ADR 0051), and the lineage of each node, to show it on the box
/// that holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PerfMap {
    pub run: Option<String>,
    pub nodes: Vec<NodePerf>,
    pub links: Vec<LinkPerf>,
    /// The ancestors of each node, from the top, the node excluded.
    #[ts(type = "Record<string, Array<string>>")]
    pub ancestors: BTreeMap<String, Vec<String>>,
}

/// A regression's state (spec): found once, `probable`; found again by the next batch,
/// `confirmed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RegressionStatus {
    Probable,
    Confirmed,
}

/// A measure of a batch that regressed against the main branch (ADR 0053).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Regression {
    /// `run`, `test:<class>::<name>`, `node:<id>`.
    pub key: String,
    /// In words: « the run », « test parses », « f ».
    pub label: String,
    /// `instructions`, `cpu_us`, `us` (a test's duration), `ns` (a node's own time).
    pub unit: String,
    pub status: RegressionStatus,
    pub verdict: crate::regression::Verdict,
}

/// `perf/regressions`: a batch against the main branch (ADR 0053).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RegressionReport {
    pub batch: Option<String>,
    pub commit: Option<String>,
    /// The main branch's commits the reference comes from, the newest first.
    pub reference: Vec<String>,
    pub source: Option<CpuSource>,
    /// How many measures were compared.
    #[ts(type = "number")]
    pub checked: u64,
    pub regressions: Vec<Regression>,
    /// Why nothing could be compared: no reference yet, a new machine; in English, for agents.
    pub note: Option<String>,
    /// The same, for the views to say in their language (relecture n° 1, m1).
    pub note_kind: Option<RegressionNote>,
    /// The main branch the reference is read on.
    pub main: Option<String>,
}

/// Why a regression report compares nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RegressionNote {
    NoRun,
    NoBatch,
    NoFinishedRun,
    NoReference,
    MachineChanged,
    /// A batch of one run: Mann-Whitney can say nothing below two (relecture n° 1, m2).
    BatchTooSmall,
}

/// `perf/regressions` and `get_regressions`: a batch (`B-…`); the latest when absent.
#[derive(
    Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, TS,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegressionParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 64))]
    pub batch: Option<String>,
}

/// A span, as [`map_of`] needs it.
pub struct Called<'a> {
    pub id: &'a str,
    pub parent: Option<&'a str>,
    pub node: Option<&'a str>,
    pub duration_ns: u64,
}

/// What each node cost, and the traffic between nodes, from a run's spans (pure). A span's
/// parent is found within the run; a span without a node gives its own time to no one, and its
/// callees count as called by the nearest span above it that has one.
pub fn map_of(spans: &[Called<'_>]) -> (Vec<NodePerf>, Vec<LinkPerf>) {
    let by_id: BTreeMap<&str, &Called<'_>> = spans.iter().map(|s| (s.id, s)).collect();
    let mut children_ns: BTreeMap<&str, u64> = BTreeMap::new();
    for span in spans {
        if let Some(parent) = span.parent.and_then(|p| by_id.get(p)) {
            *children_ns.entry(parent.id).or_default() += span.duration_ns;
        }
    }
    // The node of the nearest span above, within the run (a cycle of parents ends the walk).
    let caller = |span: &Called<'_>| -> Option<&str> {
        let mut seen = 0;
        let mut at = span.parent.and_then(|p| by_id.get(p));
        while let Some(parent) = at {
            if let Some(node) = parent.node {
                return Some(node);
            }
            seen += 1;
            if seen > spans.len() {
                return None;
            }
            at = parent.parent.and_then(|p| by_id.get(p));
        }
        None
    };
    let mut links: BTreeMap<(&str, &str), (u64, u64)> = BTreeMap::new();
    let mut nodes: BTreeMap<&str, (u64, u64, u64)> = BTreeMap::new();
    for span in spans {
        let Some(node) = span.node else { continue };
        let own = span
            .duration_ns
            .saturating_sub(children_ns.get(span.id).copied().unwrap_or(0));
        let entry = nodes.entry(node).or_default();
        entry.0 += 1;
        entry.1 += own;
        match caller(span) {
            None => entry.2 += span.duration_ns,
            Some(from) if from != node => {
                let link = links.entry((from, node)).or_default();
                link.0 += 1;
                link.1 += span.duration_ns;
            }
            // A call to itself: already in the time of the span above.
            Some(_) => {}
        }
    }
    (
        nodes
            .into_iter()
            .map(|(node, (calls, self_ns, entered_ns))| NodePerf {
                node: node.to_owned(),
                calls,
                self_ns,
                entered_ns,
            })
            .collect(),
        links
            .into_iter()
            .map(|((from, to), (calls, ns))| LinkPerf {
                from: from.to_owned(),
                to: to.to_owned(),
                calls,
                ns,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relecture n° 1, I3: the relay starts after the run, its samples move by the gap.
    #[test]
    fn the_samples_count_from_the_run_s_start() {
        let mut measured: Measured = serde_json::from_value(serde_json::json!({
            "wallUs": 1, "exitCode": 0, "accounting": "rusage", "userCpuUs": 0, "systemCpuUs": 0,
            "largestProcessBytes": 0, "treePeakBytes": null, "blockReads": 0, "blockWrites": 0,
            "ioReadBytes": null, "ioWriteBytes": null, "instructions": null,
            "counterRefused": null, "startedAtMs": 10_250,
            "samples": [{"atMs": 0, "memoryBytes": 1, "cpuUs": 0}, {"atMs": 100, "memoryBytes": 2, "cpuUs": 5}]
        }))
        .unwrap();
        measured.rebase_samples(10_000);
        assert_eq!(
            measured.samples.iter().map(|s| s.at_ms).collect::<Vec<_>>(),
            [250, 350]
        );
        assert_eq!(measured.started_at_ms, 10_000);
    }

    fn machine() -> Machine {
        Machine {
            cpu: "AMD Ryzen 9 5950X".to_owned(),
            cores: 32,
            memory_bytes: 64 << 30,
            os: "linux".to_owned(),
            kernel: "7.0.0".to_owned(),
        }
    }

    #[test]
    fn a_fingerprint_is_stable_and_follows_the_machine_and_the_source() {
        let fingerprint = machine().fingerprint(CpuSource::CpuTime);
        assert_eq!(fingerprint.len(), 16);
        assert_eq!(fingerprint, machine().fingerprint(CpuSource::CpuTime));
        assert_ne!(fingerprint, machine().fingerprint(CpuSource::Instructions));
        let other = Machine {
            kernel: "7.0.1".to_owned(),
            ..machine()
        };
        assert_ne!(fingerprint, other.fingerprint(CpuSource::CpuTime));
    }

    #[test]
    fn a_run_reads_back_as_written() {
        let run = RunRecord {
            command: vec!["cargo".to_owned(), "nextest".to_owned()],
            started_at_ms: 1,
            measured: Measured {
                wall_us: 10,
                exit_code: Some(0),
                accounting: Accounting::Rusage,
                user_cpu_us: 1,
                system_cpu_us: 2,
                largest_process_bytes: 3,
                tree_peak_bytes: None,
                block_reads: 0,
                block_writes: 0,
                io_read_bytes: None,
                io_write_bytes: None,
                instructions: None,
                counter_refused: Some("perf_event_paranoid is 4".to_owned()),
                samples: vec![],
                started_at_ms: 0,
            },
            junit: None,
            junit_problem: None,
            tests: vec![TestCase {
                name: "a".to_owned(),
                classname: None,
                file: None,
                line: None,
                status: TestStatus::Passed,
                duration_us: 5,
                started_at_us: None,
            }],
            machine: machine(),
        };
        let text = serde_json::to_string(&run).unwrap();
        assert!(text.contains("\"exitCode\":0"), "{text}");
        assert_eq!(serde_json::from_str::<RunRecord>(&text).unwrap(), run);
        assert_eq!(run.measured.cpu_source(), CpuSource::CpuTime);
    }

    #[test]
    fn the_run_s_attributes_go_after_the_project_s_own() {
        let started = RunStarted {
            run: "R-4".to_owned(),
            snapshot: 12,
            commit: None,
            branch: None,
            endpoint: Some("http://127.0.0.1:4318".to_owned()),
            token: None,
        };
        let added = started.resource_attributes();
        assert_eq!(
            merged_resource_attributes(None, &added),
            "timonier.run_id=R-4,timonier.snapshot=12"
        );
        assert_eq!(
            merged_resource_attributes(Some("service.name=demo, timonier.run_id=R-1,"), &added),
            "service.name=demo,timonier.run_id=R-4,timonier.snapshot=12"
        );
    }

    #[test]
    fn a_run_is_named_by_its_number() {
        assert_eq!(run_number("R-12"), Some(12));
        assert_eq!(run_number("R-0"), None);
        assert_eq!(run_number("12"), None);
    }

    #[test]
    fn a_node_s_time_is_its_own_and_calls_between_nodes_are_traffic() {
        let span = |id, parent, node, duration_ns| Called {
            id,
            parent,
            node,
            duration_ns,
        };
        let spans = [
            span("a", None, Some("parser"), 100),
            span("b", Some("a"), Some("store"), 30),
            span("c", Some("a"), Some("parser"), 20),
            span("d", Some("b"), Some("store"), 10),
            span("e", Some("gone"), None, 5),
            // Through a span without a node: the parser still calls the store.
            span("f", Some("a"), None, 15),
            span("g", Some("f"), Some("store"), 12),
        ];
        let (nodes, links) = map_of(&spans);
        let of = |node: &str| nodes.iter().find(|n| n.node == node).unwrap();
        // 100 - 30 - 20 - 15 in `a`, 20 in `c`: 55 in the parser, never 120.
        assert_eq!((of("parser").calls, of("parser").self_ns), (2, 55));
        assert_eq!((of("store").calls, of("store").self_ns), (3, 42));
        // Q76: the parser was entered for 100; the store only from the parser (`b`, `g`), its
        // call to itself (`d`) counted in `b`.
        assert_eq!((of("parser").entered_ns, of("store").entered_ns), (100, 0));
        assert_eq!(nodes.len(), 2);
        assert_eq!(
            links,
            [LinkPerf {
                from: "parser".to_owned(),
                to: "store".to_owned(),
                calls: 2,
                ns: 42,
            }]
        );
    }
}
