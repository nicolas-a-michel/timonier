//! The structure scope: repo, components, packages, files and external dependencies, with
//! their roles (ADR 0013, 0016, 0017).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use timonier_core::{
    EdgeKind, Node, NodeId, NodeKind, Observation, Provenance, Role, ScopeKey, ScopeUpdate,
    SourceObservations,
};

use crate::components::{self, Components, Element, Parent};
use crate::pack::Ecosystem;
use crate::project::{Context, FileEntry, Project};

/// A role needs at least this score (ADR 0017).
const ROLE_THRESHOLD: f64 = 0.5;

/// Who wins a tie (ADR 0017).
const ROLE_ORDER: [Role; 5] = [
    Role::EntryPoint,
    Role::Persistence,
    Role::Interface,
    Role::DataModel,
    Role::BackgroundTask,
];

/// The component of external dependencies, and its children by category.
const EXTERNAL: &str = "external";

impl Project {
    pub fn components(&self) -> &Components {
        &self.components
    }

    /// Computes the components again from the current graph, starting from `previous`: the
    /// component each package or file had (ADR 0016).
    pub fn recluster(&mut self, previous: &BTreeMap<NodeId, String>) {
        let context = self.context();
        let weights = self.file_weights(&context);
        let mut out = Components::default();

        // The top: packages of the repository and files outside any package.
        let mut top: Vec<Element> = Vec::new();
        let mut top_of: HashMap<&str, usize> = HashMap::new();
        let mut package_index: HashMap<usize, usize> = HashMap::new();
        for &p in &context.kept {
            let package = &self.packages[p];
            package_index.insert(p, top.len());
            top.push(Element {
                id: package.id.clone(),
                path: package.dir.clone(),
                real: package.dir.clone(),
            });
        }
        for file in self.files.values().filter(|f| f.package.is_none()) {
            top_of.insert(&file.path, top.len());
            top.push(Element {
                id: file.id.clone(),
                path: file.path.clone(),
                real: file.path.clone(),
            });
        }
        let top_index = |file: &FileEntry| match file.package {
            Some(p) => package_index.get(&p).copied(),
            None => top_of.get(file.path.as_str()).copied(),
        };
        let mut top_weights: HashMap<(usize, usize), f64> = HashMap::new();
        for ((a, b), w) in &weights {
            if let (Some(a), Some(b)) = (top_index(&self.files[a]), top_index(&self.files[b]))
                && a != b
            {
                *top_weights.entry((a.min(b), a.max(b))).or_default() += w;
            }
        }
        for &p in &context.kept {
            for dependency in self.packages[p].dependencies.iter().filter(|d| d.runtime) {
                let internal = context.kept.iter().find(|q| {
                    self.packages[**q].name == dependency.name
                        && self.packages[**q].ecosystem == self.packages[p].ecosystem
                });
                if let Some(q) = internal {
                    let (a, b) = (package_index[&p], package_index[q]);
                    *top_weights.entry((a.min(b), a.max(b))).or_default() += 1.0;
                }
            }
        }
        components::layout(&top, &top_weights, Parent::Repo, previous, None, &mut out);

        // Inside each package: its files, `src/` being the package itself.
        for &p in &context.kept {
            let package = &self.packages[p];
            let files: Vec<&FileEntry> = self
                .files
                .values()
                .filter(|f| f.package == Some(p))
                .collect();
            let index: HashMap<&str, usize> = files
                .iter()
                .enumerate()
                .map(|(i, f)| (f.path.as_str(), i))
                .collect();
            let elements: Vec<Element> = files
                .iter()
                .map(|f| {
                    let relative = if package.dir.is_empty() {
                        f.path.as_str()
                    } else {
                        &f.path[package.dir.len() + 1..]
                    };
                    Element {
                        id: f.id.clone(),
                        path: relative.to_owned(),
                        real: f.path.clone(),
                    }
                })
                .collect();
            let mut inside: HashMap<(usize, usize), f64> = HashMap::new();
            for ((a, b), w) in &weights {
                if let (Some(a), Some(b)) = (index.get(a.as_str()), index.get(b.as_str())) {
                    *inside.entry((*a.min(b), *a.max(b))).or_default() += w;
                }
            }
            components::layout(
                &elements,
                &inside,
                Parent::Package(package.id.clone()),
                previous,
                Some("src"),
                &mut out,
            );
        }
        out.clustered = top.iter().map(|e| e.id.clone()).collect();
        out.clustered
            .extend(self.files.values().map(|f| f.id.clone()));
        self.components = out;
    }

