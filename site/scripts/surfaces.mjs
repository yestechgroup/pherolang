/**
 * The ten model surfaces, in the one place the site reads them.
 *
 * Words here are authored; the *evidence* behind them is derived by
 * `scripts/sync-model.mjs` and cross-checked by `scripts/gate.mjs`:
 *
 *   * `backends` must name crates that exist under `crates/` — a
 *     `implemented` status with no crate fails the gate;
 *   * `example`, `docs` and `conformance` must exist on disk;
 *   * `golden` (where set) must exist under `examples/artifacts/`.
 *
 * The status shown on the site is therefore `implemented` (a backend consumes
 * the artifact today) or `validated` (it compiles and checks, with no in-tree
 * backend) — never a marketing adjective.
 *
 * Source of truth for the backend mapping: AGENTS.md's surface table and
 * docs/BACKENDS.md.
 */
export const SURFACES = [
  {
    id: 'mox',
    extension: '.mox',
    name: 'Domain model',
    tagline: 'Structure, relationships and behaviour',
    docs: 'docs/LANGUAGE.md',
    example: 'examples/support/support.mox',
    conformance: 'tests/conformance/models',
    golden: null,
    backends: ['phero-backend-rust', 'phero-backend-jsonschema', 'phero-backend-cedar'],
    summary:
      'Packages, classes, features, operations, enums, datatypes and vocabularies — the structural domain every other surface reads.',
  },
  {
    id: 'ddd',
    extension: '.ddd',
    name: 'Application design',
    tagline: 'Services, repositories and aggregate roots',
    docs: 'docs/DDD.md',
    example: 'examples/support/support.ddd',
    conformance: 'tests/conformance/ddd',
    golden: 'examples/artifacts/support.ddd.json',
    backends: [],
    summary:
      'Sculptor-style designs: applications, modules, services, repositories, searches, entity state machines, and executable operation bodies.',
  },
  {
    id: 'actor',
    extension: '.actor',
    name: 'Authorisation',
    tagline: 'Actors, capabilities and obligations',
    docs: 'docs/LANGUAGE.md',
    example: 'examples/support/support.actor',
    conformance: 'tests/conformance/models',
    golden: 'examples/artifacts/support.actor.json',
    backends: ['phero-backend-cedar'],
    summary:
      'Who may do what: actors, delegation, capability grants with typed `when` conditions, obligations, and Cedar policy generation.',
  },
  {
    id: 'dmn',
    extension: '.dmn',
    name: 'Decision model',
    tagline: 'Decision requirements and decision services',
    docs: 'docs/DMN.md',
    example: 'examples/support/support.dmn',
    conformance: 'tests/conformance/dmn',
    golden: 'examples/artifacts/support.dmn.json',
    backends: ['phero-backend-dmn'],
    summary:
      'The decision-requirements graph, business knowledge models, knowledge sources, decision services, and external markdown decision tables.',
  },
  {
    id: 'evt',
    extension: '.evt',
    name: 'Event contract',
    tagline: 'Versioned events, channels and subscriptions',
    docs: 'docs/EVENTS.md',
    example: 'examples/support/support.evt',
    conformance: 'tests/conformance/events',
    golden: 'examples/artifacts/support.evt.json',
    backends: [],
    summary:
      'What the system publishes and who reacts: versioned event contracts, channels, and subscriptions with their binding rules.',
  },
  {
    id: 'uc',
    extension: '.uc',
    name: 'Use case pattern',
    tagline: 'Subjects, actors and successions',
    docs: 'docs/USECASES.md',
    example: 'examples/support/support.uc',
    conformance: 'tests/conformance/usecases',
    golden: 'examples/artifacts/support.uc.json',
    backends: [],
    summary:
      'SysML-v2-style use case patterns: subjects, actors, messages, successions and `realizes` links.',
  },
  {
    id: 'cmmn',
    extension: '.cmmn',
    name: 'Case management',
    tagline: 'Cases, plans and sentry criteria',
    docs: 'docs/CMMN.md',
    example: 'examples/support/support.cmmn',
    conformance: 'tests/conformance/cmmn',
    golden: 'examples/artifacts/support.cmmn.json',
    backends: ['phero-backend-cmmn'],
    summary:
      'CMMN 1.1 case models: case files, discretionary and required plan items, sentries, and decision tasks that invoke sibling decision services.',
  },
  {
    id: 'ifml',
    extension: '.ifml',
    name: 'Interaction flow',
    tagline: 'Views, navigation and CLI commands',
    docs: 'docs/IFML.md',
    example: 'examples/support/support.ifml',
    conformance: 'tests/conformance/ifml',
    golden: 'examples/artifacts/support.manifest.json',
    backends: [],
    summary:
      'Interaction flows over a composed pattern library: views, containers, components, events, navigation, and the declaration-backed control/route manifest.',
  },
  {
    id: 'deploy',
    extension: '.deploy',
    name: 'Deployment',
    tagline: 'Components, profiles and targets',
    docs: 'docs/DEPLOY.md',
    example: 'examples/support/support.deploy',
    conformance: 'tests/conformance/deploy',
    golden: 'examples/artifacts/support.deploy.json',
    backends: [],
    summary:
      'Deployment projections: application components, target capabilities and engines, profiles, settings, and type-checked placement policies.',
  },
  {
    id: 'intent',
    extension: '.intent',
    name: 'Intent specification',
    tagline: 'Requirements, traceability and coverage',
    docs: 'docs/INTENTS.md',
    example: 'examples/support/support.intent',
    conformance: 'tests/conformance/intents',
    golden: 'examples/artifacts/support.intent.json',
    backends: [],
    summary:
      'Requirements as model data: stable ids (`LOAN-001`), links to the elements that express them and the checks that verify them, and a coverage pass that warns on every claim nobody can honor.',
  },
];

export const SURFACE_IDS = SURFACES.map((surface) => surface.id);
