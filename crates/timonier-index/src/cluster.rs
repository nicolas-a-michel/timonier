//! Leiden community detection (Traag, Waltman, van Eck 2019), deterministic and started from
//! a given partition (ADR 0016). Quality: modularity with a resolution parameter.
//!
//! Each level runs three steps until nothing moves:
//! 1. local moving: every node goes to the neighbouring community that improves quality most;
//! 2. refinement: inside each community, nodes merge again from singletons, only into
//!    well-connected parts, which keeps every community connected;
//! 3. aggregation: the refined parts become the nodes of the next level, starting in the
//!    community of step 1.
//!
//! Ties are broken by the smallest label and nodes are visited in index order: no randomness.

use std::collections::{BTreeMap, VecDeque};

/// An undirected weighted graph. Parallel edges add up; self-loops count in degrees only.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    adjacency: Vec<Vec<(usize, f64)>>,
    degree: Vec<f64>,
    /// Twice the total edge weight.
    total: f64,
}

impl Graph {
    pub fn new(nodes: usize, edges: impl IntoIterator<Item = (usize, usize, f64)>) -> Self {
        let mut weights: Vec<BTreeMap<usize, f64>> = vec![BTreeMap::new(); nodes];
        let mut degree = vec![0.0; nodes];
        let mut total = 0.0;
        for (a, b, w) in edges {
            if a >= nodes || b >= nodes || w <= 0.0 || !w.is_finite() {
                continue;
            }
            degree[a] += w;
            degree[b] += w;
            total += 2.0 * w;
            if a != b {
                *weights[a].entry(b).or_default() += w;
                *weights[b].entry(a).or_default() += w;
            }
        }
        Self {
            adjacency: weights
                .into_iter()
                .map(|m| m.into_iter().collect())
                .collect(),
            degree,
            total,
        }
    }

    pub fn len(&self) -> usize {
        self.adjacency.len()
    }

    pub fn is_empty(&self) -> bool {
        self.adjacency.is_empty()
    }
}

/// Communities of the nodes, as labels `0..k` numbered in order of first appearance. `initial`
/// gives each node its starting community (any labels); `resolution` > 1 gives smaller ones.
///
/// The algorithm runs again from its own result until nothing moves, so that a result is
/// stable: started from it, Leiden gives it back. A restart then changes nothing (ADR 0016).
pub fn leiden(graph: &Graph, initial: &[usize], resolution: f64) -> Vec<usize> {
    const MAX_PASSES: usize = 32;
    let n = graph.len();
    if n == 0 {
        return Vec::new();
    }
    if graph.total <= 0.0 {
        return (0..n).collect();
    }
    let mut partition = normalise(
        &initial
            .iter()
            .copied()
            .chain(n..)
            .take(n)
            .collect::<Vec<_>>(),
    );
    for _ in 0..MAX_PASSES {
        let next = pass(graph, &partition, resolution);
        if next == partition {
            break;
        }
        partition = next;
    }
    partition
}

/// One run of Leiden, level after level, from `initial` (labels `0..k`).
fn pass(graph: &Graph, initial: &[usize], resolution: f64) -> Vec<usize> {
    let n = graph.len();
    // Which aggregate node each original node is in, at the current level.
    let mut membership: Vec<usize> = (0..n).collect();
    let mut level = graph.clone();
    let mut partition = initial.to_vec();
    const MAX_LEVELS: usize = 64;
    for _ in 0..MAX_LEVELS {
        let moved = local_moving(&level, &mut partition, resolution);
        let refined = refine(&level, &partition, resolution);
        let parts = refined.iter().copied().max().map_or(0, |m| m + 1);
        if parts == level.len() && !moved {
            break;
        }
        if parts == level.len() {
            // Nothing merged in the refinement: the local moves are final.
            continue;
        }
        let next_partition = {
            let mut p = vec![0; parts];
            for (node, part) in refined.iter().enumerate() {
                p[*part] = partition[node];
            }
            p
        };
        level = aggregate(&level, &refined, parts);
        for m in &mut membership {
            *m = refined[*m];
        }
        partition = next_partition;
    }
    normalise(&membership.iter().map(|m| partition[*m]).collect::<Vec<_>>())
}

