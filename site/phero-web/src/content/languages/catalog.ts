/**
 * The authored half of the language catalogue.
 *
 * `@data/surfaces.json` owns every derived fact: the extension, the human
 * name, the tagline, the summary, the example path, the artifact path, the
 * status and the backend crate names. It is generated from the toolchain and
 * gated by the Site workflow’s freshness diff, so none of it may be retyped
 * here.
 *
 * This module owns only the editorial layer per surface — the two paragraphs
 * on why the surface exists, its concept vocabulary, the neighbours it is
 * checked against, the exact line range to quote from its example, and how
 * much of its compiled artifact the viewer prints. Every string here is
 * traceable to `docs/<surface>.md`; nothing is motivation the documents do not
 * state.
 *
 * `validate()` runs at import time, so a surface with no catalogue entry (or a
 * catalogue entry with no surface) fails the build instead of rendering a page
 * with a hole in it.
 */

import docsManifest from '@data/docs-manifest.json';
import surfacesJson from '@data/surfaces.json';
import excerptManifestJson from '../../../../packages/highlighting/excerpts.json';

/** The two status words `site/data/status.json` defines. */
export type SurfaceStatus = 'implemented' | 'validated';

/** One row of `@data/surfaces.json`, narrowed to the fields the pages read. */
export interface Surface {
  id: string;
  extension: string;
  name: string;
  tagline: string;
  summary: string;
  /** A repository path (e.g. `docs/LANGUAGE.md`), never a link target. */
  docs: string;
  /** Repository-relative source path, e.g. `examples/support/support.mox`. */
  example: string;
  status: SurfaceStatus;
  backends: string[];
  /** Data-relative artifact path, e.g. `artifacts/mox.json`. */
  artifact: string;
  counts: { conformanceFiles: number };
}

export interface Concept {
  /** Backticked in the definition list. */
  term: string;
  definition: string;
}

export interface LanguageEntry {
  id: string;
  /** Exactly two short paragraphs. Backticks render as inline code. */
  problem: [string, string];
  concepts: Concept[];
  /** One accurate statement about the neighbouring surfaces. */
  relations: string;
  /** Surface ids named in `relations` (rendered as links from surfaces.json). */
  related: string[];
  /**
   * The exact lines of `surface.example` the page quotes, 1-based and
   * inclusive. The range comes from `site/packages/highlighting/excerpts.json`
   * (the one manifest `sync-highlight.mjs` hashes); the note is authored here.
   * Verified at build time against the file: the range must be inside it, end
   * on a closing brace, and balance every brace, bracket and parenthesis.
   */
  excerpt: { start: number; end: number; note: string };
  /**
   * How many lines of `site/data/artifacts/<id>.json` the viewer prints. The
   * build asserts the last printed line closes a JSON object or array, so the
   * slice always stops at a structural boundary rather than mid-value.
   */
  artifactLines: number;
  /** One sentence on what the artifact holds, checked against the file. */
  artifactSummary: string;
}

/** Where the generated docs live. The paths come from docs-manifest.json. */
export const DOCS_ORIGIN = 'https://docs.pherolang.com';

/**
 * One quoted range in `site/packages/highlighting/excerpts.json`.
 *
 * That manifest is the single source of every range the site quotes: the
 * language pages, both marketing walkthroughs and `scripts/sync-highlight.mjs`
 * read it, so the bytes a page renders are exactly the bytes the artifact was
 * hashed over. It is JSON, so the import is narrowed here once.
 */
interface ExcerptSpec {
  /** The Phero surface the bytes are highlighted as. */
  surface: string;
  /** Repository-relative source path. */
  source: string;
  /** 1-based, inclusive. */
  start: number;
  /** 1-based, inclusive. */
  end: number;
}

interface ExcerptManifest {
  language: Record<string, ExcerptSpec>;
  pages: Record<string, ExcerptSpec>;
}

const excerptManifest = excerptManifestJson as unknown as ExcerptManifest;

/**
 * The manifest's range for one surface, with this catalogue's note.
 *
 * Throws at import time when the row is absent, so a surface cannot render a
 * page whose example range nothing highlights.
 */
function languageExcerpt(id: string, note: string): LanguageEntry['excerpt'] {
  const spec = excerptManifest.language[id];
  if (!spec) {
    throw new Error(
      `languages: site/packages/highlighting/excerpts.json has no language excerpt '${id}'`,
    );
  }
  return { start: spec.start, end: spec.end, note };
}

const STATUSES: readonly string[] = ['implemented', 'validated'];

function narrow(raw: (typeof surfacesJson)['surfaces'][number]): Surface {
  if (!STATUSES.includes(raw.status)) {
    throw new Error(
      `languages: unknown status '${raw.status}' for surface '${raw.id}' — ` +
        `site/data/surfaces.json defines only ${STATUSES.join(' and ')}`,
    );
  }
  return { ...raw, status: raw.status as SurfaceStatus };
}

