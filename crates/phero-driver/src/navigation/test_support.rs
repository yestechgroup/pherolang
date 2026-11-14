//! Thin `#[cfg(test)]` adapter over the shared fixture loader
//! ([`phero_testkit`], issue pherolang#260).
//!
//! The paths, the nine-surface compile order, the `app.*`/support graphs, and
//! the anti-drift assertion live in the testkit; the unit tests in this
//! crate's `navigation` modules import the pieces they need from here.
//!
//! The adapter exists for one reason, the dev-dependency cycle: Cargo allows
//! `phero-testkit` to depend on `phero-driver` while `phero-driver` dev-depends on
//! `phero-testkit`, but it then compiles this crate **twice** — once with
//! `cfg(test)` for the unit-test binary, once normally for the testkit. The
//! fixture data crosses that boundary (a [`phero_ir::graph::GraphModel`] comes
//! from the single shared `phero-ir` crate), the crate's own [`SurfaceIndex`]
//! does not: the two copies are distinct Rust types. The assertion's key-set
//! comparison is therefore shared through
//! [`phero_testkit::assert_definition_keys`]; only the index-local uniqueness
//! check stays below, where the test-build index is in hand.

use phero_ir::graph::GraphModel;

use super::SurfaceIndex;

pub(crate) use phero_testkit::{actor_graph, ddd_graph, evt_graph, fixture, fixture_graph};

/// Asserts the index's definitions are exactly the graph's nodes for
/// `display`, one-to-one.
///
/// Delegates the comparison (and the three documented exclusions — the
/// `file` node, the `.ifml` `flow` node, and the `.deploy`
/// `platformCapability` node) to the testkit, then checks the index's own
/// contract: one definition site per `(kind, name)`.
pub(crate) fn assert_definitions_match_graph(
    index: &SurfaceIndex,
    graph: &GraphModel,
    display: &str,
) {
    let definitions: Vec<(String, String)> = index
        .definitions
        .iter()
        .map(|definition| (definition.kind.clone(), definition.name.clone()))
        .collect();
    phero_testkit::assert_definition_keys(&definitions, graph, display);
    for definition in &index.definitions {
        assert_eq!(
            index
                .definitions_of(&definition.kind, &definition.name)
                .len(),
            1,
            "{display}: {} '{}' must have exactly one definition site",
            definition.kind,
            definition.name
        );
    }
}
