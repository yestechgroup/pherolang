//! The workspace's one surface identity: the nine source surfaces pherolang
//! compiles, their extensions, ids, display names, and editor aliases.
//!
//! This module owns the **one** surface enum, the **one** `ALL` order, and the
//! **one** extension table. `phero-driver` re-exports it as `SurfaceKind` (its
//! historical spelling) and `phero-highlight` re-exports it as `Surface`; both
//! are aliases of this type, never second tables. The identity lives here
//! because it is compiler-neutral path arithmetic and both hosts need it:
//! `phero-highlight` must not depend on `phero-driver` (salsa, ariadne, sigil,
//! xstate, dsntk), and the driver must not depend on the highlighter.
//! `docs/adr/0005-shared-surface-kernel-layering.md` records the boundary
//! (DR3, D2/D3).
//!
//! # Two path rules, both deliberate
//!
//! The compiler and the editor decode a path differently, and this module
//! keeps both:
//!
//! * [`Surface::of_extension`] / [`Surface::of_path`] match **exactly** — the
//!   compiler rule every reader used before this table existed: `"MOX"` names
//!   no surface, and a dot in a directory name is not an extension.
//! * [`Surface::from_path`] / [`Surface::from_id`] match an extension **ASCII
//!   case-insensitively** — the editor/highlighter rule, where a file's name
//!   is whatever the user typed (`Support.IFML` is `.ifml`).
//!
//! Collapsing the two would change behavior on one side or the other, so a
//! host picks the rule its call site already had: `pherolang check` and the
//! LSP's document classifier use the exact rule; `pherolang highlight`, the
//! LSP's hover/token layer, and the grammar generator use the tolerant one.
//!
//! # Why not `#[non_exhaustive]`
//!
//! The enum is deliberately exhaustive. Its consumers live in downstream
//! crates and match it exhaustively: the driver's `of_extension`/`extension`
//! impls and the CLI's `compile_one` dispatch (documented as "exhaustive, no
//! `_` arm"). `#[non_exhaustive]` would force a wildcard arm into each of
//! them, and a wildcard arm is exactly the drift this table exists to
//! prevent: a new surface must be a compile error at every dispatch site, not
//! a silent fallthrough.

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// One source surface of the pherolang toolchain — the same list as the
/// pipeline table in `AGENTS.md`.
///
/// The spelling of the variants is the stable `id` the site's
/// `site/data/surfaces.json` uses, and the serde wire spelling of the enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    /// `.mox` — the domain model: the Core IR `Model` every other surface
    /// resolves against.
    Mox,
    /// `.ifml` — the interaction flow (views, navigation, CLI commands).
    Ifml,
    /// `.ddd` — the application design (application, modules, services,
    /// machines).
    Ddd,
    /// `.actor` — the authorization policy (actors, capabilities, grants).
    Actor,
    /// `.evt` — the event contract (events, channels, subscriptions).
    Evt,
    /// `.deploy` — the deployment model (applications, profiles,
    /// deployments).
    Deploy,
    /// `.uc` — the use-case pattern.
    Uc,
    /// `.cmmn` — the case-management model.
    Cmmn,
    /// `.dmn` — the decision model.
    Dmn,
}

impl Surface {
    /// Every surface, in the pipeline table's order. The CLI's directory scan,
    /// the highlighter's registry/grammar generator, and the "every kind is
    /// handled" tests iterate this; a new variant joins it in the same edit as
    /// its `of_extension` arm.
    pub const ALL: [Surface; 9] = [
        Surface::Mox,
        Surface::Ifml,
        Surface::Ddd,
        Surface::Actor,
        Surface::Evt,
        Surface::Deploy,
        Surface::Uc,
        Surface::Cmmn,
        Surface::Dmn,
    ];

