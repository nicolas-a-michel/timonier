//! Components (ADR 0016): directories first, then Leiden for a level that stays too large, with
//! stable keys from one partition to the next.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use timonier_core::NodeId;

use crate::cluster::{self, Graph};

/// A level with at most this many elements stays flat (ADR 0016).
pub const MAX_FLAT: usize = 12;

/// Leiden's resolution (ADR 0016).
const RESOLUTION: f64 = 1.0;

/// Where a component hangs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Parent {
    Repo,
    Package(NodeId),
    Component(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    pub name: String,
    pub parent: Parent,
}

/// The components of the project, and the component of each element (a package or a file)
/// that has one. Keys are those of `component:<key>` ids.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Components {
    pub components: BTreeMap<String, Component>,
    pub assignment: BTreeMap<NodeId, String>,
    /// The elements the last clustering placed, in a component or alone. An element outside
    /// this set arrived since, and joins its directory neighbours (ADR 0016).
    pub clustered: BTreeSet<NodeId>,
}

impl Components {
    /// The elements of each component.
    /// Keys whose chain of parents ends at the repo or at one of `packages`: the others hang
    /// from a package that is gone (renamed, or its manifest removed).
    pub fn valid(&self, packages: &BTreeSet<NodeId>) -> BTreeSet<&str> {
        let mut valid = BTreeSet::new();
        for key in self.components.keys() {
            let mut current = key.as_str();
            let mut seen = BTreeSet::new();
            let ok = loop {
                if !seen.insert(current) {
                    break false;
                }
                match self.components.get(current).map(|c| &c.parent) {
                    Some(Parent::Repo) => break true,
                    Some(Parent::Package(id)) => break packages.contains(id),
                    Some(Parent::Component(parent)) => current = parent,
                    None => break false,
                }
            };
            if ok {
                valid.insert(key.as_str());
            }
        }
        valid
    }

    pub fn members(&self) -> BTreeMap<&str, Vec<&NodeId>> {
        let mut members: BTreeMap<&str, Vec<&NodeId>> = BTreeMap::new();
        for (item, key) in &self.assignment {
            members.entry(key.as_str()).or_default().push(item);
        }
        members
    }
}

/// One element of a level: its node id and its path, relative to the level, that directories
/// are read from. `real` is its path in the repository, for the keys.
#[derive(Clone, Debug)]
pub struct Element {
    pub id: NodeId,
    pub path: String,
    pub real: String,
}

/// Lays out one level: the repository (packages and loose files) or a package (its files).
/// `weights` are undirected weights between elements, by index; `previous` gives the component
/// each element had, to keep keys and to start Leiden. A `transparent` directory at the root of
/// the level is no component of its own, but its subdirectories are (`src/` of a package).
pub fn layout(
    elements: &[Element],
    weights: &HashMap<(usize, usize), f64>,
    parent: Parent,
    previous: &BTreeMap<NodeId, String>,
    transparent: Option<&str>,
    out: &mut Components,
) {
    let level = Level {
        elements,
        weights,
        previous,
        transparent,
    };
    let all: Vec<usize> = (0..elements.len()).collect();
    level.split(&all, "", parent, out);
}

struct Level<'a> {
    elements: &'a [Element],
    weights: &'a HashMap<(usize, usize), f64>,
    previous: &'a BTreeMap<NodeId, String>,
    transparent: Option<&'a str>,
}

impl Level<'_> {
    fn split(&self, indices: &[usize], dir: &str, parent: Parent, out: &mut Components) {
        if indices.len() <= MAX_FLAT {
            return;
        }
        // Directory (with its trailing `/`) → its name and its elements.
        let mut buckets: BTreeMap<String, (String, Vec<usize>)> = BTreeMap::new();
        let mut loose: Vec<usize> = Vec::new();
        for &i in indices {
            let path = &self.elements[i].path;
            let rest = path.strip_prefix(dir).unwrap_or(path);
            let (prefix, rest) = match self.transparent.filter(|_| dir.is_empty()) {
                Some(t) => match rest.strip_prefix(&format!("{t}/")) {
                    Some(inner) => (format!("{t}/"), inner),
                    None => (String::new(), rest),
                },
                None => (String::new(), rest),
            };
            match rest.split_once('/') {
                Some((segment, _)) => buckets
                    .entry(format!("{dir}{prefix}{segment}/"))
                    .or_insert_with(|| (segment.to_owned(), Vec::new()))
                    .1
                    .push(i),
                None => loose.push(i),
            }
        }
        // One directory holds everything: it adds nothing, go inside.
        if loose.is_empty() && buckets.len() == 1 {
            let (inner, (_, inside)) = buckets.into_iter().next().expect("one bucket");
            self.split(&inside, &inner, parent, out);
            return;
        }
        for (inner, (name, inside)) in buckets {
            if inside.len() < 2 {
                loose.extend(inside);
                continue;
            }
            let key = format!("dir/{}", real_dir(&self.elements[inside[0]], &inner));
            out.components.insert(
                key.clone(),
                Component {
                    name,
                    parent: parent.clone(),
                },
            );
            for &i in &inside {
                out.assignment
                    .insert(self.elements[i].id.clone(), key.clone());
            }
            self.split(&inside, &inner, Parent::Component(key), out);
        }
        loose.sort_unstable();
        if loose.len() > MAX_FLAT {
            cluster_loose(
                self.elements,
                self.weights,
                &loose,
                &parent,
                self.previous,
                out,
            );
        }
    }
}

