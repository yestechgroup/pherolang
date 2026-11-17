//! The shared host-side project resolver: cross-surface import gathering in
//! one filesystem-free home — the `workspace_imports` pattern generalized
//! from `import schema`/`import sigil` content to **every** surface's import
//! closure (Phase B of #117; one walker since SK3, pherolang#331).
//!
//! Traversal, path resolution, dedupe bookkeeping and fetch invocation live
//! once in [`crate::imports::walker`]. Each gatherer here is a **policy**
//! over it: which extensions route to which bucket, what a miss means, and
//! what a hit compiles into. Two contracts are special cases and keep their
//! own walks, because they are different contracts rather than flags:
//! [`gather_ddd_app_imports`] (whole-or-nothing, recursive with cycle
//! detection) and [`gather_ifml_project`] (a wrapper over
//! [`phero_ifml::walk_ifml_imports`] and its two-key identities).
//!
//! Two fetch shapes exist: **two-key** (`(importer identity, import as
//! written) -> (host-resolved key, text)`) for `.ifml`, and **path-keyed**
//! (`resolved path -> text`) for every other surface — where the walker
//! resolves relative to the importing file's directory first
//! ([`crate::imports::walker::Site`], the CLI rule), so dedupe is by resolved
//! key.
//!
//! The pinned messages (``cannot read imported file {resolved} (imported by
//! {importer})``, ``imported <kind> {key} does not compile``) and the two miss
//! contracts — artifact buckets hard-error (the CLI), the lenient domain gather
//! skips (the LSP) — are documented on the gatherers. [`IfmlProject::unresolved`]
//! is deliberately *not* an error: what to do about an unfetched import is host
//! policy.

use std::collections::BTreeMap;

use crate::engine_common::dir_of;
use crate::imports::walker::{cannot_read, walk, Fetch, ImportPolicy, Site};
use crate::workspace_imports::{collect_schema_imports, collect_sigil_imports};
use crate::{
    compile_actors_str, compile_ddd_app_str, compile_dmn_str, compile_evt_str, compile_str, render,
    Diagnostic, DomainImports,
};

/// One imported `.actor` policy: the source text plus the `.mox` domains its
/// capabilities are granted on, ready for [`compile_actor_imports`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorImport {
    /// Host-resolved key the policy was gathered under (dedup identity).
    pub key: String,
    /// The policy source text.
    pub source: String,
    /// The policy's own `.mox` imports as `(resolved key, text)`.
    pub domains: Vec<(String, String)>,
}

/// One imported `.dmn` decision model: its source, its `.mox` domains, its
/// decision-table contents and its compiled `.evt` contracts — the latter two
/// keyed by the path **as written** (the driver matches them exactly as
/// written). An unreadable table or contract is simply absent: the driver's
/// own diagnostic names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionImport {
    /// Host-resolved key the model was gathered under (dedup identity).
    pub key: String,
    /// The model source text.
    pub source: String,
    /// The model's own `.mox` imports as `(resolved key, text)`.
    pub domains: Vec<(String, String)>,
    /// Each `logic table "<path>"` clause's markdown content, keyed as
    /// written.
    pub tables: Vec<(String, String)>,
    /// Each imported `.evt` contract, compiled and keyed as written.
    pub events: Vec<(String, phero_ir::events::EventModel)>,
}

/// The import closure of one `.ifml` entry file, gathered: everything a host
/// needs to compile, type-check, and authorize the flow.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IfmlProject {
    /// Resolver bundle keyed `(importer identity, import-as-written)` — what
    /// [`phero_ifml::compile_ifml_str`] resolves against.
    pub imports: phero_ifml::IfmlImports,
    /// Every walked `.ifml` source `(identity, text)`, entry file first.
    pub sources: Vec<(String, String)>,
    /// Imported `.mox` domain sources `(resolved key, text)`, deduplicated by
    /// resolved key, in walk order.
    pub domains: Vec<(String, String)>,
    /// Imported `.actor` policies with their own gathered domains.
    pub actors: Vec<ActorImport>,
    /// Imported `.dmn` decision models with their own gathered domains and
    /// decision-table contents.
    pub decisions: Vec<DecisionImport>,
    /// Declared `.ifml`/`.mox`/`.actor`/`.dmn` imports the fetch provided
    /// nothing for, as `(importer identity, import-as-written)`. What to do
    /// about these is host policy (the CLI hard-errors, the LSP degrades).
    pub unresolved: Vec<(String, String)>,
}

/// The host's two-key fetch: `(importer identity, import as written) ->
/// (host-resolved key, text)`.
type PairFetch<'a> = dyn FnMut(&str, &str) -> Option<(String, String)> + 'a;

