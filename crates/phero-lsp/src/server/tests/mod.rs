//! LSP server test suite, one module per feature; the harness lives in
//! `harness`.

use super::*;

pub(crate) use harness::*;

mod artifact_diagnostics;
mod basics;
mod capability_joins;
mod code_action;
mod completion;
mod definition;
mod diagnostic_split;
mod dmn;
mod graph;
mod graph_cache;
mod harness;
mod hover;
mod ifml;
mod machine_refs;
mod references;
mod rename;
mod semantic_tokens;
mod symbol;
