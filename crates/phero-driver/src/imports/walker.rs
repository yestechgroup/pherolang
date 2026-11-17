//! The one import-closure walker: path resolution, dedupe, fetch invocation and
//! missing-import reporting for every surface whose imports resolve to a host
//! path. A surface supplies an [`ImportPolicy`] — its routing, its dedupe
//! identity, and what a hit or a miss means — so the gatherers in
//! [`crate::project`] differ only by policy.
//!
//! The walker never touches the filesystem: content arrives through the host's
//! [`Fetch`] closure and a miss is `None`. Hosts keep their own fetch policy
//! (the CLI is map-first from its snapshot, the LSP prefers open documents).
//!
//! Two contracts are **not** flattened into this loop, because they are
//! different contracts rather than flags: `.ddd`'s whole-or-nothing application
//! closure ([`crate::project::gather_ddd_app_imports`], recursive with cycle
//! detection) and `.ifml`'s two-key walk (`phero_ifml::walk_ifml_imports`, the
//! dependency direction is `phero-driver → phero-ifml`).

use std::path::Path;

use crate::engine_common::dir_of;

/// The host's content fetch: one resolved key in, its text out. A miss is
/// `None` — what a miss means is the [`ImportPolicy`]'s decision, and any io
/// error a host wants to name stays in the host's own side channel.
pub(crate) type Fetch<'a> = dyn FnMut(&str) -> Option<String> + 'a;

/// One written import resolved against its declaring file:
/// `dir_of(importer).join(written)`, displayed and **deliberately
/// unnormalised** (`./a.mox` stays `./a.mox`; hosts pin messages and dedupe
/// keys to these exact strings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Site {
    /// Host-resolved key of the declaring file.
    pub importer: String,
    /// The import string exactly as written.
    pub written: String,
    /// The lexically resolved path.
    pub resolved: String,
}

impl Site {
    pub(crate) fn new(importer: &str, written: &str) -> Site {
        Site {
            importer: importer.to_string(),
            written: written.to_string(),
            resolved: dir_of(importer).join(written).display().to_string(),
        }
    }

    /// The resolved path's extension, if it has one.
    pub(crate) fn extension(&self) -> Option<&str> {
        Path::new(&self.resolved)
            .extension()
            .and_then(|extension| extension.to_str())
    }
}

/// One surface's import policy: the walker owns resolution, dedupe bookkeeping,
/// fetch invocation and abort-on-first-error; the policy owns routing and
/// meaning.
pub(crate) trait ImportPolicy {
    /// `true` when this import belongs to the policy; a declined import is
    /// never fetched (unknown extensions pass through so the driver reports
    /// them).
    fn accepts(&self, site: &Site) -> bool;

    /// The dedupe identity, first-seen order: the resolved path everywhere
    /// except `.dmn`'s event bucket, which keys by the path as written.
    fn dedupe_key(&self, site: &Site) -> String {
        site.resolved.clone()
    }

    /// A miss: a hard error for the artifact buckets, a silent skip for the
    /// lenient domain gather, a recorded miss for the `.ddd` closure.
    fn missing(&mut self, site: &Site) -> Result<(), String>;

    /// A hit: compile it, push it, or nest into it.
    fn absorb(&mut self, site: &Site, text: &str, fetch: &mut Fetch<'_>) -> Result<(), String>;
}

/// Walks one file's written imports in source order through `policy`, resolving
/// each relative to `path_key`'s directory and deduplicating by
/// [`ImportPolicy::dedupe_key`]. The first `Err` aborts the walk.
pub(crate) fn walk(
    path_key: &str,
    source: &str,
    import_paths: &dyn Fn(&str) -> Vec<String>,
    fetch: &mut Fetch<'_>,
    policy: &mut dyn ImportPolicy,
) -> Result<(), String> {
    let mut seen: Vec<String> = Vec::new();
    for written in import_paths(source) {
        let site = Site::new(path_key, &written);
        if !policy.accepts(&site) {
            continue;
        }
        let key = policy.dedupe_key(&site);
        if seen.contains(&key) {
            continue;
        }
        let Some(text) = fetch(&site.resolved) else {
            // A miss is not recorded as seen: a later duplicate of a skipped
            // import is offered to the host again, exactly as the per-bucket
            // loops always have.
            policy.missing(&site)?;
            continue;
        };
        policy.absorb(&site, &text, fetch)?;
        seen.push(key);
    }
    Ok(())
}

/// The pinned missing-import message (a host may append its own io detail).
pub(crate) fn cannot_read(key: &str, importer: &str) -> String {
    format!("cannot read imported file {key} (imported by {importer})")
}