/// The repository path of a level directory (`inner`, with its trailing `/`) of an element: its
/// real path, cut where the level path of the directory ends.
fn real_dir(element: &Element, inner: &str) -> String {
    let tail = element.path.strip_prefix(inner).unwrap_or(&element.path);
    let real = element.real.strip_suffix(tail).unwrap_or(&element.real);
    real.trim_end_matches('/').to_owned()
}

/// Leiden on loose elements, started from their previous clusters (ADR 0016).
fn cluster_loose(
    elements: &[Element],
    weights: &HashMap<(usize, usize), f64>,
    loose: &[usize],
    parent: &Parent,
    previous: &BTreeMap<NodeId, String>,
    out: &mut Components,
) {
    let local: HashMap<usize, usize> = loose.iter().enumerate().map(|(l, &i)| (i, l)).collect();
    let edges: Vec<(usize, usize, f64)> = weights
        .iter()
        .filter_map(|((a, b), w)| Some((*local.get(a)?, *local.get(b)?, *w)))
        .collect();
    let graph = Graph::new(loose.len(), edges.iter().copied());
    // Previous clusters start together; the others start alone.
    let mut labels: BTreeMap<&str, usize> = BTreeMap::new();
    let initial: Vec<usize> = loose
        .iter()
        .enumerate()
        .map(|(l, &i)| {
            match previous
                .get(&elements[i].id)
                .filter(|k| k.starts_with("cluster/"))
            {
                Some(key) => {
                    let next = loose.len() + labels.len();
                    *labels.entry(key.as_str()).or_insert(next)
                }
                None => l,
            }
        })
        .collect();
    let communities = cluster::leiden(&graph, &initial, RESOLUTION);
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (l, community) in communities.iter().enumerate() {
        groups.entry(*community).or_default().push(loose[l]);
    }
    let mut degree = vec![0.0; loose.len()];
    for (a, b, w) in &edges {
        if communities[*a] == communities[*b] {
            degree[*a] += w;
            degree[*b] += w;
        }
    }
    // Keys: a community keeps the previous key most of its members had, each key once, the
    // largest overlaps first.
    let mut overlaps: Vec<(usize, usize, String)> = Vec::new();
    for (community, members) in &groups {
        let mut counts: BTreeMap<&String, usize> = BTreeMap::new();
        for i in members {
            if let Some(key) = previous
                .get(&elements[*i].id)
                .filter(|k| k.starts_with("cluster/"))
            {
                *counts.entry(key).or_default() += 1;
            }
        }
        for (key, count) in counts {
            if count * 2 > members.len() {
                overlaps.push((count, *community, key.clone()));
            }
        }
    }
    overlaps.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut keys: BTreeMap<usize, String> = BTreeMap::new();
    let mut taken: BTreeSet<String> = out.components.keys().cloned().collect();
    for (_, community, key) in overlaps {
        if !keys.contains_key(&community) && taken.insert(key.clone()) {
            keys.insert(community, key);
        }
    }
    for (community, members) in groups {
        if members.len() < 2 {
            continue;
        }
        let key = match keys.remove(&community) {
            Some(key) => key,
            None => {
                let smallest = members
                    .iter()
                    .map(|i| elements[*i].real.as_str())
                    .min()
                    .expect("members");
                let mut key = format!("cluster/{smallest}");
                let mut n = 2;
                while !taken.insert(key.clone()) {
                    key = format!("cluster/{smallest}#{n}");
                    n += 1;
                }
                key
            }
        };
        let hub = members
            .iter()
            .max_by(|a, b| {
                degree[local[a]]
                    .total_cmp(&degree[local[b]])
                    .then(elements[**b].real.cmp(&elements[**a].real))
            })
            .expect("members");
        let name = stem(&elements[*hub].real);
        out.components.insert(
            key.clone(),
            Component {
                name,
                parent: parent.clone(),
            },
        );
        for i in members {
            out.assignment.insert(elements[i].id.clone(), key.clone());
        }
    }
}

