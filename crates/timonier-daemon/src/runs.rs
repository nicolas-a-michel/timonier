//! The runs of tests that `timonier test` measured (ADR 0043): kept in
//! `.timonier/perf/perf.sqlite`, apart from the graph, which a migration may rebuild. The folder
//! keeps itself out of git, whatever the project's `.gitignore` says.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use timonier_core::budgets::{BudgetRun, BudgetSpan, BudgetsView, ComponentPerf, InsidePerf};
use timonier_core::perf::{
    Called, PerfMap, PerfRun, PerfRuns, ReceivedMetric, ReceivedSpan, RegressionReport, RunFinish,
    RunFinished, RunStart, RunStarted, RunSummary, SpanView, TestView, map_of, run_number,
};
use timonier_store::{KeptSpan, PerfStore, RunPlace, Store};

use crate::{Error, Result, git};

/// Where the measurements are, under the repository.
pub const FOLDER: &str = ".timonier/perf";
pub const DATABASE: &str = "perf.sqlite";

/// The measurements' store, opened at the first run.
pub(crate) struct Runs {
    root: PathBuf,
    store: Mutex<Option<PerfStore>>,
    /// Each run's token, which its senders must give (audit of 2026-10-21, finding 2).
    tokens: Mutex<std::collections::HashMap<u64, String>>,
}

/// 16 bytes of the system's randomness, in hexadecimal; `None` when it cannot be read.
fn random_token() -> Option<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

impl Runs {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            store: Mutex::new(None),
            tokens: Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub(crate) fn store(&self) -> Result<PerfStore> {
        let mut store = self.store.lock().map_err(|_| Error::Poisoned)?;
        if let Some(store) = store.as_ref() {
            return Ok(store.clone());
        }
        let folder = self.root.join(FOLDER);
        let io = |source| Error::Io {
            path: folder.clone(),
            source,
        };
        std::fs::create_dir_all(&folder).map_err(io)?;
        let ignore = folder.join(".gitignore");
        if !ignore.exists() {
            std::fs::write(
                &ignore,
                "# Written by Timonier: the measurements of the test runs stay out of git.\n*\n",
            )
            .map_err(io)?;
        }
        let opened = PerfStore::open(folder.join(DATABASE))?;
        *store = Some(opened.clone());
        Ok(opened)
    }

    /// The last finished run but the smoke tests, in a line: for a pull request (ADR 0056).
    pub(crate) fn last_run_line(&self) -> Option<String> {
        if !self.root.join(FOLDER).join(DATABASE).exists() {
            return None;
        }
        let run = self
            .store()
            .ok()?
            .summaries(LISTED_RUNS)
            .ok()?
            .into_iter()
            .find(|r| r.finished && !r.smoke)?;
        #[allow(clippy::cast_precision_loss)]
        let seconds = run.wall_us.unwrap_or_default() as f64 / 1e6;
        Some(format!(
            "{}: {} tests, {} failed, {seconds:.1} s{}",
            run.run,
            run.tests,
            run.failed,
            run.commit
                .as_deref()
                .map_or_else(String::new, |c| format!(", at {}", &c[..c.len().min(7)]))
        ))
    }

    /// A run begins at the graph's head snapshot, with git's view of it (ADR 0047).
    #[tracing::instrument(skip_all)]
    pub(crate) fn start(
        &self,
        graph: &Store,
        start: RunStart,
        endpoint: Option<String>,
    ) -> Result<RunStarted> {
        let place = RunPlace {
            snapshot: graph.reader()?.head()?.seq,
            commit: git::head(&self.root),
            branch: git::branch(&self.root),
            worktree: git::worktree(&self.root),
        };
        let id = self.store()?.start(start, place.clone())?;
        tracing::info!(run = id, snapshot = place.snapshot, "run started");
        let token = random_token();
        if let Some(token) = &token {
            self.tokens
                .lock()
                .map_err(|_| Error::Poisoned)?
                .insert(id, token.clone());
        }
        Ok(RunStarted {
            run: format!("R-{id}"),
            snapshot: place.snapshot,
            commit: place.commit,
            branch: place.branch,
            endpoint,
            token,
        })
    }

