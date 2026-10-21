//! The whiteboard's performance budgets checked against the runs (PLAN.md 8.8, ADR 0052): each
//! budget aims at real boxes (a drawn component's anchor, a layer's components, an arrow's ends,
//! or the tests when attached to nothing); a run measures there each incoming call (a span of
//! the boxes whose parent is outside them, Q75); the median of the last runs is compared with
//! each limit. Pure: the daemon gathers the runs.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::drift::Drift;
use crate::intent::{Intent, IntentBudget};

/// How many of the last runs with traces that reached its target a budget is judged on (spec:
/// « médiane sur plusieurs runs »), `doctor`'s smoke tests aside.
pub const RUNS: usize = 5;

/// What a limit is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum LimitKind {
    P50Ms,
    P95Ms,
    MemoryMb,
    Allocations,
    Calls,
}

/// One limit of a budget, and what the runs measured for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct LimitCheck {
    pub kind: LimitKind,
    pub limit: f64,
    /// The median of the runs that measured it; `None` when none did.
    pub measured: Option<f64>,
    pub exceeded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum BudgetState {
    /// Every measured limit holds.
    Ok,
    Exceeded,
    /// No run measured any of its limits: no span reached it yet.
    Unmeasured,
    /// What it is attached to stands for nothing in the code.
    Unanchored,
}

/// A budget of the whiteboard, checked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct BudgetCheck {
    /// `N-…`.
    pub id: String,
    pub attached_to: Option<String>,
    /// What it is on, in words: « Parser », « Daemon → Store », « the tests ».
    pub on: String,
    pub state: BudgetState,
    /// The real boxes that show it on the map: the anchors, or an arrow's target.
    pub boxes: Vec<String>,
    pub limits: Vec<LimitCheck>,
    /// The runs it was judged on, the newest first.
    pub runs: Vec<String>,
}

/// `perf/budgets`: every budget checked, and the lineage of each box, for the map.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct BudgetsView {
    pub checks: Vec<BudgetCheck>,
    /// The ancestors of each box a check names, from the top, the box excluded.
    #[ts(type = "Record<string, Array<string>>")]
    pub ancestors: BTreeMap<String, Vec<String>>,
}

/// A span of a run, as the budgets need it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetSpan {
    pub id: String,
    pub parent: Option<String>,
    pub node: Option<String>,
    /// From the run's start; `None` when not dated.
    pub start_ns: Option<u64>,
    pub duration_ns: u64,
}

/// One run, as the budgets need it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BudgetRun {
    pub run: String,
    pub spans: Vec<BudgetSpan>,
    /// Each test's duration.
    pub tests_us: Vec<u64>,
    /// The run's peak memory.
    pub peak_bytes: Option<u64>,
    /// (milliseconds from the start, bytes) every 100 ms, when a scope measured the run.
    pub samples: Vec<(u64, u64)>,
}

/// What a budget aims at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Attached to nothing: the tests.
    Tests,
    Boxes(BTreeSet<String>),
    /// An arrow: the calls of `from` to `to`.
    Path {
        from: BTreeSet<String>,
        to: BTreeSet<String>,
    },
}

/// The real boxes a drawn component or layer stands for; empty when none.
fn boxes_of(id: &str, intent: &Intent, drift: &Drift) -> BTreeSet<String> {
    let anchor = |component: &str| {
        drift
            .anchors
            .iter()
            .find(|a| a.component == component)
            .and_then(|a| a.actual.clone())
    };
    if id.starts_with("C-") {
        return anchor(id).into_iter().collect();
    }
    intent
        .components
        .iter()
        .filter(|c| c.layer.as_deref() == Some(id))
        .filter_map(|c| anchor(&c.id))
        .collect()
}