/// Labels renumbered `0..k` in order of first appearance.
fn normalise(labels: &[usize]) -> Vec<usize> {
    let mut seen: BTreeMap<usize, usize> = BTreeMap::new();
    let mut order = Vec::with_capacity(labels.len());
    for label in labels {
        let next = seen.len();
        order.push(*seen.entry(*label).or_insert(next));
    }
    order
}

/// Fast local moving. A node may also leave for an empty community, when every neighbouring
/// one costs quality. Returns whether any node moved.
fn local_moving(graph: &Graph, partition: &mut [usize], resolution: f64) -> bool {
    let n = graph.len();
    let labels = partition.iter().copied().max().map_or(0, |m| m + 1).max(n);
    let mut community_degree = vec![0.0; labels];
    let mut size = vec![0_usize; labels];
    for (node, community) in partition.iter().enumerate() {
        community_degree[*community] += graph.degree[node];
        size[*community] += 1;
    }
    // Empty labels, the largest on top: a node that leaves takes the smallest one.
    let mut empty: Vec<usize> = (0..labels).rev().filter(|c| size[*c] == 0).collect();
    let mut queue: VecDeque<usize> = (0..n).collect();
    let mut queued = vec![true; n];
    let mut moved = false;
    let mut towards: BTreeMap<usize, f64> = BTreeMap::new();
    while let Some(node) = queue.pop_front() {
        queued[node] = false;
        let current = partition[node];
        let k = graph.degree[node];
        towards.clear();
        towards.insert(current, 0.0);
        for (other, w) in &graph.adjacency[node] {
            *towards.entry(partition[*other]).or_default() += w;
        }
        community_degree[current] -= k;
        size[current] -= 1;
        let gain = |community: usize, weight: f64| {
            weight - resolution * k * community_degree[community] / graph.total
        };
        let mut best = current;
        let mut best_gain = gain(current, towards[&current]);
        for (community, weight) in &towards {
            let g = gain(*community, *weight);
            if g > best_gain + 1e-12 {
                best = *community;
                best_gain = g;
            }
        }
        // An empty community gains nothing and costs nothing.
        // A node with no link to its community (an isolated node above all) leaves it too,
        // so that no community is left disconnected.
        let unlinked = best == current && towards[&current] <= 0.0;
        if (best_gain < -1e-12 || unlinked)
            && size[current] > 0
            && let Some(free) = empty.pop()
        {
            best = free;
        }
        if size[current] == 0 && best != current {
            empty.push(current);
        }
        community_degree[best] += k;
        size[best] += 1;
        if best != current {
            partition[node] = best;
            moved = true;
            for (other, _) in &graph.adjacency[node] {
                if partition[*other] != best && !queued[*other] {
                    queued[*other] = true;
                    queue.push_back(*other);
                }
            }
        }
    }
    moved
}

