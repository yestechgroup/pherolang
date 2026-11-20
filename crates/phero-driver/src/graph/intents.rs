//! `.intent` emission: requirement nodes and their traceability edges
//! (issue pherolang#363).
//!
//! An intent contributes one `intent` node (name = the stable id) and two
//! edge families, one per link slot: `expresses` for the `element` links and
//! `verifiedBy` for the `check` links. Both point **from** the intent, so
//! `affected-by capability:…` reverse-walks into the requirements the
//! capability expresses — the impact question the coverage pass can only
//! answer one file at a time.
//!
//! Resolution goes through the graph's own node table
//! ([`Symbols::by_suffix`](super::resolve::Symbols::by_suffix), unique match
//! only) — never a second resolver over the artifacts, and never a guess:
//! several matches emit **no** edge, the graph's mirror of the coverage
//! pass's X0013. The wire artifact's names stay exactly as authored; this
//! module is where they become node ids.

use phero_ir::graph::{kinds, GraphEdge};
use phero_ir::intents::IntentModel;

use super::resolve::Symbol;
use super::{ids, Builder, FileRef};

/// Emits the file node and one `intent` node per declared requirement.
pub(super) fn nodes(builder: &mut Builder, file: &FileRef, model: &IntentModel) {
    builder.add_node(ids::file_node(&file.display, &file.application));
    for intent in &model.intents {
        builder.add_named(kinds::INTENT, &file.display, &file.application, &intent.id);
    }
}

/// Emits the traceability edges: `expresses` for every resolved `element`
/// link, `verifiedBy` for every resolved `check` link.
pub(super) fn edges(builder: &mut Builder, file: &FileRef, model: &IntentModel) {
    let mut edges: Vec<GraphEdge> = Vec::new();
    {
        let symbols = builder.symbols();
        let application = file.application.as_str();
        let display = file.display.as_str();

        for intent in &model.intents {
            let intent_id = ids::node(kinds::INTENT, display, &intent.id);
            for (links, edge_kind) in [
                (&intent.elements, kinds::EXPRESSES),
                (&intent.checks, kinds::VERIFIED_BY),
            ] {
                for link in links {
                    // The table is one-element per kind, by construction.
                    let target_kind = ids::intent_link_targets(link.kind)[0];
                    let Some(target) =
                        unique(symbols.by_suffix(target_kind, &link.name, application))
                    else {
                        continue;
                    };
                    edges.push(GraphEdge::new(
                        intent_id.clone(),
                        target.id.clone(),
                        edge_kind,
                    ));
                }
            }
        }
    }
    for edge in edges {
        builder.add_edge(edge);
    }
}

/// The unique match of a pooled lookup — the intents' no-guess rule.
fn unique(matches: Vec<&Symbol>) -> Option<&Symbol> {
    match matches.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}
