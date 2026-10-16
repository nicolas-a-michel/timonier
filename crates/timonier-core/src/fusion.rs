//! Fusion of the observations of several sources into the merged edges of one scope (ADR 0009).

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use serde::{Deserialize, Serialize};

use crate::{
    ContentHash, Edge, EdgeKey, EdgeKind, Error, IdForm, NodeId, Observation, Provenance, Result,
};

/// What one source observed in a scope, and the edge kinds it analysed completely there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceObservations {
    pub source: Provenance,
    pub observations: Vec<Observation>,
    /// Edge kinds this source analysed completely: for a precise source, edges of these kinds
    /// that it did not report do not exist.
    #[serde(default)]
    pub verified: Vec<EdgeKind>,
    /// The content hash these observations were computed on. Precise sources must give it: the
    /// store ignores observations computed on another content than the scope's current one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computed_on: Option<ContentHash>,
}

impl SourceObservations {
    pub fn new(source: Provenance, observations: Vec<Observation>) -> Self {
        Self {
            source,
            observations,
            verified: Vec::new(),
            computed_on: None,
        }
    }

    pub fn computed_on(mut self, hash: ContentHash) -> Self {
        self.computed_on = Some(hash);
        self
    }

    /// Refuses what the actual graph never stores (ADR 0009):
    /// - intent (`Intended`);
    /// - edges touching a component or the repo, which are computed by aggregation;
    /// - calls between symbols from any source but the LSP and SCIP: function → function calls
    ///   come only from them; the others report calls between files (spec).
    pub fn validate(&self) -> Result<()> {
        if self.source == Provenance::Intended {
            return Err(Error::IntendedInActualGraph);
        }
        for observation in &self.observations {
            for endpoint in [&observation.src, &observation.dst] {
                if matches!(
                    endpoint.form(),
                    IdForm::Component { .. } | IdForm::Repo { .. }
                ) {
                    return Err(Error::ComputedEndpoint {
                        id: endpoint.clone(),
                    });
                }
            }
            let between_files = is_file(&observation.src) && is_file(&observation.dst);
            if observation.kind == EdgeKind::Calls && !self.source.is_precise() && !between_files {
                return Err(Error::ApproximateSymbolCall {
                    src: observation.src.clone(),
                    dst: observation.dst.clone(),
                    provenance: self.source,
                });
            }
        }
        Ok(())
    }

    pub fn verifying(mut self, kinds: impl IntoIterator<Item = EdgeKind>) -> Self {
        self.verified.extend(kinds);
        self
    }
}

fn is_file(id: &NodeId) -> bool {
    matches!(id.form(), IdForm::File { .. })
}

