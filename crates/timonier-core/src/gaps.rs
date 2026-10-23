//! What the instrumentation of a run does not cover (ADR 0049): public functions no span was
//! seen in, spans without their `code.*` attributes, and the overhead against the runs without
//! traces. Pure: the daemon gathers the facts, this says what they mean.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::perf::CpuSource;

/// `get_instrumentation_gaps`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetInstrumentationGaps {
    /// A run (`R-…`); the last finished one with traces when absent, `doctor`'s smoke tests aside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(pattern(r"^R-[1-9][0-9]*$"))]
    pub run: Option<String>,
    /// Where the previous page of never seen functions stopped (`next_cursor`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(pattern(r"^[0-9]{1,9}$"))]
    pub cursor: Option<String>,
    /// How many never seen functions per page: 1 to 500, 100 by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 500))]
    pub limit: Option<u32>,
}

/// `get_instrumentation_conventions`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetInstrumentationConventions {
    /// The language, as the packs name it: `rust`, `typescript`.
    #[schemars(pattern(r"^[a-z][a-z0-9_+-]{0,31}$"))]
    pub language: String,
}

pub const DEFAULT_LIMIT: u32 = 100;
pub const MAX_LIMIT: u32 = 500;

/// A function or method of the graph, as the gaps need it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callable {
    pub id: String,
    pub name: String,
    /// Relative to the root.
    pub file: Option<String>,
    /// 0-based, as in the graph.
    pub line: Option<u32>,
    pub signature: Option<String>,
}

/// What a language counts as public and as test code (ADR 0049, 0077): its pack's `[code]`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageCode {
    /// Which functions are boundaries; none: no function of the language is.
    #[serde(default)]
    pub public: Option<PublicRule>,
    #[serde(default)]
    pub tests: TestRule,
    /// What the report can only approximate for the language, said once its functions count.
    #[serde(default)]
    pub approximation: Option<String>,
    /// A span's `code.function` names the function as its id does, after `<language>:`.
    #[serde(default)]
    pub function_ids: bool,
}

/// `[code.public]`: a function is public when its signature starts with one of
/// `signature_starts` or holds one of `signature_words`; with neither, when its name starts
/// with a capital letter if `capitalised`; otherwise unless its signature starts with one of
/// `hidden_starts`, holds one of `hidden_words`, or its name starts with one of
/// `hidden_names`, a member (a name holding one of `members`) being public whatever.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicRule {
    #[serde(default)]
    pub signature_starts: Vec<String>,
    #[serde(default)]
    pub signature_words: Vec<String>,
    #[serde(default)]
    pub capitalised: bool,
    #[serde(default)]
    pub hidden_starts: Vec<String>,
    #[serde(default)]
    pub hidden_words: Vec<String>,
    #[serde(default)]
    pub hidden_names: Vec<String>,
    #[serde(default)]
    pub members: Vec<String>,
}

/// `[code.tests]`: test code, which the gaps leave out: marks in an id, patterns of file names
/// and of folder names (case-insensitive), `*` standing for any text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestRule {
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub folders: Vec<String>,
}

/// Every language's rules, in the packs' order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodeRules {
    pub languages: Vec<(String, LanguageCode)>,
}

impl CodeRules {
    fn of(&self, callable: &Callable) -> Option<&LanguageCode> {
        let language = callable.id.split_once(':')?.0;
        self.languages
            .iter()
            .find(|(name, _)| name == language)
            .map(|(_, code)| code)
    }
}

/// Whether `text` matches `pattern`, where `*` stands for any text.
fn matches(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, rest)) => {
            let Some(text) = text.strip_prefix(head) else {
                return false;
            };
            if rest.is_empty() {
                return true;
            }
            (0..=text.len())
                .filter(|&at| text.is_char_boundary(at))
                .any(|at| matches(rest, &text[at..]))
        }
    }
}

/// Test code, which the gaps leave out: what any language's rules mark (an id, a file, a
/// folder), and the folders named `test` or `tests`.
pub fn is_test_code(callable: &Callable, rules: &CodeRules) -> bool {
    let file = callable.file.as_deref().unwrap_or_default();
    let name = file.rsplit('/').next().unwrap_or(file);
    let folders: Vec<String> = file.split('/').map(str::to_lowercase).collect();
    if folders.iter().any(|part| part == "test" || part == "tests") {
        return true;
    }
    rules.languages.iter().any(|(_, code)| {
        let tests = &code.tests;
        tests
            .ids
            .iter()
            .any(|mark| callable.id.contains(mark.as_str()))
            || tests.files.iter().any(|pattern| matches(pattern, name))
            || tests.folders.iter().any(|pattern| {
                let pattern = pattern.to_lowercase();
                folders.iter().any(|part| matches(&pattern, part))
            })
    })
}

