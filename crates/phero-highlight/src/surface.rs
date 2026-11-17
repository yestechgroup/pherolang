//! The highlighter's surface dispatch key, re-exported from the shared
//! workspace identity.
//!
//! [`Surface`] is [`phero_parser::Surface`]: one enum, one `ALL` order and one
//! extension table for the compiler, the CLI, the LSP and the generated
//! grammars (SK5; `docs/adr/0005-shared-surface-kernel-layering.md`, DR3).
//! The highlighter adds only its per-surface facts, and they live in
//! [`crate::metadata`]: the lexer family, the word tables borrowed from the
//! parser crates, the documentation path, and the declaration table.
//!
//! The wire spelling is the shared enum's — `#[serde(rename_all = "lowercase")]`
//! over the same variants — so the highlight JSON artifacts, the generated
//! grammar package, and `site/data/surfaces.json` are unchanged by the move.

pub use phero_parser::Surface;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn serde_uses_the_surface_id() {
        for surface in Surface::ALL {
            let json = serde_json::to_string(&surface).expect("serialize surface");
            assert_eq!(json, format!("\"{}\"", surface.id()));
            let parsed: Surface = serde_json::from_str(&json).expect("deserialize surface");
            assert_eq!(parsed, surface);
        }
    }

    #[test]
    fn from_path_reads_the_extension() {
        assert_eq!(
            Surface::from_path(Path::new("examples/support/support.mox")),
            Some(Surface::Mox)
        );
        assert_eq!(
            Surface::from_path(Path::new("Support.IFML")),
            Some(Surface::Ifml)
        );
        assert_eq!(Surface::from_path(Path::new("no-extension")), None);
        assert_eq!(Surface::from_path(Path::new("file.rs")), None);
    }
}
