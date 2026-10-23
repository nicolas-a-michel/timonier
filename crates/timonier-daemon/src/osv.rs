//! Known vulnerabilities from OSV.dev, on demand only (Q79, ADR 0055): one batched query for
//! every dependency, then the details of each vulnerability found, kept 24 hours in
//! `.timonier/cache/osv.json`. The network goes through `Fetch`, so that tests never reach it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use timonier_core::inventory::{CveCheck, Dependency, Ecosystem, Vulnerability};

/// How long an answer of OSV stands.
pub const KEPT_MS: i64 = 24 * 3_600_000;

/// At most this many details asked per inventory; the others keep their id alone.
const DETAILS: usize = 50;

const QUERY_BATCH: &str = "https://api.osv.dev/v1/querybatch";
const VULN: &str = "https://api.osv.dev/v1/vulns/";

/// A request to OSV: a URL, and the JSON body of a POST (a GET without).
pub type Fetch<'a> = &'a dyn Fn(&str, Option<&Value>) -> Result<Value, String>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Cache {
    /// `ecosystem/name@version` → the ids OSV gave, and when.
    #[serde(default)]
    found: BTreeMap<String, (i64, Vec<String>)>,
    /// id → its details, and when.
    #[serde(default)]
    details: BTreeMap<String, (i64, Detail)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Detail {
    summary: Option<String>,
    severity: Option<String>,
    aliases: Vec<String>,
}

/// An ecosystem's name in OSV's queries; `None` for the ones OSV does not know.
fn osv(ecosystem: Ecosystem) -> Option<&'static str> {
    timonier_index::tooling::packages(ecosystem).and_then(|p| p.osv)
}

fn key(dependency: &Dependency, version: &str) -> String {
    format!(
        "{}/{}@{version}",
        osv(dependency.ecosystem).unwrap_or("-"),
        dependency.name
    )
}