/// Whether the function is a boundary the conventions instrument (ADR 0049), as its language's
/// `[code.public]` says; test code never is.
pub fn is_public(callable: &Callable, rules: &CodeRules) -> bool {
    if is_test_code(callable, rules) {
        return false;
    }
    let Some(rule) = rules.of(callable).and_then(|code| code.public.as_ref()) else {
        return false;
    };
    let signature = callable
        .signature
        .as_deref()
        .unwrap_or_default()
        .trim_start();
    let name = callable.name.as_str();
    let word = |words: &[String]| {
        signature
            .split_whitespace()
            .any(|w| words.iter().any(|word| word == w))
    };
    let starts = |starts: &[String]| starts.iter().any(|s| signature.starts_with(s.as_str()));
    if !rule.signature_starts.is_empty() || !rule.signature_words.is_empty() {
        return starts(&rule.signature_starts) || word(&rule.signature_words);
    }
    if rule.capitalised {
        return name.starts_with(|c: char| c.is_uppercase());
    }
    if rule.members.iter().any(|m| name.contains(m.as_str())) {
        return true;
    }
    !(starts(&rule.hidden_starts)
        || word(&rule.hidden_words)
        || rule
            .hidden_names
            .iter()
            .any(|h| name.starts_with(h.as_str())))
}

/// A public function no span of the run was seen in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NeverSeen {
    pub id: String,
    pub name: String,
    pub file: Option<String>,
    /// 1-based, as editors and `code.line.number` count.
    pub line: Option<u32>,
}

/// Spans of one name that lack some of the `code.*` attributes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WithoutCode {
    pub name: String,
    pub spans: u64,
    /// Among `code.function.name`, `code.file.path`, `code.line.number`.
    pub missing: Vec<String>,
}

/// What the run's spans say, gathered by the daemon.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpanFacts {
    /// Spans received, the synthesized roots aside.
    pub spans: u64,
    /// The nodes a span is attached to by ID, name or line (ADR 0048).
    pub seen: BTreeSet<String>,
    pub without_code: Vec<WithoutCode>,
    /// Received spans attached to no node.
    pub unattached: u64,
    /// Counted beyond the cap of the run, not kept.
    pub dropped: u64,
}

/// What one comparable run cost, in its machine's CPU source.
#[derive(Clone, Debug, PartialEq)]
pub struct RunCost {
    pub run: String,
    pub traced: bool,
    pub cost: f64,
}

/// The overhead of the instrumentation: the median cost of the runs with traces against that of
/// the runs without, of the same commit, command and machine (ADR 0049).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Overhead {
    pub source: CpuSource,
    pub traced_runs: Vec<String>,
    pub untraced_runs: Vec<String>,
    /// Instructions, or CPU microseconds.
    pub traced: f64,
    pub untraced: f64,
    /// `(traced - untraced) / untraced`, in percent.
    pub percent: f64,
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        f64::midpoint(values[middle - 1], values[middle])
    } else {
        values[middle]
    })
}

/// The overhead, or why it is not known.
pub fn overhead(source: CpuSource, costs: &[RunCost]) -> Result<Overhead, String> {
    let of = |traced: bool| -> (Vec<String>, Vec<f64>) {
        costs
            .iter()
            .filter(|c| c.traced == traced)
            .map(|c| (c.run.clone(), c.cost))
            .unzip()
    };
    let (traced_runs, mut traced) = of(true);
    let (untraced_runs, mut untraced) = of(false);
    let (Some(traced), Some(untraced)) = (median(&mut traced), median(&mut untraced)) else {
        return Err(
            "no run without traces of the same commit, command and machine: run \
             `timonier test --without-traces -- <the same command>`"
                .to_owned(),
        );
    };
    if untraced <= 0.0 {
        return Err("the runs without traces cost nothing measurable".to_owned());
    }
    Ok(Overhead {
        source,
        traced_runs,
        untraced_runs,
        traced,
        untraced,
        percent: (traced - untraced) / untraced * 100.0,
    })
}