    /// Takes back the components of the head after a restart (ADR 0016): no clustering, so the
    /// map stays as it was. Components that hang from a package that is gone are dropped;
    /// returns `false` then.
    pub fn restore_components(&mut self, mut components: Components) -> bool {
        let before = components.components.len();
        let packages = self.kept_package_ids();
        let valid: BTreeSet<String> = components
            .valid(&packages)
            .into_iter()
            .map(str::to_owned)
            .collect();
        components.components.retain(|key, _| valid.contains(key));
        components.assignment.retain(|_, key| valid.contains(key));
        components.clustered = packages;
        components
            .clustered
            .extend(self.files.values().map(|f| f.id.clone()));
        let complete = components.components.len() == before;
        self.components = components;
        complete
    }

    fn kept_package_ids(&self) -> BTreeSet<NodeId> {
        self.context()
            .kept
            .iter()
            .map(|p| self.packages[*p].id.clone())
            .collect()
    }

    /// Undirected weights between files: their tree-sitter imports, calls and references.
    fn file_weights(&self, context: &Context) -> BTreeMap<(String, String), f64> {
        let by_id: HashMap<&NodeId, &str> = self
            .files
            .values()
            .map(|f| (&f.id, f.path.as_str()))
            .collect();
        let mut weights: BTreeMap<(String, String), f64> = BTreeMap::new();
        for file in self.files.values() {
            for o in self.observations(file, context).observations {
                if !matches!(
                    o.kind,
                    EdgeKind::Imports | EdgeKind::Calls | EdgeKind::References
                ) {
                    continue;
                }
                let (Some(a), Some(b)) = (by_id.get(&o.src), by_id.get(&o.dst)) else {
                    continue;
                };
                if a == b {
                    continue;
                }
                let key = if a < b {
                    (a.to_string(), b.to_string())
                } else {
                    (b.to_string(), a.to_string())
                };
                *weights.entry(key).or_default() += f64::from(o.weight);
            }
        }
        weights
    }