/// The known vulnerabilities of `dependencies`: from the cache when it is fresh, else from OSV
/// when `ask` (then kept); not checked when neither.
pub fn check(
    dependencies: &[Dependency],
    cache_path: &Path,
    now_ms: i64,
    ask: bool,
    fetch: Fetch<'_>,
) -> (Vec<Vulnerability>, CveCheck) {
    let mut cache: Cache = std::fs::read(cache_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let fresh = |at: i64| now_ms - at < KEPT_MS;
    // Dependencies of ecosystems OSV does not know alone: unknown, not absent.
    if !dependencies.is_empty() && dependencies.iter().all(|d| osv(d.ecosystem).is_none()) {
        let names: BTreeSet<&str> = dependencies.iter().map(|d| d.ecosystem.name()).collect();
        let names: Vec<&str> = names.into_iter().collect();
        return (
            Vec::new(),
            CveCheck::NotChecked {
                reason: format!("OSV knows none of these ecosystems ({})", names.join(", ")),
            },
        );
    }
    let versioned: Vec<(&Dependency, &str)> = dependencies
        .iter()
        .filter(|d| osv(d.ecosystem).is_some())
        .filter_map(|d| d.version.as_deref().map(|v| (d, v)))
        .collect();
    let missing: Vec<(&Dependency, &str)> = versioned
        .iter()
        .copied()
        .filter(|(d, v)| {
            !cache
                .found
                .get(&key(d, v))
                .is_some_and(|(at, _)| fresh(*at))
        })
        .collect();
    let mut asked = false;
    if !missing.is_empty() {
        if !ask {
            return (
                Vec::new(),
                CveCheck::NotChecked {
                    reason: "not asked (cve: false), and no fresh answer in the cache".to_owned(),
                },
            );
        }
        // OSV takes at most 1,000 queries a batch.
        for chunk in missing.chunks(1_000) {
            let queries: Vec<Value> = chunk
                .iter()
                .map(|(d, v)| json!({"package": {"name": d.name, "ecosystem": osv(d.ecosystem)}, "version": v}))
                .collect();
            let answer = match fetch(QUERY_BATCH, Some(&json!({ "queries": queries }))) {
                Ok(answer) => answer,
                Err(why) => {
                    return (
                        Vec::new(),
                        CveCheck::NotChecked {
                            reason: format!("OSV did not answer: {why}"),
                        },
                    );
                }
            };
            let results = answer["results"].as_array().cloned().unwrap_or_default();
            for ((d, v), result) in chunk.iter().zip(results) {
                let ids = result["vulns"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v["id"].as_str().map(str::to_owned))
                    .collect();
                cache.found.insert(key(d, v), (now_ms, ids));
            }
        }
        asked = true;
    }
    // The details, for the ids found that the cache does not hold fresh.
    let wanted: Vec<String> = versioned
        .iter()
        .flat_map(|(d, v)| {
            cache
                .found
                .get(&key(d, v))
                .map(|(_, ids)| ids.clone())
                .unwrap_or_default()
        })
        .filter(|id| !cache.details.get(id).is_some_and(|(at, _)| fresh(*at)))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .take(DETAILS)
        .collect();
    if ask {
        for id in wanted {
            if let Ok(vuln) = fetch(&format!("{VULN}{id}"), None) {
                let severity = vuln["database_specific"]["severity"]
                    .as_str()
                    .or_else(|| vuln["severity"][0]["score"].as_str())
                    .map(str::to_owned);
                cache.details.insert(
                    id,
                    (
                        now_ms,
                        Detail {
                            summary: vuln["summary"].as_str().map(str::to_owned),
                            severity,
                            aliases: vuln["aliases"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|a| a.as_str().map(str::to_owned))
                                .collect(),
                        },
                    ),
                );
                asked = true;
            }
        }
    }
    if asked {
        if let Some(folder) = cache_path.parent() {
            let _ = std::fs::create_dir_all(folder);
            // The folder keeps itself out of git, as `.timonier/perf/` does (ADR 0043).
            let ignore = folder.join(".gitignore");
            if !ignore.exists() {
                let _ = std::fs::write(
                    &ignore,
                    "# Written by Timonier: caches stay out of git.\n*\n",
                );
            }
        }
        let _ = std::fs::write(cache_path, serde_json::to_vec(&cache).unwrap_or_default());
    }
    let oldest = versioned
        .iter()
        .filter_map(|(d, v)| cache.found.get(&key(d, v)).map(|(at, _)| *at))
        .min()
        .unwrap_or(now_ms);
    let mut vulnerabilities = Vec::new();
    for (d, v) in &versioned {
        for id in cache
            .found
            .get(&key(d, v))
            .map(|(_, ids)| ids.as_slice())
            .unwrap_or_default()
        {
            let detail = cache
                .details
                .get(id)
                .map(|(_, d)| d.clone())
                .unwrap_or_default();
            vulnerabilities.push(Vulnerability {
                id: id.clone(),
                ecosystem: d.ecosystem,
                package: d.name.clone(),
                version: (*v).to_owned(),
                aliases: detail.aliases,
                summary: detail.summary,
                severity: detail.severity,
            });
        }
    }
    (
        vulnerabilities,
        CveCheck::Checked {
            at_ms: oldest,
            cached: !asked,
        },
    )
}

/// OSV over HTTPS (`ureq`), blocking: call it off the runtime. `TIMONIER_OFFLINE=1` stops it.
pub fn https(url: &str, body: Option<&Value>) -> Result<Value, String> {
    if std::env::var("TIMONIER_OFFLINE").is_ok_and(|v| v == "1") {
        return Err("TIMONIER_OFFLINE=1".to_owned());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .build()
        .into();
    let response = match body {
        Some(body) => agent
            .post(url)
            .header("content-type", "application/json")
            .send(serde_json::to_vec(body).map_err(|e| e.to_string())?),
        None => agent.get(url).call(),
    };
    let mut response = response.map_err(|e| e.to_string())?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use timonier_core::inventory::{Ecosystem, Scope};

    use super::*;

    fn dep(name: &str, version: &str) -> Dependency {
        Dependency {
            ecosystem: Ecosystem::new("cargo", 0),
            name: name.to_owned(),
            version: Some(version.to_owned()),
            license: None,
            direct: true,
            scope: Scope::Normal,
            used_by: vec![],
            function: None,
        }
    }

    #[test]
    fn vulnerabilities_come_from_osv_then_from_the_cache_for_a_day() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache/osv.json");
        let asked = RefCell::new(Vec::<String>::new());
        let fetch = |url: &str, body: Option<&Value>| -> Result<Value, String> {
            asked.borrow_mut().push(url.to_owned());
            if body.is_some() {
                Ok(json!({"results": [{"vulns": [{"id": "RUSTSEC-2020-0071"}]}, {}]}))
            } else {
                Ok(
                    json!({"summary": "Potential segfault", "aliases": ["CVE-2020-26235"],
                          "severity": [{"type": "CVSS_V3", "score": "CVSS:3.1/AV:L"}]}),
                )
            }
        };
        let deps = [dep("time", "0.1.0"), dep("serde", "1.0.0")];
        let (found, state) = check(&deps, &cache, 1_000, true, &fetch);
        assert_eq!(
            state,
            CveCheck::Checked {
                at_ms: 1_000,
                cached: false
            }
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].package, "time");
        assert_eq!(found[0].aliases, ["CVE-2020-26235"]);
        assert_eq!(found[0].severity.as_deref(), Some("CVSS:3.1/AV:L"));
        assert_eq!(asked.borrow().len(), 2);
        assert!(dir.path().join("cache/.gitignore").exists());
        // Within the day: nothing asked, from the cache, even with `cve: false`.
        let (again, state) = check(&deps, &cache, 2_000, false, &fetch);
        assert_eq!(again, found);
        assert_eq!(
            state,
            CveCheck::Checked {
                at_ms: 1_000,
                cached: true
            }
        );
        assert_eq!(asked.borrow().len(), 2);
        // A day later, and not asked: unknown, never « none ».
        let (late, state) = check(&deps, &cache, 1_000 + KEPT_MS, false, &fetch);
        assert!(late.is_empty());
        assert!(matches!(state, CveCheck::NotChecked { .. }));
    }

    #[test]
    fn osv_down_says_not_checked() {
        let dir = tempfile::tempdir().unwrap();
        let fetch =
            |_: &str, _: Option<&Value>| -> Result<Value, String> { Err("timed out".into()) };
        let (found, state) = check(
            &[dep("time", "0.1.0")],
            &dir.path().join("osv.json"),
            0,
            true,
            &fetch,
        );
        assert!(found.is_empty());
        assert_eq!(
            state,
            CveCheck::NotChecked {
                reason: "OSV did not answer: timed out".to_owned()
            }
        );
    }

    /// T-124: vcpkg's ports alone are not checked, and OSV is not asked.
    #[test]
    fn ecosystems_osv_does_not_know_are_not_checked() {
        let dir = tempfile::tempdir().unwrap();
        let fetch = |_: &str, _: Option<&Value>| -> Result<Value, String> {
            panic!("OSV must not be asked")
        };
        let mut fmt = dep("fmt", "12.2.0");
        fmt.ecosystem = Ecosystem::new("vcpkg", 6);
        let (found, state) = check(&[fmt], &dir.path().join("osv.json"), 0, true, &fetch);
        assert!(found.is_empty());
        assert!(matches!(state, CveCheck::NotChecked { .. }), "{state:?}");
    }
}
