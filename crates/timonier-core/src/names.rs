//! `.timonier/names.json` (Q96, ADR 0075): the names and roles given to the boxes of the map by
//! hand, versioned and shared like the whiteboard. The developer gives them from the map; an
//! agent may propose one where the developer gave none, and the developer's choice wins. The
//! graph keeps what the heuristics computed: the names are laid over it when it is read.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Node, NodeKind, Role};

/// The file, in `.timonier/`.
pub const FILE: &str = "names.json";

/// The format of the file.
pub const VERSION: u32 = 1;

/// A name is a few words: longer ones are refused.
pub const MAX_NAME: usize = 60;

/// A summary is a sentence or two (PLAN.md 11.10).
pub const MAX_SUMMARY: usize = 300;

/// Beyond this size the file is not read: it is versioned, a cloned repository could hold
/// anything (as `intent.json`, ADR 0039).
pub const MAX_BYTES: u64 = 1024 * 1024;

/// The roles a name may give, as a pattern of the tools' schema: `Role::ALL` (tested).
pub const ROLE_PATTERN: &str = "^(entry_point|data_model|persistence|interface|background_task|external_libraries|utility|project)$";

/// `propose_component_name` (ADR 0075): an agent proposes a name or a role for a box of the
/// map; the developer's own choice, when there is one, stays.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposeComponentName {
    /// The box's id, as the tools give it: `component:dir/crates/core`, `pkg:cargo/core`.
    #[schemars(length(min = 1, max = 512))]
    pub component: String,
    /// A name of a few words; omitted, the name stays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 60))]
    pub name: Option<String>,
    /// What the box does, in a sentence or two; omitted, the summary stays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 300))]
    pub summary: Option<String>,
    /// One of the map's roles; omitted, the role stays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(pattern(r"^(entry_point|data_model|persistence|interface|background_task|external_libraries|utility|project)$"))]
    pub role: Option<String>,
}

/// A box as the map names it, after a name was given: what the tool returns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Named {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Who gave a name or a role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GivenBy {
    Developer,
    Agent,
}

/// What a box was given: a name, a role, or both.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Given {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    /// What the box does, in a sentence or two: an agent's (the spec: « the LLM names and
    /// summarises the clusters »); the developer's names leave it (PLAN.md 11.10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Who gave the name and the role.
    pub by: GivenBy,
    /// When, as the tracking files say it (`2026-10-23T07:30:00Z`).
    pub at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Names {
    pub version: u32,
    /// By the box's node id: a package or a component.
    #[serde(default)]
    pub boxes: BTreeMap<String, Given>,
}

/// Why a name was not taken.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    #[error("names.json does not read: {0}")]
    Unreadable(String),
    #[error("names.json is of format {0}; this Timonier reads format {VERSION}")]
    Version(u32),
    #[error("a name is one line of at most {MAX_NAME} characters")]
    BadName,
    #[error("a summary is one paragraph of at most {MAX_SUMMARY} characters")]
    BadSummary,
    #[error("only a package or a component of the map is renamed")]
    NotABox,
    #[error("the developer named this box: the agent's proposal is not taken")]
    DeveloperChose,
}

impl Names {
    pub fn empty() -> Self {
        Self {
            version: VERSION,
            boxes: BTreeMap::new(),
        }
    }

    pub fn parse(text: &str) -> Result<Self, Refused> {
        let names: Names =
            serde_json::from_str(text).map_err(|e| Refused::Unreadable(e.to_string()))?;
        if names.version != VERSION {
            return Err(Refused::Version(names.version));
        }
        for given in names.boxes.values() {
            if let Some(name) = &given.name {
                check(name)?;
            }
            if let Some(summary) = &given.summary {
                check_summary(summary)?;
            }
        }
        Ok(names)
    }