    /// The stable surface id: the file extension without the dot, the
    /// `--surface` value, and the wire/HTML language-class spelling.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Surface::Mox => "mox",
            Surface::Ifml => "ifml",
            Surface::Ddd => "ddd",
            Surface::Actor => "actor",
            Surface::Evt => "evt",
            Surface::Deploy => "deploy",
            Surface::Uc => "uc",
            Surface::Cmmn => "cmmn",
            Surface::Dmn => "dmn",
        }
    }

    /// The file extension **without** the leading dot — the inverse of
    /// [`Surface::of_extension`].
    #[must_use]
    pub const fn extension(self) -> &'static str {
        self.id()
    }

    /// The surface an extension names (`"mox"`, `"ifml"`, …), or `None` for a
    /// file pherolang does not compile as a surface.
    ///
    /// Matched exactly: extensions are lowercase by convention, the way every
    /// reader resolved them. This is the compiler rule; the editor's tolerant
    /// decode is [`Surface::from_path`].
    #[must_use]
    pub fn of_extension(extension: &str) -> Option<Self> {
        Some(match extension {
            "mox" => Surface::Mox,
            "ifml" => Surface::Ifml,
            "ddd" => Surface::Ddd,
            "actor" => Surface::Actor,
            "evt" => Surface::Evt,
            "deploy" => Surface::Deploy,
            "uc" => Surface::Uc,
            "cmmn" => Surface::Cmmn,
            "dmn" => Surface::Dmn,
            _ => return None,
        })
    }

    /// The surface a path names by its extension, or `None` for an unknown or
    /// extension-less path. Exact, like [`Surface::of_extension`].
    #[must_use]
    pub fn of_path(path: &Path) -> Option<Self> {
        Self::of_extension(path.extension().and_then(|extension| extension.to_str())?)
    }

    /// The human-readable surface name (matching `site/data/surfaces.json`).
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Surface::Mox => "Domain model",
            Surface::Ifml => "Interaction flow",
            Surface::Ddd => "Application design",
            Surface::Actor => "Authorisation",
            Surface::Evt => "Event contract",
            Surface::Deploy => "Deployment",
            Surface::Uc => "Use case pattern",
            Surface::Cmmn => "Case management",
            Surface::Dmn => "Decision model",
        }
    }

    /// Alternative ids/words accepted by [`Surface::from_id`].
    ///
    /// These are editor convenience spellings, never a second taxonomy: the
    /// id and extension stay canonical.
    #[must_use]
    pub const fn aliases(self) -> &'static [&'static str] {
        match self {
            Surface::Mox => &["domain", "model"],
            Surface::Ifml => &["flow", "interaction"],
            Surface::Ddd => &["design"],
            Surface::Actor => &["policy", "authorisation", "authorization"],
            Surface::Evt => &["event"],
            Surface::Deploy => &["deployment"],
            Surface::Uc => &["usecase", "use-case", "usecases"],
            Surface::Cmmn => &["case"],
            Surface::Dmn => &["decision", "decisions"],
        }
    }

    /// Resolves a surface from its id, extension (with or without a leading
    /// dot), or an alias. Matching is ASCII case-insensitive — the
    /// editor/highlighter rule.
    #[must_use]
    pub fn from_id(text: &str) -> Option<Self> {
        let needle = text.trim().trim_start_matches('.').to_ascii_lowercase();
        Self::ALL.into_iter().find(|surface| {
            surface.id() == needle
                || surface.extension() == needle
                || surface
                    .aliases()
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(&needle))
        })
    }

    /// Resolves a surface from a path's extension, matching ASCII
    /// case-insensitively — the editor/highlighter rule (`Support.IFML` is
    /// `.ifml`). The compiler's exact decode is [`Surface::of_path`].
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?;
        Self::ALL
            .into_iter()
            .find(|surface| surface.extension().eq_ignore_ascii_case(extension))
    }
}

impl fmt::Display for Surface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_extension_round_trips_to_its_surface() {
        for surface in Surface::ALL {
            assert_eq!(
                Surface::of_extension(surface.extension()),
                Some(surface),
                "{} must name its own surface",
                surface.extension()
            );
            assert_eq!(
                Surface::of_path(Path::new(&format!(
                    "examples/support.{}",
                    surface.extension()
                ))),
                Some(surface)
            );
        }
    }

    #[test]
    fn ids_and_extensions_are_unique_and_match() {
        let mut ids = BTreeSet::new();
        for surface in Surface::ALL {
            assert!(ids.insert(surface.id()));
            assert_eq!(surface.id(), surface.extension());
            assert!(!surface.extension().starts_with('.'));
            assert!(!surface.display_name().is_empty());
        }
        assert_eq!(ids.len(), 9);
        assert_eq!(ids.len(), Surface::ALL.len());
    }

    #[test]
    fn unknown_extensions_have_no_surface() {
        assert_eq!(Surface::of_extension("txt"), None);
        assert_eq!(
            Surface::of_extension("MOX"),
            None,
            "the compiler rule is exact"
        );
        assert_eq!(Surface::of_path(Path::new("no-extension")), None);
        assert_eq!(
            Surface::of_path(Path::new("dir.with.dots/file")),
            None,
            "a dot in a directory name is not an extension"
        );
        assert_eq!(
            Surface::of_path(Path::new("examples/support.ddd")),
            Some(Surface::Ddd)
        );
    }

    #[test]
    fn from_id_accepts_id_extension_and_alias() {
        for surface in Surface::ALL {
            assert_eq!(Surface::from_id(surface.id()), Some(surface));
            assert_eq!(Surface::from_id(surface.extension()), Some(surface));
            assert_eq!(
                Surface::from_id(&format!(".{}", surface.extension())),
                Some(surface)
            );
            assert_eq!(
                Surface::from_id(&surface.id().to_ascii_uppercase()),
                Some(surface)
            );
            for alias in surface.aliases() {
                assert_eq!(Surface::from_id(alias), Some(surface), "alias {alias}");
            }
        }
        assert_eq!(Surface::from_id("unknown"), None);
        assert_eq!(Surface::from_id(""), None);
    }

    #[test]
    fn from_path_reads_the_extension_case_insensitively() {
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

    #[test]
    fn display_is_the_id() {
        for surface in Surface::ALL {
            assert_eq!(surface.to_string(), surface.id());
        }
    }
}