/** Every surface, in the order `surfaces.json` lists them. */
export const surfaces: Surface[] = surfacesJson.surfaces.map(narrow);

/** One surface by id; throws rather than returning undefined. */
export function surfaceById(id: string): Surface {
  const found = surfaces.find((surface) => surface.id === id);
  if (!found) throw new Error(`languages: no surface '${id}' in site/data/surfaces.json`);
  return found;
}

/** The previous and next surface in catalogue order, or null at the ends. */
export function neighbours(id: string): { prev: Surface | null; next: Surface | null } {
  const index = surfaces.findIndex((surface) => surface.id === id);
  if (index === -1) throw new Error(`languages: no surface '${id}' in site/data/surfaces.json`);
  return {
    prev: index > 0 ? (surfaces[index - 1] ?? null) : null,
    next: index < surfaces.length - 1 ? (surfaces[index + 1] ?? null) : null,
  };
}

/**
 * The rendered documentation URL for a surface.
 *
 * `surfaces.json` stores the repository path (`docs/LANGUAGE.md`); the public
 * URL is the generated docs manifest’s, so a docs move changes the manifest and
 * this page follows without an edit. Linking the repository path would send a
 * reader to raw markdown instead of the rendered reference.
 */
export function docsUrl(surface: Surface): string {
  const entry = docsManifest.generated.find((candidate) => candidate.source === surface.docs);
  if (!entry) {
    throw new Error(
      `languages: '${surface.docs}' is not in site/data/docs-manifest.json — ` +
        `the docs site does not publish this surface’s reference`,
    );
  }
  return `${DOCS_ORIGIN}${entry.url}`;
}

export interface Run {
  text: string;
  code: boolean;
}

/**
 * The authored strings use backticks for code, the way the docs do. Split them
 * into text and code runs instead of printing the markup or dropping the
 * distinction.
 */
export function inlineCode(text: string): Run[] {
  return text
    .split('`')
    .map((part, index) => ({ text: part, code: index % 2 === 1 }))
    .filter((run) => run.text.length > 0);
}

/**
 * The catalogue. One entry per surface, in `surfaces.json` order.
 *
 * The `excerpt` ranges come from `site/packages/highlighting/excerpts.json`
 * (the manifest the highlighter hashes), are quoted verbatim by the page, and
 * are checked at build time; the `artifactLines` values stop at a closing
 * delimiter in the committed artifact. Both are re-verified on every build, so
 * a source edit that shifts a block fails the build instead of silently
 * quoting the wrong lines.
 */
