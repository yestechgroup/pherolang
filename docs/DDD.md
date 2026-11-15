# DDD design DSL (.ddd)

## Overview

A Sculptor-style domain-driven-design layer over `.mox` domain models. The
domain model defines the data (entities, features, constraints); the `.ddd`
design defines the application architecture on top of it: the application,
its modules, the application services with their use-case operations, and
per-class design decisions (stereotype, flags, repository, state machine,
delegation targets). Parsed by **phero-syntax** (`parse_ddd`, the chumsky parser) at
compile time, lowered and validated by **phero-driver**
(`compile_ddd_str`) against the imported domains, and emitted as the
versioned `phero_ir::ddd::DddModel` artifact (the phero-ir crate).

The `.ddd` DSL lives alongside the `.mox` domain model as a *complementary*
input: a design never declares classes or features — it references the
classes the imported domains declare and records what the application makes
of them. The consumer is **codegraph**, external by decision D1 of epic
[#297](https://github.com/yestechgroup/pherolang/issues/297): Phero Lang
owns the language and its semantics, and codegraph transpiles the artifact
into the executable system. There is no in-tree **code-generation** backend
by decision rather than by omission; `phero-backend-ddd` is the in-tree
artifact backend, and its job is the artifact-only contract check.

The normative contract — entry points, resolution semantics, the validation
rules, the aggregate-derivation rule, and the capabilities
contract — is documented on `phero_driver::compile_ddd_str`; this page is the
language reference.

---

## Artifact wire format

`DddModel` is a standalone, versioned wire artifact following the phero-ir
[wire format contract](BACKENDS.md) (see also the `phero_ir::ddd` module
docs):

- Versioned independently of the domain-model marker via
  **`DDD_MODEL_FORMAT_VERSION`** (= 2), the `ActorModel` precedent.
  `DddModel::from_json` rejects absent/wrong versions with
  `IrError::UnsupportedFormatVersion` (an absent version reports as 0).
- **Provenance, not a gate.** `DddModel` carries `pheroVersion`, the
  phero-ir version that wrote it (stamped by `DddModel::new`, the Core IR
  `Model` precedent). Readers gate on `formatVersion` alone: an artifact
  written before the field existed deserializes unchanged and re-serializes
  without inventing a stamp. The artifact-only contract check is stricter
  than the reader — `pherolang artifact check` requires the provenance line
  in a contract-complete artifact. See [VERSIONING.md](VERSIONING.md).
- All struct field names serialize as `camelCase` (`formatVersion`,
  `returnType`, `optimisticLocking`, ...). Unit-only enums serialize as bare
  strings: stereotypes lowercase (`"entity" | "value" | "dto"`), built-in
  repository operations camelCase (`"findById"`, `"findAll"`,
  `"findByExample"`, `"findByKeys"`, `"save"`, `"delete"`).
- **Signature types** (DD3, issue
  [#300](https://github.com/yestechgroup/pherolang/issues/300)): an
  operation signature slot carries `DddTypeRef`, **untagged** on the wire.
  A domain type is exactly the domain IR's `TypeRef` object (adjacent
  tagging: `{"type": "class", "value": {...}}`) — the pre-DD3 bytes for
  every domain-only signature — while a DTO boundary is
  `{"dto": "BookSummary"}`. Parameters are `DddParam`
  (`{"name": ..., "type": ..., "multiplicity"?}`), the DDD twin of the
  domain's `OperationParam`; the domain IR's own types stay unchanged. The
  artifact gate stays `formatVersion` 2.
- **DTO boundaries** (DD3, issue
  [#300](https://github.com/yestechgroup/pherolang/issues/300)): a `dto`
  design carries `dtoShape` — `name` (the boundary name a `DddTypeRef`
  DTO reference spells), `source` (the resolved source class `TypeRef`,
  omitted for a standalone DTO), `direction` (`"input" | "output"`), and
  the ordered `members`. A member is
  `{name, kind, type, multiplicity?, path?, expression?}`: `kind` is
  `"select" | "derive" | "declared"`, `path` is the source feature path on
  a select, `expression` the verbatim derivation source on a derive (the
  `condition` rule), and neither on a declared member; `type` is a
  `DddTypeRef`. `Design.class`/`typeRef` are **optional**: a projection's
  `class` is its `from` source (its boundary name rides in `dtoShape.name`)
  and a standalone DTO has neither. The field is sparse — entity and value
  designs omit it — so an artifact without DTOs stays byte-identical apart
  from the provenance line, and `formatVersion` stays 2.
- Unknown fields are ignored on deserialize; every field added after v1
  must keep the `#[serde(default)]` + skip discipline. The pherolang#176/#180
  additions ride that rule: an operation's `transition` binding
  (`TransitionRef`) and its resolved guard paths (`conditionPaths`,
  `ConditionPath` rows with adjacently tagged scopes) are absent from every
  artifact that does not declare them — body-less and machine-less models
  stay byte-identical, `DDD_MODEL_FORMAT_VERSION` stays 2. The pherolang#179
  additions follow the same discipline: a design's `childOperations`
  (ordinary `ServiceOperation` rows declared by `extend` blocks) and an
  operation's `childScope` (`ChildRef { feature, class, action }` — the
  root's child feature, the child class's full name, the `Delegation.target`
  convention, and the validated action `"add" | "remove" | "save"`) appear
  only when authored.
- **Resolved identities** (issue
  [#298](https://github.com/yestechgroup/pherolang/issues/298)): every
  reference a consumer acts on carries a resolved companion alongside the
  authored spelling — `Design.typeRef` (absent for a standalone DTO),
  `Delegation.targetRef`,
  `Service.dependencyRefs`/`decisionDependencyRefs`,
  `ServiceOperation.capabilityRefs`/`emittedRefs`/`decisionRefs`,
  `SearchDef.entityRef`, and each search member's `typeRef`. The list
  companions align index-for-index with the authored lists they accompany,
  and every companion is omitted where the authored feature is absent. The
  optional provenance members — `declaredIn` (capabilities), `contract`
  (events and decisions), `version` (decisions) — are absent when the
  counterpart artifact was not supplied at compile time;
  `emittedRefs[].contract` is always absent because `.ddd` has no `.evt`
  bucket. The additions are additive: `formatVersion` stays 2, and a model
  that authors none of them stays byte-identical apart from the provenance
  line. See [Resolution semantics](#resolution-semantics) and the
  [consumer contract](#consumer-contract).
- **Value semantics** (issue
  [#299](https://github.com/yestechgroup/pherolang/issues/299)): a value
  design carries `semantics`, the fixed four-property block
  `{"identity":"none","immutable":true,"deepImmutable":true,"equality":"structural"}`
  — every member always present, `identity` and `equality` closed
  vocabularies (`"none"`, `"structural"`). The field is **sparse**: entity
  and dto designs omit it, so an artifact without value designs stays
  byte-identical apart from the provenance line and `formatVersion` stays 2.
  A consumer reads the block; it never infers the four properties from
  `"stereotype": "value"`. `pherolang artifact check` requires the block on
  every value design and refuses it on an entity or dto.
- **Declared error vocabulary and `throws`** (DD7, issue
  [#304](https://github.com/yestechgroup/pherolang/issues/304)): the model
  root carries the application-scoped `errors`, each module its private
  `errors`, and a declared/delegated/body service operation or a declared
  repository operation carries its resolved `throws` references. An
  `ErrorDecl` is `{name, description?, fields?}` with the payload fields as
  `DddParam` (the signature slot rule); an `ErrorRef` is
  `{name, scope, module?}` where `scope` is the bare lowercase
  `"application" | "module" | "prelude"` and `module` is present exactly
  for the module scope. All three keys are sparse: a vocabulary-less
  artifact and an operation with no `throws` clause stay byte-identical,
  and `formatVersion` stays 2. The prelude (`NotFound`, `Conflict`) is
  **implicit** — never emitted in `errors` and always referencable with
  `"scope":"prelude"`.
- Executable operation bodies (issue [#178](https://github.com/yestechgroup/pherolang/issues/178))
  ride the same additive fields: a service or repository operation's
  `bodies` map holds the checked body under the `"stmt"` target — the
  canonical JSON of `phero_expr::stmt::StmtBody` (an opaque string: phero-ir
  never depends on phero-expr, the `Operation.bodies`/`"rune"` discipline) —
  and a service body written `name(...) => <Repository> { ... }` also
  carries `bodyReceiver` (the primary repository bound as `self`). A
  repository body binds its own design's repository, so it records no
  receiver. Body-less operations omit both keys.
- A repository operation carries **exactly one** of a built-in op
  (`"builtin"`) or a declared signature (`returnType`/`params`).

Minimal artifact: an empty model serializes as exactly
`{"formatVersion":2,"pheroVersion":"0.3.0"}` — the version marker plus the
provenance line, whose value is the crate version that wrote it (regenerated
whenever the workspace version bumps).

```rust
let compilation = phero_driver::compile_ddd_str("library.ddd", source, &domains, &phero_driver::DomainImports::default());
let json = compilation.model.unwrap().to_json_pretty()?; // versioned artifact
let model = phero_ir::ddd::DddModel::from_json(&json)?;    // version-gated reload
```

---

## Philosophy

1. **Sculptor semantic identity** — every Sculptor design concept has a
   direct DSL equivalent (see the concept-mapping table below). No lossy
   mapping.
2. **Validated against the real domain** — design class references and
   operation signatures type-check against the imported `.mox` models; a
   typo'd class is a compile error, not a silent generation mismatch.
3. **Resolution is carried** — the artifact keeps every name exactly as
   authored and, alongside it, the identity the compiler resolved;
   consumers act on the companion and never re-implement name resolution.
4. **Loosely coupled dimensions** — capability references tie the design
   dimension to the actor-policy dimension by name only, never by
   construction.

---

## Grammar

The normative EBNF for the `.ddd` surface lives exactly once, in the parser
module docs under [*`.ddd` design
sources*](https://docs.rs/phero-syntax/latest/phero_syntax/parser/index.html)
(`crates/phero-syntax/src/parser/mod.rs`) — the single authority for `ddd_file`,
`application_decl`, `module_decl`, `service_decl`, `design_decl`,
`repository_decl`, `machine_decl`, `search_decl`, and `extend_decl`; this
page keeps only the language-level rules. The `.ddd` keyword tables in
`crates/phero-syntax/src/ddd.rs` are the keyword authority the parser and the
formatter share.

Lexical and structural rules:

- Every `.ddd` word (`application`, `base`, `module`, `service`, `extend`,
  `inject`, `entity`, `value`, `dto`, `abstract`, the flags, `repository`,
  and the built-in operation names) is a **contextual identifier**, not a
  keyword: it is special only in the grammar position it is parsed in,
  remains usable as a class or operation name elsewhere, and can be escaped
  with `^` (the `id`/`readonly` precedent).
- `import` declarations must precede the `application` declaration; there
  is exactly one application per file, and `base` must be its first member
  when present.
- A `///` doc comment run directly above a `service` or `search` declaration
  becomes its description in the artifact. Contiguous `///` lines join with
  newlines — the same semantic as the `.mox` doc collection (`docs/LANGUAGE.md`).
- Delegated operations (`name => target.operation`) copy the target's
  signature conceptually and declare none of their own; a delegated op may
  be named `save` (contextual keywords). Declared service operations may
  carry a `capability a, b` clause, an `invokes a.b, c` clause (the
  decision services the operation invokes), and an `emits A, B` clause;
  delegated service operations may carry all three clauses too (they are
  service operations), plus the `transition` binding described below,
  while declared repository operations take neither (the wire artifact's
  repository operations carry no capabilities, invocations, emissions, or
  transition bindings, so accepting one would silently drop data). A
  service operation's clauses are fixed in the canonical order
  `capability? invokes? emits? transition? throws? when?` — the order the
  formatter emits, so a source writing `when` before `emits` (or
  `transition` anywhere but between `emits` and `throws`) is a syntax
  error, not a formatting opportunity. A declared repository operation
  takes `throws? when?` after its signature; a built-in operation takes no
  `throws` (its failures are structural, not declared — a named syntax
  error). A service body may also declare decision
  dependencies with `inject decision <qualified-name>;` — the second
  `inject` form, alongside the plain `inject <name>;` dependency form
  (`decision` is contextual: special only right after `inject`,
  `^decision` escapes; `invokes` is special only in the clause position,
  `^invokes` escapes).
- **Transition binding** (issue
  [#176](https://github.com/yestechgroup/pherolang/issues/176)) — a
  *delegated* operation may carry `transition <Trigger>`, the explicit
  declaration of the machine-bound transition the delegation fires
  (replacing the consumer's per-state inference):

  ```
  service PayRunService {
      inject PayRunRepository;
      submit  => PayRunRepository.save transition Submit;
      reverse => PayRunRepository.save transition PayRunStatus.Reverse;
  }
  ```

  The machine is the one bound to the delegation target's repository
  design; the trigger must be one that machine declares — an explicit
  `via` name or a bare arrow's derived name (the lowerCamel of the target
  state), never a name that is both (that binding is ambiguous and
  refused). The qualified spelling names the machine by its **enum type
  name** — `PayRunStatus.Reverse`, not a machine name, because Phero Lang
  machine declarations are field-named (`machine status: PayRunStatus`) so
  a machine-name qualifier does not exist as authored. A module carrying
  more than one machine-bearing design **requires** the qualifier (E0352);
  a binding on a declared operation, or on a delegation that is not a
  repository `save`, is refused (E0351); an unknown trigger is E0350 with
  the declared trigger vocabulary (did-you-mean included). The artifact
  carries the binding as authored — `TransitionRef { trigger, machine }`
  on the operation (issue [#143](https://github.com/yestechgroup/pherolang/issues/143)'s
  lowering contract; see "The lowering contract" below). A binding that
  resolves also pins the machine's class as the operation's own-record
  condition scope — the multi-machine guard shape of issue
  [#180](https://github.com/yestechgroup/pherolang/issues/180).
- A service or repository operation may carry a leading `protected`
  modifier (Sculptor's visibility): the operation stays off the public
  interface. `protected` is contextual — in leading position it is the
  modifier, anywhere else an ordinary name (`^protected` escapes).
- **Child-scoped operations** (issue
  [#179](https://github.com/yestechgroup/pherolang/issues/179)) — a
  module-level `extend <Class> { ... }` block declares operations that act
  on the aggregate root's contained children (the root-side counterpart of
  a child aggregate's own lifecycle):

  ```
  extend PayRun {
      boolean addLine(PayRunLineDraft line) => lines.add;
      boolean removeLine(String lineId) => lines.remove;
      boolean saveLine(PayRunLineDraft line) => lines.save when (line.quantity > 0);
  }
  ```

  Each child operation is `protected? signature "=>" <feature>.<action>
  when? ;` — the `signature` is the service-operation production (its
  return type is required, as for a declared service operation), and the
  dotted pair is exactly two names: the root class's child feature and the
  action. The actions are the closed set `add`, `remove`, and `save`;
  `add`/`remove` require a many-valued feature, while `save` (insert or
  replace) is legal for a single-valued child too. A class may carry
  several blocks — they merge in source order — and child-operation names
  must be unique per extend target. `extend` is a contextual word like the
  rest, and the block stays in the module holding the root's design (the
  same-module coupling rule searches follow).
- **Executable operation bodies** (issue
  [#178](https://github.com/yestechgroup/pherolang/issues/178)) — a service
  operation may replace its signature-only body with a closed procedural
  statement block, naming its primary repository after `=>`:

  ```
  submit(PayRunLineDraft draft) capability SubmitPayRun => PayRunRepository {
      let run = self.findById("current") else error NotFound("PayRun", "current");
      guard run.total >= 0 else error Conflict("total");
      let units = run.lines.map(line => line.quantity).sum();
      run.total = units;
      let saved = self.save(run);
      let audit = PayRunAuditRepository.record("submit");
  }
  ```

  The optional leading return type makes the operation value-returning
  (absent is void); the `capability`/`invokes`/`emits`/`transition`/`when`
  clauses keep their canonical order **before** the `=>`. The `=>` name is
  one repository designed in the same module and injected by the service —
  it binds as `self`, and every other injected repository binds by its bare
  dependency name. A declared repository operation takes the same block
  after its optional `when` guard (`Type? name(params) when? { ... }`) and
  binds its own design's repository as `self`; a built-in repository
  operation takes no body (a named syntax error). The body is
  **orchestration-only** (see [The executable-body posture](#the-executable-body-posture)):
  the statement subset is closed — `let`, `let … else error …`,
  assignment to a local's feature path, `guard … else error …`,
  `if`/`else`, and `return` — and every other statement is a parse error
  naming the construct. A raise names an error of the declared vocabulary
  (application- and module-scoped declarations plus the standard prelude),
  and its arguments type-check exactly against the declared payload; a
  value-returning body must return on every successful path. A body may
  call a domain `op` on a loaded local (`run.submit()`), which is the one
  widening the orchestration posture rests on.
- The repository built-ins are `findById`, `findAll`, `findByExample`,
  `findByKeys`, `save`, and `delete` — the consumer knows their signatures.
  A body's receiver-call surface synthesizes only `findById` (an
  `Option<E>`), `findAll` (a `List<E>`), `save` (`E`), and `delete`
  (`boolean`); `findByExample`/`findByKeys` are outside the closed subset
  and are refused as unknown operations.
- Multiplicity annotations on return types and parameters (`String[]`,
  `Book[0,3]`) lower into the artifact's cardinality slots
  (`returnMultiplicity` / parameter `multiplicity`).
- A `search` projection's members may appear in any order (the formatter
  canonicalizes them); repeated `text`/`filters`/`sort`/`document`/
  `capability` clauses merge, and repeats of the single members (`entity`,
  `ranking`, `analyzer`, `pagination`) are idempotent — the first
  declaration wins.
- A document entry's expression is captured **raw** (tokens up to the `;`,
  nested brackets and string literals respected) and stored verbatim in the
  artifact; parsing and type-checking it is the driver's job (rule 11). The
  expression language is the Tier-2 neutral expression language with the
  entity's features as `self` — string concatenation via `+` is rule R9 of
  `docs/EXPRESSIONS.md`.

---

## Resolution semantics

Names resolve with the `.mox` cross-package rules against the union
namespace of the imported domains (the same rule multi-file `.mox` compiles
follow). A bare single-segment name must be unique across all domain
packages; when several packages declare it the error is
``ambiguous type `X`; qualify as `p.X``` with the matching packages as
help. A qualified `pkg.Name` must match exactly one package. When the
design declares `base`, that package becomes the preferred resolution
scope: a bare name the base declares resolves there before the
unique-across-packages lookup runs. `base` itself must name an existing
domain package.

Authored names stay exactly as written — unqualified where written,
package-qualified where written — and remain the round-trip, diagnostics,
and source-mapping spelling. Alongside every reference a consumer acts on,
the artifact carries the **resolved companion**: the identity the compiler
resolved, so a consumer acts on that companion instead of re-implementing
the bare / `base`-preferred / unique-across-packages rules above.

| Authored field | Resolved companion | Carries |
|---|---|---|
| `Design.class` | `typeRef` | the resolved design class (`TypeRef`); absent for a standalone DTO |
| `Delegation.target` | `targetRef` | the delegated service, repository, or search |
| `Service.dependencies[]` | `dependencyRefs[]` | one target identity per injected dependency |
| `Service.decisionDependencies[]` | `decisionDependencyRefs[]` | one decision-service identity per injection |
| `ServiceOperation.capabilities[]` | `capabilityRefs[]` | the capability identity, plus `declaredIn` when an `.actor` artifact was supplied |
| `SearchDef.capabilities[]` | `capabilityRefs[]` | the capability identity guarding the search |
| `ServiceOperation.emits[]` | `emittedRefs[]` | the event identity; `contract` stays absent (no `.evt` bucket) |
| `ServiceOperation.invokes[]` | `decisionRefs[]` | the decision-service identity, plus `contract`/`version` when a `.dmn` artifact was supplied |
| `SearchDef.entity` | `entityRef` | the searched class (`TypeRef`) |
| `SearchDef.text`, `filters`, `sort` members | each member's `typeRef` | the referenced feature's declared type |

The companion shapes are fixed:

- `typeRef`, `entityRef`, and a search member's `typeRef` use the domain
  IR's `TypeRef` (adjacent tagging: `{"type":"class","value":{...}}`);
  search members may be any feature type, primitives and enums included.
  The field is optional on the wire so older and hand-built artifacts keep
  loading; a successful compile carries it for every reference it resolved.
  A standalone DTO has no `typeRef` (it has no class), and a signature's
  DTO form is not a `TypeRef` at all: `{"dto":"BookSummary"}`, joined to
  the design whose `dtoShape.name` matches.
- `targetRef` and each `dependencyRefs[]` entry are
  `{kind, qualifiedName, module}`. `kind` is the bare
  `"service" | "repository" | "search"` tag, `qualifiedName` is the target
  declaration's identity — `<Application>.<module>.<Name>`, or
  `<module>.<Name>` when the design declares no application — and `module`
  is the design module that declares it. A delegation's `operation` stays
  authored: it names the operation on the resolved target.
- `capabilityRefs[]` entries are `{name, declaredIn?}`, index-aligned with
  `capabilities` (operation and search lists alike). `declaredIn` names the
  actor block in the supplied `.actor` model that declares the capability
  (the first declaring block when several do), and is absent when no actor
  artifact was supplied.
- `emittedRefs[]` entries are `{name, contract?}`, index-aligned with
  `emits`. `contract` is always absent today: the `.ddd` compile has no
  `.evt` bucket, so event references are unresolved by design.
- `decisionRefs[]` and `decisionDependencyRefs[]` entries are
  `{service, contract?, version?}`, index-aligned with `invokes` and
  `decisionDependencies`. `contract` names the `.dmn` artifact that
  supplied the service and `version` the version that artifact declares;
  both are absent when no decision model was supplied.

Alignment is positional: entry `i` is the companion of authored name `i`.
An empty authored list omits its companion list, and a populated authored
list has an equally long companion list.

---

## Consumer contract

The artifact is the contract between Phero Lang and its consumer; this is
the short version to implement against without reading `phero-driver`.

**Pairing.** A `DddModel` is meaningful only with the domain `Model` it was
compiled against: the design names classes and features, the domain defines
them. Carry the pair together. The design's optional dimensions pair with
the artifacts the compile was given — the `ActorModel`s supply capabilities
(recorded as `declaredIn`) and the `DmnModel`s supply decision services
(recorded as `contract`/`version`). A `.ddd` compile has no `.evt` bucket,
so event contracts pair at the union-integrity layer (`pherolang check`),
never through `emittedRefs`. None of the pairing needs the `.mox`, `.actor`,
`.evt`, or `.dmn` sources.

**Gate.** Accept exactly `formatVersion` 2; `DddModel::from_json` and
`pherolang artifact check` both gate on it. `pheroVersion` is provenance:
ignore it for compatibility.

**Act on the resolved companion, not the authored spelling.**

- **Classes and features** — `Design.typeRef`, `SearchDef.entityRef`, and
  each search member's `typeRef` are the domain IR's `TypeRef`; join the
  domain `Model` by `(package, name)`.
- **Services, repositories, searches** — `Delegation.targetRef` and
  `Service.dependencyRefs` identify a design declaration by
  `qualifiedName` and `module`. That `qualifiedName` is the semantic
  graph's node name for the declaration
  (`<Application>.<module>.<Name>`, `<module>.<Name>` without an
  application), so graph edges and design references join by the same
  string; `kind` selects the node kind. A delegation's `operation` names
  the operation on the resolved target.
- **Capabilities, events, decisions** — `capabilityRefs`, `emittedRefs`,
  `decisionRefs`, and `decisionDependencyRefs` are index-aligned with
  their authored name lists; read the companion at the same index.
- **Values** — a design carrying `semantics` is a strictly immutable value
  object: no identity, immutable, deep-immutable, structural equality. The
  block is the contract; never infer it from `stereotype`. Emit a
  constructor-only type whose inputs are exactly the stored features in
  declaration order — private or immutable fields, no public setters, no
  `Default`-then-mutate path, and no mutable view of nested values or
  collections escaping the constructor (expose immutable collection
  views). Implement equality and hash over the stored features in
  declaration order; derived features are computed on read, never
  construction inputs, never equality members, never persisted. A
  persistence surrogate id is the consumer's internal detail at most:
  never on the type's public surface, never equality-bearing.
- **DTOs** — a `dtoShape` is a transport type: emit exactly the declared
  members, in order, under the names as written (the left-hand side of a
  projection is the wire name). A `select` copies the source feature at
  `path` (joined by `.`); a `derive` evaluates `expression` against the
  source instance (the source class is `self`, and the expression text is
  verbatim, typed by the compiler); a `declared` member has no source
  counterpart. A DTO is not persisted, is not an aggregate member, and
  carries no invariants. `direction: "output"` marks a boundary no caller
  can supply — never accept one in a parameter position; a return value
  may be output. Join a `DddTypeRef` domain form to the domain `Model` by
  `(package, name)`, and its `{"dto": name}` form to the design whose
  `dtoShape.name` matches.
- **Errors** — emit one error type per `ErrorDecl` with exactly the
  declared payload fields, in order: `errors` at the model root, each
  module's `errors` beside its services. A raised error propagates with its
  payload unchanged. `throws` is **exhaustive**: a consumer may rely on it
  for exhaustiveness checking, status mapping, or documentation. A
  reference whose `scope` is `"prelude"` names the fixed standard shape
  (`NotFound(resource, key)`, `Conflict(detail)`) — never redefine it, and
  never expect a prelude declaration in `errors`. A `"module"` reference
  names its declaring module; such a private error never crosses the
  module boundary in a well-formed artifact.
- **Flags** — `Design.flags` already carries the effective value (`auditable`
  and `optimisticLocking` are on for a plain entity design), so never
  re-apply the source defaults.

**What an absent optional member means.** `declaredIn`, `contract`, and
`version` absent means "the counterpart artifact was not supplied at
compile time" — unresolved by design, not an error. The authored name is
still recorded and the companion still exists; only the cross-dimension
join is open. A consumer that needs the join must require the counterpart
artifact; one that does not may ignore it.

**What `pherolang artifact check` guarantees.** From the serialized artifact
alone, with no sources or models present, it checks the version gate and the
DDD contract: every authored reference carries its resolved companion (a
standalone DTO is the exception — it has no class), the parallel lists are
index-aligned, the search references are complete, stereotype/flag
combinations are legal, every value design carries its `semantics` block
(and no entity or dto design does), every `dto` design carries a `dtoShape`
(and no other design does) with its member order preserved and well-formed
kinds — `select` carries a `path`, `derive` an `expression`, `declared`
neither, and output names are unique — every DTO reference resolves to a
`dtoShape` name in the
artifact, no output DTO fills a parameter slot, every error name/scope
resolves with no prelude shadow and every payload field name unique, every
`throws` reference resolves to a declared error or the prelude with a
consistent scope/module, every body raise appears in its operation's
`throws`, every delegation's `throws` is a superset of its target's, and
`pheroVersion` is present and well-formed. A hand-edited or truncated artifact that drops a
companion, a `semantics` block, or a `dtoShape`, or invents an illegal
combination fails with a diagnostic naming the design or reference. It does
not re-typecheck against the domain — that needs the paired `Model` — and it
cannot re-check the value rules (R-V1 to R-V3), which read the `.mox`
classes, nor the DTO member typing, which reads the source class.

---

## Validation rules

1. **base** — an unknown base package is an error naming it.
2. **Uniqueness** (application-wide unless noted) — module names; service
   names; design class references (one design per class per application,
   keyed by the resolved class); repository names; operation names within
   one service; operation names within one repository; DTO boundary names
   (`E0371`; a mirror DTO's name is its source class and is exempt from the
   shadow half of the rule).
3. **Stereotype targets** — every design's `class` must resolve to a class
   in the domain model, with the DTO exceptions: a **standalone** DTO
   (`dto Name { ... }`) has no source class at all, a **projection**
   (`dto Name from Source { ... }`) resolves its `from` source, and a
   **mirror** resolves its own name. A projection may coexist with its
   source's entity design — the boundary is not a second design of the
   class.
4. **Flag/stereotype compatibility** — `scaffold`, `auditable`, and
   `optimisticLocking` are entity-only; `nonPersistent` is value/dto-only;
   `cache` is legal on any design. `auditable` and `optimisticLocking`
   default **on** for entity designs (Sculptor's polarity): the artifact
   lowers them to `true` unless the design opts out with the negated form
   (`!auditable`, `!optimisticLocking`). Negating a flag that defaults off
   (`!scaffold`, `!nonPersistent`, `!cache`) is rejected wherever written.
5. **Repository placement** — only entity designs may declare a repository.
6. **Aggregate boundary** — see the derivation rule below; an entity design
   derived as a non-root may not declare a repository.
7. **Signature type-check** — declared service and repository operation
   signatures type-check against the domain model or the application's DTO
   boundaries: the `phero-syntax` primitives resolve everywhere, every
   other reference resolves through the bare/qualified rules above (any
   declared kind — class, enum, datatype, interface, vocabulary), and a
   single-segment name that resolves to no domain type but matches a
   projection or standalone DTO boundary is that DTO. The slot lowers to
   `DddTypeRef`: the resolved domain `TypeRef`, or `{"dto":"Name"}`. Domain
   resolution wins deterministically when a name matches both — a
   projection/standalone name shadowing a domain type is the `E0371`
   collision error, while a mirror DTO deliberately resolves as its source
   class. A DTO in a **parameter** slot must be input-capable: an output
   DTO (one with a derived member) is refused (`E0370`); a return type may
   be output. A multiplicity annotation on a return type or parameter
   lowers into the artifact's cardinality slots.
8. **Delegations** — the target must resolve to a service, a repository,
   or a search declared anywhere in the application, and the operation must
   name an operation on it (repository built-ins included; a search exposes
   only the virtual operation `search`). Service → repository and
   service → search delegation across different modules is the
   module-coupling error ("interaction between a Service in one Module and
   a Repository/Search in another Module is not allowed; go via a
   Service"); service → service across modules is allowed.
9. **inject** — every dependency name must resolve to a service, a
   repository, or a search of the application.
10. **Capabilities** — only `compile_ddd_str_with_actors` validates: each
    declared capability name must exist in the union of the actor model's
    blocks' capabilities. Plain compiles record the names unvalidated —
    the design dimension is loosely coupled to the policy dimension by
    design.
11. **Searches** — a search's name is unique application-wide. Its `entity`
    line is required, resolves like a design target, and must bind to an
    entity design **of the same module** (the repository coupling rule;
    non-root entities may be searched — a projection over a contained
    entity is legitimate). Text fields must name a direct feature of the
    entity (inherited features included) that is text-like: the string
    primitives, enums, datatypes — the string-family default the constraint
    rules use — and vocabularies whose key facet is a string primitive;
    boosts are at least 1 and analyzers non-empty. Filters and sorts name
    direct features that are not class references. Document field names are
    unique per search, and every document expression is parsed and
    type-checked with the entity as `self` (any value type is legal; the
    consumer decides the rendered form). Pagination bounds satisfy
    `1 ≤ limit ≤ max`.
12. **Value contract** — a `value` design is a strictly immutable value
    object (Sculptor's ValueObject with no `!immutable` opt-out, issue
    [#299](https://github.com/yestechgroup/pherolang/issues/299)): no
    identity, immutable, deep-immutable (nested values and collections),
    and structural equality over the stored features (derived excluded) —
    the four properties in [Value objects](#value-objects), carried in the
    artifact as `semantics`. Every stored feature of the resolved class
    (own and inherited, derived features excluded) must carry the domain
    model's `readonly` modifier. The sub-rules below are checked against
    the **application's designs**, not the `.mox` file alone — the same
    class may be a `value` in one application and an `entity` in another,
    and that is legal:
    - **R-V1 — no identity.** A `value` whose target class declares an
      `id`-modified feature is an error naming the design and the feature.
      Workaround: drop the `id` (a value is identified by its state) or
      design the class as an `entity`.
    - **R-V2 — no owned mutable state.** A `value` may not own a
      `contains` whose target class is designed as an `entity` in the same
      application. Containment of other values is the normal nested-value
      case.
    - **R-V3 — no entity references.** A `value` may not declare a
      `refers` whose target is designed as an `entity` in the same
      application. This is an error, not a warning: a mutable entity
      handle makes the value's observable state mutable and turns
      structural equality into identity comparison. Workaround: refer by
      id — `refers String personId`.
    - **No abstract values.** `abstract value` is an error — abstractness
      belongs to a hierarchy, and a value has no identity to inherit. A
      shared shape without identity is a `.mox` interface or datatype.

    A changeable class designs as `entity`; a transport shape as `dto`.
13. **Aggregate reference constraint** — an outside class may hold a cross
    reference only to an aggregate's root: for every cross-reference
    (`refers`) feature whose target sits under a containment chain (a
    non-root aggregate member), the referring class must share the target's
    containment root. Computed over the whole union like the
    aggregate-derivation rule, and skipped when the verdict would be a
    guess (the target or source sits under two distinct roots, or under a
    containment cycle).
14. **scaffold** — a scaffolded entity design must declare a repository:
    `scaffold` stands for Sculptor's generated CRUD operations (which also
    imply the service), and the artifact synthesizes nothing — so the author
    declares the repository the scaffolding
    would hang off. The service remains the author's to declare.
15. **dto-with-ops error** — a `dto` design whose class carries `ops` is an
    error, once per op, naming the design and the op (`E0365`): a dto is a
    behavior-free transport shape, and behavior on it is a modeling
    mistake (see the layering heuristic under "Domain operations vs
    service operations"). `derived` features stay legal — a computed wire
    field, not behavior. The check runs for the **mirror** form only
    (whose boundary *is* the class's shape); a projection reads a source
    class that may legitimately carry behavior, so its ops are not the
    boundary's. It runs only against a fully lowered union, and an error
    drops the artifact.
16. **Typed status fields** — an entity design whose class carries a
    feature named `status` or `state` typed as a bare `String` is an
    error: a status-like field is a lifecycle, and the bare-`String`
    shape is the silent no-machine case. Declare an enum in the domain
    model and a `machine '<field>: <Enum>'` on the design. Value objects
    and dtos are exempt (the check is entity semantics, the machine
    rule's scope), and enum-typed fields stay legal — an enum without a
    machine is a legitimate modeling choice (the canonical fixture's
    `MediaStatus status` field is the shape).
17. **Decision links** — only `compile_ddd_str_with_decisions` validates:
    every decision-service reference (an operation's `invokes` name, a
    service's `inject decision` name) must resolve by its **last segment**
    to a decision service declared in the union of the provided `.dmn`
    models' services (the names pool across every provided model, the
    rule-10 shape), else the reference errors `E0250 unknown decision
    service '<name>' invoked by operation '<op>' of service '<service>'`
    (injections: `E0251 unknown decision service '<name>' injected by
    service '<service>'`) — spanned on the reference, with the
    did-you-mean closest declared service when one is a plausible typo,
    and the exhaustive list otherwise. Plain compiles (and any empty
    decisions bucket) record the names unvalidated — the design dimension
    is loosely coupled to the decision dimension by design. Every
    `import "<path>.dmn"` the design declares must be provided under that
    path when a non-empty decisions bucket is supplied, else
    `imported decision model "<path>" was not provided`; `.dmn` imports
    are not domains and never lower as `.mox` sources — a plain compile
    (pherolang#154).
18. **Version pins** — an `invokes` entry or an `inject decision` line
    may declare `version compatible` or `version "X.Y.Z"` (an exact
    semver). The pin narrows rule 17's resolution against the models'
    declared `version` metadata: an exact pin resolves to the same-name
    service at that version (zero matches is **E0252**, naming the
    versions available; two or more candidates declaring the same
    version is **E0253**, ambiguous); `compatible` resolves to the
    highest declared version in the highest major present (a model
    whose version is absent or not strict `X.Y.Z` cannot satisfy a pin —
    **E0254** when none can). Unpinned references keep the rule-17
    behavior; the compatibility contract itself is the decision model's
    (same major = compatible — see the `.dmn` surface's Versioning
    section).
    ignores them outright.
19. **Executable operation bodies** — a body is typed against the
    operation's declared parameters and output (absent for a void
    operation) and the repository receivers described above. The statement
    subset is closed and **orchestration-only** (see
    [The executable-body posture](#the-executable-body-posture)); every
    expression slot is phero-expr source checked under the same typing rules
    as a `when` guard. The failure contract is hard: an unknown feature or
    operation, a type mismatch, a non-boolean `guard` or `if` condition,
    an unknown raise (a name outside the declared vocabulary), a raise whose
    arguments do not match the declared payload (arity or type), a raise the
    operation's `throws` does not declare, a whole-local reassignment, a
    cross-aggregate load, and a value-returning path that does not `return`
    are all `E0360` (`invalid service body in operation '<op>': <slot>:
    <detail>`, repository bodies worded the same way). An unresolved `=>`
    receiver is `E0359` (`unknown body receiver '<name>' on operation '<op>'
    of service '<s>'`) — the name must be a repository designed in the
    operation's module that the service injects, written as a single
    (unqualified) repository name. The checked body lowers to
    `bodies["stmt"]`; nothing is attached when any diagnostic fired. Unlike
    the sigil-lowered Rune bodies' generate-time deferral, the `.ddd`
    compiler never attaches an unchecked body. The raise vocabulary is
    **declared**, never hardcoded: rules 21–22 own the declarations, the
    prelude, and the `throws` subset/propagation contract.
20. **DTO boundaries** — a `dto` design is checked as a transport shape
    (DD3, issue
    [#300](https://github.com/yestechgroup/pherolang/issues/300); the
    vocabulary is [Data transfer objects](#data-transfer-objects)):
    - **Placement** — no identity semantics, aggregate membership,
      repository, machine, or invariants, and no `op` (rule 15).
      `nonPersistent` and `cache` stay legal; `scaffold`, `auditable`, and
      `optimisticLocking` are entity-only (`E0372`, the transport-shape
      wording of rule 4/rule 5).
    - **Class** — a standalone DTO declares none; a projection resolves
      its `from` source; a mirror resolves its own name (rule 3).
    - **Name** — application-wide unique and no shadowing of a domain
      type (`E0371`); a mirror DTO's name is its source class and is
      exempt.
    - **Members** — a mirror copies every stored feature (own and
      inherited, derived excluded) in declaration order. A projection's
      `name;` selects a feature, `name = path;` selects a nested feature
      path, and `name = expr;` derives a value typed with the source class
      as `self` (`E0367` unknown member/path, `E0368` invalid member
      type). A standalone member declares `Type name;`, resolving to a
      primitive, enum, datatype, vocabulary, value, or another DTO
      boundary. A member may not carry or select an entity-designed class
      (`E0369`) — project its fields, or refer to it by id. Duplicate
      output names are errors (`E0366`); declaration order is wire order.
    - **Direction** — any derived member makes the boundary `output`,
      otherwise `input`; an output DTO in a parameter slot is `E0370`,
      while a return type may be output. A projection over a
      behavior-rich source is legal (rule 15's error is the mirror's).

21. **Declared error vocabulary** — `error <Name>(<payload>);` declares
    one error at application or module scope (DD7, issue
    [#304](https://github.com/yestechgroup/pherolang/issues/304)). The
    payload fields reuse the operation-parameter production and resolve like
    signature types: the domain union and the application's DTO boundaries
    are the universe (a DTO payload is data, and an *output* DTO is legal in
    a payload — an error crosses a boundary outbound). Rules:
    - **Uniqueness** — error names are application-wide unique (like
      service names): a duplicate is `E0374`, and the first declaration keeps
      resolving.
    - **No prelude shadowing** — `NotFound(resource: string, key: string)`
      and `Conflict(detail: string)` are the standard prelude: always
      available, never emitted in `errors`, and a user declaration with
      either name is `E0374`.
    - **Payload** — a field name is unique per declaration (`E0378`), and a
      field may not carry a class designed as an `entity` in the same
      application (`E0376`): an error payload is serializable data, so refer
      to an entity by id, embed a `value`, or carry a DTO.
    - **Visibility** — application-scoped errors are visible everywhere; a
      module-scoped error is private to its module, and naming one from
      another module is `E0375`, whose message names the owning module and
      the application-scope fix. Application-scoped errors are the
      cross-module contract.
    - **Artifact** — the declarations lower to the model root's `errors`
      (application scope) and each module's `errors` (module scope), with the
      doc comment as `description`; the prelude lowers nothing.
22. **`throws`** — a declared, delegated, or body service operation and a
    declared repository operation may carry `throws <Error>, …`; the clause
    is fixed between `transition` and `when` on a service operation and
    directly after the signature on a repository operation (rule 19's
    boundaries). The contract:
    - **Resolution** — a bare name resolves against the prelude, the
      application-wide declarations, and the operation's own module
      declarations (`E0373` names the in-scope vocabulary; `E0375` names a
      foreign module's private error). The artifact carries the resolved
      `ErrorRef` (`scope`, and the declaring module for a module-scoped
      error).
    - **Explicit and complete** — every raise in a body must appear in the
      operation's `throws`; an undeclared raise is `E0360` naming the
      declared set. There is **no inference**: a `throws` entry with no
      reachable raise stays legal (an interface promise), and a body with
      no `throws` clause raises nothing. A prelude error is declared in
      `throws` like any other — no implicit propagation.
    - **Delegation is contract-preserving** — a delegation's `throws` must
      be a **superset** of the delegated operation's (`E0377`, naming each
      missing error). A target whose missing error is module-private names
      the application-scope fix: the private error cannot be part of a
      cross-module contract.
    - **Built-ins** — a built-in repository operation takes no `throws`; its
      failures are structural, not declared.
23. **Child operations** (`extend`) — the target class must resolve to a
    domain class with an **entity design in the same module** that is an
    aggregate root (a repository-bearing entity — rule 6's root predicate);
    an unresolved class, a missing design, and a design without a
    repository are all `E0358 unknown extend target`. The child feature must
    exist on the target class (the `extends` chain included) and be a
    containment or cross reference (`E0355`); the action must be one of
    `add`, `remove`, `save` (`E0357`), with `add`/`remove` requiring a
    many-valued feature (`E0356`). A child operation's `when` guard is
    typed with the operation's declared parameters in scope **and** the
    child feature's target class as `self` — `line.quantity` reads a
    parameter, a bare `quantity` reads the child's feature, and the root's
    own features are out of scope. A resolved guard lowers verbatim as
    `condition` and carries no `conditionPaths` (the resolved-path lowering
    belongs to service operations). The lowered operation rides the target
    design's `childOperations` with its `childScope`.

## The executable-body posture

The `.ddd` statement body is **orchestration-only**, and that is a permanent
posture, not a temporary gap: it orchestrates one aggregate and calls domain
behaviour; it does not implement algorithms.

What a body may do:

- load the primary aggregate (`self.findById`/`self.findAll` on the `=>`
  receiver, or the operation's own repository for a repository body);
- validate with `guard`/`let … else error` and raise a declared error;
- bind values with `let`, branch with `if`/`else`, and return;
- assign an aggregate **feature path** (`run.total = units;`); and
- call a domain `op` on a loaded local (`run.submit()`), typed against the
  class's operations, return multiplicity included.

What a body may not do — each is a compile-time rejection naming the
construct, because widening the body later is additive while narrowing it is
not:

- **loops** (`for`, `while`) — move the iteration into a domain `op`;
- **local reassignment** (`x = …` with a one-segment path) and **compound
  assignment** (`+=`, `-=`, …) — assign a feature path instead;
- **collection mutation** (`push`, `remove`, `clear`) — move it into a
  domain `op`;
- **`try`/`catch`** — a body raises an error, it never catches one
  (`throws` is the failure contract);
- **cross-aggregate loads** — one body binds one primary repository, so
  `findById`/`findAll` through any other injected repository is refused;
- **expression statements** — bind the call's result (`let x = …;`);
- **lambdas** beyond what the expression language already supports.

Not yet (deliberately out of scope, not accidental omissions): compensation
and saga orchestration, retry policies, and compiler-generated error
hierarchies. A domain error hierarchy is a consumer concern over the flat
declared vocabulary; the body never catches.

## State machines

An entity design may carry one lifecycle state machine — a
`machine <field>: <Enum>` block, written after the optional repository
block (the formatter's canonical order; one per design). `machine`,
`initial`, `terminal`, and `via` are contextual words like every `.ddd`
word. The normative grammar (`machine_decl`, `machine_member`,
`transition`, `guard_clause`) lives in the parser module docs
(`crates/phero-syntax/src/parser/mod.rs`); the canonical fixture's machine gives
the shape:

```
machine status: PayRunStatus {
    initial Draft;
    Draft -> Calculating via Calculate;
    Calculating -> AwaitingApproval via Submit;
    AwaitingApproval -> Approved via Approve when (total > 0);
    Approved -> Processing via Schedule;
    Processing -> Posted via Post;
    Draft -> Reversed;
    terminal Reversed;
    terminal Posted;
}
```

The driver validates the machine before it lowers (`lower_design_machine`
in `crates/phero-driver/src/ddd.rs`; behavior pinned by
`crates/phero-driver/tests/ddd_machines.rs`):

- Only an **entity** design carries a machine — a value object is
  immutable and a dto is a transport shape; neither moves through states.
- The machine's type must resolve to an **enum**, and the bound feature
  (`<field>`) must exist on the designed class and carry exactly that enum
  type.
- **Closed over the enum** — every state name (the `initial`, a `terminal`,
  and every transition endpoint) must be a literal of that enum: a typo'd
  endpoint is an error naming the enum and its declared literals, never a
  new state. The enum's literal list is the single vocabulary of the
  lifecycle.
- Guards are predicates: a `when` condition on a transition — and on any
  service or repository operation — must type as `boolean` (the actor,
  deploy, and cmmn conditions' rule).
- Exactly one `initial` state. Duplicate terminal declarations and
  duplicate identical edges are rejected.
- **Determinism** — one trigger name may leave a state for at most one
  target (the statechart event rule).
- **Terminal states take no outgoing transitions**; every non-terminal
  state takes at least one (no dead ends — declare it `terminal` or add an
  outgoing edge).
- **Reachability** — every state is reachable from the initial state
  (breadth-first over the edges).
- Edge guards and operation `when` conditions type-check through the shared
  expression language (phero-expr) with the designed class as the implicit
  `self` — and guard literals against the machine field must be machine
  states: only the declared states bind as names, so a member of the bound
  enum that the machine never declares is a type error (it can never hold
  on this machine).

**Condition binding.** Machine states bind as enum-typed *names* — guards
read `status == AwaitingApproval`, never `status == "AwaitingApproval"`.
The string literal is a type error under the same-type equality rule (R2 of
`docs/EXPRESSIONS.md`); the anti-magic-strings stance is deliberate. The enum-literal namespace (rule L3 of `docs/EXPRESSIONS.md`) is the general typed spelling beyond a machine's own bound states: `status == PayRunStatus.Draft` names any declared member of the field's enum as a typed value, and a string literal that names no declared member is the closed-set membership error rather than the generic mismatch. Edge
guards bind only the machine's own states (a foreign machine's states are
unbound — cross-entity state references are a type error). Comparisons
against other features are not machine-checked: their name operands
resolve as features.

**Condition scoping** (issue
[#180](https://github.com/yestechgroup/pherolang/issues/180)) — a service
operation's `when` guard is not single-entity. Three scoping regimes, by
the module's machine count:

- **One machine-bearing design** (the original shape): that design's class
  is the guard's implicit `self`; bare feature names and machine states
  resolve against it. A guard with no injected path lowers no
  `conditionPaths` — the resolution is derivable from the artifact's
  machine alone.
- **No machine-bearing design**: an op-level guard has no own-record scope
  at all and is refused (`does not have exactly one machine-bearing
  design`), unless a resolved `transition` binding pins the scope (the
  joint shape with issue #176 above).
- **Several machine-bearing designs** (multi-machine): there is no
  implicit self — bare names must resolve *uniquely*:
  - machine states bind when exactly one machine of the module declares
    the name; a state two machines declare never binds, and referencing it
    unqualified is the `E0354` ambiguous-machine-scope error with the
    `qualify as `<EnumType>.<State>`` help — the enum-literal spelling
    (`PayRunStatus.Draft`) resolves it;
  - each machine's bound field is addressable two ways: by its authored
    field name when that name is unique across the module's machines, and
    always by the **enum-derived spelling** — the lowerCamel of the
    machine's enum type name (`machine status: PayRunStatus` →
    `payRunStatus`). A field name two machines bind (`status` on both
    designs) is the `E0354` error unqualified, with the spellings in the
    help;
  - a qualified path (`TimecardRepository.status`) resolves against the
    service's injected repositories — the first segment must match an
    injection name **exactly as declared** (case-sensitive; a near-miss is
    the `E0353` unknown-condition-path error with the did-you-mean), the
    repository must be designed in the same module, and the remaining
    segments are fields of the repository's class (`E0353` names the class
    and suggests when a segment is no such field). A path that resolves
    but mistypes is ordinary typing: the same-type equality error, spanned
    on the condition.

**Resolved condition paths.** The artifact carries the guard's resolution
as `conditionPaths` on the operation — `ConditionPath { scope, field,
fieldType }` rows, scope adjacently tagged `{"type":"ownRecord"}` or
`{"type":"injected","value":{"repository":…,"class":…}}` (the `repository`
is the injection name as declared, the `class` the repository design's
class; `field` is the dotted remainder, `fieldType` the field's declared
type). A row lowers exactly when the path's resolution is **not derivable
from the artifact alone**: injected paths always (the repository→class
binding is design knowledge the domain model does not carry), and
own-record paths in a multi-machine module (where the owning machine is
not a priori derivable). The `condition` source string stays verbatim;
consumers build on the resolved form (issue #143's lowering contract)
instead of re-parsing guards.

**Artifact shape.** The machine lowers to `machine` on the design (a
`phero_ir::ddd::StateMachine`): the bound `field`, the enum `TypeRef`, the
`initial` state, the `terminals`, and the `transitions` — the `via` trigger
(`None` for a bare arrow) and the guard as the verbatim source text between
the parentheses (the `when`-condition rule). The state set is **derived**,
never serialized as a separate list: `StateMachine::states()` is the
initial state, then the terminals, then the remaining transition endpoints
in first-appearance order; terminalness is membership in `terminals`.

**xstate-ir lowering.** `phero_driver::machines::lower_design_machines` maps
every designed machine onto the
[restingstate](https://github.com/yestechgroup/restingstate) toolchain's
language-neutral `xstate_ir::Machine` (rev-pinned as `xstate-ir` in the
workspace `Cargo.toml`; the sigil precedent). The mapping is total and
lossless for the v1 surface: `id` is the design class name; `context` is
one field (the bound feature, typed by the enum's qualified name, default
the initial state); states follow `StateMachine::states()` with terminals
as `final` nodes and the rest `atomic`; transitions attach to their source
in artifact order, and the guard maps to a named guard whose name is the
condition's source text. A bare arrow's event is **derived**: the lowerCamel
of the target state (`Posted` → `posted`, `AwaitingApproval` →
`awaitingApproval`). The committed goldens pin both forms —
`tests/conformance/ddd/payrun.ddd.json` (the design artifact) and
`tests/conformance/ddd/payrun.xstate.json` (the lowered machine);
regenerate with `PHERO_UPDATE_FIXTURES=1 cargo test -p phero-driver --test
ddd_machine_golden`.

**Derived events.** Every designed machine derives the `state_changed` event
contract (entity + field — `phero_driver::machines::derived_events`), which
is what makes the push plane's `state_changed` vocabulary reachable for a
plain CRUD model once its lifecycle is modeled.

## Value objects

A `value` design means **strict value semantics** — all four properties,
together, with no opt-out (issue
[#299](https://github.com/yestechgroup/pherolang/issues/299)):

| Property | Meaning |
|---|---|
| **No identity** | The class has no identity-bearing feature. Two values with equal stored state are interchangeable. |
| **Immutable** | All stored state is fixed at construction: no setters, no public mutable fields, no mutation of nested state after construction. |
| **Deep** | Immutability includes nested values and collections: no mutable view of owned state escapes the constructor. |
| **Structural equality** | Equality and hashing are over the stored features (derived features excluded), never over a handle. |

There is no `!immutable` flag and no mutable-value spelling: the
stereotype is the contract. A class that must mutate is designed
`entity`; a shape that carries data between layers is designed `dto`.

Construction is one step — the constructor inputs are exactly the stored
features, in declaration order, so a consumer emits a constructor-only
type with no `Default`-then-mutate path. Derived features are computed on
read; they are neither construction inputs nor equality members. A
persistence surrogate id is the consumer's internal detail at most: it
never appears on the value's public surface and never participates in
equality.

The rules that keep the properties honest (R-V1 to R-V3 under rule 12)
are checked against the **application's designs**, not the `.mox` file
alone: the same class may be a `value` in one application and an
`entity` in another, and that is legal — each design states its own
semantics. The artifact records the four properties as the design's
`semantics` block rather than leaving a consumer to infer them from
`"stereotype": "value"` (see [Artifact wire format](#artifact-wire-format)
and the [consumer contract](#consumer-contract)).

Workarounds, when the model wants something a value may not have:

| The model needs … | Write … |
|---|---|
| identity, a lifecycle, or mutation | an `entity` design |
| a shape that crosses a layer boundary (no identity, no behavior) | a `dto` design (DD3) |
| a reference to an entity | refer by id: `refers String personId` |
| a class that already declares an `id` | drop the `id` — the value is identified by its state |
| shared shape without identity | a `.mox` interface or datatype, not `abstract value` |

The shipped example is `examples/support/support.mox`'s `RefundDecision`:
it keeps `readonly String decisionId` as stored state — an ordinary
stored feature, no longer marked `id` — and stays a value design
(`value RefundDecision nonPersistent` in `support.ddd`).

## Data transfer objects

A `dto` design is a **transport shape**: the type that crosses a layer
boundary. It is not a domain concept — no identity semantics, no aggregate
membership, no repository, no machine, and no invariants — and it is
behavior-free: an `op` on a dto is an error (rule 15), while a `derived`
member is a legal computed wire field. A DTO is input-capable exactly when
it has no derived members.

Three forms, by what the declaration says:

```
dto User nonPersistent                       // mirror: every stored feature of the class
dto BookSummary from Book {                  // projection
    title;
    isbn;
    rating = playLength / 60;
    authorName = category.name;
}
dto CreateBookRequest {                      // standalone
    String title;
    String isbn;
}
```

| Form | Shape | Members |
|---|---|---|
| mirror | `dto User` | every stored feature of the class (own and inherited, derived excluded), in declaration order |
| projection | `dto BookSummary from Book { ... }` | the block's members, in declaration order |
| standalone | `dto CreateBookRequest { ... }` | the declared members, in declaration order |

A projection member is one of three things, and the left-hand name is
always the **wire name**:

| Write | Meaning |
|---|---|
| `name;` | select the source feature `name` |
| `name = path;` | select the plain feature path (inherited features included), renaming it to `name` |
| `name = expr;` | a derived member: `expr` is typed and evaluated with the source class as `self` |

A selected/renamed path whose final source feature is `derived` is
computed, not copied: it lowers as a `derive` member (the path text is the
expression) and makes the boundary `output`. A mirror excludes derived
features entirely.

A standalone member is `Type name;` (the usual multiplicity annotation
included) and exists only in the standalone form — a projection's block
takes the two shorthand member forms above, and the two block shapes are
not interchangeable. Declared types resolve to primitives, enums,
datatypes, vocabularies, values, and other DTO boundaries; an entity-class
member is rejected — project the entity's fields instead, or refer to it
by id. The same rejection applies to a selected or derived member whose
type is a designed entity.

Member order is wire order and the contract. Duplicate output names are
errors; a rename changes the wire name, never the source path. A member
carries at most one cardinality: a selected/declared feature keeps its
multiplicity, and a derived expression's `T[]`/`T[0..1]` result lowers to
the corresponding cardinality, but a nested shape (`List<Option<T>>` or the
reverse) is rejected — the artifact has one multiplicity slot per member.

**Direction** is derived, never authored: any derived member makes the
boundary `output`; a boundary with none is `input`. An output DTO cannot
fill an operation parameter slot — a caller has no value for a derived
member — while a return type may be output. A projection with only
selections is therefore input-capable, and a mirror always is (mirroring
leaves the source's derived features out).

**Names.** Boundary names are application-wide unique and must not shadow
a domain type (class, enum, datatype, interface, or vocabulary). A mirror
DTO is exempt: its name *is* its source class. A projection may coexist
with its source class's entity design — the boundary is not a second
design of the class. A consequence of the mirror's name: a signature
spelling it resolves to the **domain class**, so a mirror DTO has no
separate DTO-typed slot — use a projection or standalone boundary when an
operation needs a DTO parameter or return. DTO-typed values are opaque to
operation bodies: they can be passed and returned, but field access is not
part of the transport contract.

**Placement.** A dto carries no repository, no machine, and no entity-only
flag (`scaffold`, `auditable`, `optimisticLocking`); `nonPersistent` and
`cache` stay legal, and `nonPersistent` is the canonical spelling for a
boundary that never reaches storage. A dto is never persisted, never an
aggregate member, and carries no invariants; its members are data only.

**Shared projection AST.** A DTO projection's assignment and a search
`document` entry are the same AST node (`DddProjectionMember::Assign`)
with the same raw-expression capture; parsing and typing are the driver's.
Converging the two surfaces further — `document DtoName;` as a search's
document shape — is a follow-up, not shipped.

## Aggregate derivation

The aggregate boundary derives from the domain model's containment graph,
standing in for Sculptor's `belongsTo`/`!aggregateRoot` markers: a class B
is *contained* when some class A owns a containment feature (`contains`)
whose type resolves to B, transitively. An entity design whose class is
contained by another stereotyped entity's class is a non-root — it may not
declare a repository. The closure is computed only when every imported
domain lowered (a failed domain may hide containments, and a wrong
non-root verdict is worse than none).

## Domain operations vs service operations

Mox class `op`s and `.ddd` service operations are deliberately different
layers, and the split is the layering heuristic:

- **Behavior confined to one aggregate stays a class `op`.** It lives in
  the domain layer, is owned by the aggregate's class, and generates as an
  instance method regardless of the class's stereotype.
- **Behavior spanning aggregates, needing a transaction, or sitting at an
  entry point is the cue for a `.ddd` service operation.** The service
  operation delegates down to the aggregate-owned ops; it never
  re-implements them.
- **Authorization attaches at the service-operation/capability boundary**
  (the `.actor` dimension). A domain `op` is not an entry point, so ops
  never take `capability` clauses — guarding a domain method would bypass
  the service boundary the policy dimension is defined over.

A `dto`-designed class carrying `ops` is therefore an error under
validation rule 15 (one per op): a transport shape carries data between
layers, and behavior declared on it is almost always behavior that belongs
on the aggregate's class or on a service. A projection over a
behavior-rich source class stays legal — its boundary does not carry the
source's ops — and the escape hatch for behavior is the aggregate-owned
`op` or a service operation, never a body on a DTO. A body-less `op` on a
mirror DTO's class is an error too: the DTO *is* the class's shape.

**Child-scoped operations** (`extend`, rule 23) are the root-side slice of
the same heuristic: `boolean addLine(PayRunLineDraft line) => lines.add;`
declares, on the root's design, an operation that reaches the aggregate's
contained child through the root — no outside cross reference to a
non-root member (rule 13) is needed. The child's own behavior stays a
domain `op` on the child class; the `extend` operation is the root-owned
entry point over it, and the aggregate boundary stays intact.

## The lowering contract (issue #143)

Phero Lang aligns with [codegraph#590](https://github.com/magick93/codegraph):
**public service operations lower to capability-gated routes downstream** —
the generator emits one route per public operation, guarded by the
operation's declared capabilities, with server-side `when`/`never_both`
enforcement. Phero Lang's role is to make the surface the route is built
from *declarative*, which issues
[#176](https://github.com/yestechgroup/pherolang/issues/176) and
[#180](https://github.com/yestechgroup/pherolang/issues/180) complete:

- the declared `transition` (`TransitionRef`) is the lowering contract for
  the machine-bound transition a delegated `save` fires — the consumer
  lowers the declared trigger verbatim (state patch = the trigger's target
  state, conflicts via the machine guard) and pins it in its hurl/openapi
  contracts, instead of guessing from state names;
- the resolved condition paths (`conditionPaths`) are the lowering
  contract for the guard: injected-scope reads go through the injected
  repository on the handler's already-loaded context, own-record reads
  through the bound entity — a closed subset the refusal of which names
  the construct;
- the checked statement body (`bodies["stmt"]`, issue
  [#178](https://github.com/yestechgroup/pherolang/issues/178)) is the
  lowering contract for the operation's behavior: the consumer decodes the
  `StmtBody`, maps `self`/bare-name receivers through `bodyReceiver` and the
  service's injections, and lowers the typed expressions with the shared
  R1–R9 semantics. The lowering is the consumer's (codegraph), external by
  decision D1; no in-tree backend emits `.ddd` bodies, so the artifact is
  the contract, and the compiler never attaches an unchecked one.

Because the lowering is external, compiles add **no "lowers to nothing"
diagnostics**: the artifact is the contract, and the consumer owns the route
emission. A `protected` operation stays legal but off the public interface —
a capability or transition carried only by `protected` operations reads as
dead to the coverage pass, whose
warning-severity lint (`grantedCapabilityUncarried`, the
[pherolang#128](https://github.com/yestechgroup/pherolang/issues/128)
asymmetry) is the existing dead-trigger WARN; no new diagnostic bridges
the gap.

## Capabilities and the actor dimension

A service operation's `capability` clause names actor capabilities. Only
the library entry point `compile_ddd_str_with_actors` validates the names
against a caller-provided actor-model **slice**; capability names pool
across every model, so a host passes all of the application's `.actor`
artifacts. An empty slice is the plain `compile_ddd_str` behavior (the
records-unvalidated loose-mapping contract); `capabilityRefs` still records
each capability name, without `declaredIn`. Since pherolang#128 the CLI
project pipeline and the LSP pass the sibling artifacts, so the check is a
compile-time language error — spanned on the capability clause, with the
`unknownCapability` code carrying the did-you-mean closest declared name
(a case-variant match wins outright; otherwise the nearest name within an
edit budget). Nothing in the design artifact constructs policy:
authorization remains the `.actor` dimension's job (see the actor-policy
section of the language reference).

Both directions of the design↔actor wiring resolve at **compile time**
(pherolang#128): the CLI project pipeline and the LSP resolve every
capability clause against the pooled actor artifacts (existence, rule 10
above) and every grant against the pooled design artifacts (coverage —
`compile_actors_str_with_designs`, the actor engine's coverage pass
diagnosing `capability 'X' is granted but no operation or search carries
it` spanned on the grant entry, code `grantedCapabilityUncarried`). The
union integrity pass (`phero_driver::check_integrity`, register row
**V12**) keeps both findings as its **lint channel** (warning severity)
for hosts that gather artifacts without compiling — the compiler's
diagnostics are the errors; `pherolang check <dir>` prints the lint only
for files phase 1 checked clean. Checking a design or policy alone stays
per-surface and silent.

### Capability nouns on aggregate members (pherolang#144)

A capability is granted on a class (`capability X on <Class>`), and the
class's place in the aggregate machinery decides whether that noun is a
sound grant. With designs provided, `NounClassification`
(`crates/phero-driver/src/ddd/graph.rs`) — a pure table over the union model
and the designs, exactly rule 6's predicate — classifies every resolved
design class:

| verdict | class | a grant on it |
|---|---|---|
| `Value` | a value design — no aggregate to escape | clean |
| `Root` | an entity with a repository that no stereotyped entity contains | clean |
| `NonRoot` | an entity with a repository that another stereotyped entity's class contains (transitively) | **warning** |
| none | no design, a dto, or a repository-less entity | silent |

The warning is `capability '<X>' is granted on '<Class>', a non-root
aggregate member` (plus `of '<Root>'` and a "grant the capability on the
aggregate root instead" help when the containment chain tops out at exactly
one class), spanned on the class reference. Granting on a non-root member
hangs generation bindings (codegraph#596), so the warning points at the
root; an ambiguous chain names no root. `compile_actor_file` builds the
table only when designs are provided **and** every imported domain lowered
(`union_model`, the rule-6 discipline: a domain that failed to lower may
hide containment, and a wrong verdict is worse than none — its diagnostics
already block the artifact); an empty design slice, and the `None` table it
passes through `lower_actors`, is the exact plain `compile_actors_str`
behavior, byte for byte. The `.ddd` inline
`actors` block path always passes no table: actor lowering and the design
registries never co-occur there (domains lower through the shared
per-file union pipeline before designs exist, and
`compile_ddd_str_with_actors` receives pre-lowered `ActorModel`s).

## Events and the event-contract dimension

A service operation's `emits` clause names the events the operation
produces — the production-side counterpart of the `.evt` layer's
contracts, channels, and subscriptions (a fragment, in the position the
grammar fixes for it — after the `capability` clause, before the `when`
guard):

```
service PayRunService {
    boolean approve() capability ApprovePayRun emits PayRunStateChanged when (status == AwaitingApproval);
}
```

Like `capability` names, emitted event names are **loosely coupled**: the
names lower into the artifact exactly as authored, and a plain
`compile_ddd_str` never validates them — the `.ddd` compile has no `.evt`
bucket. The `emittedRefs` companion list records the same names, with
`contract` absent: the unresolved-by-design shape. Both directions of the
design↔event wiring are enforced by the union integrity pass
(`phero_driver::check_integrity`, register row **V12**):

- **Emission existence** — every emitted event name on every service
  operation must name an event declared in some provided `.evt` contract:
  `operation 'approve' of service 'PayRunService' emits unknown event
  'PayRunStateChanged'`, help listing the declared event names.
- **Emission coverage** — every declared event must be emitted by at
  least one service operation across the designs: `event 'E' is declared
  but no operation emits it`. Channel `publishes` entries and
  subscription `events [...]` lists are **consumption-side** wiring — they
  route events to consumers and never satisfy production.

Checking a design alone (or a contract alone) stays per-surface and
silent: an `emits` clause in a cluster without `.evt` artifacts is not an
error, and a contract in a cluster without designs is not either — the
checks fire only when the counterpart surface is present in the checked
union (`pherolang check <dir>`).

## Decisions and the decision dimension

A service declares which decision services it relies on without knowing
how the decisions are computed — the `.ddd` side of the `.dmn` decision
surface (issue pherolang#78). Two forms, in the positions the grammar fixes:

```
service LoanService {
    inject decision underwriting.RiskScore;
    boolean approve() capability ApproveLoan invokes RiskScore, FraudScreen when (status == AwaitingApproval);
}
```

An operation's `invokes` clause names the decision services the operation
invokes; `inject decision <qualified-name>;` names a decision service the
whole service injects. Like `capability` and `emits` names, decision
references are **loosely coupled** at the wire layer: they lower into the
artifact as their last segment exactly as the resolution universe spells
them, and a plain `compile_ddd_str` never validates them. Only the library
entry point `compile_ddd_str_with_decisions` validates the links (rule 17
above) against a caller-provided `(path, DmnModel)` bucket — the same
shape the `.uc` import buckets use. Decision services pool across every
provided model, so a host gathers the sibling `.dmn` artifacts the way it
pools actors. An empty bucket is the plain behavior. The `decisionRefs` and
`decisionDependencyRefs` companions exist for every reference; their
`contract` and `version` members fill only when a decision model was
supplied at compile time.

The `.dmn` dimension stays the authority: it declares the decision
services (with their inputs, outputs, and encapsulated decisions); the
design only records the dependency edges. Imports of `.dmn` files are not
domains — the design's `import "<path>.dmn"` declarations resolve the
decision bucket at compile time (rule 17's provision check) and lower
nothing into the domain union.

---

## CLI usage

```
pherolang check library.ddd                  # validate design + imported domains
pherolang ir library.ddd -o library.ddd.json # emit the DddModel artifact
pherolang fmt --check library.ddd            # canonical formatting gate
pherolang artifact check library.ddd.json    # validate the artifact's DDD contract
```

- Import paths resolve **relative to the `.ddd` file's directory**; each
  imported `.mox` is read from disk and compiled through the ordinary
  domain pipeline. A missing import file is a clean error naming the
  resolved path. An imported domain may itself declare `import schema` and
  `import sigil` — the CLI provides that content too, so designs target
  lowered JSON-Schema and Rune types like any other class (see
  [Importing Rune models](LANGUAGE.md#importing-rune-models)).
- Diagnostics come from every file involved and render grouped per file,
  the way `.actor` compiles render them.
- `pherolang ir` on a `.ddd` file emits only the design artifact; the domain
  IR is emitted by running `ir` on the `.mox` files themselves.
- `pherolang fmt` formats `.ddd` files with the canonical layout rules:
  imports hoisted into a tight first section (each ending in `;`), then
  the `application` block; `base` first, design flags canonicalized to
  `scaffold auditable optimisticLocking nonPersistent cache`, search
  members canonicalized to `entity text filters sort document ranking
  analyzer pagination capability` (clause contents keep their source
  order; document expressions are re-emitted verbatim), and every
  operation/`inject` line ending in `;`. An `extend <Class>` header
  expands like a service/design header, and each child operation is one
  canonical line — `boolean addLine(PayRunLineDraft line) => lines.add;`,
  with an optional leading `protected` and the trailing `when (...)` in
  the service-operation order.
- Directory batch mode (`pherolang check <dir>`) picks `.ddd` files up like
  every other surface: each design in the directory compiles independently,
  imports resolved relative to it.

---

## Sculptor concept mapping

Sculptor concepts mapped to `.ddd`, and where the mapping deliberately
differs or has no counterpart. The third column is the summary; the
rationale for each divergence is in [Deliberate divergences](#deliberate-divergences).

| Sculptor | Phero Lang `.ddd` | Deliberately different / absent |
|---|---|---|
| Entity | `entity` design | — |
| ValueObject | `value` design — strictly immutable: no identity, immutable, deep (nested values and collections), structural equality over stored features (D3) | **no `!immutable` opt-out** (strictly forbidden); no `id`-bearing feature, no entity `contains`, no entity `refers` — refer by id instead (`refers String personId`) |
| BasicType | `.mox` datatype (referenced in signatures, never designed) | a datatype wraps one platform type; a multi-field value is a class designed `value` |
| Repository | `repository` block on an entity design | six built-in operations (`findById`, `findAll`, `findByExample`, `findByKeys`, `save`, `delete`) plus declared signatures |
| Service (+ injected `=>` delegation) | `service` with `inject` lines and `name => target.operation` operations | a cross-module service → repository/search delegation is an error; targets carry a resolved identity (`targetRef`) |
| `scaffold` / `auditable` / `optimisticLocking` / `!aggregateRoot`-style negations | design flags (entity-only; `auditable`/`optimisticLocking` default on for entities, `!flag` opts out) | `!immutable` is absent (D3) — not an opt-out but a refused spelling; the artifact stores the effective flag values, never the raw spelling |
| `belongsTo` / `!aggregateRoot` | derived from the domain model's containment graph (see aggregate derivation) | not authorable: derivation only (DD4 adds an authored override) |
| Module | `module` label grouping services and designs | one application per file; the module is a design label, not a deployment boundary (DD8) |
| child-scoped aggregate operations | `extend <Root> { ... }` block (issue [#179](https://github.com/yestechgroup/pherolang/issues/179)) | `extend` is not class inheritance (naming under review in DD4) |
| DTO | `dto` design in three forms: mirror (`dto User`), projection (`dto BookSummary from Book { ... }`), standalone (`dto CreateBookRequest { ... }`) | real parity, shaped differently: a projection selects (`name;`), renames a path (`name = path;`), or derives (`name = expr;`, source class as `self`); no identity, aggregate membership, repository, machine, invariants, or behavior; `input`/`output` is derived from the member set, and an output boundary is not a parameter; a standalone DTO has no domain class |
| `findByQuery` / `findByCondition` finders | `search` projection (intent only: text fields with boosts, filters, sorts, ranking, pagination, document projections) — the backend chooses the engine (Postgres FTS, Tantivy, OpenSearch, ...) | no query language and no engine-specific ordering; the engine is the consumer's choice |
| generated `GET /things/search?q=` endpoints | consumer concern — the artifact is backend-independent | — |
| `Set` / `List` collections | `.mox` collection kinds on a multiplicity: `T[]` is an ordered list, `T[] { unique }` an ordered distinct list, `T[] { set }` an unordered distinct set (DD6); an operation **signature** carries only the multiplicity | no collection kind on a signature slot: the kind belongs to the feature, and a signature is the callee's contract. `Collection<T>`/`List<T>`/`Set<T>` are spelled by multiplicity + collection constraint, never as generic types |
| `Bag` / `ordered` collections | — | absent: a multiset is a storage need, and declaration/insertion order is the only semantic order (DD6) |
| `Map` | — | deliberate non-goal (DD6): no JSON Schema map type, unspecified key order, and no in-tree backend need. Workaround: a pair class + `contains` when the map is a domain concept, an opaque datatype wrapping a platform map for an implementation detail, or `refers` to an entity keyed by the map key |
| `fetch` / `cascade` / `orderby` / `orderColumn` | — | absent: persistence behavior belongs to the backend that owns the storage |
| Traits | interface `extends` in the `.mox` domain model (signature-sharing half) | absent as a composition feature |
| Gap classes | body-less `.mox` `op` as the hand-written hook | absent: whole-file generation, no generated-base/subclass split |
| Composite natural keys | several `id` features on one class | no composite-key declaration; the `id` set is the key |
| Date validation (`past` / `future`), custom validators | schema constraints (`unique` precedent) | absent: no runtime validator hook; constraints are schema-only |

## Deliberate divergences

Sculptor concepts Phero Lang intentionally does not carry, and why. The
mapping table's third column is the summary; this section is the detailed
rationale.

- **`!immutable` (mutable value objects)** — Sculptor lets a ValueObject
  opt out of immutability. Phero Lang's `value` means strict value
  semantics instead (rule 12, issue
  [#299](https://github.com/yestechgroup/pherolang/issues/299)): a class
  that must mutate is an `entity`, and a value that needs to name an
  entity refers by id (`refers String personId`) rather than holding a
  mutable handle. A mutable value is not a modeling style but a
  contradiction — structural equality over state that can change is not
  equality.
- **Composite natural keys** — expressible already: several `id` features on
  one class lower with `isId`, and the composite key is that set; the
  Tier-1 backends give the group its meaning (docs/LANGUAGE.md keeps `id`
  "carried; semantics land with Tier 1").
- **Collection size constraints (`size`, `notEmpty`)** — expressible as the
  multiplicity annotation (`String[1..10] tags`), which the JSON Schema
  backend already lowers to `minItems`/`maxItems`; a constraint-block
  spelling would duplicate it.
- **Collection kinds in signatures (`Set` / `List` / `Bag` / `Map`)** — the
  `.mox` domain model carries the semantic collection kind on the *feature*
  (`T[]` list, `T[] { unique }` ordered distinct list, `T[] { set }`
  unordered distinct set — DD6, issue
  [#303](https://github.com/yestechgroup/pherolang/issues/303)); the `.ddd`
  signature grammar carries multiplicity only. A signature's collection kind
  is the callee's contract: if a set is needed in a return, return a
  DTO/value with a set feature rather than growing the signature grammar.
  `Bag` is out (a multiset is a storage need), and `Map` is a deliberate
  non-goal: JSON Schema has no map type, key ordering is unspecified, and no
  in-tree backend needs one — model a domain map as a pair class plus
  `contains`, an implementation detail as an opaque datatype, or a keyed
  entity lookup as `refers` (see `docs/LANGUAGE.md`, *Collections*).
- **Persistence ordering (`orderby` / `orderColumn`)** — JPA
  `@OrderBy`/`@OrderColumn` metadata, a storage knob, not a domain fact. A
  list's order is declaration/insertion order; a user-controlled order is an
  explicit ordinal feature (`int position`) plus a list, and a set has no
  semantic order. A backend that needs a different storage ordering derives
  it from its own configuration, not from the artifact.
- **Fetch / cascade** — JPA persistence knobs with aggregate-derived
  defaults in Sculptor's generator; the artifact is backend-independent,
  so persistence behavior belongs to the backend that owns the storage.
- **Date validation (`past` / `future`) and custom validators** — runtime
  validation semantics; Phero Lang's constraint system is deliberately
  schema-only (the `unique` precedent), and there is no runtime validator
  hook to hang them on.
- **Gap classes** — Sculptor's generated-base/handwritten-subclass split is
  a generation strategy; Phero Lang generates whole files, and a body-less
  `op` stays an abstract hook implemented by hand in target code — the
  same separation without a second class.
- **Traits** — a mixin composition feature of Sculptor's Java generation;
  no Phero Lang surface carries it yet. If a need lands, it belongs in the
  `.mox` domain model (interface `extends` covers the signature-sharing
  half today).
- **Composite value types (BasicType)** — Sculptor's `BasicType` is a
  multi-attribute embeddable (`Money{currency, amount}`); Phero Lang
  datatypes wrap a single platform type opaquely. Model the composite as
  a class designed `value` (rule 12 then requires its features
  `readonly`); a true embeddable datatype awaits a deliberate `.mox`
  language extension.
- **Multi-target arrows (`A -> B, C`)** — deliberately excluded from the
  machine surface: an unguarded external choice is not a deterministic
  machine; write one edge per transition.
- **Transition entry/exit actions and timers** — v1 transitions carry only
  trigger, guard, and target; domains.toml approval chains and timers are
  downstream concerns.
- **`.actor` guards binding machine states** — the actor dimension cannot
  yet reference a design's machine states cross-file (a fast-follow);
  today only `.ddd`-internal conditions bind them.

---

## Worked example

`tests/conformance/ddd/library.ddd`, the canonical fixture, designs a
library application over `tests/conformance/ddd/library.mox`. The fence
below is the fixture verbatim, its import repointed so it compiles from
this document (the docs fences are compiled by CI):

```ddd
import "../tests/conformance/ddd/library.mox";

application Library {
    base nz.example.library
    module media {
        /// Coordinates lending of physical media across branches.
        service MediaService {
            inject PhysicalMediaRepository;
            inject MovieRepository;
            inject PersonService;
            inject MediaSearch;
            boolean borrow(String mediaId, Person borrower) capability BorrowMedia;
            PhysicalMedia registerMedia(MediaStatus status);
            protected save => PhysicalMediaRepository.save;
            registerBorrower => PersonService.register;
            findMedia => MediaSearch.search;
        }
        entity Library scaffold cache repository LibraryRepository {
            findById;
            findAll;
            save;
            delete;
            boolean renameLibrary(String name);
        }
        entity Book
        abstract entity Media
        entity Chapter auditable
        entity Movie repository MovieRepository {
            findById;
            findAll;
            save;
            delete;
            Movie findByTitle(String title);
        }
        entity PhysicalMedia scaffold repository PhysicalMediaRepository {
            findById;
            findAll;
            save;
            delete;
            PhysicalMedia findByStatus(MediaStatus status);
        }
        /// Full-text search over the movie catalogue.
        search MediaSearch {
            entity Movie
            text {
                title boost 3
                synopsis analyzer "english"
            }
            filters {
                genre
            }
            sort {
                title
            }
            document {
                headline = title;
                blurb = title + " - " + synopsis;
            }
            ranking bm25
            analyzer "english"
            pagination {
                limit 20
                max 100
                cursor
            }
            capability SearchMedia, SearchMediaAdmin
        }
    }
    module person {
        /// Keeps the member register current.
        service PersonService {
            inject PersonRepository;
            register => PersonRepository.save;
            findByName => PersonRepository.findByName;
        }
        entity Person repository PersonRepository {
            findById;
            findAll;
            findByExample;
            findByKeys;
            save;
            delete;
            protected Person findByName(String fullName);
        }
        search MemberDirectory {
            entity Person
            text {
                fullName boost 2
            }
            filters {
                joinedOn
            }
            sort {
                fullName
                joinedOn
            }
            document {
                displayName = fullName;
            }
            ranking tfIdf
            pagination {
                limit 10
            }
        }
    }
}
```

Notes on the example: `Chapter` declares no repository because it is
contained by `Book` (a non-root aggregate, rule 6); `Book` and `Media`
design the inheritance pair the domain declares; `MediaService.save`
delegates to a repository of its own module while `registerBorrower`
delegates across modules to `PersonService` (service → service, allowed).

---

## Conformance

`tests/conformance/ddd/` holds the canonical fixture set: `library.mox`
(the domain), `library.ddd` (the design, formatter-canonical), and
`library.ddd.json` (the committed artifact). The driver golden test
byte-compares the compiled artifact, the CLI fmt gate enforces the
canonical formatting, and `pherolang artifact check` validates the artifact
against the DDD contract — resolved companions, aligned parallel lists,
search references, stereotype/flag legality, value `semantics`, DTO
`dtoShape`s and `DddTypeRef` references, and provenance — not just its wire
shape. Regenerate goldens with
`PHERO_UPDATE_FIXTURES=1 cargo test --workspace`.

The machine-bearing pair `payrun.mox`/`payrun.ddd` (formatter-canonical)
commits two goldens: `payrun.ddd.json` (the design artifact) and
`payrun.xstate.json` (the lowered xstate-ir machine), byte-compared by
`crates/phero-driver/tests/ddd_machine_golden.rs`; the machine validation
rules are pinned by `crates/phero-driver/tests/ddd_machines.rs`.

The multi-machine pair `timesheet.mox`/`timesheet.ddd` (formatter-canonical)
is the transition-binding and condition-scoping fixture (issues
pherolang#176/#180): two machine-bearing designs in one module, qualified
transition bindings, an injected-repository SoD guard, and the enum
-derived own-record spelling. Its committed golden `timesheet.ddd.json`
carries the `transition` and `conditionPaths` fields; the acceptance rules
are pinned by `crates/phero-driver/tests/ddd_transitions.rs` and
`crates/phero-driver/tests/ddd_conditions.rs`.

The child-operation pair `childops.mox`/`childops.ddd` (formatter-canonical)
is the pherolang#179 fixture: a `PayRun` aggregate root with a contained
`PayRunLine[] lines` collection and a single-valued `PayRunNote` child, a
`dto PayRunLineDraft` parameter type, and an `extend PayRun` block with
`add`/`remove`/`save` child operations (one guarded by a parameter field and
one by a child feature). Its committed golden `childops.ddd.json` carries
the design's `childOperations` with each operation's `childScope`, and the
mirror DTO's `dtoShape` (its source class, `input` direction, and selected
members); the acceptance rules are pinned by
`crates/phero-driver/tests/ddd_child_ops.rs`.

The DTO forms, member typing, direction derivation, name rules, and the DTO
error surface (duplicate/unknown/mistyped members, entity members, name
collisions, non-input parameters, entity-only constructs) are pinned by
`crates/phero-driver/tests/ddd/dto_semantics.rs`; the dto-with-ops error is
pinned by `crates/phero-driver/tests/ddd/rule_15_dto_ops.rs`. The artifact
contract refuses a dto design without `dtoShape`, a `dtoShape` on any other
stereotype, malformed or duplicate members, a DTO reference that names no
boundary in the artifact, and an output DTO in a parameter slot — from the
serialized artifact alone.

The executable-body pair `bodies.mox`/`bodies.ddd` (formatter-canonical)
is the pherolang#178/#304 fixture: a void service body over the full
statement subset (let-else, guard, assignment, `sum`/`size`/`any`/`all`
reductions, `self.findById`/`self.save`, a bare-name injected-repository
call, and a domain `op` call on the loaded aggregate), a value-returning
`if`/`else` body with `capability`/`when` clauses, a repository-operation
body guarded by `when`, and an injected secondary repository. Its committed
golden `bodies.ddd.json` carries each operation's `bodies` `"stmt"` target,
the service bodies' `bodyReceiver`, the application `errors`, and every
operation's resolved `throws`; the acceptance and failure rules are pinned
by `crates/phero-driver/tests/ddd_bodies.rs`.

The declared-error pair `errors.mox`/`errors.ddd` (formatter-canonical) is
the DD7 fixture (issue pherolang#304): an application error with a typed
enum payload, a module-private error, a DTO payload, a body raising the
prelude and a declared error, a domain `op` call on a loaded local, and a
delegation whose `throws` is a superset of its target's. Its committed
golden `errors.ddd.json` carries the root and module `errors` lists and the
resolved `throws` references (application, module, and prelude scopes); the
negative matrix — unknown error, duplicate/prelude-shadowed declaration,
foreign private error, entity payload, duplicate field, narrowed delegation,
undeclared body raise, payload mismatch, and the orchestration-only
exclusions — is pinned by `crates/phero-driver/tests/ddd/errors.rs` and
`crates/phero-driver/tests/ddd_bodies.rs`.

The `examples/` side ships `support.ddd` (entity/value/dto designs,
searches, capabilities validated against `support.actor` through
`compile_ddd_str_with_actors`) and `orders.ddd` (the aggregate-derivation
rule over `ecommerce.mox`), byte-pinned against `examples/artifacts/` by
`crates/phero-driver/tests/examples_artifacts.rs`. Its `RefundDecision` is
the strict-value example: `decisionId` is stored state, not an `id`, and
the design carries the `semantics` block. Note that
`examples/library/library.mox` itself is *not* designable: its `Writer.books`
cross reference escapes the Library aggregate (rule 13).

The value rules (rule 12's R-V1 to R-V3) are pinned by the driver's
negative-compile matrix, one case per rule: no in-tree backend consumes
`.ddd`, so the design-aware generator is codegraph's, external by D1, and
the in-tree proof is the rule matrix plus the artifact contract.