/// What `budget` aims at, and its words; `None` when it stands for nothing in the code.
pub fn target_of(
    budget: &IntentBudget,
    intent: &Intent,
    drift: &Drift,
) -> (Option<Target>, String) {
    let name = |id: &str| {
        intent
            .components
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.clone())
            .or_else(|| {
                intent
                    .layers
                    .iter()
                    .find(|l| l.id == id)
                    .map(|l| l.name.clone())
            })
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| id.to_owned())
    };
    let Some(on) = budget.attached_to.as_deref() else {
        return (Some(Target::Tests), "the tests".to_owned());
    };
    if let Some(edge) = intent.edges.iter().find(|e| e.id == on) {
        let (from, to) = (
            boxes_of(&edge.src, intent, drift),
            boxes_of(&edge.dst, intent, drift),
        );
        let words = format!("{} → {}", name(&edge.src), name(&edge.dst));
        return if from.is_empty() || to.is_empty() {
            (None, words)
        } else {
            (Some(Target::Path { from, to }), words)
        };
    }
    let boxes = boxes_of(on, intent, drift);
    (
        (!boxes.is_empty()).then_some(Target::Boxes(boxes)),
        name(on),
    )
}

/// Whether `node` is `boxes` or inside one of them.
fn inside(node: &str, boxes: &BTreeSet<String>, ancestors: &BTreeMap<String, Vec<String>>) -> bool {
    boxes.contains(node)
        || ancestors
            .get(node)
            .is_some_and(|line| line.iter().any(|a| boxes.contains(a)))
}

/// What one run measured of a target: each call's duration, the calls, the peak memory during
/// them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measure {
    pub latencies_ms: Vec<f64>,
    pub calls: u64,
    pub memory_mb: Option<f64>,
}

/// The incoming calls of `target` in `run` (Q75).
pub fn measure(
    target: &Target,
    run: &BudgetRun,
    ancestors: &BTreeMap<String, Vec<String>>,
) -> Measure {
    #[allow(clippy::cast_precision_loss)]
    let mb = |bytes: u64| bytes as f64 / 1e6;
    #[allow(clippy::cast_precision_loss)]
    let ms = |ns: u64| ns as f64 / 1e6;
    if *target == Target::Tests {
        #[allow(clippy::cast_precision_loss)]
        let latencies_ms: Vec<f64> = run.tests_us.iter().map(|us| *us as f64 / 1e3).collect();
        return Measure {
            calls: latencies_ms.len() as u64,
            latencies_ms,
            memory_mb: run.peak_bytes.map(mb),
        };
    }
    let by_id: BTreeMap<&str, &BudgetSpan> = run.spans.iter().map(|s| (s.id.as_str(), s)).collect();
    let in_set = |span: &BudgetSpan, set: &BTreeSet<String>| {
        span.node
            .as_deref()
            .is_some_and(|n| inside(n, set, ancestors))
    };
    let parent_of = |span: &BudgetSpan| span.parent.as_deref().and_then(|p| by_id.get(p)).copied();
    let calls: Vec<&BudgetSpan> = run
        .spans
        .iter()
        .filter(|span| match target {
            Target::Tests => false,
            Target::Boxes(boxes) => {
                in_set(span, boxes) && !parent_of(span).is_some_and(|p| in_set(p, boxes))
            }
            Target::Path { from, to } => {
                in_set(span, to) && parent_of(span).is_some_and(|p| in_set(p, from))
            }
        })
        .collect();
    let memory_mb = (!run.samples.is_empty())
        .then(|| {
            calls
                .iter()
                .filter_map(|span| {
                    let start = span.start_ns? / 1_000_000;
                    let end = (span.start_ns? + span.duration_ns) / 1_000_000;
                    run.samples
                        .iter()
                        .filter(|(at, _)| *at + 100 >= start && *at <= end + 100)
                        .map(|(_, bytes)| *bytes)
                        .max()
                })
                .max()
                .map(mb)
        })
        .flatten();
    Measure {
        latencies_ms: calls.iter().map(|s| ms(s.duration_ns)).collect(),
        calls: calls.len() as u64,
        memory_mb,
    }
}