const entries: LanguageEntry[] = [
  {
    id: 'mox',
    problem: [
      'A `.mox` file holds one package and its declarations: classes and their features, enums, datatypes, interfaces, vocabularies, and package-level annotations. It is the only surface that declares structure — every other surface reads this one.',
      'The compiler resolves and validates it into one Core IR `Model`, and backends generate from that artifact rather than from the syntax tree. Relationships are checked where they are written: `refers` pairs with `refers` and `contains` with `container`, opposites are mutual and validated, and an `expr` body is typed against the features it names.',
    ],
    concepts: [
      { term: 'package', definition: 'One per file; the namespace every declaration joins.' },
      {
        term: 'class',
        definition: 'Attributes and relationships, optionally `extends`-ing another class.',
      },
      {
        term: 'refers / contains',
        definition:
          'A cross reference versus by-value ownership. The `container` back-pointer is the owner slot, and opposites are mutual and validated.',
      },
      {
        term: 'derived',
        definition: 'A computed feature: it carries an `expr` body and no stored value.',
      },
      {
        term: 'op',
        definition:
          'An operation. An `expr` body is typed and lowered by every backend; a `rust` or other target body is embedded verbatim by the backend that owns that target.',
      },
      {
        term: 'enum',
        definition:
          'Integer values with optional labels (`Open as "open" = 0`). Backends emit real enums; canonical JSON uses the literal name.',
      },
      {
        term: 'datatype',
        definition:
          'An opaque wrapper around a platform type, with per-target bindings and at most one `format` hint.',
      },
      {
        term: 'vocabulary',
        definition:
          'An external code set: the declaration names the source, version and facets, and the snapshot is vendored and pinned by sha256 in `model.lock`.',
      },
      {
        term: 'inline actors',
        definition:
          'A policy block declared next to the domain. It lands on the package as an `ActorsDef` and validates exactly like a standalone `.actor` file.',
      },
    ],
    relations:
      'Everything else in the stack reads this surface. `.ddd` designs reference its classes, `.actor` capabilities bind to them and type-check their `when` conditions against them, and the payload fields, inputs, participants, case-file items and `data:` bindings of `.evt`, `.dmn`, `.uc`, `.cmmn` and `.ifml` all resolve against the same union namespace.',
    related: ['ddd', 'actor', 'evt', 'dmn', 'uc', 'cmmn', 'ifml'],
    excerpt: languageExcerpt(
      'mox',
      'One class: attributes, an enum-typed feature, a relation with its opposite, a derived feature, and two operations with checked expression bodies.',
    ),
    artifactLines: 54,
    artifactSummary:
      'One `Model` for the whole package: enums, datatypes, interfaces, classes and their features, with a stable feature `id` per class.',
  },

  {
    id: 'ddd',
    problem: [
      'The `.mox` domain says what the data is. The `.ddd` design says what the application makes of it: the application, its modules, the services and their use-case operations, and per-class design decisions — stereotype, flags, repository, state machine, search projection, delegation.',
      'A design never declares classes or features; it references the classes the imported domains declare. Design references and operation signatures type-check against those domains, so a misspelt class or a mismatched parameter is a compile error rather than a mismatch that surfaces in generated code. Resolution is validation-only: the artifact keeps every name exactly as authored.',
    ],
    concepts: [
      {
        term: 'application',
        definition: 'One per file, after the imports; `base` names the domain package when present.',
      },
      {
        term: 'module',
        definition: 'A label grouping services and designs inside the application.',
      },
      {
        term: 'service',
        definition:
          'Application services with use-case operations; `inject` declares the dependencies, and an operation may carry `capability`, `invokes`, `emits` and `transition` clauses.',
      },
      {
        term: 'design',
        definition:
          '`entity`, `value`, `dto` and `abstract`, with flags such as `scaffold`, `cache` and `auditable`.',
      },
      {
        term: 'repository',
        definition:
          'A per-entity repository block: built-in operations are named, and a declared one may be `protected`.',
      },
      {
        term: 'search',
        definition:
          'An intent-only search projection — text fields with boosts, filters, sorts, ranking, pagination and document projections. The backend chooses the engine.',
      },
      {
        term: 'machine',
        definition:
          "An entity state machine, closed over its enum: every state is a literal of the machine’s enum type.",
      },
      {
        term: 'operation body',
        definition:
          'A declared service or repository operation may carry an executable statement body (`let`, `guard`, `return`) that the driver checks at compile time.',
      },
    ],
    relations:
      'The design is the hinge between the domain and everything built on it: it reads `.mox`, names `.actor` capabilities from its service operations, links an operation to a `.dmn` decision service (`invokes`, or `inject decision` on the module), and its operation emissions are checked against `.evt` names. `.uc` `realizes` its service operations, `.cmmn` executes them, and `.deploy` binds a component to one of its modules by path.',
    related: ['mox', 'actor', 'dmn', 'evt', 'uc', 'cmmn', 'deploy'],
    excerpt: languageExcerpt(
      'ddd',
      'One service: injected dependencies, operations that name capabilities and emissions, an operation with a checked statement body, and two delegations — `close` to the repository, `findTickets` to the search projection.',
    ),
    artifactLines: 54,
    artifactSummary:
      'One `DddModel`: the application, and per module the services, designs, repositories, searches and machines.',
  },

  {
    id: 'actor',
    problem: [
      'Policy can live in a standalone `.actor` file beside the domain it governs: the file imports one or more `.mox` domains and declares actors, capabilities, grants, obligations and separation-of-duty constraints.',
      'The compiled policy set is the union of the file’s blocks followed by every imported domain’s inline `actors` blocks, so capabilities and `when` conditions type-check against the real classes and the separation-of-duty checks span files. A construct the decidable subset cannot express is a generate-time error naming the capability — never a quietly weakened rule.',
    ],
    concepts: [
      {
        term: 'actor / agent',
        definition:
          'A principal. `agent` declares an autonomous LLM agent, `actor` a human; a declared kind is inherited down an `extends` chain.',
      },
      {
        term: 'extends',
        definition:
          'Confers authority: an actor’s effective permits are its own grants plus its ancestors’.',
      },
      {
        term: 'capability … on …',
        definition:
          'A named action bound to a domain class. That class is what the capability’s `when` conditions are typed against.',
      },
      {
        term: 'grant',
        definition:
          'A block of `permit` and `forbid` entries, optionally guarded by `when (…)`, a session-kind modifier, and obligations.',
      },
      {
        term: 'when',
        definition:
          'A typed boolean guard over the capability’s class. Only the decidable subset survives Cedar generation.',
      },
      {
        term: 'obligation',
        definition: 'A declared duty, referenced by name from a grant entry.',
      },
      {
        term: 'never_both',
        definition: 'A separation-of-duty pair: no principal may hold both capabilities.',
      },
      {
        term: 'delegation',
        definition: 'A grant made `to` another principal — one hop, checked structurally.',
      },
      {
        term: 'cedar { … }',
        definition:
          'An escape hatch for a rule the DSL cannot express, carried verbatim into the generated policy.',
      },
    ],
    relations:
      'A `.actor` file is a policy dimension over imported domains, not a second model of them. `.ddd` service operations name these capabilities by name only, `.uc` checks `requires:` against the permits held by the participating actors (and refuses a `never_both` pair), `.cmmn` roles bind to actor names, and `.ifml` resolves `requires:` and `roles:` against the capability and actor tables union-wide.',
    related: ['mox', 'ddd', 'uc', 'cmmn', 'ifml'],
    excerpt: languageExcerpt(
      'actor',
      'Five capabilities on `Ticket`, two declared obligations, and the `Customer` and `Agent` grants: an unconditional permit, a guarded one, and one that attaches both obligations.',
    ),
    artifactLines: 21,
    artifactSummary:
      'One `ActorModel`: the block with its actors, capabilities, grants, obligations and `neverBoth` pairs — the artifact the Cedar backend consumes.',
  },

  {
    id: 'dmn',
    problem: [
      'A `.dmn` file declares one decision model whose central structure is the decision-requirements graph: input data, decisions, business knowledge models, knowledge sources, decision services, and the three requirement kinds — information, knowledge and authority — that wire the graph.',
      'Logic is attached to a node, never the node itself: a decision with no `logic` clause at all is a valid, first-class declaration, which is the architecture-first reading of DMN. Inputs, outputs and parameters resolve against the imported `.mox` domains, and a decision’s `emits` block names the `.evt` contracts its outcome publishes.',
    ],
    concepts: [
      { term: 'decision_model', definition: 'One per file, with a `namespace`.' },
      {
        term: 'inputs',
        definition: 'Typed input data; each entry resolves against the imported domain union.',
      },
      {
        term: 'decision',
        definition:
          'A graph node with `output` or `outputs`, and optional logic. The logic is an implementation detail of the node.',
      },
      {
        term: 'knowledge model',
        definition:
          'A reusable function with declared parameters and an output type (`model assess_refund { … }`).',
      },
      {
        term: 'knowledge source',
        definition: 'The authority behind decisions: a name and a reference.',
      },
      {
        term: 'requirements',
        definition: 'The three edge groups: `information`, `knowledge` and `authority`.',
      },
      {
        term: 'decision service',
        definition:
          'The invocation contract: `inputs`, `outputs` and the set of nodes it `encapsulates`.',
      },
      {
        term: 'logic kinds',
        definition:
          '`table "<path>"` for an external markdown decision table, `expression "<FEEL>"`, and `invoke <model> { … }`.',
      },
      {
        term: 'emits',
        definition: 'Maps a decision outcome to declared events, checked against the `.evt` contracts.',
      },
    ],
    relations:
      'A decision model consumes domain types and declared events — never a design or policy artifact: `.mox` supplies the types, `.evt` the contracts it may emit to. The link is one-way. `.ddd` service operations point at a decision service (`invokes`, or `inject decision` on the module, optionally version-pinned), and `.cmmn` decision tasks invoke a service such as `RefundDecisions.AssessRefund` as case work. A decision model imports neither.',
    related: ['mox', 'evt', 'ddd', 'cmmn'],
    excerpt: languageExcerpt(
      'dmn',
      'Two decisions: one whose logic is a FEEL expression, one whose logic is an external markdown decision table. The third, `RefundAssessment`, composes them with an `invoke` and emits `RefundApproved`.',
    ),
    artifactLines: 40,
    artifactSummary:
      'One `DmnModel`: inputs, knowledge sources, business knowledge models, decisions, the three requirement groups, services and views — the shape the DMN backend lowers into its engine model.',
  },

  {
    id: 'evt',
    problem: [
      'An `.evt` file declares the contracts: events with typed payload fields and a free-form `version` string, the channels that publish them, and the subscriptions that deliver event sets to named consumers.',
      'It is deliberately transport-independent — nothing about brokers, wire encodings or delivery is written down, and webhook targets stay runtime data. Payload field types resolve against the imported `.mox` domains, so a misspelt type is a compile error; a subscription `when` filter is typed boolean against every event in the subscription’s list, because a filter is a per-event routing decision.',
    ],
    concepts: [
      {
        term: 'event … version "…"',
        definition:
          'A named contract with typed payload fields. The version string is pinned verbatim and carries no ordering semantics — consumers decide what it means.',
      },
      {
        term: 'governance lines',
        definition:
          '`owner`, `classification`, `retention` and `pii [ … ]` inside an event body: verbatim labels the driver never interprets.',
      },
      {
        term: 'cloudEvents { … }',
        definition:
          'Declared `type`, `source` and optional `subject` attributes. Once the block is present they are authoritative for a downstream generator.',
      },
      {
        term: 'channel',
        definition:
          'A named channel and its `publishes` membership, optionally `of <scope>`: the topic-template dimension its topics are parameterized by.',
      },
      {
        term: 'subscription',
        definition:
          'The events, the one consumer, an optional `scope`, an optional `when` filter, and an optional `delivery` policy.',
      },
      {
        term: 'when (…)',
        definition:
          'A raw phero-expr filter, typed boolean against every subscribed event. A field absent from any of them is an error by design.',
      },
      {
        term: 'delivery { … }',
        definition:
          '`mode` (`at_most_once`, `at_least_once`, `effectively_once`) plus attempts, backoff, timeout and a dead-letter channel declared in the same file.',
      },
      {
        term: 'payload types',
        definition: 'Any declared domain kind: class, enum, datatype, interface or vocabulary.',
      },
    ],
    relations:
      'Events are contracts over the domain, not a model of it: payloads resolve against `.mox`, and `.dmn` decisions publish to them with `emits` — the edge is one-way, so an `.evt` file never references a decision model. `.uc` uses an event only as a message payload type; its occurrences are scenario-local lifetime points, never events. In `.cmmn`, a case reacts to case-file transitions, so the event contracts stay the wider integration vocabulary.',
    related: ['mox', 'dmn', 'uc', 'cmmn'],
    excerpt: languageExcerpt(
      'evt',
      'Two versioned events, the two channels that publish them, and one of the two subscriptions — the refund desk consumer. The same file declares `TicketRaised`, `TicketResolved` and an analytics subscription.',
    ),
    artifactLines: 40,
    artifactSummary:
      'One `EventModel`: the events with their payload fields and governance lines, the channels, and the subscriptions with their consumers.',
  },

  {
    id: 'uc',
    problem: [
      'A `.uc` file declares use cases in the SysML v2 pattern: a subject — the classifier whose behaviour is specified — the actors participating, the messages exchanged between anchored points of their behaviour, the successions that order it, and the domain service operations the use case realizes.',
      'One abstraction note matters: an `occurrence` is a scenario-local lifetime point of one participant, not an `.evt` event. Imported event contracts enter a use case only as message payload types. The surface imports four buckets — raw `.mox` domains plus pre-compiled `.ddd`, `.actor` and `.evt` artifacts — and cross-checks the scenario against each of them.',
    ],
    concepts: [
      { term: 'usecase', definition: 'The declaration; its body is one scenario.' },
      {
        term: 'subject',
        definition: 'The classifier under specification, typed by a domain class.',
      },
      {
        term: 'actor',
        definition:
          'A participant: a bare policy actor, or a domain-typed one. Only bare policy actors contribute capability coverage.',
      },
      {
        term: 'occurrence',
        definition:
          'A named lifetime point of a participant. Scenario-local — it is never an event.',
      },
      {
        term: 'message',
        definition:
          'An exchange `of <payload>` between two anchored points: an occurrence, or the end of a performed step.',
      },
      {
        term: 'performed step',
        definition:
          '`perform <name>: <operation> requires [ <capability> ]` — a subject-performed step anchored to a design operation.',
      },
      {
        term: 'first … then …',
        definition: 'A succession ordering two points; `start` and `done` are the bounds.',
      },
      {
        term: 'realizes',
        definition:
          'Names the design service operation the use case realizes, cross-checked against the imported `.ddd` artifact.',
      },
      {
        term: 'requires',
        definition:
          'Capability coverage, checked structurally against the permits the participating actors hold.',
      },
    ],
    relations:
      'Four imports, four checks: `.mox` types the participants and payloads, `.ddd` supplies the service operations `realizes` names, `.actor` supplies the actor principals and the permits `requires:` is checked against, and `.evt` supplies message payload types only. A scenario that needs both halves of a declared `never_both` pair is refused.',
    related: ['mox', 'ddd', 'actor', 'evt'],
    excerpt: languageExcerpt(
      'uc',
      'The `RaiseRefund` scenario: a subject step bound to a design operation with a required capability, actor occurrences, two messages (one carrying an event payload), a succession chain from `start` to `done`, and the `realizes` link.',
    ),
    artifactLines: 16,
    artifactSummary:
      'One `UseCaseModel`: each use case with its subject, participants, occurrences, messages, successions and realized operations.',
  },

  {
    id: 'cmmn',
    problem: [
      'A `.cmmn` file declares a case: the adaptive, event-and-condition-driven work coordinated around a domain — the CMMN 1.1 notion of case management, not a workflow and not a state machine. The plan declares the work that is available (tasks, service tasks, milestones, grouped into stages) and the criteria under which each item activates, without prescribing a sequence.',
      'The division of responsibility is explicit: the domain owns the data and its invariants, the domain lifecycle (`.ddd` machines) owns an aggregate’s valid transitions, the case owns the work, and the engine executes. Sibling tasks are concurrent by default; ordering exists only where a criterion imposes it.',
    ],
    concepts: [
      {
        term: 'case',
        definition: 'The declaration. Plan items and case-file items share one namespace per case.',
      },
      {
        term: 'roles',
        definition: '`role <name> as <Actor>`: the `.actor` principals who fill the work.',
      },
      {
        term: 'case_file',
        definition:
          'Typed case-file items (`Ticket[1..1] ticket`), resolved against the imported domain.',
      },
      { term: 'stage', definition: 'A grouping of plan items.' },
      {
        term: 'task',
        definition:
          'A human task. A `discretionary task` is available but not required; a plain `task` is required once activated.',
      },
      {
        term: 'service item',
        definition:
          '`execute <service>.<operation>`: automation bound to an operation the imported design declares.',
      },
      {
        term: 'decision item',
        definition:
          '`invokes <DecisionModel>.<service>` with `inputs` and `outputs`: the sibling `.dmn` decision service as case work.',
      },
      { term: 'milestone', definition: 'A reachable goal state.' },
      {
        term: 'entry / exit criteria',
        definition:
          '`on <item>.<event> when (<expr>)`: the sentries that activate or terminate an item.',
      },
    ],
    relations:
      'A case coordinates work over the other surfaces: `.mox` types the case file, `.actor` names the role principals, `.ddd` supplies the operations a `service` item executes, `.dmn` supplies the decision service a `decision` item invokes, and `.evt` stays the wider integration vocabulary — the case reacts to case-file transitions, not to events. The CMMN backend lowers the artifact to the engine’s `CaseDefinition` JSON.',
    related: ['mox', 'actor', 'ddd', 'dmn', 'evt'],
    excerpt: languageExcerpt(
      'cmmn',
      'The automated half of the assessment stage: a `service` item bound to a designed operation, a `decision` item invoking the sibling decision model’s `AssessRefund` service, and the discretionary finance approval.',
    ),
    artifactLines: 56,
    artifactSummary:
      'One `CmmnModel`: cases with their roles, case file, plan items and sentries — the shape the CMMN backend lowers into a `CaseDefinition`.',
  },

  {
    id: 'ifml',
    problem: [
      'An `.ifml` file describes the interaction model: views and containers, the components inside them, the events those components raise, and where navigation goes — semantically aligned with OMG IFML 1.0, in a C-like, expression-heavy syntax instead of XML.',
      'The domain defines the data; the flow defines what the user sees and does with it. Entity and field references are checked against the imported `.mox` domains and `.actor` policies, the reference pattern library is composed with `use`, and a declaration-backed control and route manifest is derived after checking, so every route target in it exists by construction.',
    ],
    concepts: [
      {
        term: 'view',
        definition: 'A top-level interaction unit, with `params` and a label; containers nest inside it.',
      },
      {
        term: 'component',
        definition:
          '`list`, `table`, `details`, `form` and the chart kinds, with `data:` bound to a domain type.',
      },
      {
        term: 'fields / column',
        definition:
          'A component’s bindings (`fields: [ticketNo, title]`), and computed columns such as `column "Refund" -> expr refundCents / 100`.',
      },
      {
        term: 'on <event> -> …',
        definition:
          'A component event whose handler navigates, or invokes a declared action. An `if` guard may filter it.',
      },
      {
        term: 'navigate(…)',
        definition: 'A target view or container, with parameter bindings.',
      },
      {
        term: 'action',
        definition:
          'A declared action with a domain `target:`. The declaration is what makes an event’s action reference resolve.',
      },
      {
        term: 'use',
        definition:
          'Instantiates a module from the reference pattern library with input bindings.',
      },
      {
        term: 'command',
        definition: 'A CLI command declaration with typed arguments and options: the binary’s command surface.',
      },
      {
        term: 'manifest',
        definition:
          'The control and route manifest, derived after checking — never parsed, and declaration-backed by construction.',
      },
    ],
    relations:
      'Flows are checked against the rest of the stack: `.mox` types the data bindings, `.actor` supplies the capabilities and actors that `requires:` and `roles:` resolve against (an empty policy set skips the authorization pass entirely), and `.dmn` decision services supply data a details component can bind. `.deploy` binds a `frontend` component to a flow by path, and the control and route manifest is derived from the checked model.',
    related: ['mox', 'actor', 'dmn', 'deploy'],
    excerpt: languageExcerpt(
      'ifml',
      'The `Inbox` view: parameters bound to the domain, a `use` of the library’s Inbox module, and a list component whose `select` event navigates to the queue with a row binding.',
    ),
    artifactLines: 30,
    artifactSummary:
      'One `IfmlModel`: views with their containers, components, events and navigation, plus the declared actions, commands and module imports.',
  },

  {
    id: 'deploy',
    problem: [
      'A `.deploy` file is a projection, not a new architecture: it declares a logical `application` of named components and their `connects`, reusable `profiles` (a target plus defaults, policies and resource mappings), and `deployments` that project one application through one profile with `configure` overrides.',
      'Targets are implementation-provided, never authored: a profile names one of the closed targets, and the driver’s capability, engine and setting tables define what each can host. Incompatibility is a compile error, not a generated artifact — validate before generating. The effective configuration is component baselines ⊕ profile defaults ⊕ overrides, later levels winning.',
    ],
    concepts: [
      {
        term: 'application',
        definition:
          'The logical architecture: components and `connects`. Nothing here says where anything runs.',
      },
      {
        term: 'component',
        definition:
          'A named unit with a kind (`api`, `frontend`, `database`), settings, environment and bindings.',
      },
      {
        term: 'design / flow',
        definition:
          'A component bound to a `.ddd` module or an `.ifml` flow by the import path as written plus the entry name.',
      },
      {
        term: 'connects',
        definition: 'A directed relationship between two components (`connects web -> api`).',
      },
      {
        term: 'profile',
        definition:
          '`target`, `defaults`, `mappings`, and `require` / `prohibit` policies. Profiles are reusable across applications.',
      },
      {
        term: 'target',
        definition:
          'One of the closed targets: `standalone`, `dockerCompose`, `kubernetes`, `cloudflareWorkers`.',
      },
      {
        term: 'deployment',
        definition: '`use <profile>` plus `configure` overrides: the concrete projection.',
      },
      {
        term: 'settings',
        definition:
          'Typed values addressed by path (`api.replicas: 3`), checked against the target’s setting table.',
      },
      {
        term: 'policies',
        definition:
          'phero-expr conditions over the effective settings, evaluated against baselines, defaults and overrides.',
      },
    ],
    relations:
      'A deployment model composes the other artifacts: it imports pre-compiled `.ddd` designs and `.ifml` flows and binds components to them by path. The `.mox` domain and `.actor` policy reach it only through the design it projects — the model carries authored intent only, never containers, manifests or clouds.',
    related: ['ddd', 'ifml'],
    excerpt: languageExcerpt(
      'deploy',
      'The logical application: an `api` component bound to the `desk` design module, a `frontend` bound to the flow, a managed database, and the `connects` that wire them.',
    ),
    artifactLines: 56,
    artifactSummary:
      'One `DeployModel`: applications with their components and connections, the profiles, and the deployments that project them.',
  },

  {
    id: 'intent',
    problem: [
      'An `.intent` file states requirements as model data: one `intent` declaration per requirement, with a stable id (`LOAN-001`) that requirements, artifacts and receipts key on, a quoted statement, and links to the model elements that express it and the checks that verify it.',
      'Requirements never leave the model again: the coverage pass resolves every link against the application\u2019s compiled artifacts and warns on every intent nothing expresses and nothing verifies — honesty over reach. A broken link is a spanned error with the nearest candidate named.',
    ],
    concepts: [
      {
        term: 'intent',
        definition: 'A stable id, a statement, and the links that trace it into the model.',
      },
      {
        term: 'element link',
        definition:
          '`element class:Ticket;` — a class, capability, event, view, command or operation that expresses the intent.',
      },
      {
        term: 'check link',
        definition:
          '`check decision:RefundAuthority;` — a designed class\u2019s invariants or a decision that verifies it.',
      },
      {
        term: 'stable id',
        definition: 'An uppercase lead segment plus at least one all-digits segment, sliced exactly as written.',
      },
      {
        term: 'coverage pass',
        definition:
          'The project-level pass behind `pherolang check`: unmapped intents warn (`X0009`/`X0010`), broken links error (`X0011`\u2013`X0014`).',
      },
    ],
    relations:
      'Intents link across the whole stack and are resolved per application: `class` against the imported `.mox` domain, `capability` against `.actor`, `event` against `.evt`, `view`/`command` against `.ifml`, `op` and `invariant` against `.ddd` designs, and `decision` against `.dmn` decision models.',
    related: ['mox', 'actor', 'evt', 'ifml', 'ddd', 'dmn'],
    excerpt: languageExcerpt(
      'intent',
      'The refund-governance requirement: the elements that express it and the decision that verifies it, all resolved against the compiled stack.',
    ),
    artifactLines: 24,
    artifactSummary:
      'One `IntentModel`: the intents in declaration order, each with its statement, its `element` links and its `check` links — names exactly as authored.',
  },

  {
    id: 'rosetta',
    problem: [
      'A `.rosetta` file is a FINOS Rune DSL model, the language the Common Domain Model is written in: namespaces of types, choices, enums, functions, rules and reports, with cardinalities, conditions and annotations.',
      'Phero Lang parses and resolves Rune in tree, and the result is checked against the Java reference implementation, so a Rune model behaves here as it does upstream. A `.mox` domain imports one with `import sigil`, and its namespaces become ordinary packages every other surface can reference.',
    ],
    concepts: [
      {
        term: 'namespace',
        definition: 'The unit a Rune file declares; other files import it by name, not by path.',
      },
      {
        term: 'type',
        definition: 'A data type: attributes with a type and a cardinality such as `(1..1)` or `(0..*)`.',
      },
      {
        term: 'func',
        definition: 'A function: `inputs`, one `output`, conditions, and the `set` operations that compute it.',
      },
      {
        term: 'condition',
        definition: 'A boolean rule over a type or a function\u2019s inputs, written in the Rune expression language.',
      },
      {
        term: 'pool',
        definition:
          'The `.rosetta` files under a file\u2019s directory: a compile resolves the namespaces the file imports from it, exactly as `import sigil` does.',
      },
    ],
    relations:
      'A `.mox` domain imports a Rune model with `import sigil`, and its namespaces lower into packages that `.actor` capabilities, `.ddd` designs and every backend see as ordinary domain classes.',
    related: ['mox'],
    excerpt: languageExcerpt(
      'rosetta',
      'A Rune function: its inputs and output with cardinalities, a condition on the inputs, and the operation that sets the output.',
    ),
    artifactLines: 12,
    artifactSummary:
      'One `sigil-model/1` model: the builtin `com.rosetta.model` files first, then every resolved Rune file with its namespace and elements, and the resolution diagnostics.',
  },
];

