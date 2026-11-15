//! The nine language surfaces the highlighter understands.
//!
//! A [`Surface`] names one file kind and its compiler surface. The enum is the
//! highlighter's dispatch key: it selects the lexer family, the word tables,
//! the parser, and the semantic walker. The spelling is the same stable `id`
//! the site's `site/data/surfaces.json` uses.

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// One Phero Lang surface, in compiler order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Surface {
    /// `.mox` — the domain model surface.
    Mox,
    /// `.ifml` — the interaction-flow surface.
    Ifml,
    /// `.ddd` — the application-design surface.
    Ddd,
    /// `.actor` — the authorization-policy surface.
    Actor,
    /// `.evt` — the event-contract surface.
    Evt,
    /// `.deploy` — the deployment-projection surface.
    Deploy,
    /// `.uc` — the use-case-pattern surface.
    Uc,
    /// `.cmmn` — the case-management surface.
    Cmmn,
    /// `.dmn` — the decision-model surface.
    Dmn,
}

impl Surface {
    /// Every surface, in compiler order.
    pub const ALL: [Self; 9] = [
        Self::Mox,
        Self::Ifml,
        Self::Ddd,
        Self::Actor,
        Self::Evt,
        Self::Deploy,
        Self::Uc,
        Self::Cmmn,
        Self::Dmn,
    ];

    /// The stable surface id (also the wire spelling and the HTML language
    /// class suffix).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Mox => "mox",
            Self::Ifml => "ifml",
            Self::Ddd => "ddd",
            Self::Actor => "actor",
            Self::Evt => "evt",
            Self::Deploy => "deploy",
            Self::Uc => "uc",
            Self::Cmmn => "cmmn",
            Self::Dmn => "dmn",
        }
    }

    /// The file extension **without** the leading dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        self.id()
    }

    /// The human-readable surface name (matching `site/data/surfaces.json`).
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Mox => "Domain model",
            Self::Ifml => "Interaction flow",
            Self::Ddd => "Application design",
            Self::Actor => "Authorisation",
            Self::Evt => "Event contract",
            Self::Deploy => "Deployment",
            Self::Uc => "Use case pattern",
            Self::Cmmn => "Case management",
            Self::Dmn => "Decision model",
        }
    }

    /// Alternative ids/words accepted by [`Surface::from_id`].
    ///
    /// These are editor convenience spellings, never a second taxonomy: the
    /// id and extension stay canonical.
    #[must_use]
    pub const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Mox => &["domain", "model"],
            Self::Ifml => &["flow", "interaction"],
            Self::Ddd => &["design"],
            Self::Actor => &["policy", "authorisation", "authorization"],
            Self::Evt => &["event"],
            Self::Deploy => &["deployment"],
            Self::Uc => &["usecase", "use-case", "usecases"],
            Self::Cmmn => &["case"],
            Self::Dmn => &["decision", "decisions"],
        }
    }

    /// Resolves a surface from its id, extension (with or without a leading
    /// dot), or an alias. Matching is ASCII case-insensitive.
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

    /// Resolves a surface from a path's extension.
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
    fn ids_and_extensions_are_unique_and_match() {
        let mut ids = BTreeSet::new();
        for surface in Surface::ALL {
            assert!(ids.insert(surface.id()));
            assert_eq!(surface.id(), surface.extension());
            assert!(!surface.extension().starts_with('.'));
            assert!(!surface.display_name().is_empty());
        }
        assert_eq!(ids.len(), 9);
    }

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