    /// What was measured of a run, and the root span of each of its tests.
    #[tracing::instrument(skip_all)]
    pub(crate) fn finish(&self, finish: RunFinish) -> Result<RunFinished> {
        let id = run_number(&finish.run).ok_or_else(|| Error::UnknownRun(finish.run.clone()))?;
        let fingerprint = finish
            .record
            .machine
            .fingerprint(finish.record.measured.cpu_source());
        let test_spans = self.store()?.finish(id, finish.record)?;
        tracing::info!(run = id, test_spans, "run finished");
        Ok(RunFinished {
            run: finish.run,
            fingerprint,
            test_spans,
        })
    }
}

/// What a run says of the instrumentation (ADR 0049), for `get_instrumentation_gaps`.
pub struct RunFacts {
    pub run: String,
    pub snapshot: u64,
    pub callables: Vec<timonier_core::gaps::Callable>,
    pub spans: timonier_core::gaps::SpanFacts,
    pub overhead: Result<timonier_core::gaps::Overhead, String>,
}

impl Runs {
    /// The facts of run `id`, the last finished one with traces when `None`; `None` when there is none, or
    /// it has not finished. The functions are those of the head.
    #[tracing::instrument(skip_all)]
    pub(crate) fn facts(&self, graph: &Store, id: Option<u64>) -> Result<Option<RunFacts>> {
        if !self.root.join(FOLDER).join(DATABASE).exists() {
            return Ok(None);
        }
        let store = self.store()?;
        let Some(id) = (match id {
            Some(id) => Some(id),
            None => store.latest_traced()?,
        }) else {
            return Ok(None);
        };
        let Some(run) = store.run(id)?.filter(|run| run.record.is_some()) else {
            return Ok(None);
        };
        let overhead = match store.comparable_costs(id)? {
            Some((source, costs)) => timonier_core::gaps::overhead(source, &costs),
            None => Err("the run did not finish".to_owned()),
        };
        Ok(Some(RunFacts {
            run: format!("R-{id}"),
            snapshot: run.place.snapshot,
            callables: graph.reader()?.callables()?,
            spans: store.span_facts(id)?,
            overhead,
        }))
    }
}

/// The spans one run gives the views at most (ADR 0051): the timeline and the tree stay quick.
pub const VIEW_SPANS: usize = 20_000;
/// The runs the views list.
pub const LISTED_RUNS: u32 = 50;

impl Runs {
    /// `perf/runs`.
    pub(crate) fn listed(&self) -> Result<PerfRuns> {
        if !self.root.join(FOLDER).join(DATABASE).exists() {
            return Ok(PerfRuns::default());
        }
        Ok(PerfRuns {
            runs: self.store()?.summaries(LISTED_RUNS)?,
        })
    }