/// Gathers the full import closure of one `.ifml` entry file through
/// [`phero_ifml::walk_ifml_imports`]: `.ifml` imports feed the resolver
/// bundle and recurse; `.mox` imports land in [`IfmlProject::domains`];
/// `.actor`/`.dmn` imports become [`ActorImport`]/[`DecisionImport`]s whose
/// own imports are fetched under their resolved key (the host obligation the
/// walker's contract states); declined surface imports land in
/// [`IfmlProject::unresolved`] and unknown extensions pass through ignored.
/// The traversal is `phero_ifml`'s; this is only the host policy over it.
pub fn gather_ifml_project(
    entry_identity: &str,
    entry_source: &str,
    fetch: &mut PairFetch<'_>,
) -> IfmlProject {
    let mut project = IfmlProject::default();
    let walk = {
        let mut classifier = IfmlClassifier {
            project: &mut project,
        };
        let mut classify =
            |importer: &str, import: &str| classifier.classify(importer, import, fetch);
        phero_ifml::walk_ifml_imports(entry_identity, entry_source, &mut classify)
    };
    project.sources = walk.sources;
    project
}

/// The classification behind [`gather_ifml_project`]: one bucket per extension,
/// all fed by the host's two-key fetch.
struct IfmlClassifier<'a> {
    project: &'a mut IfmlProject,
}

impl IfmlClassifier<'_> {
    fn classify(
        &mut self,
        importer: &str,
        import: &str,
        fetch: &mut PairFetch<'_>,
    ) -> Option<(String, String)> {
        let Some((key, text)) = fetch(importer, import) else {
            if is_imported_surface(import) {
                self.project
                    .unresolved
                    .push((importer.to_string(), import.to_string()));
            }
            return None;
        };
        if import.ends_with(".ifml") {
            self.project.imports.provide(importer, import, text.clone());
            return Some((key, text));
        }
        if import.ends_with(".mox") {
            if !self
                .project
                .domains
                .iter()
                .any(|(existing, _)| *existing == key)
            {
                self.project.domains.push((key, text));
            }
        } else if import.ends_with(".actor") {
            self.absorb_actor(&key, &text, fetch);
        } else if import.ends_with(".dmn") {
            self.absorb_decision(&key, &text, fetch);
        }
        None
    }

    /// An `.actor` policy with its own `.mox` imports, fetched under the
    /// policy's resolved key (the host obligation).
    fn absorb_actor(&mut self, key: &str, text: &str, fetch: &mut PairFetch<'_>) {
        if self.project.actors.iter().any(|actor| actor.key == key) {
            return;
        }
        let mut actor = ActorImport {
            key: key.to_string(),
            source: text.to_string(),
            domains: Vec::new(),
        };
        for path in actor_paths(text) {
            match fetch(key, &path) {
                Some((domain_key, domain_text)) => {
                    push_unique(&mut actor.domains, domain_key, domain_text);
                }
                None => self.project.unresolved.push((key.to_string(), path)),
            }
        }
        self.project.actors.push(actor);
    }

    /// A `.dmn` model with its own `.mox` imports (`.evt` imports ride through
    /// this loop too — the driver's lowering filters them out of the domain
    /// union), its compiled `.evt` contracts, and its table contents.
    fn absorb_decision(&mut self, key: &str, text: &str, fetch: &mut PairFetch<'_>) {
        if self
            .project
            .decisions
            .iter()
            .any(|decision| decision.key == key)
        {
            return;
        }
        let imports = dmn_import_paths(text);
        let mut decision = DecisionImport {
            key: key.to_string(),
            source: text.to_string(),
            domains: Vec::new(),
            tables: Vec::new(),
            events: Vec::new(),
        };
        for path in imports {
            match fetch(key, &path) {
                Some((domain_key, domain_text)) => {
                    push_unique(&mut decision.domains, domain_key, domain_text);
                }
                None => self.project.unresolved.push((key.to_string(), path)),
            }
        }
        // An unreadable or non-compiling contract is absent — the DMN rule-1
        // diagnostic names the authored import string.
        for import in dmn_import_paths(text) {
            if !import.ends_with(".evt")
                || decision
                    .events
                    .iter()
                    .any(|(existing, _)| *existing == import)
            {
                continue;
            }
            if let Some((event_key, event_text)) = fetch(key, &import) {
                let compilation = compile_evt_str(
                    &event_key,
                    &event_text,
                    &decision.domains,
                    &DomainImports::default(),
                );
                if let Some(model) = compilation.model {
                    decision.events.push((import, model));
                }
            }
        }
        decision.tables =
            gather_tables(text, |table| fetch(key, table).map(|(_, content)| content));
        self.project.decisions.push(decision);
    }
}

/// Appends `(key, text)` to `bucket` unless the key is already there.
fn push_unique(bucket: &mut Vec<(String, String)>, key: String, text: String) {
    if !bucket.iter().any(|(existing, _)| *existing == key) {
        bucket.push((key, text));
    }
}

/// Compiles every gathered `.actor` policy in order against its own domains.
/// The first policy that fails to compile wins:
/// ``imported actor model {key} does not compile`` plus its error lines.
pub fn compile_actor_imports(actors: &[ActorImport]) -> Result<Vec<phero_ir::ActorModel>, String> {
    let mut models = Vec::new();
    for actor in actors {
        let imports = domain_imports(&actor.domains)?;
        let compilation = compile_actors_str(&actor.key, &actor.source, &actor.domains, &imports);
        models.push(finish_model(
            "imported actor model",
            &actor.key,
            compilation.model,
            &compilation.diagnostics,
        )?);
    }
    Ok(models)
}