/// The value at `p` (0 to 1) of `values`, nearest rank; `None` when empty.
pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let rank = ((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted.get(rank - 1).copied()
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    Some(if sorted.len().is_multiple_of(2) {
        f64::midpoint(sorted[middle - 1], sorted[middle])
    } else {
        sorted[middle]
    })
}

/// `budget` checked on the measures of the runs (the newest first), each run's figure first,
/// then the median of those that have one.
pub fn check(
    budget: &IntentBudget,
    target: Option<&Target>,
    on: String,
    runs: &[(String, Measure)],
) -> BudgetCheck {
    let boxes = match target {
        Some(Target::Boxes(boxes)) => boxes.iter().cloned().collect(),
        Some(Target::Path { to, .. }) => to.iter().cloned().collect(),
        _ => Vec::new(),
    };
    #[allow(clippy::cast_precision_loss)]
    let limits: Vec<(LimitKind, f64)> = [
        (LimitKind::P50Ms, budget.p50_ms),
        (LimitKind::P95Ms, budget.p95_ms),
        (LimitKind::MemoryMb, budget.memory_mb),
        (LimitKind::Allocations, budget.allocations.map(|n| n as f64)),
        (LimitKind::Calls, budget.calls.map(|n| n as f64)),
    ]
    .into_iter()
    .filter_map(|(kind, limit)| limit.map(|l| (kind, l)))
    .collect();
    let checked: Vec<LimitCheck> = limits
        .into_iter()
        .map(|(kind, limit)| {
            #[allow(clippy::cast_precision_loss)]
            let per_run: Vec<f64> = runs
                .iter()
                .filter_map(|(_, m)| match kind {
                    LimitKind::P50Ms => percentile(&m.latencies_ms, 0.5),
                    LimitKind::P95Ms => percentile(&m.latencies_ms, 0.95),
                    LimitKind::MemoryMb => m.memory_mb,
                    // Nothing counts allocations yet (ADR 0052).
                    LimitKind::Allocations => None,
                    LimitKind::Calls => (m.calls > 0).then_some(m.calls as f64),
                })
                .collect();
            let measured = median(&per_run);
            LimitCheck {
                kind,
                limit,
                exceeded: measured.is_some_and(|m| m > limit),
                measured,
            }
        })
        .collect();
    let state = if target.is_none() {
        BudgetState::Unanchored
    } else if checked.iter().any(|c| c.exceeded) {
        BudgetState::Exceeded
    } else if checked.iter().all(|c| c.measured.is_none()) {
        BudgetState::Unmeasured
    } else {
        BudgetState::Ok
    };
    BudgetCheck {
        id: budget.id.clone(),
        attached_to: budget.attached_to.clone(),
        on,
        state,
        boxes,
        limits: checked,
        runs: runs.iter().map(|(run, _)| run.clone()).collect(),
    }
}

/// Every budget of `intent`, checked on `runs`, the newest first: each budget takes the last
/// [`RUNS`] that reached its target (relecture n° 2, I-1).
pub fn checks(
    intent: &Intent,
    drift: &Drift,
    runs: &[BudgetRun],
    ancestors: &BTreeMap<String, Vec<String>>,
) -> Vec<BudgetCheck> {
    intent
        .budgets
        .iter()
        .map(|budget| {
            let (target, on) = target_of(budget, intent, drift);
            // The last runs that reached what it is on: a run of one test, or of another
            // runner, says nothing of it (relecture n° 2, I-1).
            let measures: Vec<(String, Measure)> = match &target {
                Some(target) => runs
                    .iter()
                    .map(|run| (run.run.clone(), measure(target, run, ancestors)))
                    .filter(|(_, measured)| measured.calls > 0)
                    .take(RUNS)
                    .collect(),
                None => Vec::new(),
            };
            check(budget, target.as_ref(), on, &measures)
        })
        .collect()
}

/// `get_perf`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetPerf {
    /// A component of the code or of the whiteboard: its id (`pkg:cargo/parser`, `C-1`) or its
    /// name (`parser`, `Parser`).
    #[schemars(length(min = 1, max = 256))]
    pub component: String,
    /// A run (`R-…`); the last finished one with traces when absent, `doctor`'s smoke tests aside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(pattern(r"^R-[1-9][0-9]*$"))]
    pub run: Option<String>,
}

/// A function or file inside a component, and what it cost.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InsidePerf {
    pub node: String,
    pub name: String,
    pub calls: u64,
    pub self_ms: f64,
}

