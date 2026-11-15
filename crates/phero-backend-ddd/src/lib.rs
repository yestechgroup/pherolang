//! The `.ddd` design-artifact backend.
//!
//! This backend consumes **only** [`phero_ir::ddd::DddModel`]. It has two
//! jobs, both of which a downstream consumer (codegraph) can run without
//! the `.mox` sources, the driver, or the compiler:
//!
//! * [`check_contract`] validates the artifact against the contract the
//!   compiler is required to satisfy: resolved companions for every
//!   reference, effective design decisions, aligned parallel lists, search
//!   member types, and the strict value-semantics block (present and fixed
//!   on every `value` design, absent on every `entity`/`dto`). A
//!   hand-edited or truncated artifact fails with a [`Violation`] naming the
//!   JSON path.
//! * [`canonical_json`] / [`canonical_json_string`] serialize the artifact
//!   with a canonical (alphabetical) key order through
//!   [`phero_backend_common::canonical_key_order`], so hashes and diffs
//!   are stable regardless of which `serde_json` map feature is active in
//!   a consumer's dependency graph.
//!
//! The wire format itself is owned by [`phero_ir::ddd`]; this crate never
//! changes it. [`pherolang artifact check`] uses [`check_contract`] for
//! the artifact-only half of the DDD contract.
//!
//! [`pherolang artifact check`]: https://github.com/yestechgroup/pherolang

mod canonical;
mod contract;

pub use canonical::{canonical_json, canonical_json_string};
pub use contract::{check_contract, Violation};