/// Compiles every gathered `.dmn` decision model in order against its own
/// domains, tables, and events — the [`compile_actor_imports`] discipline.
pub fn compile_decision_imports(
    decisions: &[DecisionImport],
) -> Result<Vec<phero_ir::dmn::DmnModel>, String> {
    let mut models = Vec::new();
    for decision in decisions {
        let imports = domain_imports(&decision.domains)?;
        let compilation = compile_dmn_str(
            &decision.key,
            &decision.source,
            &decision.domains,
            &imports,
            &decision.tables,
            &decision.events,
        );
        models.push(finish_model(
            "imported decision model",
            &decision.key,
            compilation.model,
            &compilation.diagnostics,
        )?);
    }
    Ok(models)
}

/// Compiles every gathered `.mox` domain and unions the packages into one Core
/// IR model — the union [`phero_ifml::check_ifml`] binds expressions against.
/// Any diagnostics in any domain fail the union as ``imported domain {path}
/// does not compile``; the empty input is the valid empty union (what empty
/// means is host policy).
pub fn compile_domain_union(domains: &[(String, String)]) -> Result<phero_ir::Model, String> {
    let mut union = phero_ir::Model::new();
    if domains.is_empty() {
        return Ok(union);
    }
    let imports = domain_imports(domains)?;
    for (path, source) in domains {
        let compilation = compile_str(path, source, &imports);
        if !compilation.diagnostics.is_empty() {
            let rendered = render(path, source, &compilation.diagnostics);
            return Err(format!(
                "imported domain {path} does not compile:\n{rendered}"
            ));
        }
        let Some(model) = compilation.model else {
            return Err(format!("imported domain {path} does not compile"));
        };
        union.packages.extend(model.packages);
    }
    Ok(union)
}

/// Gathers the `.mox` domains one domain-surface file imports, each resolved
/// relative to `path_key`'s directory and deduplicated by resolved key. A
/// missing import skips silently (the LSP's degrade-gracefully discipline) —
/// hosts wanting the CLI's hard error capture it in their own closure side
/// channel. This deliberately deduplicates by *resolved* key (the CLI's
/// original `read_domain_pair` used the raw import string).
pub fn gather_domain_sources(
    path_key: &str,
    source: &str,
    import_paths: impl Fn(&str) -> Vec<String>,
    fetch: &mut Fetch<'_>,
) -> Vec<(String, String)> {
    gather_domains(path_key, source, &import_paths, fetch, &mut |_| Ok(())).unwrap_or_default()
}

/// One written `.ddd` import site — the span anchor a host uses for a spanned
/// tree-level diagnostic (a cycle, a two-root fragment, a root imported as a
/// fragment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DddFragmentSite {
    /// The importing file's resolved path.
    pub importer: String,
    /// The import string as written.
    pub import: String,
    /// The resolved path the import denotes (relative to the importer).
    pub resolved: String,
    /// Span of the `import` declaration, `import` keyword included.
    pub span: phero_syntax::Span,
}

/// The `.ddd` import closure of a design root (DD8, issue pherolang#305):
/// fragments in first-visit order, plus the misses and cycles the host reports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DddFragments {
    /// Each fragment as `(host-resolved path, source)`, first visit first.
    pub fragments: Vec<(String, String)>,
    /// Imports the fetch could not provide, as written.
    pub missing: Vec<String>,
    /// The import path that closed a cycle, when one exists.
    pub cycles: Vec<String>,
    /// Every written `.ddd` import, in visit order.
    pub sites: Vec<DddFragmentSite>,
}

/// Gathers a `.ddd` root's `.ddd` import closure, recursing through fragments
/// (their imports resolve against *their* directory). Cycles and misses are
/// reported, never looped on.
///
/// This is the `.ddd` **special case**: a root plus its fragments is one unit
/// of compilation, so it walks depth-first with a visiting stack instead of
/// the shared per-file loop. It keeps that walker's resolution ([`Site`]) and
/// message ([`cannot_read`]) vocabulary, so keys and texts are identical.
pub fn gather_ddd_fragments(path_key: &str, source: &str, fetch: &mut Fetch<'_>) -> DddFragments {
    fn walk_fragments(
        path: &str,
        source: &str,
        fetch: &mut Fetch<'_>,
        visiting: &mut Vec<String>,
        visited: &mut std::collections::BTreeSet<String>,
        out: &mut DddFragments,
    ) {
        let Some(ast) = phero_syntax::parse_ddd(source).ast else {
            return;
        };
        for import in &ast.imports {
            if !import.path.ends_with(".ddd") {
                continue;
            }
            let site = Site::new(path, &import.path);
            out.sites.push(DddFragmentSite {
                importer: site.importer,
                import: import.path.clone(),
                resolved: site.resolved.clone(),
                span: import.span,
            });
            if visiting.contains(&site.resolved) {
                out.cycles.push(site.resolved);
                continue;
            }
            if !visited.insert(site.resolved.clone()) {
                continue;
            }
            let Some(fragment) = fetch(&site.resolved) else {
                out.missing.push(import.path.clone());
                continue;
            };
            visiting.push(site.resolved.clone());
            walk_fragments(&site.resolved, &fragment, fetch, visiting, visited, out);
            visiting.pop();
            out.fragments.push((site.resolved, fragment));
        }
    }

    let mut out = DddFragments::default();
    let mut visiting = vec![path_key.to_string()];
    let mut visited = std::collections::BTreeSet::from([path_key.to_string()]);
    walk_fragments(
        path_key,
        source,
        fetch,
        &mut visiting,
        &mut visited,
        &mut out,
    );
    out
}