    pub fn to_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    /// Gives `id` a name and a role, or takes them back when both are `None`. `node` is the
    /// box as the graph has it: only packages and components are renamed. An agent gives
    /// nothing where the developer gave something.
    pub fn give(
        &mut self,
        node: &Node,
        name: Option<&str>,
        role: Option<Role>,
        by: GivenBy,
        at: &str,
    ) -> Result<(), Refused> {
        if !matches!(node.kind, NodeKind::Package | NodeKind::Component) {
            return Err(Refused::NotABox);
        }
        let key = node.id.as_str();
        let summary = self.boxes.get(key).and_then(|g| g.summary.clone());
        if by == GivenBy::Agent
            && self.boxes.get(key).is_some_and(|g| {
                g.by == GivenBy::Developer && (g.name.is_some() || g.role.is_some())
            })
        {
            return Err(Refused::DeveloperChose);
        }
        let name = name.map(str::trim).filter(|n| !n.is_empty());
        if let Some(name) = name {
            check(name)?;
        }
        // What the heuristics give is no choice: the same name or role is not kept.
        let name = name.filter(|n| *n != node.name).map(str::to_owned);
        let role = role.filter(|r| Some(*r) != node.role);
        if name.is_none() && role.is_none() && summary.is_none() {
            self.boxes.remove(key);
        } else {
            self.boxes.insert(
                key.to_owned(),
                Given {
                    name,
                    role,
                    summary,
                    by,
                    at: at.to_owned(),
                },
            );
        }
        Ok(())
    }

    /// An agent's summary of a box, which a name or a role the developer gave leaves alone;
    /// `None` takes it back.
    pub fn summarise(
        &mut self,
        node: &Node,
        summary: Option<&str>,
        at: &str,
    ) -> Result<(), Refused> {
        if !matches!(node.kind, NodeKind::Package | NodeKind::Component) {
            return Err(Refused::NotABox);
        }
        let summary = summary.map(str::trim).filter(|s| !s.is_empty());
        if let Some(summary) = summary {
            check_summary(summary)?;
        }
        let key = node.id.as_str();
        match (self.boxes.get_mut(key), summary) {
            (Some(given), summary) => {
                given.summary = summary.map(str::to_owned);
                if given.name.is_none() && given.role.is_none() && given.summary.is_none() {
                    self.boxes.remove(key);
                }
            }
            (None, Some(summary)) => {
                self.boxes.insert(
                    key.to_owned(),
                    Given {
                        name: None,
                        role: None,
                        summary: Some(summary.to_owned()),
                        by: GivenBy::Agent,
                        at: at.to_owned(),
                    },
                );
            }
            (None, None) => {}
        }
        Ok(())
    }

    /// The summaries, by box id.
    pub fn summaries(&self) -> BTreeMap<String, String> {
        self.boxes
            .iter()
            .filter_map(|(id, g)| g.summary.clone().map(|s| (id.clone(), s)))
            .collect()
    }

    /// Lays the names and roles over the graph's nodes.
    pub fn apply(&self, nodes: &mut [Node]) {
        if self.boxes.is_empty() {
            return;
        }
        for node in nodes
            .iter_mut()
            .filter(|n| matches!(n.kind, NodeKind::Package | NodeKind::Component))
        {
            if let Some(given) = self.boxes.get(node.id.as_str()) {
                if let Some(name) = &given.name {
                    node.name.clone_from(name);
                }
                if given.role.is_some() {
                    node.role = given.role;
                }
            }
        }
    }
}

fn check_summary(summary: &str) -> Result<(), Refused> {
    if summary.chars().count() > MAX_SUMMARY
        || summary.chars().any(char::is_control)
        || summary.trim().is_empty()
    {
        return Err(Refused::BadSummary);
    }
    Ok(())
}