function validate(list: LanguageEntry[]): LanguageEntry[] {
  const seen = new Set<string>();

  for (const entry of list) {
    if (seen.has(entry.id)) throw new Error(`languages: duplicate catalogue entry '${entry.id}'`);
    seen.add(entry.id);
    surfaceById(entry.id);

    for (const id of entry.related) {
      if (id === entry.id) {
        throw new Error(`languages: catalogue entry '${entry.id}' lists itself as a neighbour`);
      }
      surfaceById(id);
    }

    const { start, end } = entry.excerpt;
    if (!Number.isInteger(start) || !Number.isInteger(end) || start < 1 || end < start) {
      throw new Error(`languages: catalogue entry '${entry.id}' has an invalid excerpt range`);
    }

    // The range must be the manifest's, not a second copy: the page quotes
    // these bytes and `sync-highlight.mjs` hashes the manifest's, so the two
    // can only agree if there is one row.
    const surface = surfaceById(entry.id);
    const spec = excerptManifest.language[entry.id];
    if (!spec) {
      throw new Error(
        `languages: site/packages/highlighting/excerpts.json has no language excerpt '${entry.id}'`,
      );
    }
    if (spec.surface !== entry.id) {
      throw new Error(
        `languages: excerpt '${entry.id}' is highlighted as surface '${spec.surface}', expected '${entry.id}'`,
      );
    }
    if (spec.source !== surface.example) {
      throw new Error(
        `languages: excerpt '${entry.id}' cites '${spec.source}', but ` +
          `site/data/surfaces.json names '${surface.example}' — one of them is stale`,
      );
    }
    if (
      !Number.isInteger(spec.start) ||
      !Number.isInteger(spec.end) ||
      spec.start < 1 ||
      spec.end < spec.start
    ) {
      throw new Error(
        `languages: excerpt '${entry.id}' has an invalid range ${spec.start}–${spec.end} ` +
          'in site/packages/highlighting/excerpts.json',
      );
    }
    if (start !== spec.start || end !== spec.end) {
      throw new Error(
        `languages: catalogue entry '${entry.id}' quotes lines ${start}–${end}, but ` +
          `site/packages/highlighting/excerpts.json says ${spec.start}–${spec.end}`,
      );
    }

    if (!Number.isInteger(entry.artifactLines) || entry.artifactLines < 1) {
      throw new Error(`languages: catalogue entry '${entry.id}' has an invalid artifact line count`);
    }
  }

  for (const surface of surfaces) {
    if (!seen.has(surface.id)) {
      throw new Error(
        `languages: site/data/surfaces.json lists '${surface.id}' but the catalogue has no entry for it`,
      );
    }
    // Resolve the docs link here, so a missing manifest row fails at import
    // rather than when a page happens to be rendered.
    docsUrl(surface);
  }

  return list;
}

export const languages: LanguageEntry[] = validate(entries);

/** One catalogue entry by surface id; throws rather than returning undefined. */
export function entryById(id: string): LanguageEntry {
  const found = languages.find((entry) => entry.id === id);
  if (!found) throw new Error(`languages: no catalogue entry '${id}'`);
  return found;
}
