//! The bridge (ADR 0032): what the home view proposes to look at first, ranked by the rule of
//! Q45: what needs a decision (a tracking file that does not read, a blocked task, a cycle
//! added), then the boxes with files to read again, untracked ones first.

use std::collections::{HashMap, HashSet};

use timonier_core::graph::Graph;
use timonier_core::protocol::{LOOK_LIMIT, LookItem, NodeDebt, ReviewSummary, TrackingProblem};
use timonier_core::tracking::{Plan, Session, TaskStatus, utc};
use timonier_core::{NodeId, NodeKind};

use crate::view::review_box;

/// What to look at first, at most [`LOOK_LIMIT`].
pub fn look(
    problems: &[TrackingProblem],
    plan: Option<&Plan>,
    review: &ReviewSummary,
    debt: &HashMap<NodeId, NodeDebt>,
    graph: &Graph,
) -> Vec<LookItem> {
    let mut items: Vec<LookItem> = problems
        .iter()
        .map(|p| LookItem::Problem {
            file: p.file.clone(),
        })
        .collect();
    items.extend(
        plan.iter()
            .flat_map(|plan| &plan.milestones)
            .flat_map(|m| &m.tasks)
            .filter(|t| t.status == TaskStatus::Blocked)
            .map(|t| LookItem::Blocked {
                task: t.id.clone(),
                title: t.title.clone(),
            }),
    );
    items.extend(review.cycles.iter().filter_map(|cycle| {
        let first = cycle.first()?;
        Some(LookItem::Cycle {
            boxes: cycle.clone(),
            path: path(graph, &first.id),
        })
    }));
    items.extend(in_debt(debt, graph).into_iter().filter_map(|(id, owed)| {
        let node = graph.node(&id)?;
        Some(LookItem::Debt {
            item: review_box(graph, node),
            debt: owed,
            path: path(graph, &id),
        })
    }));
    items.truncate(LOOK_LIMIT);
    items
}

/// The boxes in debt, the most precise of them: a box whose whole debt is in one of its boxes
/// gives way to it. Untracked first, then by files to read again, then by name.
fn in_debt(debt: &HashMap<NodeId, NodeDebt>, graph: &Graph) -> Vec<(NodeId, NodeDebt)> {
    let is_box = |id: &NodeId| {
        graph
            .node(id)
            .is_some_and(|n| matches!(n.kind, NodeKind::Package | NodeKind::Component))
    };
    let boxes: Vec<(&NodeId, &NodeDebt)> = debt
        .iter()
        .filter(|(id, owed)| owed.files > 0 && is_box(id))
        .collect();
    let covered: HashSet<&NodeId> = boxes
        .iter()
        .filter_map(|(id, owed)| {
            let parent = graph.node(id)?.parent.as_ref()?;
            (debt.get(parent).map(|p| p.files) == Some(owed.files)).then_some(parent)
        })
        .collect();
    let mut kept: Vec<(NodeId, NodeDebt)> = boxes
        .into_iter()
        .filter(|(id, _)| !covered.contains(id))
        .map(|(id, owed)| (id.clone(), *owed))
        .collect();
    let name = |id: &NodeId| graph.node(id).map_or("", |n| n.name.as_str());
    kept.sort_by(|(a, x), (b, y)| {
        (y.untracked > 0)
            .cmp(&(x.untracked > 0))
            .then(y.files.cmp(&x.files))
            .then(name(a).cmp(name(b)))
            .then(a.cmp(b))
    });
    kept
}

/// The ancestors of a node from the top of the map, the repository and the node left out.
pub fn path(graph: &Graph, id: &NodeId) -> Vec<String> {
    let mut ancestors: Vec<String> = graph
        .ancestors(id)
        .skip(1)
        .filter(|a| graph.node(a).is_some_and(|n| n.kind != NodeKind::Repo))
        .map(ToString::to_string)
        .collect();
    ancestors.reverse();
    ancestors
}