/// The gathered import closure of one `.ddd` design **root**, domains included
/// (DD8): the transitive fragments, every `.mox` domain the root **or any
/// fragment** imports (each resolved relative to its declaring file), and the
/// misses a host must report. One gatherer for the CLI reader, the driver's
/// downstream importers, and the LSP snapshot — a design application is
/// compiled whole or not at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DddAppImports {
    /// Each fragment as `(host-resolved path, source)`, first visit first.
    pub fragments: Vec<(String, String)>,
    /// Each imported `.mox` domain as `(resolved key, source)`, in first
    /// appearance order across the root and its fragments.
    pub domains: Vec<(String, String)>,
    /// `.ddd` fragment imports the fetch could not provide, as written.
    pub missing: Vec<String>,
    /// Domain imports the fetch could not provide, as
    /// `(resolved key, importing file)`.
    pub missing_domains: Vec<(String, String)>,
    /// The import path that closed a `.ddd` cycle, when one exists.
    pub cycles: Vec<String>,
    /// Every written `.ddd` import across the root and its fragments, in
    /// visit order.
    pub sites: Vec<DddFragmentSite>,
}

/// Gathers a `.ddd` root's whole import closure: fragments (transitively), and
/// the `.mox` domains declared by the root **and** by every fragment, each
/// resolved relative to its declaring file. Misses and cycles are reported,
/// never fatal — that is host policy. **Whole-or-nothing**: it must not be
/// flattened into the shared per-file walk, because compiling a root alone
/// silently drops its fragments' modules.
pub fn gather_ddd_app_imports(
    path_key: &str,
    source: &str,
    fetch: &mut Fetch<'_>,
) -> DddAppImports {
    let fragments = gather_ddd_fragments(path_key, source, fetch);
    let mut gathered = DddAppImports {
        fragments: fragments.fragments,
        domains: Vec::new(),
        missing: fragments.missing,
        missing_domains: Vec::new(),
        cycles: fragments.cycles,
        sites: fragments.sites,
    };
    gather_ddd_domains(
        path_key,
        source,
        fetch,
        &mut gathered.domains,
        &mut gathered.missing_domains,
    );
    for (fragment_path, fragment_source) in &gathered.fragments {
        gather_ddd_domains(
            fragment_path,
            fragment_source,
            fetch,
            &mut gathered.domains,
            &mut gathered.missing_domains,
        );
    }
    gathered
}

/// The `.mox` imports one `.ddd` source declares (`.ddd` fragments and `.dmn`
/// decision models are other buckets).
fn ddd_domain_imports(source: &str) -> Vec<String> {
    phero_syntax::parse_ddd(source)
        .ast
        .map(|ast| {
            ast.imports
                .into_iter()
                .map(|import| import.path)
                .filter(|import| !import.ends_with(".ddd") && !import.ends_with(".dmn"))
                .collect()
        })
        .unwrap_or_default()
}

/// Appends the `.mox` domains `source` imports to `domains`, deduplicated by
/// resolved key across the root and every fragment gathered before it; a miss
/// lands in `missing` as `(resolved key, importer)`.
fn gather_ddd_domains(
    path_key: &str,
    source: &str,
    fetch: &mut Fetch<'_>,
    domains: &mut Vec<(String, String)>,
    missing: &mut Vec<(String, String)>,
) {
    let result = walk(
        path_key,
        source,
        &ddd_domain_imports,
        fetch,
        &mut DomainPolicy {
            domains,
            on_miss: &mut |site| {
                missing.push((site.resolved.clone(), site.importer.clone()));
                Ok(())
            },
        },
    );
    debug_assert!(
        result.is_ok(),
        "the `.ddd` domain closure records misses instead of failing"
    );
}

/// The artifact kinds a path-keyed surface gathers into.
#[derive(Debug, Clone, Copy)]
enum Kind {
    Domain,
    Design,
    Actor,
    Event,
    Decision,
    Flow,
}

/// One surface's bucket handler: the routed kind, the site, the fetched text,
/// and the host fetch a nested gather reuses.
type Absorb<'a> = dyn FnMut(Kind, &Site, &str, &mut Fetch<'_>) -> Result<(), String> + 'a;

/// Walks `source`'s imports into a surface's typed buckets: `routes` names the
/// extensions the surface declares and `absorb` compiles each fetched import
/// into its bucket. Unknown extensions pass through and every miss is the
/// pinned read error.
fn walk_buckets(
    path_key: &str,
    source: &str,
    import_paths: &dyn Fn(&str) -> Vec<String>,
    fetch: &mut Fetch<'_>,
    routes: &[(&str, Kind)],
    absorb: &mut Absorb<'_>,
) -> Result<(), String> {
    walk(
        path_key,
        source,
        import_paths,
        fetch,
        &mut Routed { routes, absorb },
    )
}

