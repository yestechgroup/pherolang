//! Black-box `pherolang` CLI tests: one module per command group or
//! surface; shared helpers live in `support`.

mod support;

mod actor;
mod applications;
mod batch;
mod cmmn;
mod completions;
mod core;
mod ddd;
mod ddd_decisions;
mod ddd_events;
mod deploy;
mod dmn;
mod evt;
mod examples;
mod fmt;
mod gen;
mod gen_cedar;
mod gen_tools;
mod grammar;
mod graph;
mod highlight;
mod ifml;
mod import_schema;
mod import_sigil;
mod integrity;
mod ir;
mod lsp;
mod manifest;
mod multi;
mod project;
mod uc;
mod verify;
mod vocab;