/// The refinement of each community: well-connected parts, merged from singletons.
fn refine(graph: &Graph, partition: &[usize], resolution: f64) -> Vec<usize> {
    let n = graph.len();
    let mut refined: Vec<usize> = (0..n).collect();
    let mut size = vec![1_usize; n];
    let mut part_degree = graph.degree.clone();
    let labels = partition.iter().copied().max().map_or(0, |m| m + 1);
    let mut community_degree = vec![0.0; labels];
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); labels];
    for (node, community) in partition.iter().enumerate() {
        community_degree[*community] += graph.degree[node];
        members[*community].push(node);
    }
    // Weight from each node, and from each part, to the rest of its community.
    let inside = |node: usize| -> f64 {
        graph.adjacency[node]
            .iter()
            .filter(|(o, _)| partition[*o] == partition[node])
            .map(|(_, w)| w)
            .sum()
    };
    let mut part_out: Vec<f64> = (0..n).map(inside).collect();
    let mut towards: BTreeMap<usize, f64> = BTreeMap::new();
    for community in members.iter().filter(|m| m.len() > 1) {
        let total_c = community_degree[partition[community[0]]];
        for &node in community {
            if size[refined[node]] != 1 {
                continue;
            }
            let k = graph.degree[node];
            let to_rest = inside(node);
            // Only a node well connected to its community merges.
            if to_rest < resolution * k * (total_c - k) / graph.total {
                continue;
            }
            towards.clear();
            for (other, w) in &graph.adjacency[node] {
                if partition[*other] == partition[node] && refined[*other] != refined[node] {
                    *towards.entry(refined[*other]).or_default() += w;
                }
            }
            let mut best: Option<(usize, f64)> = None;
            for (part, weight) in &towards {
                let kp = part_degree[*part];
                let well_connected =
                    part_out[*part] >= resolution * kp * (total_c - kp) / graph.total;
                if !well_connected {
                    continue;
                }
                let g = weight - resolution * k * kp / graph.total;
                if g >= 0.0 && best.is_none_or(|(_, b)| g > b + 1e-12) {
                    best = Some((*part, g));
                }
            }
            if let Some((part, _)) = best {
                let own = refined[node];
                let weight = towards[&part];
                part_out[part] = part_out[part] + to_rest - 2.0 * weight;
                part_degree[part] += k;
                size[part] += 1;
                size[own] = 0;
                refined[node] = part;
            }
        }
    }
    normalise(&refined)
}

/// The graph whose nodes are the parts.
fn aggregate(graph: &Graph, parts: &[usize], count: usize) -> Graph {
    let mut edges = Vec::new();
    for (node, neighbours) in graph.adjacency.iter().enumerate() {
        for (other, w) in neighbours {
            if node < *other {
                edges.push((parts[node], parts[*other], *w));
            }
        }
    }
    let mut next = Graph::new(count, edges);
    // Degrees carry over whole: the weight now inside a part counts as its self-loop.
    let mut degree = vec![0.0; count];
    for (node, part) in parts.iter().enumerate() {
        degree[*part] += graph.degree[node];
    }
    next.degree = degree;
    next.total = graph.total;
    next
}