/// The routing policy behind [`walk_buckets`]: extension → [`Kind`].
struct Routed<'a> {
    routes: &'a [(&'a str, Kind)],
    absorb: &'a mut Absorb<'a>,
}

impl Routed<'_> {
    fn route(&self, site: &Site) -> Option<Kind> {
        let extension = site.extension()?;
        self.routes
            .iter()
            .find(|(each, _)| *each == extension)
            .map(|(_, kind)| *kind)
    }
}

impl ImportPolicy for Routed<'_> {
    fn accepts(&self, site: &Site) -> bool {
        self.route(site).is_some()
    }

    fn missing(&mut self, site: &Site) -> Result<(), String> {
        Err(cannot_read(&site.resolved, &site.importer))
    }

    fn absorb(&mut self, site: &Site, text: &str, fetch: &mut Fetch<'_>) -> Result<(), String> {
        let kind = self
            .route(site)
            .expect("absorb is only reached for an accepted import");
        (self.absorb)(kind, site, text, fetch)
    }
}

/// The buckets a `.uc` (or `.cmmn`) use-case file's imports resolve to: the
/// raw domains plus the pre-compiled `.ddd`/`.actor`/`.evt` artifacts
/// [`crate::compile_usecase_str`] consumes, keyed by resolved path. A missing
/// import anywhere (nested gathering included) errors ``cannot read imported
/// file {resolved} (imported by {importer})``; a non-compiling artifact names
/// its surface. Unknown extensions pass through so the driver reports them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UcBuckets {
    /// Each imported `.mox` domain as `(resolved key, source)`.
    pub domains: Vec<(String, String)>,
    /// Each imported `.ddd` design, compiled.
    pub designs: Vec<(String, phero_ir::ddd::DddModel)>,
    /// Each imported `.actor` policy, compiled.
    pub actors: Vec<(String, phero_ir::ActorModel)>,
    /// Each imported `.evt` contract, compiled.
    pub events: Vec<(String, phero_ir::events::EventModel)>,
}

/// Gathers a `.uc` file's imports into [`UcBuckets`]: paths resolve relative
/// to `path_key`'s directory, duplicates gather once, and each artifact's own
/// `.mox` imports resolve relative to the artifact's resolved key (the CLI
/// rule — `pherolang check` on the artifact itself).
pub fn gather_usecase_imports(
    path_key: &str,
    source: &str,
    import_paths: impl Fn(&str) -> Vec<String>,
    fetch: &mut Fetch<'_>,
) -> Result<UcBuckets, String> {
    let mut buckets = UcBuckets::default();
    walk_buckets(
        path_key,
        source,
        &import_paths,
        fetch,
        &[
            ("mox", Kind::Domain),
            ("ddd", Kind::Design),
            ("actor", Kind::Actor),
            ("evt", Kind::Event),
        ],
        &mut |kind, site, text, fetch| {
            let key = site.resolved.clone();
            match kind {
                Kind::Domain => buckets.domains.push((key, text.to_string())),
                Kind::Design => {
                    let model = gather_design_model(&key, text, fetch)?;
                    buckets.designs.push((key, model));
                }
                Kind::Actor => {
                    let model = gather_actor_model(&key, text, fetch)?;
                    buckets.actors.push((key, model));
                }
                Kind::Event => {
                    let model = gather_event_model(&key, text, fetch)?;
                    buckets.events.push((key, model));
                }
                _ => unreachable!("the use-case routes declare no artifact flow"),
            }
            Ok(())
        },
    )?;
    Ok(buckets)
}

/// The buckets a `.cmmn` case file's imports resolve to: the `.uc` buckets
/// plus the pre-compiled `.dmn` decision models a `decision` task's `invokes`
/// clause resolves against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CmmnBuckets {
    /// Each imported `.mox` domain as `(resolved key, source)`.
    pub domains: Vec<(String, String)>,
    /// Each imported `.ddd` design, compiled.
    pub designs: Vec<(String, phero_ir::ddd::DddModel)>,
    /// Each imported `.actor` policy, compiled.
    pub actors: Vec<(String, phero_ir::ActorModel)>,
    /// Each imported `.evt` contract, compiled.
    pub events: Vec<(String, phero_ir::events::EventModel)>,
    /// Each imported `.dmn` decision model, compiled.
    pub decisions: Vec<(String, phero_ir::dmn::DmnModel)>,
}

/// Gathers a `.cmmn` file's imports into [`CmmnBuckets`] — the four use-case
/// buckets through [`gather_usecase_imports`] (identical discovery, dedup and
/// error text), then a decision pass over the same imports. The two passes are
/// the pinned error order.
pub fn gather_case_imports(
    path_key: &str,
    source: &str,
    import_paths: &dyn Fn(&str) -> Vec<String>,
    fetch: &mut Fetch<'_>,
) -> Result<CmmnBuckets, String> {
    let uc = gather_usecase_imports(path_key, source, |source| import_paths(source), fetch)?;
    let mut decisions = Vec::new();
    walk_buckets(
        path_key,
        source,
        import_paths,
        fetch,
        &[("dmn", Kind::Decision)],
        &mut |_, site, text, fetch| {
            let model = gather_decision_model(&site.resolved, text, fetch)?;
            decisions.push((site.resolved.clone(), model));
            Ok(())
        },
    )?;
    Ok(CmmnBuckets {
        domains: uc.domains,
        designs: uc.designs,
        actors: uc.actors,
        events: uc.events,
        decisions,
    })
}