/// `get_perf`: what a component cost in a run, and its budgets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ComponentPerf {
    pub node: String,
    pub name: String,
    pub run: String,
    /// Its incoming calls (Q75), their latency, the peak memory during them.
    pub calls: u64,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub memory_mb: Option<f64>,
    /// The time spent in it, its calls out aside.
    pub self_ms: f64,
    /// What inside it took the most time, at most ten.
    pub inside: Vec<InsidePerf>,
    /// The tests whose spans reached it.
    pub tests: Vec<String>,
    /// The whiteboard's budgets on it.
    pub budgets: Vec<BudgetCheck>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::Anchor;
    use crate::intent::{IntentComponent, IntentEdge, IntentEdgeKind, IntentLayer, Policy};

    fn budget(id: &str, on: Option<&str>) -> IntentBudget {
        IntentBudget {
            id: id.to_owned(),
            attached_to: on.map(str::to_owned),
            p50_ms: None,
            p95_ms: Some(5.0),
            memory_mb: None,
            allocations: None,
            calls: Some(3),
        }
    }

    fn intent() -> Intent {
        let mut intent = Intent::empty();
        intent.layers.push(IntentLayer {
            id: "L-1".into(),
            name: "Core".into(),
        });
        for (id, name, layer) in [
            ("C-1", "Parser", Some("L-1")),
            ("C-2", "Store", None),
            ("C-3", "Web", None),
        ] {
            intent.components.push(IntentComponent {
                id: id.into(),
                name: name.into(),
                layer: layer.map(str::to_owned),
                role: String::new(),
            });
        }
        intent.edges.push(IntentEdge {
            id: "E-1".into(),
            src: "C-1".into(),
            dst: "C-2".into(),
            kind: IntentEdgeKind::DependsOn,
            policy: Policy::Allowed,
        });
        intent
    }

    fn drift() -> Drift {
        let anchor = |component: &str, actual: Option<&str>| Anchor {
            component: component.into(),
            name: component.into(),
            actual: actual.map(str::to_owned),
            candidates: 1,
        };
        Drift {
            anchors: vec![
                anchor("C-1", Some("pkg:cargo/parser")),
                anchor("C-2", Some("pkg:cargo/store")),
                anchor("C-3", None),
            ],
            ..Drift::default()
        }
    }

    fn span(id: &str, parent: Option<&str>, node: &str, ms: u64) -> BudgetSpan {
        BudgetSpan {
            id: id.into(),
            parent: parent.map(str::to_owned),
            node: Some(node.into()),
            start_ns: Some(0),
            duration_ns: ms * 1_000_000,
        }
    }

    fn ancestors() -> BTreeMap<String, Vec<String>> {
        [
            (
                "rust:parser::parse",
                vec!["pkg:cargo/parser", "file:parser/src/lib.rs"],
            ),
            (
                "rust:parser::lex",
                vec!["pkg:cargo/parser", "file:parser/src/lib.rs"],
            ),
            (
                "rust:store::write",
                vec!["pkg:cargo/store", "file:store/src/lib.rs"],
            ),
            ("rust:tests::main", vec!["pkg:cargo/tests"]),
        ]
        .into_iter()
        .map(|(n, a)| (n.to_owned(), a.into_iter().map(str::to_owned).collect()))
        .collect()
    }

    /// A test calls the parser, which lexes (inside) and writes to the store (a path).
    fn run(name: &str, parse_ms: u64) -> BudgetRun {
        BudgetRun {
            run: name.into(),
            spans: vec![
                span("t", None, "rust:tests::main", 100),
                span("p", Some("t"), "rust:parser::parse", parse_ms),
                span("l", Some("p"), "rust:parser::lex", 1),
                span("w", Some("p"), "rust:store::write", 2),
                span("p2", Some("t"), "rust:parser::parse", 2),
            ],
            tests_us: vec![4_000, 6_000],
            peak_bytes: Some(80_000_000),
            samples: vec![],
        }
    }

    #[test]
    fn a_component_s_calls_are_its_spans_called_from_outside() {
        let target = Target::Boxes(["pkg:cargo/parser".to_owned()].into());
        let measured = measure(&target, &run("R-1", 7), &ancestors());
        // `parse` twice from the test; `lex` is the parser calling itself.
        assert_eq!(measured.calls, 2);
        assert_eq!(measured.latencies_ms, [7.0, 2.0]);
        let path = Target::Path {
            from: ["pkg:cargo/parser".to_owned()].into(),
            to: ["pkg:cargo/store".to_owned()].into(),
        };
        assert_eq!(
            measure(&path, &run("R-1", 7), &ancestors()).latencies_ms,
            [2.0]
        );
        let tests = measure(&Target::Tests, &run("R-1", 7), &ancestors());
        assert_eq!((tests.calls, tests.memory_mb), (2, Some(80.0)));
    }

    /// Relecture n° 2, I-1: a run that does not reach the budget's target is no run of it; the
    /// last [`RUNS`] that do are.
    #[test]
    fn a_budget_takes_the_last_runs_that_reached_it() {
        let elsewhere = |name: &str| BudgetRun {
            spans: vec![span("x", None, "rust:tests::other", 3)],
            ..run(name, 1)
        };
        let runs = [
            elsewhere("R-7"),
            run("R-6", 9),
            run("R-5", 9),
            run("R-4", 9),
            run("R-3", 9),
            run("R-2", 9),
            run("R-1", 1),
        ];
        let checks = checks(
            &Intent {
                budgets: vec![budget("N-1", Some("C-1"))],
                ..intent()
            },
            &drift(),
            &runs,
            &ancestors(),
        );
        assert_eq!(checks[0].runs, ["R-6", "R-5", "R-4", "R-3", "R-2"]);
        let none = super::checks(
            &Intent {
                budgets: vec![budget("N-1", Some("C-1"))],
                ..intent()
            },
            &drift(),
            &[elsewhere("R-1")],
            &ancestors(),
        );
        assert_eq!(none[0].state, BudgetState::Unmeasured);
    }

    #[test]
    fn a_budget_is_judged_on_the_median_of_the_runs() {
        let runs = [run("R-3", 9), run("R-2", 8), run("R-1", 1)];
        let checks = checks(
            &Intent {
                budgets: vec![budget("N-1", Some("C-1"))],
                ..intent()
            },
            &drift(),
            &runs,
            &ancestors(),
        );
        let [parser] = checks.as_slice() else {
            panic!("{checks:?}")
        };
        assert_eq!(parser.on, "Parser");
        assert_eq!(parser.boxes, ["pkg:cargo/parser"]);
        assert_eq!(parser.runs, ["R-3", "R-2", "R-1"]);
        // p95 per run: 9, 8, 2 → median 8 > 5; calls 2 ≤ 3.
        let p95 = &parser.limits[0];
        assert_eq!(
            (p95.kind, p95.measured, p95.exceeded),
            (LimitKind::P95Ms, Some(8.0), true)
        );
        assert_eq!(
            (parser.limits[1].measured, parser.limits[1].exceeded),
            (Some(2.0), false)
        );
        assert_eq!(parser.state, BudgetState::Exceeded);
    }

    #[test]
    fn a_layer_an_arrow_the_tests_and_what_stands_for_nothing() {
        let intent = Intent {
            budgets: vec![
                budget("N-1", Some("L-1")),
                budget("N-2", Some("E-1")),
                budget("N-3", None),
                budget("N-4", Some("C-3")),
            ],
            ..intent()
        };
        let checks = checks(&intent, &drift(), &[run("R-1", 1)], &ancestors());
        let states: Vec<_> = checks.iter().map(|c| (c.on.as_str(), c.state)).collect();
        assert_eq!(
            states,
            [
                ("Core", BudgetState::Ok),
                ("Parser → Store", BudgetState::Ok),
                // Tests of 4 and 6 ms: the p95 is 6 > 5.
                ("the tests", BudgetState::Exceeded),
                ("Web", BudgetState::Unanchored),
            ]
        );
        assert_eq!(checks[1].boxes, ["pkg:cargo/store"]);
        // Nothing ran yet: unmeasured, not ok.
        let none = super::checks(&intent, &drift(), &[], &ancestors());
        assert_eq!(none[0].state, BudgetState::Unmeasured);
    }

    #[test]
    fn percentiles_by_nearest_rank() {
        let values: Vec<f64> = (1..=20).map(f64::from).collect();
        assert_eq!(percentile(&values, 0.5), Some(10.0));
        assert_eq!(percentile(&values, 0.95), Some(19.0));
        assert_eq!(percentile(&[3.0], 0.95), Some(3.0));
        assert_eq!(percentile(&[], 0.5), None);
    }
}