/// Modularity of a partition, for tests and measurements.
pub fn modularity(graph: &Graph, partition: &[usize], resolution: f64) -> f64 {
    if graph.total <= 0.0 {
        return 0.0;
    }
    let labels = partition.iter().copied().max().map_or(0, |m| m + 1);
    let mut inside = vec![0.0; labels];
    let mut degree = vec![0.0; labels];
    for (node, neighbours) in graph.adjacency.iter().enumerate() {
        degree[partition[node]] += graph.degree[node];
        for (other, w) in neighbours {
            if partition[*other] == partition[node] {
                inside[partition[node]] += w;
            }
        }
    }
    (0..labels)
        .map(|c| inside[c] / graph.total - resolution * (degree[c] / graph.total).powi(2))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `groups` cliques of `size` nodes, chained by one edge between consecutive cliques.
    fn cliques(groups: usize, size: usize) -> Vec<(usize, usize, f64)> {
        let mut edges = Vec::new();
        for g in 0..groups {
            for a in 0..size {
                for b in a + 1..size {
                    edges.push((g * size + a, g * size + b, 1.0));
                }
            }
            if g + 1 < groups {
                edges.push((g * size, (g + 1) * size, 1.0));
            }
        }
        edges
    }

    #[test]
    fn finds_the_cliques_from_singletons() {
        let graph = Graph::new(30, cliques(3, 10));
        let communities = leiden(&graph, &(0..30).collect::<Vec<_>>(), 1.0);
        for g in 0..3 {
            let first = communities[g * 10];
            assert!(
                (0..10).all(|i| communities[g * 10 + i] == first),
                "{communities:?}"
            );
        }
        assert_eq!(communities.iter().max(), Some(&2));
        assert!(modularity(&graph, &communities, 1.0) > 0.6);
    }

    #[test]
    fn is_deterministic_and_keeps_a_good_starting_partition() {
        let graph = Graph::new(30, cliques(3, 10));
        let start: Vec<usize> = (0..30).map(|i| i / 10).collect();
        assert_eq!(leiden(&graph, &start, 1.0), start);
        let once = leiden(&graph, &vec![0; 30], 1.0);
        assert_eq!(leiden(&graph, &vec![0; 30], 1.0), once);
    }

    #[test]
    fn a_disconnected_starting_community_is_split() {
        // Two separate cliques given one label: Leiden never keeps a disconnected community.
        let mut edges = cliques(1, 5);
        edges.extend(cliques(1, 5).into_iter().map(|(a, b, w)| (a + 5, b + 5, w)));
        let graph = Graph::new(10, edges);
        let communities = leiden(&graph, &[0; 10], 1.0);
        assert_ne!(communities[0], communities[5], "{communities:?}");
        assert!((0..5).all(|i| communities[i] == communities[0]));
        assert!((5..10).all(|i| communities[i] == communities[5]));
    }

    #[test]
    fn a_result_is_stable() {
        let n = 120;
        let mut edges = Vec::new();
        for i in 0..n {
            for step in [1, 2, 5] {
                edges.push((i, (i / 12) * 12 + (i % 12 + step) % 12, 1.0));
            }
            if i % 7 == 0 {
                edges.push((i, (i * 13 + 5) % n, 1.0));
            }
        }
        let graph = Graph::new(n, edges);
        let once = leiden(&graph, &(0..n).collect::<Vec<_>>(), 1.0);
        assert_eq!(leiden(&graph, &once, 1.0), once);
    }

    #[test]
    fn an_isolated_node_leaves_its_starting_community() {
        // A clique and an isolated node, started together.
        let graph = Graph::new(6, cliques(1, 5));
        let communities = leiden(&graph, &[0; 6], 1.0);
        assert!(
            (0..5).all(|i| communities[i] == communities[0]),
            "{communities:?}"
        );
        assert_ne!(communities[5], communities[0], "{communities:?}");
    }

    #[test]
    fn isolated_nodes_and_empty_graphs() {
        assert!(leiden(&Graph::new(0, []), &[], 1.0).is_empty());
        assert_eq!(leiden(&Graph::new(3, []), &[0, 0, 0], 1.0), [0, 1, 2]);
    }

    /// ADR 0016: a small change moves less than 5 % of the nodes, when starting from the
    /// previous partition.
    #[test]
    fn a_small_change_moves_few_nodes() {
        let groups = 10;
        let size = 30;
        let n = groups * size;
        // Ten dense groups with some noise between them, deterministic.
        let mut edges = Vec::new();
        for i in 0..n {
            for step in [1, 3, 7, 11] {
                let j = (i / size) * size + (i % size + step) % size;
                edges.push((i, j, 1.0));
            }
            if i % 17 == 0 {
                edges.push((i, (i * 7 + 31) % n, 1.0));
            }
        }
        let graph = Graph::new(n, edges.clone());
        let before = leiden(&graph, &(0..n).collect::<Vec<_>>(), 1.0);
        // One node is rewired towards another group.
        let mut changed = edges;
        changed.retain(|(a, _, _)| *a != 42);
        for k in 0..3 {
            changed.push((42, 200 + k, 1.0));
        }
        let after = leiden(&Graph::new(n, changed), &before, 1.0);
        // Labels are renumbered: compare as "same community as node x" relations.
        let moved = (0..n)
            .filter(|&i| {
                let anchor = (i / size) * size + if i % size == 0 { 1 } else { 0 };
                (before[i] == before[anchor]) != (after[i] == after[anchor])
            })
            .count();
        assert!(moved * 100 < 5 * n, "{moved} of {n} moved");
    }
}
