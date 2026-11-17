//! The one surface table, re-exported from the workspace's shared identity.
//!
//! The enum, its `ALL` order and the extension table live in
//! [`phero_parser::Surface`] (SK5; `docs/adr/0005-shared-surface-kernel-layering.md`,
//! DR3 and D2/D3): the driver, the highlighter, the CLI and the LSP share one
//! identity, and the highlighter does not gain a dependency on this crate.
//!
//! [`SurfaceKind`] is the driver's historical spelling of that type, so every
//! call site keeps working unchanged — including the exhaustive matches:
//! the shared enum is deliberately not `#[non_exhaustive]`, because the
//! driver's `of_extension`/`extension` impls and the CLI's `compile_one`
//! dispatch must fail to compile when a surface is added, not fall through a
//! wildcard arm.
//!
//! Hosts dispatch on this enum instead of re-listing extensions: the CLI's
//! per-surface compile pipeline (`check`/`ir`/`build`/`graph` and the union
//! integrity pass) and the LSP's publish/snapshot dispatch both resolve a
//! path to a [`SurfaceKind`] through the shared table. Adding a surface is one
//! enum edit (the shared `of_extension` arm and `ALL` entry), one pipeline
//! arm, and the host's own arm.
//!
//! The table is pure path arithmetic: no filesystem access, no compilation.
//! Extensions are matched exactly, the way every reader resolved them before
//! this table existed (the editor's case-insensitive decode is the shared
//! type's `from_path`, not this module's rule).

pub use phero_parser::Surface as SurfaceKind;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn every_extension_round_trips_to_its_kind() {
        for kind in SurfaceKind::ALL {
            assert_eq!(
                SurfaceKind::of_extension(kind.extension()),
                Some(kind),
                "{} must name its own kind",
                kind.extension()
            );
        }
    }

    #[test]
    fn extensions_are_unique() {
        let mut extensions: Vec<&str> = SurfaceKind::ALL
            .iter()
            .map(|kind| kind.extension())
            .collect();
        extensions.sort_unstable();
        extensions.dedup();
        assert_eq!(extensions.len(), SurfaceKind::ALL.len());
    }

    #[test]
    fn unknown_extensions_have_no_surface() {
        assert_eq!(SurfaceKind::of_extension("txt"), None);
        assert_eq!(
            SurfaceKind::of_extension("MOX"),
            None,
            "extensions are exact"
        );
        assert_eq!(SurfaceKind::of_path(Path::new("no-extension")), None);
        assert_eq!(
            SurfaceKind::of_path(Path::new("dir.with.dots/file")),
            None,
            "a dot in a directory name is not an extension"
        );
        assert_eq!(
            SurfaceKind::of_path(Path::new("examples/support.ddd")),
            Some(SurfaceKind::Ddd)
        );
    }
}