/// Gathers and compiles one imported `.dmn` decision model: its `.mox` imports
/// resolve relative to its resolved key, its `.evt` imports gather through
/// [`gather_dmn_events`], and each `logic table "<path>"` content is fetched
/// the same way and keyed as written.
fn gather_decision_model(
    key: &str,
    text: &str,
    fetch: &mut Fetch<'_>,
) -> Result<phero_ir::dmn::DmnModel, String> {
    let domains = gather_domains_required(key, text, &dmn_import_paths, fetch)?;
    let imports = domain_imports(&domains)?;
    let dir = dir_of(key);
    let tables = gather_tables(text, |table| fetch(&dir.join(table).display().to_string()));
    let events = gather_dmn_events(key, text, fetch)?;
    let compilation = compile_dmn_str(key, text, &domains, &imports, &tables, &events);
    finish_model(
        "imported decision model",
        key,
        compilation.model,
        &compilation.diagnostics,
    )
}

/// A `.deploy` file's compiled designs and flows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeployPairs {
    /// Each imported `.ddd` design, compiled, keyed by its resolved path.
    pub designs: Vec<(String, phero_ir::ddd::DddModel)>,
    /// Each imported `.ifml` flow, compiled, keyed by its resolved path.
    pub flows: Vec<(String, phero_ir::ifml::IfmlModel)>,
}

impl DeployPairs {
    /// Compiles one imported `.ifml` flow. The walker recurses under
    /// as-written import strings (only the entry identity is a resolved
    /// disk-path key), so the bridge keeps an identity → resolved-key map per
    /// flow: each import resolves relative to the resolved key its importer was
    /// actually fetched under, never the process CWD. The flow's
    /// `.actor`/`.mox`/`.dmn` imports ride through the same bridge but the
    /// deploy lowering consumes none, so they drop.
    fn absorb_flow(&mut self, key: &str, text: &str, fetch: &mut Fetch<'_>) -> Result<(), String> {
        let mut keys: BTreeMap<String, String> = BTreeMap::new();
        keys.insert(key.to_string(), key.to_string());
        let mut bridge = |importer: &str, import: &str| -> Option<(String, String)> {
            let base = keys
                .get(importer)
                .cloned()
                .unwrap_or_else(|| importer.to_string());
            let resolved_key = dir_of(&base).join(import).display().to_string();
            let text = fetch(&resolved_key)?;
            keys.insert(import.to_string(), resolved_key.clone());
            Some((resolved_key, text))
        };
        let project = gather_ifml_project(key, text, &mut bridge);
        match phero_ifml::compile_ifml_str(key, text, &project.imports) {
            Ok(compilation) => {
                self.flows.push((key.to_string(), compilation.model));
                Ok(())
            }
            Err(_) => Err(format!("imported flow {key} does not compile")),
        }
    }
}

/// Gathers a `.deploy` file's imports into [`DeployPairs`] — the CLI's
/// `read_deploy_pair`/`read_design_model`/`read_flow_model` discipline.
pub fn gather_deploy_imports(
    path_key: &str,
    source: &str,
    import_paths: impl Fn(&str) -> Vec<String>,
    fetch: &mut Fetch<'_>,
) -> Result<DeployPairs, String> {
    let mut pairs = DeployPairs::default();
    walk_buckets(
        path_key,
        source,
        &import_paths,
        fetch,
        &[("ddd", Kind::Design), ("ifml", Kind::Flow)],
        &mut |kind, site, text, fetch| {
            let key = &site.resolved;
            match kind {
                Kind::Design => {
                    let model = gather_design_model(key, text, fetch)?;
                    pairs.designs.push((key.clone(), model));
                }
                Kind::Flow => pairs.absorb_flow(key, text, fetch)?,
                _ => unreachable!("the deploy routes declare no other bucket"),
            }
            Ok(())
        },
    )?;
    Ok(pairs)
}

/// Gathers and compiles one imported `.ddd` design **application**: the whole
/// root — its transitive fragments and every domain the root or a fragment
/// imports, each resolved relative to its declaring file (DD8).
fn gather_design_model(
    key: &str,
    text: &str,
    fetch: &mut Fetch<'_>,
) -> Result<phero_ir::ddd::DddModel, String> {
    let gathered = gather_ddd_app_imports(key, text, fetch);
    if let Some(missing) = gathered.missing.first() {
        return Err(format!(
            "imported design fragment \"{missing}\" was not provided"
        ));
    }
    if let Some((resolved, importer)) = gathered.missing_domains.first() {
        return Err(cannot_read(resolved, importer));
    }
    if let Some(cycle) = gathered.cycles.first() {
        return Err(format!("design import cycle through {cycle}"));
    }
    let imports = domain_imports(&gathered.domains)?;
    let fragments: Vec<crate::DddFragment> = gathered
        .fragments
        .iter()
        .map(|(path, source)| crate::DddFragment {
            path: path.clone(),
            source: source.clone(),
        })
        .collect();
    let compilation = compile_ddd_app_str(key, text, &fragments, &gathered.domains, &imports);
    finish_model(
        "imported design",
        key,
        compilation.model,
        &compilation.diagnostics,
    )
}