    /// `perf/run`: `None` when there is no such run.
    #[tracing::instrument(skip_all)]
    pub(crate) fn view(&self, graph: &Store, run: &str) -> Result<Option<PerfRun>> {
        let Some(id) = run_number(run) else {
            return Ok(None);
        };
        if !self.root.join(FOLDER).join(DATABASE).exists() {
            return Ok(None);
        }
        let store = self.store()?;
        let Some(stored) = store.run(id)? else {
            return Ok(None);
        };
        let summary = store
            .summaries(LISTED_RUNS.max(1000))?
            .into_iter()
            .find(|s| s.run == run);
        let Some(summary) = summary else {
            return Ok(None);
        };
        let origin = stored.started_at_ms.saturating_mul(1_000_000);
        let mut spans = store.spans(id)?;
        let left_out = spans.len().saturating_sub(VIEW_SPANS) as u64;
        // The roots and the earliest spans first: what the timeline opens on.
        spans.sort_by_key(|s| (!s.synthesized, s.start_ns.unwrap_or(0)));
        spans.truncate(VIEW_SPANS);
        let reader = graph.reader()?;
        let mut nodes = BTreeMap::new();
        for node in spans.iter().filter_map(|s| s.node.as_deref()) {
            if nodes.contains_key(node) {
                continue;
            }
            let name = timonier_core::NodeId::parse(node.to_owned())
                .ok()
                .and_then(|id| reader.node_at(&id, u64::MAX).ok().flatten())
                .map_or_else(|| node.to_owned(), |n| n.name);
            nodes.insert(node.to_owned(), name);
        }
        let (tests, samples) = stored
            .record
            .as_ref()
            .map_or_else(Default::default, |record| {
                let tests = (0_u32..)
                    .zip(&record.tests)
                    .map(|(ord, t)| TestView {
                        ord,
                        name: t.name.clone(),
                        classname: t.classname.clone(),
                        status: t.status,
                        duration_us: t.duration_us,
                        file: t.file.clone(),
                        line: t.line,
                    })
                    .collect();
                (tests, record.measured.samples.clone())
            });
        Ok(Some(PerfRun {
            summary,
            tests,
            spans: spans
                .into_iter()
                .map(|s| SpanView {
                    trace: s.trace_id,
                    id: s.span_id,
                    parent: s.parent_span_id,
                    test: s.test.and_then(|t| u32::try_from(t).ok()),
                    name: s.name,
                    start_ns: s.start_ns.map(|ns| ns.saturating_sub(origin)),
                    duration_ns: s.duration_ns,
                    status: s.status,
                    synthesized: s.synthesized,
                    node: s.node,
                    function: s.code.function,
                    file: s.code.file,
                    line: s.code.line,
                })
                .collect(),
            spans_left_out: left_out,
            samples,
            nodes,
        }))
    }

    /// `perf/map`: what each node cost in the run, the last finished one with traces by default,
    /// with each node's lineage for the map.
    #[tracing::instrument(skip_all)]
    pub(crate) fn map(&self, graph: &Store, run: Option<&str>) -> Result<PerfMap> {
        if !self.root.join(FOLDER).join(DATABASE).exists() {
            return Ok(PerfMap::default());
        }
        let store = self.store()?;
        let id = match run {
            Some(run) => run_number(run),
            None => store.latest_traced()?,
        };
        let Some(id) = id else {
            return Ok(PerfMap::default());
        };
        let spans = store.spans(id)?;
        let called: Vec<Called<'_>> = spans
            .iter()
            .filter(|s| !s.synthesized)
            .map(|s| Called {
                id: &s.span_id,
                parent: s.parent_span_id.as_deref(),
                node: s.node.as_deref(),
                duration_ns: s.duration_ns,
            })
            .collect();
        let (nodes, links) = map_of(&called);
        let mut reader = graph.reader()?;
        let graph = crate::view::load(&mut reader, &self.root)?;
        let ancestors = nodes
            .iter()
            .filter_map(|n| {
                let id = timonier_core::NodeId::parse(n.node.clone()).ok()?;
                let mut line: Vec<String> = graph
                    .ancestors(&id)
                    .skip(1)
                    .map(|a| a.as_str().to_owned())
                    .collect();
                line.reverse();
                Some((n.node.clone(), line))
            })
            .collect();
        Ok(PerfMap {
            run: Some(format!("R-{id}")),
            nodes,
            links,
            ancestors,
        })
    }
}