/// `crates/timonier-store/src/write.rs` → `write`; `crates/timonier-store` → `timonier-store`.
fn stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.split_once('.')
        .map_or(name, |(stem, _)| stem)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(path: &str) -> Element {
        Element {
            id: NodeId::file(path).unwrap(),
            path: path.to_owned(),
            real: path.to_owned(),
        }
    }

    fn keys(components: &Components) -> Vec<String> {
        components
            .components
            .iter()
            .map(|(k, c)| format!("{k} {} {:?}", c.name, c.parent))
            .collect()
    }

    #[test]
    fn small_levels_stay_flat() {
        let elements: Vec<Element> = (0..12)
            .map(|i| element(&format!("tests/t{i}.rs")))
            .collect();
        let mut out = Components::default();
        layout(
            &elements,
            &HashMap::new(),
            Parent::Repo,
            &BTreeMap::new(),
            Some("src"),
            &mut out,
        );
        assert_eq!(out, Components::default());
    }

    #[test]
    fn directories_group_before_clustering() {
        // 10 loose files, 3 tests, 2 examples, 1 bench: 16 > 12.
        let mut paths: Vec<String> = (0..10).map(|i| format!("src/f{i}.rs")).collect();
        paths.extend((0..3).map(|i| format!("tests/t{i}.rs")));
        paths.extend((0..2).map(|i| format!("examples/e{i}.rs")));
        paths.push("benches/b.rs".into());
        let elements: Vec<Element> = paths.iter().map(|p| element(p)).collect();
        let mut out = Components::default();
        let package = Parent::Package(NodeId::package("cargo", "p").unwrap());
        layout(
            &elements,
            &HashMap::new(),
            package.clone(),
            &BTreeMap::new(),
            Some("src"),
            &mut out,
        );
        assert_eq!(
            keys(&out),
            [
                format!("dir/examples examples {package:?}"),
                format!("dir/tests tests {package:?}"),
            ]
        );
        assert_eq!(out.assignment.len(), 5);
    }

    #[test]
    fn a_large_flat_directory_is_clustered_and_keeps_its_keys() {
        // Two groups of 8 files, dense inside, one edge between.
        let elements: Vec<Element> = (0..16)
            .map(|i| element(&format!("src/m{i:02}.rs")))
            .collect();
        let mut weights = HashMap::new();
        for group in 0..2 {
            for a in 0..8 {
                for b in a + 1..8 {
                    weights.insert((group * 8 + a, group * 8 + b), 1.0);
                }
            }
        }
        weights.insert((0, 8), 1.0);
        let mut out = Components::default();
        layout(
            &elements,
            &weights,
            Parent::Repo,
            &BTreeMap::new(),
            Some("src"),
            &mut out,
        );
        assert_eq!(
            out.components.keys().collect::<Vec<_>>(),
            ["cluster/src/m00.rs", "cluster/src/m08.rs"]
        );
        // The hub (most connected inside) names the cluster: m00 and m08 have one more edge.
        assert_eq!(out.components["cluster/src/m00.rs"].name, "m00");
        // Next time, a file moves (m07 now works with the second group): keys stay.
        weights.retain(|(a, b), _| *a != 7 && *b != 7);
        for b in 8..16 {
            weights.insert((7, b), 1.0);
        }
        let previous = out.assignment.clone();
        let mut next = Components::default();
        layout(
            &elements,
            &weights,
            Parent::Repo,
            &previous,
            Some("src"),
            &mut next,
        );
        assert_eq!(
            next.components.keys().collect::<Vec<_>>(),
            ["cluster/src/m00.rs", "cluster/src/m08.rs"]
        );
        let m07 = NodeId::file("src/m07.rs").unwrap();
        assert_eq!(next.assignment[&m07], "cluster/src/m08.rs");
        let changed = elements
            .iter()
            .filter(|e| previous.get(&e.id) != next.assignment.get(&e.id))
            .count();
        assert_eq!(changed, 1);
    }

    #[test]
    fn src_is_transparent_but_its_subdirectories_count() {
        // 11 loose files, `src/views/` (2), `tests/` (2) and `src/tests/` (2): 17 > 12.
        let mut paths: Vec<String> = (0..11).map(|i| format!("src/f{i}.rs")).collect();
        paths.extend(
            [
                "src/views/a.rs",
                "src/views/b.rs",
                "tests/t0.rs",
                "tests/t1.rs",
            ]
            .map(String::from),
        );
        paths.extend(["src/tests/u0.rs", "src/tests/u1.rs"].map(String::from));
        let elements: Vec<Element> = paths.iter().map(|p| element(p)).collect();
        let mut out = Components::default();
        layout(
            &elements,
            &HashMap::new(),
            Parent::Repo,
            &BTreeMap::new(),
            Some("src"),
            &mut out,
        );
        assert_eq!(
            out.components.keys().collect::<Vec<_>>(),
            ["dir/src/tests", "dir/src/views", "dir/tests"]
        );
        assert_eq!(out.components["dir/src/tests"].name, "tests");
    }

    #[test]
    fn real_directories_name_the_keys() {
        let e = Element {
            id: NodeId::file("crates/x/src/views/a.rs").unwrap(),
            path: "src/views/a.rs".into(),
            real: "crates/x/src/views/a.rs".into(),
        };
        assert_eq!(real_dir(&e, "src/views/"), "crates/x/src/views");
        assert_eq!(stem("crates/timonier-store"), "timonier-store");
        assert_eq!(stem("web/src/App.tsx"), "App");
    }
}
