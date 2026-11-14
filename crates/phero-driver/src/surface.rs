//! The one surface table: every file extension pherolang compiles and the
//! [`SurfaceKind`] it names.
//!
//! Hosts dispatch on this enum instead of re-listing extensions: the CLI's
//! per-surface compile pipeline (`check`/`ir`/`build`/`graph` and the union
//! integrity pass) and the LSP's publish/snapshot dispatch both resolve a
//! path to a [`SurfaceKind`] here. Adding a surface is one `match` arm in
//! [`SurfaceKind::of_extension`] (the compiler names every other exhaustive
//! match), one [`SurfaceKind::ALL`] entry, and the host's own pipeline arm.
//!
//! The table is pure path arithmetic: no filesystem access, no compilation.
//! Extensions are matched exactly, the way every reader resolved them
//! before this table existed.

use std::path::Path;

/// One source surface of the pherolang toolchain — the same list as the
/// pipeline table in `AGENTS.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SurfaceKind {
    /// `.mox` domain model: the Core IR `Model` every other surface
    /// resolves against.
    Mox,
    /// `.ifml` interaction flow (views, navigation, CLI commands).
    Ifml,
    /// `.ddd` design (application, modules, services, machines).
    Ddd,
    /// `.actor` actor policy (actors, capabilities, grants).
    Actor,
    /// `.evt` event contract (events, channels, subscriptions).
    Evt,
    /// `.deploy` deployment model (applications, profiles, deployments).
    Deploy,
    /// `.uc` use case pattern.
    Uc,
    /// `.cmmn` case management model.
    Cmmn,
    /// `.dmn` decision model.
    Dmn,
}

impl SurfaceKind {
    /// Every surface, in the pipeline table's order. The CLI's directory
    /// scan and the "every kind is handled" tests iterate this; a new
    /// variant joins it in the same edit as its `of_extension` arm.
    pub const ALL: [SurfaceKind; 9] = [
        SurfaceKind::Mox,
        SurfaceKind::Ifml,
        SurfaceKind::Ddd,
        SurfaceKind::Actor,
        SurfaceKind::Evt,
        SurfaceKind::Deploy,
        SurfaceKind::Uc,
        SurfaceKind::Cmmn,
        SurfaceKind::Dmn,
    ];

    /// The surface an extension names (`"mox"`, `"ifml"`, …), or `None` for
    /// a file pherolang does not compile as a surface. Matched exactly: the
    /// extensions are lowercase by convention, the way every reader
    /// resolved them.
    pub fn of_extension(extension: &str) -> Option<Self> {
        Some(match extension {
            "mox" => SurfaceKind::Mox,
            "ifml" => SurfaceKind::Ifml,
            "ddd" => SurfaceKind::Ddd,
            "actor" => SurfaceKind::Actor,
            "evt" => SurfaceKind::Evt,
            "deploy" => SurfaceKind::Deploy,
            "uc" => SurfaceKind::Uc,
            "cmmn" => SurfaceKind::Cmmn,
            "dmn" => SurfaceKind::Dmn,
            _ => return None,
        })
    }

    /// The surface a path names by its extension, or `None` for an unknown
    /// (or extension-less) path.
    pub fn of_path(path: &Path) -> Option<Self> {
        Self::of_extension(path.extension().and_then(|extension| extension.to_str())?)
    }

    /// The kind's file extension, without the dot — the inverse of
    /// [`SurfaceKind::of_extension`].
    pub fn extension(self) -> &'static str {
        match self {
            SurfaceKind::Mox => "mox",
            SurfaceKind::Ifml => "ifml",
            SurfaceKind::Ddd => "ddd",
            SurfaceKind::Actor => "actor",
            SurfaceKind::Evt => "evt",
            SurfaceKind::Deploy => "deploy",
            SurfaceKind::Uc => "uc",
            SurfaceKind::Cmmn => "cmmn",
            SurfaceKind::Dmn => "dmn",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