/// The sessions started since the review at `since_ms`; all of them before the first review.
/// The journal keeps seconds: one started in the second of the review counts as before it.
pub fn sessions_since(sessions: &[Session], since_ms: Option<i64>) -> u32 {
    let since = since_ms.map(utc);
    let count = sessions
        .iter()
        .filter(|s| since.as_ref().is_none_or(|since| s.started_at > *since))
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use timonier_core::Node;
    use timonier_core::protocol::ReviewBox;
    use timonier_core::tracking::{Milestone, SessionSnapshots, SessionStatus, Stamp, Task};

    fn id(s: &str) -> NodeId {
        NodeId::parse(s).unwrap()
    }

    fn graph() -> Graph {
        let node = |i: &str, kind: NodeKind, parent: Option<&str>| {
            let mut node = Node::new(id(i), kind, i.rsplit('/').next().unwrap());
            if let Some(parent) = parent {
                node = node.with_parent(id(parent));
            }
            node
        };
        Graph::new(
            vec![
                node("repo:r", NodeKind::Repo, None),
                node("pkg:cargo/a", NodeKind::Package, Some("repo:r")),
                node(
                    "component:dir/a/src",
                    NodeKind::Component,
                    Some("pkg:cargo/a"),
                ),
                node("pkg:cargo/b", NodeKind::Package, Some("repo:r")),
                node("pkg:cargo/c", NodeKind::Package, Some("repo:r")),
                node(
                    "component:dir/c/x",
                    NodeKind::Component,
                    Some("pkg:cargo/c"),
                ),
                node(
                    "component:dir/c/y",
                    NodeKind::Component,
                    Some("pkg:cargo/c"),
                ),
            ],
            Vec::new(),
        )
    }

    fn owed(files: u32, untracked: u32) -> NodeDebt {
        NodeDebt {
            files,
            of: 10,
            untracked,
        }
    }

    fn task(n: u32, status: TaskStatus) -> Task {
        Task {
            id: format!("T-{n}"),
            title: format!("task {n}"),
            criterion: None,
            status,
            components: Vec::new(),
            sessions: Vec::new(),
            created: Stamp {
                at: "2026-10-20T08:00:00Z".into(),
                session: "S-2026-10-20-1".into(),
            },
            notes: Vec::new(),
        }
    }

    fn names(items: &[LookItem]) -> Vec<String> {
        items
            .iter()
            .map(|item| match item {
                LookItem::Problem { file } => format!("problem {file}"),
                LookItem::Blocked { task, .. } => format!("blocked {task}"),
                LookItem::Cycle { boxes, .. } => format!("cycle {}", boxes[0].name),
                LookItem::Debt { item, debt, .. } => format!("{} {}", item.id, debt.files),
            })
            .collect()
    }

    #[test]
    fn alerts_come_first_then_the_most_precise_boxes_in_debt() {
        let graph = graph();
        let debt: HashMap<NodeId, NodeDebt> = [
            ("repo:r", owed(9, 1)),
            // All of a's debt is in its component: the component stands for it.
            ("pkg:cargo/a", owed(3, 0)),
            ("component:dir/a/src", owed(3, 0)),
            ("pkg:cargo/b", owed(5, 0)),
            // c's debt spreads over its components: c stays, and so do they.
            ("pkg:cargo/c", owed(1, 1)),
            ("component:dir/c/x", owed(1, 1)),
        ]
        .into_iter()
        .map(|(i, d)| (id(i), d))
        .collect();
        let plan = Plan {
            version: 1,
            milestones: vec![Milestone {
                id: "M-1".into(),
                title: "m".into(),
                tasks: vec![
                    task(1, TaskStatus::Blocked),
                    task(2, TaskStatus::InProgress),
                ],
            }],
        };
        let problems = vec![TrackingProblem {
            file: "todo.json".into(),
            message: "broken".into(),
        }];
        let items = look(
            &problems,
            Some(&plan),
            &ReviewSummary::default(),
            &debt,
            &graph,
        );
        assert_eq!(
            names(&items),
            [
                "problem todo.json",
                "blocked T-1",
                // Untracked first: x holds c's debt whole.
                "component:dir/c/x 1",
                "pkg:cargo/b 5",
                "component:dir/a/src 3",
            ]
        );
        let LookItem::Debt { path, .. } = &items[2] else {
            panic!("a box")
        };
        assert_eq!(path, &["pkg:cargo/c"]);
    }

    #[test]
    fn a_cycle_leads_to_its_first_box_and_five_at_most() {
        let graph = graph();
        let named = |i: &str| ReviewBox {
            id: id(i),
            kind: NodeKind::Component,
            name: i.into(),
            role: None,
            within: None,
        };
        let review = ReviewSummary {
            cycles: vec![vec![named("component:dir/c/x"), named("component:dir/c/y")]],
            ..ReviewSummary::default()
        };
        let debt: HashMap<NodeId, NodeDebt> = ["pkg:cargo/a", "pkg:cargo/b", "pkg:cargo/c"]
            .into_iter()
            .map(|i| (id(i), owed(2, 0)))
            .collect();
        let problems: Vec<TrackingProblem> = (0..3)
            .map(|n| TrackingProblem {
                file: format!("sessions/S-2026-10-20-{n}.json"),
                message: String::new(),
            })
            .collect();
        let items = look(&problems, None, &review, &debt, &graph);
        assert_eq!(items.len(), LOOK_LIMIT);
        let LookItem::Cycle { path, .. } = &items[3] else {
            panic!("the cycle after the alerts: {items:?}")
        };
        assert_eq!(path, &["pkg:cargo/c"]);
        assert_eq!(names(&items)[4], "pkg:cargo/a 2");
    }

    #[test]
    fn sessions_count_from_the_review() {
        let session = |at: &str| Session {
            version: 1,
            id: "S-2026-10-20-1".into(),
            agent: "a".into(),
            objective: "o".into(),
            tasks: Vec::new(),
            status: SessionStatus::Ended,
            started_at: at.into(),
            ended_at: None,
            snapshots: SessionSnapshots {
                start: 1,
                end: None,
            },
            summary: None,
            todo: Vec::new(),
            changes: None,
            budgets: Vec::new(),
        };
        let sessions = [
            session("2026-10-20T08:00:00Z"),
            session("2026-10-20T10:00:00Z"),
        ];
        // 2026-10-20T09:00:00Z.
        assert_eq!(sessions_since(&sessions, Some(1_792_486_800_000)), 1);
        assert_eq!(sessions_since(&sessions, None), 2);
    }
}