fn check(name: &str) -> Result<(), Refused> {
    if name.chars().count() > MAX_NAME
        || name.chars().any(char::is_control)
        || name.trim().is_empty()
    {
        return Err(Refused::BadName);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeId;

    fn node(id: &str, kind: NodeKind, name: &str) -> Node {
        Node::new(NodeId::parse(id).unwrap(), kind, name).with_role(Role::Utility)
    }

    #[test]
    fn the_developer_names_a_box_and_its_role_and_the_graph_shows_them() {
        let mut names = Names::empty();
        let store = node("pkg:cargo/store", NodeKind::Package, "store");
        names
            .give(
                &store,
                Some(" Storage "),
                Some(Role::Persistence),
                GivenBy::Developer,
                "t",
            )
            .unwrap();
        let mut nodes = vec![store.clone(), node("file:a.rs", NodeKind::File, "a.rs")];
        names.apply(&mut nodes);
        assert_eq!(
            (nodes[0].name.as_str(), nodes[0].role),
            ("Storage", Some(Role::Persistence))
        );
        assert_eq!(nodes[1].name, "a.rs");
        // Round trip, and back to the computed name and role.
        let read = Names::parse(&names.to_text()).unwrap();
        assert_eq!(read, names);
        names
            .give(&store, None, None, GivenBy::Developer, "t")
            .unwrap();
        assert!(names.boxes.is_empty());
    }

    #[test]
    fn the_developer_s_choice_wins_over_the_agent_s() {
        let mut names = Names::empty();
        let core = node("component:dir/core", NodeKind::Component, "core");
        names
            .give(&core, Some("Model"), None, GivenBy::Agent, "t")
            .unwrap();
        names
            .give(&core, Some("Domain"), None, GivenBy::Developer, "t")
            .unwrap();
        assert_eq!(
            names.give(&core, Some("Other"), None, GivenBy::Agent, "t"),
            Err(Refused::DeveloperChose)
        );
        assert_eq!(
            names.boxes["component:dir/core"].name.as_deref(),
            Some("Domain")
        );
    }

    #[test]
    fn an_agent_summarises_what_the_developer_named_and_the_names_keep_it() {
        let mut names = Names::empty();
        let core = node("component:dir/core", NodeKind::Component, "core");
        names
            .give(&core, Some("Domain"), None, GivenBy::Developer, "t")
            .unwrap();
        names
            .summarise(&core, Some("The model of orders and carts."), "t")
            .unwrap();
        // The developer renames: the summary stays; gives the name back: the summary stays.
        names
            .give(&core, Some("Model"), None, GivenBy::Developer, "t")
            .unwrap();
        names
            .give(&core, None, None, GivenBy::Developer, "t")
            .unwrap();
        assert_eq!(
            names.summaries()["component:dir/core"],
            "The model of orders and carts."
        );
        names.summarise(&core, None, "t").unwrap();
        assert!(names.boxes.is_empty());
        assert_eq!(
            names.summarise(&core, Some(&"x".repeat(301)), "t"),
            Err(Refused::BadSummary)
        );
    }

    #[test]
    fn the_tool_s_role_pattern_names_every_role() {
        let names: Vec<&str> = Role::ALL.iter().map(|r| r.as_str()).collect();
        assert_eq!(ROLE_PATTERN, format!("^({})$", names.join("|")));
    }

    #[test]
    fn what_is_not_a_box_or_not_a_name_is_refused() {
        let mut names = Names::empty();
        let file = node("file:a.rs", NodeKind::File, "a.rs");
        assert_eq!(
            names.give(&file, Some("x"), None, GivenBy::Developer, "t"),
            Err(Refused::NotABox)
        );
        let pkg = node("pkg:cargo/a", NodeKind::Package, "a");
        assert_eq!(
            names.give(&pkg, Some("two\nlines"), None, GivenBy::Developer, "t"),
            Err(Refused::BadName)
        );
        assert_eq!(
            names.give(&pkg, Some(&"x".repeat(61)), None, GivenBy::Developer, "t"),
            Err(Refused::BadName)
        );
        assert!(matches!(Names::parse("{"), Err(Refused::Unreadable(_))));
        assert_eq!(Names::parse(r#"{"version": 2}"#), Err(Refused::Version(2)));
    }
}