/// Gathers and compiles one imported `.actor` policy — the
/// [`compile_actor_imports`] shape for a single policy.
fn gather_actor_model(
    key: &str,
    text: &str,
    fetch: &mut Fetch<'_>,
) -> Result<phero_ir::ActorModel, String> {
    let actor = ActorImport {
        key: key.to_string(),
        source: text.to_string(),
        domains: gather_domains_required(key, text, &|source| actor_paths(source), fetch)?,
    };
    compile_actor_imports(std::slice::from_ref(&actor))
        .map(|mut models| models.pop().expect("one policy, one model"))
}

/// The raw import strings one surface file declares, in source order (empty
/// when the file does not parse — the surface's compile reports that). This is
/// the one place each surface's import syntax is read for gathering; `.mox`
/// (`import schema`/`import sigil` content) and `.ifml` (the two-key walk)
/// gather through their own paths.
pub fn import_paths(surface: crate::SurfaceKind, source: &str) -> Vec<String> {
    macro_rules! declared {
        ($parsed:expr) => {
            $parsed
                .ast
                .map(|ast| ast.imports.into_iter().map(|import| import.path).collect())
                .unwrap_or_default()
        };
    }
    match surface {
        crate::SurfaceKind::Ddd => declared!(phero_syntax::parse_ddd(source)),
        crate::SurfaceKind::Actor => declared!(phero_syntax::parse_actors(source)),
        crate::SurfaceKind::Evt => declared!(phero_syntax::parse_evt(source)),
        crate::SurfaceKind::Dmn => declared!(phero_syntax::parse_dmn(source)),
        crate::SurfaceKind::Uc => declared!(phero_syntax::parse_usecases(source)),
        crate::SurfaceKind::Cmmn => declared!(phero_syntax::parse_cmmn(source)),
        crate::SurfaceKind::Deploy => declared!(phero_syntax::parse_deploy(source)),
        crate::SurfaceKind::Mox | crate::SurfaceKind::Ifml => Vec::new(),
    }
}

/// [`import_paths`] for `.dmn`, whose list hosts split by extension: `.mox`
/// paths join the domain gather, `.evt` paths [`gather_dmn_events`].
pub fn dmn_import_paths(source: &str) -> Vec<String> {
    import_paths(crate::SurfaceKind::Dmn, source)
}

/// [`import_paths`] for `.actor`.
fn actor_paths(source: &str) -> Vec<String> {
    import_paths(crate::SurfaceKind::Actor, source)
}

/// [`import_paths`] for `.evt`.
fn event_paths(source: &str) -> Vec<String> {
    import_paths(crate::SurfaceKind::Evt, source)
}