impl Runs {
    /// The budgets of `.timonier/intent.json` checked on the last runs with traces (ADR 0052);
    /// `None` when the whiteboard has no budget (or does not read).
    #[tracing::instrument(skip_all)]
    pub(crate) fn budgets(&self, graph: &Store) -> Result<Option<BudgetsView>> {
        let text =
            match crate::tracking::read_intent(&self.root.join(".timonier").join("intent.json")) {
                Ok(text) => text,
                Err(_) => return Ok(None),
            };
        let Ok(intent) = timonier_core::intent::parse(&text) else {
            return Ok(None);
        };
        let intent = intent.normalized();
        if intent.budgets.is_empty() {
            return Ok(None);
        }
        let mut reader = graph.reader()?;
        let graph = crate::view::load(&mut reader, &self.root)?;
        let drift = timonier_core::drift::drift(&intent, &timonier_core::actual::actual(&graph));
        let lineage = |node: &str, into: &mut BTreeMap<String, Vec<String>>| {
            if into.contains_key(node) {
                return;
            }
            let Ok(id) = timonier_core::NodeId::parse(node.to_owned()) else {
                return;
            };
            let mut line: Vec<String> = graph
                .ancestors(&id)
                .skip(1)
                .map(|a| a.as_str().to_owned())
                .collect();
            line.reverse();
            into.insert(node.to_owned(), line);
        };
        let mut ancestors = BTreeMap::new();
        let mut runs = Vec::new();
        if self.root.join(FOLDER).join(DATABASE).exists() {
            let store = self.store()?;
            // Every finished run with traces but the smoke tests; each budget then takes the
            // last of them that reached what it is on (relecture n° 1, m5; n° 2, I-1).
            let judged = store
                .summaries(LISTED_RUNS)?
                .into_iter()
                .filter(|r| r.finished && r.traced && !r.smoke);
            for summary in judged {
                let Some(id) = run_number(&summary.run) else {
                    continue;
                };
                let spans = store.spans(id)?;
                for node in spans.iter().filter_map(|s| s.node.as_deref()) {
                    lineage(node, &mut ancestors);
                }
                let record = store.run(id)?.and_then(|r| r.record);
                let origin = summary.started_at_ms.saturating_mul(1_000_000);
                runs.push(BudgetRun {
                    run: summary.run,
                    spans: spans
                        .into_iter()
                        .filter(|s| !s.synthesized)
                        .map(|s| BudgetSpan {
                            id: s.span_id,
                            parent: s.parent_span_id,
                            node: s.node,
                            start_ns: s.start_ns.map(|ns| ns.saturating_sub(origin)),
                            duration_ns: s.duration_ns,
                        })
                        .collect(),
                    tests_us: record
                        .as_ref()
                        .map(|r| r.tests.iter().map(|t| t.duration_us).collect())
                        .unwrap_or_default(),
                    peak_bytes: summary.memory_bytes,
                    samples: record
                        .map(|r| {
                            r.measured
                                .samples
                                .iter()
                                .map(|s| (s.at_ms, s.memory_bytes))
                                .collect()
                        })
                        .unwrap_or_default(),
                });
            }
        }
        let checks = timonier_core::budgets::checks(&intent, &drift, &runs, &ancestors);
        let mut shown = BTreeMap::new();
        for check in &checks {
            for node in &check.boxes {
                lineage(node, &mut shown);
            }
        }
        Ok(Some(BudgetsView {
            checks,
            ancestors: shown,
        }))
    }
}

