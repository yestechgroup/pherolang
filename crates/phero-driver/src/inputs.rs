//! The typed compile-input bundle every one-shot artifact entry point takes
//! (SK4 of epic pherolang#328, DR6 of
//! [ADR 0005](../../../docs/adr/0005-shared-surface-kernel-layering.md)):
//! one value carrying the buckets a surface compile resolves its imports
//! against, in place of parallel `(path, payload)` slice parameters.
//!
//! The bundle is **inert by construction** (the epic's named risk): it
//! stores exactly what the host gathered — no resolution, no validation, no
//! caching, no global state. Each surface reads only the buckets it
//! consumes through the typed accessors below, and a bucket a surface does
//! not consume is never consulted. If a field here ever starts answering a
//! resolution question it has become a second
//! [`DomainTypes`](phero_expr::DomainTypes) or `Symbols`, which is out of
//! scope.
//!
//! Keys are the **host-resolved paths** the gatherers in
//! [`crate::project`] hand back (a decision-table content is keyed by the
//! path the `.dmn` file writes, because that is how the driver matches it),
//! in discovery order. An empty bucket is the plain-compile behavior; a
//! missing bucket is an empty slice, never a positional argument.
//!
//! ```
//! use phero_driver::CompileInputs;
//!
//! let domains = [(
//!     "library.mox".to_string(),
//!     "package nz.example.library\n\nclass Book { String title }".to_string(),
//! )];
//! let source = concat!(
//!     "import \"library.mox\"\n",
//!     "\n",
//!     "application Library {\n",
//!     "    base nz.example.library\n",
//!     "\n",
//!     "    module catalogue {\n",
//!     "        entity Book repository BookRepository {\n",
//!     "            findById;\n",
//!     "        }\n",
//!     "    }\n",
//!     "}\n",
//! );
//! let inputs = CompileInputs::from_domains(&domains);
//! let compilation = phero_driver::compile_ddd_str("library.ddd", source, &inputs);
//! assert!(compilation.diagnostics.is_empty());
//! ```

use crate::imports::{DomainImports, MachineSources};
use crate::project::{CmmnBuckets, DeployPairs, UcBuckets};

/// The gather-once compile inputs of one artifact compilation.
///
/// Fields are private and read through accessors so a surface takes only
/// the buckets it consumes: `.evt` reads `domains` + `imports`, `.ddd` adds
/// `actors` + `decisions` + `events`, `.deploy` reads `designs` + `flows`,
/// `.dmn` reads `domains` + `imports` + `tables` + `events`, `.uc`/`.cmmn`
/// read the four/five artifact buckets, and `.actor` adds `designs`. A
/// wrong bucket is a compile error, not a runtime mismatch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompileInputs {
    /// Raw `.mox` domain texts, keyed by the host-resolved path the surface
    /// file's import resolves to.
    domains: Vec<(String, String)>,
    /// Pre-compiled `.ddd` design applications, keyed by resolved path.
    designs: Vec<(String, phero_ir::ddd::DddModel)>,
    /// Pre-compiled `.actor` policies, keyed by resolved path — or `None`
    /// when the host has **no actor models for this application** (see
    /// [`CompileInputs::actors`] for the provenance this records).
    actors: Option<Vec<(String, phero_ir::ActorModel)>>,
    /// Pre-compiled `.evt` event contracts, keyed by resolved path (`as
    /// written` for a `.dmn` file's own contract bucket).
    events: Vec<(String, phero_ir::events::EventModel)>,
    /// Pre-compiled `.dmn` decision models, keyed by resolved path.
    decisions: Vec<(String, phero_ir::dmn::DmnModel)>,
    /// Pre-compiled `.ifml` flows, keyed by resolved path.
    flows: Vec<(String, phero_ir::ifml::IfmlModel)>,
    /// External decision-table markdown contents, keyed by the `logic table
    /// "<path>"` string **as written** (the `.dmn` matching rule).
    tables: Vec<(String, String)>,
    /// The `import schema`/`import sigil` content of the domain files.
    imports: DomainImports,
    /// The `.ddd` machine-reference TypeScript texts, keyed by the
    /// **(design path, machine path)** pair as the host names them (issue
    /// pherolang#388).
    machines: MachineSources,
}

