//! Compiler-backed syntax highlighting for every Phero Lang surface.
//!
//! # The compiler is the source of truth
//!
//! This crate owns no grammar. Each of the nine surfaces (`.mox`, `.ifml`,
//! `.ddd`, `.actor`, `.evt`, `.deploy`, `.uc`, `.cmmn`, `.dmn`) is lexed by the
//! lexer the compiler uses and parsed by the parser the compiler uses; the
//! semantic pass is a walk over the spanned AST those produce. Keyword tables
//! are borrowed from [`phero_syntax::words`] and [`phero_ifml::words`], never
//! copied, so a language change is a parser change plus a drift test, not a
//! second list to keep in sync. There is **no second parser** here.
//!
//! # Bytes in the core, UTF-16 at the boundary
//!
//! Compiler spans are byte offsets, so the core [`HighlightSpan`] is too. Every
//! editor protocol speaks UTF-16 code units; [`to_utf16`] is the one conversion
//! point ([`Utf16HighlightSpan`]) and it is total and panic-free.
//!
//! # Never-fail, best effort
//!
//! [`highlight`] cannot panic and has no error return. A source that does not
//! parse still yields its lexical highlights and marks the result
//! [`degraded`](HighlightResult::degraded); a partial AST is used even when the
//! parser reported errors, because partial structure is better than none.
//! Overlay bounds are validated against the source, invalid spans are dropped,
//! and the final list is sorted, non-overlapping, non-empty, and in bounds.
//!
//! # The wire and rendering surfaces
//!
//! [`HighlightArtifact`] is the versioned JSON artifact
//! ([`HIGHLIGHT_FORMAT_VERSION`]); [`to_html`] renders a deterministic fragment
//! with no theme colours; [`crate::metadata`] carries the TextMate scopes and
//! LSP token/modifier mappings. `docs/HIGHLIGHTING.md` owns the normative wire
//! contract.
//!
//! # Example
//!
//! ```
//! use phero_highlight::{highlight, HighlightClass, HighlightOptions, Surface};
//!
//! let source = "class Book { String title }";
//! let result = highlight(source, Surface::Mox, HighlightOptions::default());
//! assert!(!result.degraded);
//! let class = result
//!     .spans
//!     .iter()
//!     .find(|span| &source[span.start..span.end] == "Book")
//!     .expect("the class name is highlighted");
//! assert_eq!(class.class, HighlightClass::Type);
//! ```

pub mod class;
pub mod engine;
pub mod hash;
pub mod html;
pub mod json;
pub mod metadata;
pub mod surface;
pub mod utf16;

pub use class::{HighlightClass, HighlightModifier, HighlightModifiers, HighlightSpan};
pub use engine::{
    highlight, HighlightDiagnostic, HighlightMode, HighlightOptions, HighlightResult,
};
pub use hash::sha256_source_hash;
pub use html::{modifier_class, to_html};
pub use json::{
    Generator, HighlightArtifact, HighlightError, WireSpan, GENERATOR_NAME, GENERATOR_VERSION,
    HIGHLIGHT_FORMAT_VERSION,
};
pub use metadata::{
    all_metadata, class_scope, is_builtin_type, keyword_class, lsp_modifier, lsp_token_type,
    metadata_for, scope_for, LexerKind, SurfaceMetadata, ALL as SURFACE_METADATA,
};
pub use surface::Surface;
pub use utf16::{to_utf16, Utf16HighlightSpan};