impl Runs {
    /// `get_perf` (ADR 0052): `component` found by its id, by the drawn component it is the
    /// anchor of, or by its name; `Err` says why there is nothing to give.
    #[tracing::instrument(skip_all)]
    pub(crate) fn component_perf(
        &self,
        graph: &Store,
        component: &str,
        run: Option<&str>,
    ) -> Result<std::result::Result<ComponentPerf, String>> {
        let mut reader = graph.reader()?;
        let loaded = crate::view::load(&mut reader, &self.root)?;
        let actual = timonier_core::actual::actual(&loaded);
        let intent = crate::tracking::read_intent(&self.root.join(".timonier").join("intent.json"))
            .ok()
            .and_then(|text| timonier_core::intent::parse(&text).ok())
            .map(|i| i.normalized());
        let drift = intent
            .as_ref()
            .map(|i| timonier_core::drift::drift(i, &actual));
        let wanted = timonier_core::drift::normalize(component);
        let found = timonier_core::NodeId::parse(component.to_owned())
            .ok()
            .filter(|id| loaded.contains(id))
            .map(|id| id.as_str().to_owned())
            .or_else(|| {
                let (intent, drift) = (intent.as_ref()?, drift.as_ref()?);
                let drawn = intent.components.iter().find(|c| {
                    c.id == component || timonier_core::drift::normalize(&c.name) == wanted
                })?;
                drift
                    .anchors
                    .iter()
                    .find(|a| a.component == drawn.id)?
                    .actual
                    .clone()
            })
            .or_else(|| {
                actual
                    .boxes
                    .iter()
                    .find(|b| timonier_core::drift::normalize(&b.name) == wanted)
                    .map(|b| b.id.clone())
            });
        let Some(node) = found else {
            return Ok(Err(format!(
                "no component `{component}` in the code nor on the whiteboard: give a box's id or name"
            )));
        };
        if !self.root.join(FOLDER).join(DATABASE).exists() {
            return Ok(Err(
                "no run yet: run the tests with `timonier test -- <command>`".to_owned(),
            ));
        }
        let store = self.store()?;
        let id = match run {
            Some(run) => run_number(run),
            None => store.latest_traced()?,
        };
        let Some(id) = id else {
            return Ok(Err(
                "no run with traces yet: run the tests with `timonier test`".to_owned(),
            ));
        };
        let Some(stored) = store.run(id)? else {
            return Ok(Err(format!("no run R-{id}")));
        };
        let spans = store.spans(id)?;
        let mut ancestors = BTreeMap::new();
        for span in &spans {
            let Some(at) = span.node.as_deref() else {
                continue;
            };
            if ancestors.contains_key(at) {
                continue;
            }
            if let Ok(at_id) = timonier_core::NodeId::parse(at.to_owned()) {
                let mut line: Vec<String> = loaded
                    .ancestors(&at_id)
                    .skip(1)
                    .map(|a| a.as_str().to_owned())
                    .collect();
                line.reverse();
                ancestors.insert(at.to_owned(), line);
            }
        }
        let boxes: std::collections::BTreeSet<String> = [node.clone()].into();
        let target = timonier_core::budgets::Target::Boxes(boxes.clone());
        let origin = stored.started_at_ms.saturating_mul(1_000_000);
        let record = stored.record.as_ref();
        let budget_run = BudgetRun {
            run: format!("R-{id}"),
            spans: spans
                .iter()
                .filter(|s| !s.synthesized)
                .map(|s| BudgetSpan {
                    id: s.span_id.clone(),
                    parent: s.parent_span_id.clone(),
                    node: s.node.clone(),
                    start_ns: s.start_ns.map(|ns| ns.saturating_sub(origin)),
                    duration_ns: s.duration_ns,
                })
                .collect(),
            tests_us: Vec::new(),
            peak_bytes: None,
            samples: record
                .map(|r| {
                    r.measured
                        .samples
                        .iter()
                        .map(|s| (s.at_ms, s.memory_bytes))
                        .collect()
                })
                .unwrap_or_default(),
        };
        let measured = timonier_core::budgets::measure(&target, &budget_run, &ancestors);
        let called: Vec<Called<'_>> = spans
            .iter()
            .filter(|s| !s.synthesized)
            .map(|s| Called {
                id: &s.span_id,
                parent: s.parent_span_id.as_deref(),
                node: s.node.as_deref(),
                duration_ns: s.duration_ns,
            })
            .collect();
        let (costs, _) = map_of(&called);
        let within =
            |at: &str| at == node || ancestors.get(at).is_some_and(|line| line.contains(&node));
        #[allow(clippy::cast_precision_loss)]
        let ms = |ns: u64| ns as f64 / 1e6;
        let mut inside: Vec<_> = costs.iter().filter(|c| within(&c.node)).collect();
        inside.sort_by_key(|c| std::cmp::Reverse(c.self_ns));
        let name_of = |at: &str| {
            timonier_core::NodeId::parse(at.to_owned())
                .ok()
                .and_then(|i| loaded.node(&i).map(|n| n.name.clone()))
                .unwrap_or_else(|| at.to_owned())
        };
        let tests: std::collections::BTreeSet<String> = spans
            .iter()
            .filter(|s| !s.synthesized && s.node.as_deref().is_some_and(within))
            .filter_map(|s| s.test_name.clone())
            .collect();
        let budgets = self
            .budgets(graph)?
            .map(|view| {
                view.checks
                    .into_iter()
                    .filter(|c| c.boxes.contains(&node))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Ok(ComponentPerf {
            name: name_of(&node),
            run: format!("R-{id}"),
            calls: measured.calls,
            p50_ms: timonier_core::budgets::percentile(&measured.latencies_ms, 0.5),
            p95_ms: timonier_core::budgets::percentile(&measured.latencies_ms, 0.95),
            memory_mb: measured.memory_mb,
            self_ms: inside.iter().map(|c| ms(c.self_ns)).sum(),
            inside: inside
                .iter()
                .take(10)
                .map(|c| InsidePerf {
                    node: c.node.clone(),
                    name: name_of(&c.node),
                    calls: c.calls,
                    self_ms: ms(c.self_ns),
                })
                .collect(),
            tests: tests.into_iter().collect(),
            budgets,
            node,
        }))
    }
}

/// One measure of one run: in words, its unit, its value.
type Measures = BTreeMap<String, (String, &'static str, f64)>;

impl Runs {
    /// What each measure of run `id` was (ADR 0053): the run's CPU work, each test's duration,
    /// each node's own time.
    fn measures(
        &self,
        store: &PerfStore,
        summary: &RunSummary,
        names: &dyn Fn(&str) -> String,
    ) -> Result<Measures> {
        let mut measures = Measures::new();
        let Some(id) = run_number(&summary.run) else {
            return Ok(measures);
        };
        #[allow(clippy::cast_precision_loss)]
        match (summary.instructions, summary.cpu_us) {
            (Some(count), _) => {
                measures.insert(
                    "run".into(),
                    ("the run".into(), "instructions", count as f64),
                );
            }
            (None, Some(us)) => {
                measures.insert("run".into(), ("the run".into(), "cpu_us", us as f64));
            }
            _ => {}
        }
        if let Some(record) = store.run(id)?.and_then(|r| r.record) {
            for test in &record.tests {
                let key = format!(
                    "test:{}::{}",
                    test.classname.as_deref().unwrap_or(""),
                    test.name
                );
                #[allow(clippy::cast_precision_loss)]
                measures.insert(
                    key,
                    (format!("test {}", test.name), "us", test.duration_us as f64),
                );
            }
        }
        let spans = store.spans(id)?;
        let called: Vec<Called<'_>> = spans
            .iter()
            .filter(|s| !s.synthesized)
            .map(|s| Called {
                id: &s.span_id,
                parent: s.parent_span_id.as_deref(),
                node: s.node.as_deref(),
                duration_ns: s.duration_ns,
            })
            .collect();
        // Past the retention, what each node cost was kept (ADR 0043).
        let nodes = if called.is_empty() {
            store.aggregates(id)?
        } else {
            map_of(&called).0
        };
        for node in nodes {
            #[allow(clippy::cast_precision_loss)]
            measures.insert(
                format!("node:{}", node.node),
                (names(&node.node), "ns", node.self_ns as f64),
            );
        }
        Ok(measures)
    }

    /// `perf/regressions` (ADR 0053): a batch of runs, the latest by default, against the median
    /// of the last 5 commits of the main branch measured on the same machine and command.
    #[tracing::instrument(skip_all)]
    pub(crate) fn regressions(
        &self,
        graph: &Store,
        batch: Option<&str>,
    ) -> Result<RegressionReport> {
        use timonier_core::perf::{Regression, RegressionNote, RegressionStatus};
        if !self.root.join(FOLDER).join(DATABASE).exists() {
            return Ok(RegressionReport {
                note: Some("no run yet".into()),
                note_kind: Some(RegressionNote::NoRun),
                ..RegressionReport::default()
            });
        }
        let store = self.store()?;
        let all = store.summaries(1000)?;
        let finished: Vec<&RunSummary> = all
            .iter()
            .filter(|r| r.finished && r.batch.is_some())
            .collect();
        let Some(name) = batch
            .map(str::to_owned)
            .or_else(|| finished.first().and_then(|r| r.batch.clone()))
        else {
            return Ok(RegressionReport {
                note: Some("no batch yet: run `timonier test --runs auto -- <command>`".into()),
                note_kind: Some(RegressionNote::NoBatch),
                ..RegressionReport::default()
            });
        };
        let candidates: Vec<&RunSummary> = finished
            .iter()
            .copied()
            .filter(|r| r.batch.as_deref() == Some(name.as_str()))
            .collect();
        let Some(first) = candidates.first() else {
            return Ok(RegressionReport {
                batch: Some(name),
                note: Some("no finished run in this batch".into()),
                note_kind: Some(RegressionNote::NoFinishedRun),
                ..RegressionReport::default()
            });
        };
        let same = |r: &&RunSummary| {
            r.fingerprint == first.fingerprint
                && r.command == first.command
                && r.traced == first.traced
        };
        let main = git::main_branch(&self.root);
        // The last 5 commits of the main branch, the candidate's aside, the newest first.
        let mut reference: Vec<String> = Vec::new();
        for run in finished.iter().copied().filter(same) {
            let (Some(commit), Some(branch)) = (&run.commit, &run.branch) else {
                continue;
            };
            // A run on a modified tree never stands for its commit (Q77).
            if *branch == main
                && run.worktree.is_none()
                && Some(commit) != first.commit.as_ref()
                && !reference.contains(commit)
            {
                reference.push(commit.clone());
            }
        }
        reference.truncate(timonier_core::budgets::RUNS);
        let source = first.cpu_source;
        if reference.is_empty() {
            let elsewhere = all.iter().any(|r| {
                r.finished
                    && r.branch.as_deref() == Some(main.as_str())
                    && r.fingerprint != first.fingerprint
            });
            return Ok(RegressionReport {
                batch: Some(name),
                commit: first.commit.clone(),
                source,
                note_kind: Some(if elsewhere {
                    RegressionNote::MachineChanged
                } else {
                    RegressionNote::NoReference
                }),
                main: Some(main.clone()),
                note: Some(if elsewhere {
                    format!(
                        "the machine changed: the reference starts again with the next runs on `{main}`"
                    )
                } else {
                    format!("no reference yet: run the same tests on `{main}` first")
                }),
                ..RegressionReport::default()
            });
        }
        let mut reader = graph.reader()?;
        let loaded = crate::view::load(&mut reader, &self.root)?;
        let names = |node: &str| {
            timonier_core::NodeId::parse(node.to_owned())
                .ok()
                .and_then(|id| loaded.node(&id).map(|n| n.name.clone()))
                .unwrap_or_else(|| {
                    node.rsplit([':', '/', '#'])
                        .next()
                        .unwrap_or(node)
                        .to_owned()
                })
        };
        let measures_of = |runs: &[&RunSummary]| -> Result<Vec<Measures>> {
            runs.iter()
                .map(|r| self.measures(&store, r, &names))
                .collect()
        };
        let candidate = measures_of(&candidates)?;
        let snapshots: Vec<Vec<Measures>> = reference
            .iter()
            .map(|commit| {
                let runs: Vec<&RunSummary> = finished
                    .iter()
                    .copied()
                    .filter(same)
                    .filter(|r| {
                        r.commit.as_ref() == Some(commit)
                            && r.branch.as_deref() == Some(main.as_str())
                            && r.worktree.is_none()
                    })
                    .collect();
                measures_of(&runs)
            })
            .collect::<Result<_>>()?;
        let values = |runs: &[Measures], key: &str| -> Vec<f64> {
            runs.iter()
                .filter_map(|m| m.get(key).map(|(_, _, v)| *v))
                .collect()
        };
        let judge = |runs: &[Measures], key: &str| {
            let reference: Vec<Vec<f64>> = snapshots
                .iter()
                .map(|s| values(s, key))
                .filter(|v| !v.is_empty())
                .collect();
            timonier_core::regression::judge(&values(runs, key), &reference)
        };
        // The batch before it, on the same commit: what confirms a regression (spec).
        let previous: Option<String> = finished
            .iter()
            .copied()
            .filter(same)
            // The same code: the same commit, and the same changes over it (Q77).
            .filter(|r| {
                r.commit == first.commit
                    && r.worktree == first.worktree
                    && r.batch.as_deref() != Some(name.as_str())
            })
            .find_map(|r| r.batch.clone());
        let before = match &previous {
            Some(previous) => {
                let runs: Vec<&RunSummary> = finished
                    .iter()
                    .copied()
                    .filter(|r| r.batch.as_ref() == Some(previous))
                    .collect();
                Some(measures_of(&runs)?)
            }
            None => None,
        };
        let keys: std::collections::BTreeSet<String> =
            candidate.iter().flat_map(|m| m.keys().cloned()).collect();
        let mut checked = 0;
        let mut regressions = Vec::new();
        for key in keys {
            let Some(verdict) = judge(&candidate, &key) else {
                continue;
            };
            checked += 1;
            if !verdict.regressed {
                continue;
            }
            let (label, unit) = candidate
                .iter()
                .find_map(|m| m.get(&key).map(|(l, u, _)| (l.clone(), *u)))
                .unwrap_or_default();
            let confirmed = before
                .as_ref()
                .and_then(|b| judge(b, &key))
                .is_some_and(|v| v.regressed);
            regressions.push(Regression {
                key,
                label,
                unit: unit.to_owned(),
                status: if confirmed {
                    RegressionStatus::Confirmed
                } else {
                    RegressionStatus::Probable
                },
                verdict,
            });
        }
        Ok(RegressionReport {
            batch: Some(name),
            commit: first.commit.clone(),
            reference,
            source,
            checked,
            regressions,
            // One run: Mann-Whitney can find nothing (relecture n° 1, m2).
            note: (candidates.len() < 2).then(|| {
                "one run in this batch: run `timonier test --runs auto` to compare".to_owned()
            }),
            note_kind: (candidates.len() < 2).then_some(RegressionNote::BatchTooSmall),
            main: Some(main),
        })
    }
}

/// Where the receiver hands what it takes (ADR 0048): the runs that take it, each span
/// attached to its node of the graph, at the head.
pub(crate) struct RunSink {
    pub(crate) runs: Arc<Runs>,
    pub(crate) graph: Store,
}

/// What the receiver took, by the run it names; what names no run of ours goes.
fn by_run<T>(items: Vec<T>, run: impl Fn(&T) -> &str) -> BTreeMap<u64, Vec<T>> {
    let mut runs: BTreeMap<u64, Vec<T>> = BTreeMap::new();
    for item in items {
        if let Some(id) = run_number(run(&item)) {
            runs.entry(id).or_default().push(item);
        }
    }
    runs
}

impl RunSink {
    /// The store, if run `id` takes what comes now.
    /// The store, when run `id` takes spans now and `token` is its own.
    fn taking(&self, id: u64, token: Option<&str>) -> Option<PerfStore> {
        let own = self
            .runs
            .tokens
            .lock()
            .ok()?
            .get(&id)
            .is_some_and(|t| Some(t.as_str()) == token);
        if !own {
            tracing::debug!(run = id, "spans without the run's token, dropped");
            return None;
        }
        let store = self.runs.store().ok()?;
        store.accepts(id).ok()?.then_some(store)
    }
}

impl timonier_otlp::Sink for RunSink {
    fn spans(&self, token: Option<&str>, spans: Vec<ReceivedSpan>) {
        let reader = self.graph.reader().ok();
        for (id, spans) in by_run(spans, |s| s.run.as_str()) {
            let Some(store) = self.taking(id, token) else {
                tracing::debug!(
                    run = id,
                    spans = spans.len(),
                    "spans of no open run, dropped"
                );
                continue;
            };
            let kept: Vec<KeptSpan> = spans
                .into_iter()
                .map(|span| {
                    let found = reader
                        .as_ref()
                        .and_then(|r| crate::attach::attach(r, &self.runs.root, &span));
                    let (node, attached_by) =
                        found.map_or((None, None), |(n, a)| (Some(n), Some(a)));
                    KeptSpan {
                        span,
                        node,
                        attached_by,
                    }
                })
                .collect();
            match store.add_spans(id, kept) {
                Ok(added) => tracing::debug!(
                    run = id,
                    kept = added.kept,
                    dropped = added.dropped,
                    "spans received"
                ),
                Err(error) => tracing::warn!(run = id, %error, "spans not kept"),
            }
        }
    }

    fn metrics(&self, token: Option<&str>, metrics: Vec<ReceivedMetric>) {
        for (id, points) in by_run(metrics, |m| m.run.as_str()) {
            let Some(store) = self.taking(id, token) else {
                continue;
            };
            if let Err(error) = store.add_metrics(id, points) {
                tracing::warn!(run = id, %error, "metric points not kept");
            }
        }
    }
}