/// Merges the observations of a scope:
/// - for an edge kind verified by a precise source (Scip, Lsp), approximate observations
///   (TreeSitter, Inferred) are discarded: they are either contradicted or confirmed better;
/// - an edge seen by several sources keeps the best-ranked provenance, with its confidence and
///   weight (spec « Fusion »).
///
/// The result is sorted by edge key. Inputs are validated first ([`SourceObservations::validate`]).
pub fn fuse(sources: &[SourceObservations]) -> Result<Vec<Edge>> {
    let mut verified_precisely = BTreeSet::new();
    for source in sources {
        source.validate()?;
        if source.source.is_precise() {
            verified_precisely.extend(source.verified.iter().copied());
        }
    }

    let mut merged: BTreeMap<EdgeKey, Edge> = BTreeMap::new();
    for source in sources {
        for observation in &source.observations {
            if source.source.is_approximate() && verified_precisely.contains(&observation.kind) {
                continue;
            }
            let candidate = Edge {
                src: observation.src.clone(),
                dst: observation.dst.clone(),
                kind: observation.kind,
                provenance: source.source,
                confidence: observation.confidence,
                weight: observation.weight,
            };
            match merged.entry(observation.key()) {
                Entry::Vacant(slot) => {
                    slot.insert(candidate);
                }
                Entry::Occupied(mut slot) => {
                    let current = slot.get_mut();
                    if candidate.provenance.rank() > current.provenance.rank() {
                        *current = candidate;
                    } else if candidate.provenance == current.provenance {
                        // The same source reported the edge twice: count both occurrences.
                        current.weight = current.weight.saturating_add(candidate.weight);
                        if candidate.confidence > current.confidence {
                            current.confidence = candidate.confidence;
                        }
                    }
                }
            }
        }
    }
    Ok(merged.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Confidence, NodeId};

    fn id(name: &str) -> NodeId {
        NodeId::parse(format!("rust:{name}")).unwrap()
    }

    fn file(path: &str) -> NodeId {
        NodeId::file(path).unwrap()
    }

    fn seen(src: &NodeId, dst: &NodeId, kind: EdgeKind) -> Observation {
        Observation::new(src.clone(), dst.clone(), kind)
    }

    #[test]
    fn an_edge_seen_by_several_sources_keeps_the_best_provenance() {
        let (a, b) = (id("a::f"), id("b::g"));
        let edges = fuse(&[
            SourceObservations::new(
                Provenance::TreeSitter,
                vec![
                    seen(&a, &b, EdgeKind::References)
                        .with_confidence(Confidence::new(0.5).unwrap()),
                ],
            ),
            SourceObservations::new(Provenance::Lsp, vec![seen(&a, &b, EdgeKind::References)]),
        ])
        .unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].provenance, Provenance::Lsp);
        assert_eq!(edges[0].confidence, Confidence::CERTAIN);
    }

    #[test]
    fn a_tree_sitter_edge_contradicted_by_the_lsp_is_removed() {
        let (a, b, c) = (id("a::f"), id("b::g"), id("c::h"));
        let edges = fuse(&[
            SourceObservations::new(
                Provenance::TreeSitter,
                vec![
                    seen(&a, &b, EdgeKind::References),
                    seen(&a, &c, EdgeKind::References),
                ],
            ),
            SourceObservations::new(Provenance::Lsp, vec![seen(&a, &b, EdgeKind::References)])
                .verifying([EdgeKind::References]),
        ])
        .unwrap();
        assert_eq!(edges.iter().map(|e| &e.dst).collect::<Vec<_>>(), [&b]);
    }

    #[test]
    fn verification_is_per_edge_kind() {
        let (a, b) = (file("a.rs"), file("b.rs"));
        let edges = fuse(&[
            SourceObservations::new(
                Provenance::TreeSitter,
                vec![
                    seen(&a, &b, EdgeKind::Imports),
                    seen(&a, &b, EdgeKind::Calls),
                ],
            ),
            // The LSP checked the calls of the file (and found none), not its imports.
            SourceObservations::new(Provenance::Lsp, vec![]).verifying([EdgeKind::Calls]),
        ])
        .unwrap();
        assert_eq!(
            edges.iter().map(|e| e.kind).collect::<Vec<_>>(),
            [EdgeKind::Imports]
        );
    }

    #[test]
    fn precise_function_calls_supersede_approximate_file_calls() {
        // Spec: function → function calls only from the LSP or SCIP, file → file otherwise.
        let (fa, fb) = (file("a.rs"), file("b.rs"));
        let (f, g) = (id("a::f"), id("b::g"));
        let sources = [
            SourceObservations::new(
                Provenance::TreeSitter,
                vec![seen(&fa, &fb, EdgeKind::Calls).with_weight(3)],
            ),
            SourceObservations::new(
                Provenance::Lsp,
                vec![seen(&f, &g, EdgeKind::Calls).with_weight(3)],
            )
            .verifying([EdgeKind::Calls]),
        ];
        let edges = fuse(&sources).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(
            (&edges[0].src, &edges[0].dst, edges[0].provenance),
            (&f, &g, Provenance::Lsp)
        );
        // Without the LSP, the file-level edge stays.
        assert_eq!(fuse(&sources[..1]).unwrap()[0].src, fa);
    }

    #[test]
    fn manifest_edges_survive_precise_verification() {
        let (p, q) = (
            NodeId::package("cargo", "a").unwrap(),
            NodeId::package("cargo", "b").unwrap(),
        );
        let edges = fuse(&[
            SourceObservations::new(
                Provenance::Manifest,
                vec![seen(&p, &q, EdgeKind::DependsOn)],
            ),
            SourceObservations::new(Provenance::Lsp, vec![]).verifying([EdgeKind::DependsOn]),
        ])
        .unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].provenance, Provenance::Manifest);
    }

    #[test]
    fn approximate_sources_cannot_verify() {
        let (a, b) = (file("a.rs"), file("b.rs"));
        let edges = fuse(&[
            SourceObservations::new(Provenance::Inferred, vec![seen(&a, &b, EdgeKind::Calls)]),
            SourceObservations::new(Provenance::TreeSitter, vec![]).verifying([EdgeKind::Calls]),
        ])
        .unwrap();
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn approximate_sources_report_calls_between_files_only() {
        let (f, g) = (id("a::f"), id("b::g"));
        let symbol_calls =
            SourceObservations::new(Provenance::TreeSitter, vec![seen(&f, &g, EdgeKind::Calls)]);
        assert!(matches!(
            symbol_calls.validate(),
            Err(Error::ApproximateSymbolCall { .. })
        ));
        // Other kinds between symbols stay allowed, and so do precise calls.
        assert!(
            SourceObservations::new(
                Provenance::TreeSitter,
                vec![seen(&f, &g, EdgeKind::References)]
            )
            .validate()
            .is_ok()
        );
        assert!(
            SourceObservations::new(Provenance::Lsp, vec![seen(&f, &g, EdgeKind::Calls)])
                .validate()
                .is_ok()
        );
        let manifest =
            SourceObservations::new(Provenance::Manifest, vec![seen(&f, &g, EdgeKind::Calls)]);
        assert!(matches!(
            manifest.validate(),
            Err(Error::ApproximateSymbolCall { .. })
        ));
    }

    #[test]
    fn edges_touching_computed_boxes_are_refused() {
        let component = NodeId::component("servers").unwrap();
        let observations = SourceObservations::new(
            Provenance::Lsp,
            vec![seen(&component, &id("a::f"), EdgeKind::References)],
        );
        assert!(matches!(
            fuse(&[observations]),
            Err(Error::ComputedEndpoint { .. })
        ));
    }

    #[test]
    fn duplicates_from_one_source_add_up() {
        let (a, b) = (file("a.rs"), file("b.rs"));
        let edges = fuse(&[SourceObservations::new(
            Provenance::TreeSitter,
            vec![
                seen(&a, &b, EdgeKind::Calls).with_weight(2),
                seen(&a, &b, EdgeKind::Calls).with_weight(5),
            ],
        )])
        .unwrap();
        assert_eq!(edges[0].weight, 7);
    }

    #[test]
    fn intent_never_enters_the_actual_graph() {
        let (a, b) = (id("a::f"), id("b::g"));
        let result = fuse(&[SourceObservations::new(
            Provenance::Intended,
            vec![seen(&a, &b, EdgeKind::DependsOn)],
        )]);
        assert_eq!(result, Err(Error::IntendedInActualGraph));
    }

    #[test]
    fn the_result_is_sorted_and_deterministic() {
        let ids: Vec<_> = ["c", "a", "b"].iter().map(|n| id(n)).collect();
        let observations = ids
            .iter()
            .map(|dst| seen(&ids[0], dst, EdgeKind::References))
            .collect();
        let edges = fuse(&[SourceObservations::new(
            Provenance::TreeSitter,
            observations,
        )])
        .unwrap();
        let keys: Vec<_> = edges.iter().map(Edge::key).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }
}
