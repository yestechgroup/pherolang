//! phero-ifml — the IFML interaction-flow DSL parser.
//!
//! The IFML DSL is a text-based interaction-modeling language, semantically
//! aligned with [OMG IFML 1.0](https://www.omg.org/spec/IFML/) but using a
//! modern, C-like expression syntax instead of XML/XMI. It models screens
//! (views), their components (lists, forms, details), user events, and the
//! navigation/action flows between them; a complementary domain model
//! (`.mox`) supplies the entities those views bind to. The language
//! reference lives in
//! [docs/IFML.md](https://github.com/yestechgroup/pherolang/blob/main/docs/IFML.md).
//!
//! # Parsing
//!
//! The surface is lexed by logos (`lexer`) and parsed by chumsky
//! (`parser`) into a spanned syntax tree (`ast`), then lowered into the
//! [`phero_ir::ifml::IfmlModel`] artifact (`lower`). The **normative grammar
//! (EBNF)** lives in the `parser` module docs — the one grammar authority
//! for the surface; `docs/IFML.md` links there and stays non-normative.
//! [`parse_ifml`] / [`parse_ifml_file`] return the model, and
//! [`parse_ifml_indexed`] returns the same model plus the [`IfmlIndex`] span
//! side-table. Spans are byte offsets; the LSP maps them to UTF-16.
//!
//! # Resolving
//!
//! [`compile_ifml_str`] is the compilation stage on top of parsing: it
//! pulls in the main file's `.ifml` imports through the [`IfmlImports`]
//! bundle (filesystem-free, like `phero_driver::DomainImports`), builds the
//! module table, and resolves/validates every `use` statement against it.
//! Resolution results live in the index
//! ([`ModuleUseSite::resolved_module`]) — the returned model is the main
//! file's own parse, unchanged, so artifacts stay byte-identical. See
//! `resolve` for the semantics and the diagnostic codes.
//!
//! Diagnostics are the workspace's one shared record
//! ([`phero_diagnostics::Diagnostic`]) tagged `(path, record)` per file —
//! the crate has no diagnostic type of its own (SK2 of epic
//! pherolang#328); the module docs of `resolve` record the shape and the
//! file-tag decision.
//!
//! # Type-checking
//!
//! [`check_ifml`] re-checks the type-checkable expressions of a compiled
//! model against an optional domain model (the union `phero_ir::Model` every
//! domain surface lowers into), via `phero_expr`'s
//! [`DomainTypes`](phero_expr::DomainTypes) — the same
//! checker `.mox` operation bodies use. [`check_ifml_with_actors`] adds
//! the authorization dimension (`requires:`/`roles:` against compiled
//! `.actor` policies); [`check_ifml_with_decisions`] adds the decision
//! dimension (`data:` names against the decision services of compiled
//! `.dmn` imports, after the domain classes). Constructs the shared
//! expression language cannot type (bare calls such as `today()`, regex
//! operators, non-integral numbers) are silently skipped, still deferred
//! to generation time. See `check` for the walk order and diagnostic
//! codes.
//!
//! # Downstream compatibility
//!
//! `use phero_ifml::*;` re-exports every IR type exactly as the historical
//! `codegraph-ifml-dsl` crate did, so consumers of the moved crate need only
//! swap the dependency.

pub mod ast;
mod check;
mod expand;
mod fmt;
mod index;
pub mod lexer;
mod lower;
mod manifest;
mod parser;
mod resolve;
pub mod words;

pub use check::{check_ifml, check_ifml_with_actors, check_ifml_with_decisions};
pub use expand::expand_ifml;
pub use fmt::format_ifml;
pub use index::{
    parse_ifml_indexed, AtSite, AuthKind, AuthSite, DataSite, IfmlIndex, IndexFile, IndexParam,
    LiteralKind, ModuleDeclSite, ModuleUseSite, NamedSite, TargetSite, UseOverride,
};
pub use manifest::manifest_ifml;
pub use parser::*;
pub use phero_diagnostics::{ifml_check, ifml_lints, ifml_resolver, Diagnostic, Severity};
pub use phero_ir::ifml::*;
pub use phero_ir::ifml_manifest::{IfmlManifest, IFML_MANIFEST_FORMAT_VERSION};
pub use resolve::{
    compile_ifml_lenient, compile_ifml_str, walk_ifml_imports, IfmlCompilation, IfmlImportWalk,
    IfmlImports, E_AMBIGUOUS_MODULE, E_CIRCULAR, E_DUPLICATE_MODULE, E_IMPORT_NOT_PROVIDED,
    E_INPUT_TYPE, E_MISSING_INPUT, E_PARSE, E_UNKNOWN_INPUT, E_UNKNOWN_MODULE,
};