    /// The component an element hangs under: its own; for an element that arrived since the
    /// last clustering, the one most of its directory neighbours have (ADR 0016). An element the
    /// clustering left alone stays alone.
    fn component_of(
        &self,
        id: &NodeId,
        dir: &str,
        neighbours: &[(&NodeId, &str)],
        valid: &BTreeSet<&str>,
    ) -> Option<String> {
        let valid_key = |key: &&String| valid.contains(key.as_str());
        if let Some(key) = self.components.assignment.get(id).filter(valid_key) {
            return Some(key.clone());
        }
        if self.components.clustered.contains(id) {
            return None;
        }
        let mut votes: BTreeMap<&String, usize> = BTreeMap::new();
        for (other, other_dir) in neighbours {
            if *other_dir == dir
                && *other != id
                && let Some(key) = self.components.assignment.get(*other).filter(valid_key)
            {
                *votes.entry(key).or_default() += 1;
            }
        }
        votes
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0)))
            .map(|(key, _)| key.clone())
    }

    pub(crate) fn structure_with(&self, context: &Context) -> ScopeUpdate {
        let (kept, externals) = (&context.kept, &context.externals);
        let component_id =
            |key: &str| NodeId::component(key).expect("component keys are valid ids");
        // Where each package and file hangs.
        let dir_of = |path: &str| path.rsplit_once('/').map_or("", |(d, _)| d).to_owned();
        let package_dirs: Vec<(NodeId, String)> = kept
            .iter()
            .map(|p| (self.packages[*p].id.clone(), dir_of(&self.packages[*p].dir)))
            .collect();
        let package_neighbours: Vec<(&NodeId, &str)> = package_dirs
            .iter()
            .map(|(id, d)| (id, d.as_str()))
            .collect();
        let file_dirs: Vec<(NodeId, String, Option<usize>)> = self
            .files
            .values()
            .map(|f| (f.id.clone(), dir_of(&f.path), f.package))
            .collect();
        // Components that still hang from the repo or a kept package, by the package (or the
        // repo) at the root of their chain: an element only goes into one of its own.
        let package_ids: BTreeSet<NodeId> =
            kept.iter().map(|p| self.packages[*p].id.clone()).collect();
        let valid = self.components.valid(&package_ids);
        let root_of = |key: &str| -> Option<NodeId> {
            let mut current = key;
            loop {
                match &self.components.components.get(current)?.parent {
                    Parent::Repo => return None,
                    Parent::Package(id) => return Some(id.clone()),
                    Parent::Component(parent) => current = parent,
                }
            }
        };
        let mut placed: BTreeMap<NodeId, String> = BTreeMap::new();
        let top_valid: BTreeSet<&str> = valid
            .iter()
            .copied()
            .filter(|k| root_of(k).is_none())
            .collect();
        for (id, dir) in &package_dirs {
            if let Some(key) = self.component_of(id, dir, &package_neighbours, &top_valid) {
                placed.insert(id.clone(), key);
            }
        }
        let mut by_package: BTreeMap<Option<usize>, Vec<(&NodeId, &str)>> = BTreeMap::new();
        for (id, dir, package) in &file_dirs {
            by_package
                .entry(*package)
                .or_default()
                .push((id, dir.as_str()));
        }
        let mut valid_in: BTreeMap<Option<usize>, BTreeSet<&str>> = BTreeMap::new();
        for package in by_package.keys() {
            let root = package.map(|p| self.packages[p].id.clone());
            valid_in.insert(
                *package,
                valid
                    .iter()
                    .copied()
                    .filter(|k| root_of(k) == root)
                    .collect(),
            );
        }
        for (id, dir, package) in &file_dirs {
            if let Some(key) = self.component_of(id, dir, &by_package[package], &valid_in[package])
            {
                placed.insert(id.clone(), key);
            }
        }
        // Components with something in them, and their ancestors.
        let mut used: BTreeSet<&str> = BTreeSet::new();
        for key in placed.values() {
            let mut key = key.as_str();
            while used.insert(key) {
                match &self.components.components[key].parent {
                    Parent::Component(parent) => key = parent,
                    Parent::Repo | Parent::Package(_) => break,
                }
            }
        }

        let package_roles: BTreeMap<usize, Role> =
            kept.iter().map(|p| (*p, self.package_role(*p))).collect();
        let mut nodes = vec![Node::new(
            self.repo.clone(),
            NodeKind::Repo,
            self.repo_name.clone(),
        )];
        for key in &used {
            let component = &self.components.components[*key];
            let parent = match &component.parent {
                Parent::Repo => self.repo.clone(),
                Parent::Package(id) => id.clone(),
                Parent::Component(parent) => component_id(parent),
            };
            let role = self.component_role(key, &placed, &package_roles);
            nodes.push(
                Node::new(
                    component_id(key),
                    NodeKind::Component,
                    component.name.clone(),
                )
                .with_parent(parent)
                .with_role(role),
            );
        }
        for (&p, role) in &package_roles {
            let package = &self.packages[p];
            let parent = placed
                .get(&package.id)
                .map_or_else(|| self.repo.clone(), |k| component_id(k));
            nodes.push(
                Node::new(package.id.clone(), NodeKind::Package, package.name.clone())
                    .with_parent(parent)
                    .with_role(*role),
            );
        }
        // External dependencies, grouped by usage category.
        if !externals.is_empty() {
            let external = component_id(EXTERNAL);
            nodes.push(
                Node::new(external.clone(), NodeKind::Component, EXTERNAL)
                    .with_parent(self.repo.clone())
                    .with_role(Role::ExternalLibraries),
            );
            let mut categories: BTreeSet<String> = BTreeSet::new();
            for ((ecosystem, name), id) in externals {
                let category = self.category(*ecosystem, name);
                let group = component_id(&format!("{EXTERNAL}/{category}"));
                if categories.insert(category.to_owned()) {
                    nodes.push(
                        Node::new(group.clone(), NodeKind::Component, category)
                            .with_parent(external.clone())
                            .with_role(Role::ExternalLibraries),
                    );
                }
                nodes.push(
                    Node::new(id.clone(), NodeKind::Package, name.clone())
                        .with_parent(group)
                        .with_role(Role::ExternalLibraries),
                );
            }
        }
        for file in self.files.values() {
            let parent = match placed.get(&file.id) {
                Some(key) => component_id(key),
                None => file
                    .package
                    .map_or_else(|| self.repo.clone(), |p| self.packages[p].id.clone()),
            };
            let name = file
                .path
                .rsplit('/')
                .next()
                .unwrap_or(&file.path)
                .to_owned();
            nodes.push(
                Node::new(file.id.clone(), NodeKind::File, name)
                    .with_parent(parent)
                    .with_language(file.language.clone()),
            );
        }
        let mut observations = Vec::new();
        for &p in kept {
            let package = &self.packages[p];
            for dependency in package.dependencies.iter().filter(|d| d.runtime) {
                let internal = self.packages.iter().enumerate().find(|(i, q)| {
                    q.ecosystem == package.ecosystem
                        && q.name == dependency.name
                        && kept.contains(i)
                });
                let dst = match internal {
                    Some((_, q)) => Some(q.id.clone()),
                    None => externals
                        .get(&(package.ecosystem, dependency.name.clone()))
                        .cloned(),
                };
                if let Some(dst) = dst {
                    observations.push(Observation::new(
                        package.id.clone(),
                        dst,
                        EdgeKind::DependsOn,
                    ));
                }
            }
        }
        ScopeUpdate::new(ScopeKey::structure())
            .with_nodes(nodes)
            .with_observations(SourceObservations::new(Provenance::Manifest, observations))
    }

    fn category<'a>(&'a self, ecosystem: Ecosystem, name: &str) -> &'a str {
        self.categories
            .get(&(ecosystem, name.to_owned()))
            .map_or("other", String::as_str)
    }

    /// A package's role: its libraries, the paths its files import, the signals of its code.
    fn package_role(&self, package: usize) -> Role {
        let files: Vec<&FileEntry> = self
            .files
            .values()
            .filter(|f| f.package == Some(package))
            .collect();
        let info = &self.packages[package];
        let libraries: BTreeSet<&str> = info
            .dependencies
            .iter()
            .filter(|d| d.runtime)
            .map(|d| d.name.as_str())
            .collect();
        choose(&self.scores(&files, &libraries, Some(info.ecosystem)))
    }

    /// A component's role: from its files inside a package; at the top, the role most of its
    /// packages share.
    fn component_role(
        &self,
        key: &str,
        placed: &BTreeMap<NodeId, String>,
        package_roles: &BTreeMap<usize, Role>,
    ) -> Role {
        let inside = |id: &NodeId| {
            let mut current = placed.get(id).map(String::as_str);
            while let Some(k) = current {
                if k == key {
                    return true;
                }
                current = match &self.components.components[k].parent {
                    Parent::Component(parent) => Some(parent.as_str()),
                    _ => None,
                };
            }
            false
        };
        let mut votes: BTreeMap<Role, usize> = BTreeMap::new();
        for (p, role) in package_roles {
            if inside(&self.packages[*p].id) && *role != Role::Utility {
                *votes.entry(*role).or_default() += 1;
            }
        }
        if let Some(best) = votes.values().max().copied() {
            return ROLE_ORDER
                .iter()
                .copied()
                .find(|r| votes.get(r) == Some(&best))
                .unwrap_or(Role::Utility);
        }
        let files: Vec<&FileEntry> = self.files.values().filter(|f| inside(&f.id)).collect();
        choose(&self.scores(&files, &BTreeSet::new(), None))
    }

    /// Scores of each role over a set of files (ADR 0017). Test, example and bench files say
    /// nothing about the architecture.
    fn scores(
        &self,
        files: &[&FileEntry],
        libraries: &BTreeSet<&str>,
        ecosystem: Option<Ecosystem>,
    ) -> BTreeMap<Role, f64> {
        let files: Vec<&FileEntry> = files
            .iter()
            .copied()
            .filter(|f| !is_test_path(&f.path))
            .collect();
        let languages: BTreeSet<_> = files.iter().map(|f| &f.language).collect();
        let mut scores: BTreeMap<Role, f64> = BTreeMap::new();
        let rules = self
            .rules
            .iter()
            .filter(|(language, rules)| {
                languages.contains(language) || ecosystem == Some(rules.ecosystem)
            })
            .flat_map(|(_, rules)| rules.roles.iter());
        for rule in rules {
            if rule
                .libraries
                .iter()
                .any(|l| libraries.contains(l.as_str()))
            {
                *scores.entry(rule.role).or_default() += f64::from(rule.confidence);
            }
            let imported = files.iter().any(|f| {
                f.facts
                    .imports
                    .iter()
                    .any(|i| rule.paths.iter().any(|p| path_matches(&i.path, p)))
            });
            if imported {
                *scores.entry(rule.role).or_default() += f64::from(rule.confidence);
            }
        }
        if !files.is_empty() {
            for role in ROLE_ORDER {
                let with = files
                    .iter()
                    .filter(|f| f.facts.signals.iter().any(|s| s.role == role))
                    .count();
                if with > 0 {
                    *scores.entry(role).or_default() += with as f64 / files.len() as f64;
                }
            }
        }
        scores
    }
}