/// The `logic table "<path>"` paths one `.dmn` source names, in source order,
/// duplicates included.
pub fn dmn_table_paths(source: &str) -> Vec<String> {
    phero_syntax::parse_dmn(source)
        .ast
        .and_then(|ast| ast.model)
        .map(|model| {
            model
                .decisions
                .iter()
                .filter_map(|decision| match &decision.logic {
                    Some(phero_syntax::DmnLogic::Table { source, .. }) => Some(source.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The decision-table contents one `.dmn` source names, keyed by the path **as
/// written**. `fetch` is asked for each written path; a miss is simply not
/// provided (the driver's unprovided-table diagnostic names it) and a duplicate
/// reference never re-provides.
fn gather_tables(
    source: &str,
    mut fetch: impl FnMut(&str) -> Option<String>,
) -> Vec<(String, String)> {
    let mut tables: Vec<(String, String)> = Vec::new();
    for table in dmn_table_paths(source) {
        if tables.iter().any(|(existing, _)| *existing == table) {
            continue;
        }
        if let Some(content) = fetch(&table) {
            tables.push((table, content));
        }
    }
    tables
}

/// Gathers the `.evt` contracts one `.dmn` file imports, each resolved relative
/// to `path_key`'s directory, compiled through the ordinary event pipeline, and
/// keyed by the import string **as written**. A missing or non-compiling
/// contract is a clean error naming it; duplicates gather once.
pub fn gather_dmn_events(
    path_key: &str,
    source: &str,
    fetch: &mut Fetch<'_>,
) -> Result<Vec<(String, phero_ir::events::EventModel)>, String> {
    let mut events: Vec<(String, phero_ir::events::EventModel)> = Vec::new();
    walk(
        path_key,
        source,
        &dmn_import_paths,
        fetch,
        &mut DmnEvents {
            events: &mut events,
        },
    )?;
    Ok(events)
}

/// The `.dmn` event-contract bucket: `.evt` imports only, keyed as written.
struct DmnEvents<'a> {
    events: &'a mut Vec<(String, phero_ir::events::EventModel)>,
}

impl ImportPolicy for DmnEvents<'_> {
    fn accepts(&self, site: &Site) -> bool {
        site.extension() == Some("evt")
    }

    fn dedupe_key(&self, site: &Site) -> String {
        site.written.clone()
    }

    fn missing(&mut self, site: &Site) -> Result<(), String> {
        Err(cannot_read(&site.resolved, &site.importer))
    }

    fn absorb(&mut self, site: &Site, text: &str, fetch: &mut Fetch<'_>) -> Result<(), String> {
        let model = gather_event_model(&site.resolved, text, fetch)?;
        self.events.push((site.written.clone(), model));
        Ok(())
    }
}

/// Gathers and compiles one imported `.evt` contract: its own `.mox` imports
/// resolve relative to its resolved key.
fn gather_event_model(
    key: &str,
    text: &str,
    fetch: &mut Fetch<'_>,
) -> Result<phero_ir::events::EventModel, String> {
    let domains = gather_domains_required(key, text, &|source| event_paths(source), fetch)?;
    let imports = domain_imports(&domains)?;
    let compilation = compile_evt_str(key, text, &domains, &imports);
    finish_model(
        "imported contract",
        key,
        compilation.model,
        &compilation.diagnostics,
    )
}

/// The `.mox` domain bucket: every written import lands as
/// `(resolved key, text)`, in source order, deduplicated by the shared walker
/// and against keys an earlier file already appended. What a miss means is
/// `on_miss` — skipped (the lenient gather), fatal (an imported artifact's own
/// domains), or recorded (the `.ddd` application closure).
struct DomainPolicy<'a> {
    domains: &'a mut Vec<(String, String)>,
    on_miss: &'a mut dyn FnMut(&Site) -> Result<(), String>,
}

impl ImportPolicy for DomainPolicy<'_> {
    fn accepts(&self, site: &Site) -> bool {
        !self
            .domains
            .iter()
            .any(|(existing, _)| *existing == site.resolved)
    }

    fn missing(&mut self, site: &Site) -> Result<(), String> {
        (self.on_miss)(site)
    }

    fn absorb(&mut self, site: &Site, text: &str, _fetch: &mut Fetch<'_>) -> Result<(), String> {
        self.domains.push((site.resolved.clone(), text.to_string()));
        Ok(())
    }
}

/// Gathers `source`'s `.mox` domains into a fresh bucket; what a miss means
/// is `on_miss`.
fn gather_domains(
    key: &str,
    text: &str,
    import_paths: &dyn Fn(&str) -> Vec<String>,
    fetch: &mut Fetch<'_>,
    on_miss: &mut dyn FnMut(&Site) -> Result<(), String>,
) -> Result<Vec<(String, String)>, String> {
    let mut domains = Vec::new();
    walk(
        key,
        text,
        import_paths,
        fetch,
        &mut DomainPolicy {
            domains: &mut domains,
            on_miss,
        },
    )?;
    Ok(domains)
}

/// Gathers an imported artifact's own `.mox` domains under the **required**
/// contract: a miss is the pinned read error naming the artifact.
fn gather_domains_required(
    key: &str,
    text: &str,
    import_paths: &dyn Fn(&str) -> Vec<String>,
    fetch: &mut Fetch<'_>,
) -> Result<Vec<(String, String)>, String> {
    gather_domains(key, text, import_paths, fetch, &mut |site| {
        Err(cannot_read(&site.resolved, &site.importer))
    })
}

/// The `import schema`/`import sigil` content a gathered domain set needs —
/// one place for the two providers every compile entry point takes.
fn domain_imports(domains: &[(String, String)]) -> Result<DomainImports, String> {
    let schemas = collect_schema_imports(domains).map_err(|error| error.to_string())?;
    let sigil = collect_sigil_imports(domains).map_err(|error| error.to_string())?;
    Ok(DomainImports {
        schemas,
        sigil: sigil.imports,
    })
}

/// A model-less compilation is the pinned failure: `{kind} {key} does not
/// compile`, plus one `  {path}: {message}` line per error diagnostic.
fn finish_model<T>(
    kind: &str,
    key: &str,
    model: Option<T>,
    diagnostics: &[(String, Diagnostic)],
) -> Result<T, String> {
    model.ok_or_else(|| does_not_compile(kind, key, diagnostics))
}

/// The pinned failure message — the exact shape the CLI's
/// `read_design_model`/`read_actor_model`/`read_event_model` emit.
fn does_not_compile(kind: &str, key: &str, diagnostics: &[(String, Diagnostic)]) -> String {
    let messages: Vec<String> = diagnostics
        .iter()
        .filter(|(_, diagnostic)| diagnostic.is_error())
        .map(|(path, diagnostic)| format!("{path}: {}", diagnostic.message))
        .collect();
    format!(
        "{kind} {key} does not compile{}",
        if messages.is_empty() {
            String::new()
        } else {
            format!(":\n  {}", messages.join("\n  "))
        }
    )
}

/// `true` for the extensions [`IfmlProject::unresolved`] tracks; other
/// extensions pass through ignored (parity with today's walker hosts).
fn is_imported_surface(import: &str) -> bool {
    import.ends_with(".ifml")
        || import.ends_with(".mox")
        || import.ends_with(".actor")
        || import.ends_with(".dmn")
}