impl CompileInputs {
    /// The empty bundle: no buckets, no import content — the plain-compile
    /// behavior every tracked query passes.
    pub fn new() -> Self {
        Self::default()
    }

    /// A bundle over one domain set, for the domain-only surfaces
    /// (`.evt`/`.ddd`/`.actor`) and for the tracked queries' callers. The
    /// sources are copied into the bundle — it owns what it carries.
    pub fn from_domains(domains: &[(String, String)]) -> Self {
        Self::new().with_domains(domains.to_vec())
    }

    /// Adds raw `.mox` domain texts keyed by resolved path.
    pub fn with_domains<I>(mut self, domains: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        self.domains.extend(domains);
        self
    }

    /// Adds pre-compiled `.ddd` design applications keyed by resolved path.
    pub fn with_designs<I>(mut self, designs: I) -> Self
    where
        I: IntoIterator<Item = (String, phero_ir::ddd::DddModel)>,
    {
        self.designs.extend(designs);
        self
    }

    /// Declares the application's `.actor` set, keyed by resolved path —
    /// **possibly empty**. This is the provenance [`CompileInputs::actors`]
    /// documents: the host knows this application's policies and they are
    /// exactly these.
    pub fn with_actors<I>(mut self, actors: I) -> Self
    where
        I: IntoIterator<Item = (String, phero_ir::ActorModel)>,
    {
        self.actors = Some(actors.into_iter().collect());
        self
    }

    /// Adds pre-compiled `.evt` event contracts keyed by resolved path.
    pub fn with_events<I>(mut self, events: I) -> Self
    where
        I: IntoIterator<Item = (String, phero_ir::events::EventModel)>,
    {
        self.events.extend(events);
        self
    }

    /// Adds pre-compiled `.dmn` decision models keyed by resolved path.
    pub fn with_decisions<I>(mut self, decisions: I) -> Self
    where
        I: IntoIterator<Item = (String, phero_ir::dmn::DmnModel)>,
    {
        self.decisions.extend(decisions);
        self
    }

    /// Adds pre-compiled `.ifml` flows keyed by resolved path.
    pub fn with_flows<I>(mut self, flows: I) -> Self
    where
        I: IntoIterator<Item = (String, phero_ir::ifml::IfmlModel)>,
    {
        self.flows.extend(flows);
        self
    }

    /// Adds decision-table markdown contents keyed by the path as written.
    pub fn with_tables<I>(mut self, tables: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        self.tables.extend(tables);
        self
    }

    /// Provides the domain files' `import schema`/`import sigil` content.
    pub fn with_imports(mut self, imports: DomainImports) -> Self {
        self.imports = imports;
        self
    }

    /// Adds the `.ddd` machine-reference TypeScript texts, keyed by the
    /// **(design path, machine path)** pair as the host names them (issue
    /// pherolang#388).
    pub fn with_machines<I>(mut self, sources: I) -> Self
    where
        I: IntoIterator<Item = (String, String, String)>,
    {
        for (design_path, machine_path, source) in sources {
            self.machines.insert(design_path, machine_path, source);
        }
        self
    }

    /// The raw `.mox` domain texts keyed by resolved path.
    pub fn domains(&self) -> &[(String, String)] {
        &self.domains
    }

    /// The pre-compiled `.ddd` design applications keyed by resolved path.
    pub fn designs(&self) -> &[(String, phero_ir::ddd::DddModel)] {
        &self.designs
    }

