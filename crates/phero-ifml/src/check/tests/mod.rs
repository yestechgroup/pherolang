//! The check-layer test suite: one module per concern; the shop-model
//! fixtures and `diags` helpers live in `support`.

use super::*;

use phero_diagnostics::Diagnostic;

mod support;

mod authorization;
mod decisions;
mod instantiated_modules;
mod interaction_reference;
mod lints;