fn choose(scores: &BTreeMap<Role, f64>) -> Role {
    let mut best: Option<(Role, f64)> = None;
    for role in ROLE_ORDER {
        if let Some(score) = scores.get(&role).copied()
            && score >= ROLE_THRESHOLD - 1e-9
            && best.is_none_or(|(_, b)| score > b + 1e-9)
        {
            best = Some((role, score));
        }
    }
    best.map_or(Role::Utility, |(role, _)| role)
}

/// An import names a rule's path, or something inside it.
fn path_matches(import: &str, rule: &str) -> bool {
    import == rule
        || ["::", "/", "."]
            .iter()
            .any(|separator| import.starts_with(&format!("{rule}{separator}")))
}

/// Tests, examples and benches (ADR 0017).
pub(crate) fn is_test_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.split('/').any(|segment| {
        matches!(
            segment,
            "tests" | "test" | "__tests__" | "examples" | "benches"
        )
    }) || name.contains(".test.")
        || name.contains(".spec.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_need_a_score_and_ties_follow_the_order() {
        assert_eq!(choose(&BTreeMap::new()), Role::Utility);
        assert_eq!(
            choose(&BTreeMap::from([(Role::DataModel, 0.4)])),
            Role::Utility
        );
        assert_eq!(
            choose(&BTreeMap::from([(Role::DataModel, 0.5)])),
            Role::DataModel
        );
        assert_eq!(
            choose(&BTreeMap::from([
                (Role::DataModel, 0.8),
                (Role::Persistence, 0.8)
            ])),
            Role::Persistence
        );
        assert_eq!(
            choose(&BTreeMap::from([
                (Role::Interface, 0.8),
                (Role::EntryPoint, 1.3)
            ])),
            Role::EntryPoint
        );
    }

    #[test]
    fn paths_and_tests_are_recognised() {
        assert!(path_matches(
            "std::net::TcpListener",
            "std::net::TcpListener"
        ));
        assert!(path_matches(
            "tokio::net::TcpListener::bind",
            "tokio::net::TcpListener"
        ));
        assert!(path_matches("node:http", "node:http"));
        assert!(!path_matches("vscodex", "vscode"));
        for test in [
            "crates/a/tests/x.rs",
            "web/src/App.test.tsx",
            "a/__tests__/b.ts",
            "x/examples/e.rs",
            "src/test/unit/m.test.ts",
        ] {
            assert!(is_test_path(test), "{test}");
        }
        for code in ["crates/a/src/lib.rs", "web/src/testing.ts", "src/latest.rs"] {
            assert!(!is_test_path(code), "{code}");
        }
    }
}