    /// The pre-compiled `.actor` policies keyed by resolved path.
    ///
    /// `None` means the host has **no actor models for this application**;
    /// `Some(…)` — an empty set included — means the application's actor
    /// set is known. That is provenance, not a policy flag: the `.ddd`
    /// engine consumes the models and treats an empty set exactly like no
    /// models (capability names stay unvalidated, the plain compile's
    /// behavior), while every other surface treats `None` as an empty
    /// bucket. The distinction is carried because the two hosts genuinely
    /// know different things — the tracked queries and the gatherers know
    /// no policies, the CLI/LSP pipelines pool the application's whole
    /// `.actor` set — and collapsing it would make the bundle lie about what
    /// its host gathered.
    pub fn actors(&self) -> Option<&[(String, phero_ir::ActorModel)]> {
        self.actors.as_deref()
    }

    /// The pre-compiled `.evt` event contracts keyed by resolved path.
    pub fn events(&self) -> &[(String, phero_ir::events::EventModel)] {
        &self.events
    }

    /// The pre-compiled `.dmn` decision models keyed by resolved path.
    pub fn decisions(&self) -> &[(String, phero_ir::dmn::DmnModel)] {
        &self.decisions
    }

    /// The pre-compiled `.ifml` flows keyed by resolved path.
    pub fn flows(&self) -> &[(String, phero_ir::ifml::IfmlModel)] {
        &self.flows
    }

    /// The decision-table contents keyed by the path as written.
    pub fn tables(&self) -> &[(String, String)] {
        &self.tables
    }

    /// The domain files' `import schema`/`import sigil` content.
    pub fn imports(&self) -> &DomainImports {
        &self.imports
    }

    /// The `.ddd` machine-reference TypeScript texts, keyed by the
    /// **(design path, machine path)** pair (issue pherolang#388).
    pub fn machines(&self) -> &MachineSources {
        &self.machines
    }
}

impl From<UcBuckets> for CompileInputs {
    /// The use-case gatherer's four buckets as one bundle. The actor bucket
    /// is declared (the `.uc` compile always passes one), so `actors()` is
    /// `Some` here.
    fn from(buckets: UcBuckets) -> Self {
        Self::new()
            .with_domains(buckets.domains)
            .with_designs(buckets.designs)
            .with_actors(buckets.actors)
            .with_events(buckets.events)
    }
}

impl From<CmmnBuckets> for CompileInputs {
    /// The case gatherer's five buckets as one bundle ([`UcBuckets`]'s
    /// conversion plus the decision bucket).
    fn from(buckets: CmmnBuckets) -> Self {
        Self::new()
            .with_domains(buckets.domains)
            .with_designs(buckets.designs)
            .with_actors(buckets.actors)
            .with_events(buckets.events)
            .with_decisions(buckets.decisions)
    }
}

impl From<DeployPairs> for CompileInputs {
    /// The deployment gatherer's design and flow pairs as one bundle.
    fn from(pairs: DeployPairs) -> Self {
        Self::new()
            .with_designs(pairs.designs)
            .with_flows(pairs.flows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `.ddd` provenance rule: an unset actor bucket is `None`, a
    /// declared-but-empty one is `Some(&[])` — the host's two claims, never
    /// collapsed.
    #[test]
    fn an_unset_actor_bucket_is_distinct_from_a_known_empty_one() {
        assert_eq!(CompileInputs::new().actors(), None);
        assert_eq!(CompileInputs::new().with_actors([]).actors(), Some(&[][..]));
    }

    /// A bucket a surface does not consume stays empty rather than
    /// positional: the accessors are the only way in.
    #[test]
    fn unset_buckets_are_empty_slices() {
        let inputs = CompileInputs::from_domains(&[("a.mox".to_string(), "package a".to_string())]);
        assert_eq!(inputs.domains().len(), 1);
        assert!(inputs.designs().is_empty());
        assert!(inputs.events().is_empty());
        assert!(inputs.decisions().is_empty());
        assert!(inputs.flows().is_empty());
        assert!(inputs.tables().is_empty());
        assert_eq!(inputs.imports(), &DomainImports::default());
    }
}