/// The report of `get_instrumentation_gaps`, one page of the never seen functions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InstrumentationGaps {
    pub run: String,
    pub snapshot: u64,
    pub spans: u64,
    pub public_functions: u64,
    pub never_seen_count: u64,
    pub never_seen: Vec<NeverSeen>,
    /// Give it as `cursor` for the next page; absent on the last.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub without_code: Vec<WithoutCode>,
    pub unattached: u64,
    pub dropped: u64,
    pub overhead: Option<Overhead>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overhead_unknown: Option<String>,
    /// What the report could only approximate.
    pub approximations: Vec<String>,
}

/// Where the report of a run comes from.
pub struct Facts<'a> {
    pub run: String,
    pub snapshot: u64,
    pub callables: &'a [Callable],
    pub spans: &'a SpanFacts,
    pub overhead: Result<Overhead, String>,
    /// What each language counts as public and as test code.
    pub rules: &'a CodeRules,
}

/// The report, its never seen functions from `cursor`, `limit` of them.
pub fn report(facts: Facts<'_>, cursor: usize, limit: usize) -> InstrumentationGaps {
    let public: Vec<&Callable> = facts
        .callables
        .iter()
        .filter(|c| is_public(c, facts.rules))
        .collect();
    let never_seen: Vec<NeverSeen> = public
        .iter()
        .filter(|c| !facts.spans.seen.contains(&c.id))
        .map(|c| NeverSeen {
            id: c.id.clone(),
            name: c.name.clone(),
            file: c.file.clone(),
            line: c.line.map(|l| l + 1),
        })
        .collect();
    let end = cursor.saturating_add(limit).min(never_seen.len());
    let page = never_seen
        .get(cursor.min(end)..end)
        .unwrap_or_default()
        .to_vec();
    let approximations = facts
        .rules
        .languages
        .iter()
        .filter_map(|(language, code)| {
            let said = code.approximation.as_ref()?;
            let prefix = format!("{language}:");
            public
                .iter()
                .any(|c| c.id.starts_with(&prefix))
                .then(|| said.clone())
        })
        .collect();
    let (overhead, overhead_unknown) = match facts.overhead {
        Ok(overhead) => (Some(overhead), None),
        Err(why) => (None, Some(why)),
    };
    InstrumentationGaps {
        run: facts.run,
        snapshot: facts.snapshot,
        spans: facts.spans.spans,
        public_functions: public.len() as u64,
        never_seen_count: never_seen.len() as u64,
        next_cursor: (end < never_seen.len()).then(|| end.to_string()),
        never_seen: page,
        without_code: facts.spans.without_code.clone(),
        unattached: facts.spans.unattached,
        dropped: facts.spans.dropped,
        overhead,
        overhead_unknown,
        approximations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn callable(id: &str, signature: &str, file: &str) -> Callable {
        Callable {
            id: id.to_owned(),
            name: id.rsplit([':', '#', '.']).next().unwrap().to_owned(),
            file: Some(file.to_owned()),
            line: Some(4),
            signature: Some(signature.to_owned()),
        }
    }

    /// Rules as a pack gives them: Rust's `pub`, Go's capital letter, Python's `_`.
    fn rules() -> CodeRules {
        let code = |public: PublicRule, tests: TestRule| LanguageCode {
            public: Some(public),
            tests,
            approximation: None,
            function_ids: false,
        };
        CodeRules {
            languages: vec![
                (
                    "rust".to_owned(),
                    code(
                        PublicRule {
                            signature_starts: vec!["pub ".to_owned(), "pub(".to_owned()],
                            ..PublicRule::default()
                        },
                        TestRule {
                            ids: vec!["::tests::".to_owned()],
                            ..TestRule::default()
                        },
                    ),
                ),
                (
                    "go".to_owned(),
                    code(
                        PublicRule {
                            capitalised: true,
                            ..PublicRule::default()
                        },
                        TestRule {
                            files: vec!["*_test.go".to_owned()],
                            ..TestRule::default()
                        },
                    ),
                ),
                (
                    "python".to_owned(),
                    LanguageCode {
                        approximation: Some("Python: by the name".to_owned()),
                        ..code(
                            PublicRule {
                                hidden_names: vec!["_".to_owned()],
                                ..PublicRule::default()
                            },
                            TestRule {
                                folders: vec!["*.tests".to_owned()],
                                ..TestRule::default()
                            },
                        )
                    },
                ),
            ],
        }
    }

    #[test]
    fn patterns_stand_for_any_text_with_a_star() {
        assert!(matches("*_test.go", "cart_test.go"));
        assert!(matches("test_*.py", "test_cart.py"));
        assert!(matches("*.test.*", "a.test.ts"));
        assert!(!matches("*.test.*", "a.ts"));
        assert!(matches("exact", "exact"));
        assert!(!matches("exact", "exactly"));
        assert!(matches("*", ""));
    }

    #[test]
    fn a_language_s_rules_say_what_is_public_and_what_is_test_code() {
        let rules = rules();
        let public = |id: &str, signature: &str, file: &str| {
            is_public(&callable(id, signature, file), &rules)
        };
        assert!(public("rust:demo::f", "pub fn f()", "src/lib.rs"));
        assert!(public("rust:demo::g", "  pub(crate) fn g()", "src/lib.rs"));
        assert!(!public("rust:demo::p", "fn p()", "src/lib.rs"));
        assert!(!public("rust:demo::tests::h", "pub fn h()", "src/lib.rs"));
        assert!(public("go:shop/cart#Total", "func Total()", "cart/cart.go"));
        assert!(!public(
            "go:shop/cart#round",
            "func round()",
            "cart/cart.go"
        ));
        // Any language's test marks apply to every function.
        assert!(!public("rust:demo::t", "pub fn t()", "src/a_test.go"));
        assert!(public("python:shop#total", "def total():", "src/shop.py"));
        assert!(!public(
            "python:shop#_round",
            "def _round():",
            "src/shop.py"
        ));
        assert!(!public(
            "python:shop#total",
            "def total():",
            "src/Shop.Tests/a.py"
        ));
        // The folders `test` and `tests` are test code whatever the language.
        assert!(!public("rust:demo::f", "pub fn f()", "tests/it.rs"));
        assert!(!public("go:shop#Total", "func Total()", "Test/a.go"));
        // A language without rules has no public function.
        assert!(!public("cobol:shop#total", "total", "src/shop.cbl"));
    }

    #[test]
    fn the_overhead_compares_medians_and_says_when_it_cannot() {
        let cost = |run: &str, traced, cost| RunCost {
            run: run.to_owned(),
            traced,
            cost,
        };
        let costs = [
            cost("R-1", true, 120.0),
            cost("R-2", true, 130.0),
            cost("R-3", true, 900.0),
            cost("R-4", false, 100.0),
        ];
        let overhead = overhead(CpuSource::CpuTime, &costs).unwrap();
        assert_eq!((overhead.traced, overhead.untraced), (130.0, 100.0));
        assert!((overhead.percent - 30.0).abs() < 1e-9);
        assert_eq!(overhead.untraced_runs, ["R-4"]);
        let none = super::overhead(CpuSource::CpuTime, &costs[..3]).unwrap_err();
        assert!(none.contains("--without-traces"), "{none}");
    }

    #[test]
    fn the_report_pages_the_never_seen_functions() {
        let callables: Vec<Callable> = (0..5)
            .map(|i| callable(&format!("rust:demo::f{i}"), "pub fn f()", "src/lib.rs"))
            .chain([callable("rust:demo::p", "fn p()", "src/lib.rs")])
            .collect();
        let spans = SpanFacts {
            spans: 3,
            seen: ["rust:demo::f0".to_owned()].into(),
            without_code: vec![WithoutCode {
                name: "bare".to_owned(),
                spans: 2,
                missing: vec!["code.file.path".to_owned()],
            }],
            unattached: 2,
            dropped: 0,
        };
        let rules = rules();
        let facts = |cursor, limit| {
            report(
                Facts {
                    run: "R-1".to_owned(),
                    snapshot: 4,
                    callables: &callables,
                    spans: &spans,
                    overhead: Err("none".to_owned()),
                    rules: &rules,
                },
                cursor,
                limit,
            )
        };
        let first = facts(0, 3);
        assert_eq!((first.public_functions, first.never_seen_count), (5, 4));
        let ids: Vec<_> = first.never_seen.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["rust:demo::f1", "rust:demo::f2", "rust:demo::f3"]);
        assert_eq!(first.never_seen[0].line, Some(5));
        assert_eq!(first.next_cursor.as_deref(), Some("3"));
        let last = facts(3, 3);
        assert_eq!(last.never_seen.len(), 1);
        assert_eq!(last.next_cursor, None);
        assert!(facts(40, 3).never_seen.is_empty());
        assert_eq!(first.overhead_unknown.as_deref(), Some("none"));
        assert!(first.approximations.is_empty());
    }
}
