# IFML DSL

> **Learning the language?** Start with the [interaction flows
> guide](guide/ifml/overview.md): overview, tutorial, how-tos, and reference,
> written for people using the language. This page is the engineering
> contract.

## Overview

A Domain-Specific Language that is semantically aligned with [OMG IFML 1.0](https://www.omg.org/spec/IFML/) but uses a modern, C-like expression-heavy syntax instead of XML/XMI. Lexed by a **logos** lexer and parsed by **chumsky** into a spanned syntax tree, then lowered into a versioned `IfmlModel` artifact; the same parser and byte-offset spans back the language server.

The IFML DSL lives alongside the `.mox` domain model as a *complementary* input: the domain model defines the data (entities, fields, constraints), the IFML DSL defines the interaction model (views, navigation, events, data binding). The parser crate is **`phero-ifml`**; the artifact types live in **`phero_ir::ifml`** (the phero-ir crate). Downstream consumers — today the [codegraph](https://github.com/magick93/codegraph) UI generators — ingest the artifact and emit running applications.

---

## Artifact wire format

`IfmlModel` is a standalone, versioned wire artifact following the phero-ir [wire format contract](BACKENDS.md) (see also the `phero_ir::ifml` module docs):

- Versioned independently of the domain-model marker via **`IFML_MODEL_FORMAT_VERSION`** (= 1), the `ActorModel` precedent. `IfmlModel::from_json` rejects absent/wrong versions with `IrError::UnsupportedFormatVersion`.
- All struct field names serialize as `camelCase` (`formatVersion`, `eventType`, `moduleUses`, ...).
- Enums with payloads are *adjacently* tagged: `{"type": "<variant>", "value": <payload>}` (tags camelCase: `"navigate"`, `"stringLit"`, `"fieldExpr"`, `"bar"`). Unit-only enums serialize as bare camelCase strings (`"eq"`, `"regexMatch"`).
- Unknown fields are ignored on deserialize; `PropertyAssignment::span` is parse-only and never serialized.

Minimal artifact: an empty model serializes as exactly `{"formatVersion":2}` — every struct `Vec` field is `#[serde(default, skip_serializing_if = "Vec::is_empty")]` (the `ActorModel`/`DddModel` discipline), so empty vecs are omitted on write and absent vecs read back as empty.

`pherolang artifact check` classifies a non-empty IFML artifact by its
`views`/`domains`/`actions`/`commands`/`imports` keys (rule 3 of the
classifier's precedence list — ahead of the DDD `modules` shape, so a
view-and-module model classifies as an IfmlModel). Two shape-sniffing
limitations are documented and pinned: the **empty** IFML model
serializes as exactly `{"formatVersion":2}` and classifies as an
ActorModel, and a **module-only** model (no other IFML key) shape-matches
the DDD design keys and classifies as a DddModel (see the classification
rules in the CLI's `artifact_check` module).

```rust
let model = phero_ifml::parse_ifml(source)?;
let json = model.to_json_pretty()?;      // versioned artifact
let model = IfmlModel::from_json(&json)?; // version-gated reload
```

---

## Philosophy

1. **Semantic identity with IFML** — every IFML concept has a direct DSL equivalent. No lossy mapping.
2. **C-like expression syntax** — conditions, filters, parameter expressions use familiar infix/prefix syntax.
3. **Composable blocks** — view containers nest, components nest, events nest inside components.
4. **Schema-aware** — entity/field references are validated against the loaded domain model.
5. **Text-first, diagram-aligned** — the text representation is the source of truth; diagrams are derived views.

---

## Syntax Examples

### Minimal CRUD

```ifml
domain "sales" {
    schema "sales";
}

view "CustomerList" {
    component "grid" {
        type: list;
        data: Customer;
        fields: [name, email, phone, status];

        on select(row) -> navigate("CustomerDetail", {
            customerId: row.id
        });
    }
}

view "CustomerDetail" {
    params { customerId: Uuid };

    component "info" {
        type: details;
        data: Customer;

        on edit -> navigate("CustomerEdit", {
            customerId: params.customerId
        });
    }
}

view "CustomerEdit" {
    params { customerId: Uuid };

    component "form" {
        type: form;
        data: Customer;
        mode: edit;

        on save(values) -> action("UpdateCustomer", {
            body: values;
            on success -> navigate("CustomerDetail", {
                customerId: params.customerId
            });
            on error -> stay;
        });

        on cancel -> navigate("CustomerDetail");
    }
}
```

### Multi-Step Wizard (conditional navigation)

```ifml
view "WizardPage" {
    xor: true;

    container "Step1" {
        default: true;

        component "personalInfo" {
            type: form;
            data: Customer;
            fields: [name, email, phone];

            on submit(values) -> navigate("Step2", {
                name: values.name,
                email: values.email
            });
        }
    }

    container "Step2" {
        component "addressInfo" {
            type: form;
            data: Address;

            on submit(values) -> navigate("Step3", {
                street: values.street,
                city: values.city
            });
        }
    }

    container "Step3" {
        component "review" {
            type: details;
            data: CustomerReview;

            on confirm -> action("CreateCustomer");

            on back -> navigate("Step2");
        }
    }
}
```

### Dashboard with Data Flows

```ifml
view "Dashboard" {
    on load -> refresh("recentOrders");

    component "recentOrders" {
        type: list;
        data: Order;
        fields: [id, customerName, total, date];
        filter: date == today();
    }

    component "topProducts" {
        type: list;
        data: Product;
        fields: [name, salesCount];
        filter: salesCount > 1000;
        sort: "salesCount desc";
    }
}
```

### Search → Results → Detail

```ifml
view "SearchPage" {
    component "searchForm" {
        type: form;
        data: ProductSearchQuery;

        on submit(values) -> navigate("SearchPage", {
            query: values.term
        });
    }

    component "results" {
        type: list;
        data: Product;
        fields: [name, sku, price];
        filter: name ~= params.query;

        on select(row) -> navigate("ProductDetail", {
            productId: row.id
        });
    }
}
```

### Modal Dialog

```ifml
view "DeleteConfirm" {
    modal: true;

    component "confirmForm" {
        type: form;
        data: ConfirmDelete;

        on submit(values) -> action("DeleteProduct", {
            on success -> navigate("ProductList");
            on error -> stay;
        });

        on cancel -> navigate("ProductList");
    }
}
```

### Charts

Components of `type: chart` carry an inline `chart_decl`. The kind
vocabulary is closed — `bar | line | pie | radar | metric | org` — and an
unknown kind is a parse error.

```ifml
view "Analytics" {
    component "revenueByRegion" {
        type: chart;
        chart bar {
            label: region;
            values: [revenue];
            aggregate: sum;      // sum | avg | count | min | max
        }
    }

    component "team" {
        chart org {
            hierarchy: managerId;   // self-referencing FK to walk
            labels: [name, title];  // ordered node label projections
            details: [email];       // ordered node detail projections
            maxDepth: 4;            // client-side render pruning
        }
    }

    component "modeled" {
        // Placement of a chart modeled elsewhere (e.g. codegraph's
        // `[charts]` table): the string is a permissive reference the
        // consumer resolves against its own graph — the artifact never
        // does.
        chart: "headcount";
    }
}
```

Series kinds aggregate their `value_fields`; an absent `aggregate:` stays
unspecified in the artifact (consumers apply their own default, e.g.
codegraph models `count`). An org chart uses `hierarchy`/`labels`/
`details`/`maxDepth` instead of `label`/`values`. A component cannot carry
both a `chart: "<name>"` reference and an inline chart declaration — that
combination is a parse error naming the component.

### CLI Commands

A command-line application is the same interaction model in a different
interaction vocabulary. Top-level `command` declarations — peers of
`view`/`action`/`module`/`actor` — describe the terminal surface and bind
to the same domain types and actions the views bind to; subcommands nest
to the invocation path (`tickets show TICKET-1`).

```ifml
command "tickets" {
    description: "Work the support ticket queue";
    requires: [Agent];

    option state: TicketState = "open";
    option limit: Int = 20;

    output: table;

    on run -> action("ListTickets");

    command "show" {
        argument ticketNo: String;

        output: details;

        on run -> action("GetTicket");
    }

    command "refund" {
        argument ticketNo: String;

        prompt amount: number;
        prompt confirm: confirm = false;

        on run if amount > 0 -> action("RefundTicket", {
            ticketNo: ticketNo;
        });
    }
}
```

Body vocabulary:

| Member | Meaning |
|---|---|
| `argument <name>: <Type>;` | Positional input, required by nature (what is optional is an option). |
| `option <name>: <Type> (= default)?;` | Invocation-line input, optional by nature. |
| `prompt <name>: <kind> (= default)?;` | Interactive input collected while the command runs — kinds: `text`, `secret`, `number`, `confirm`, `select`, `multiselect`, `date`, or a custom identifier. |
| `on <event> (if expr)? -> …` | The shared event-handler vocabulary; `on run -> action("X", { … })` binds the command to a domain action. |
| `use "Module" …` | Resolved and validated like any other use site (expansion keeps them as references — a command has no containers/components to splice into). |
| nested `command` | Subcommands, to any depth. |
| properties (`description:`, `output:`, …) | Free property bag; `description` (string) and `requires` (identifier list) are extracted into dedicated artifact fields, the view discipline. |

Checking (`check_ifml`) treats a command's typed arguments and options as
the bindings for its `if` guards and event expressions — domain types
included, so `if agent.active` navigates `User`'s features. Generic
identifier types and prompt names are deferred, not errors; option and
prompt defaults get the structural literal check (the resolver's rule).
The example `examples/support/cli.ifml` is the reference consumer; the deferred
CLI ideas (modality profiles, CLI pattern modules, doc/completion
generation, TUI constructs) are tracked in the follow-up issue.

---

## Grammar

**The normative grammar is the EBNF in the parser module docs:**
[`crates/phero-ifml/src/parser/mod.rs`](../crates/phero-ifml/src/parser/mod.rs).
That module is the single grammar authority for the surface — the
grammar-doc tests fail when a word table and the EBNF drift — so this page
does not restate it. What follows is a non-normative orientation:

- **Entry point** — a file is an import/domain prologue followed by `view`,
  `action`, `module`, `actor`, and `command` declarations.
- **Views and containers** — one body shape: optional `params` and `label`,
  property assignments, then nested containers, components, event handlers,
  conditions, and module uses.
- **Components** — properties first, then typed members (`column`, `field`,
  `chart`, conditions), then event handlers; chart kinds are a closed
  vocabulary (see [Charts](#charts)).
- **Events** — `on <type> (params)? (requires: [...])? (if expr)? -> <action>;`
  with navigate, refresh, action invocation, or stay.
- **Actions and modules** — `action` (properties + events) and `module`
  (required `input`/`output` parameter blocks; module `use` composes other
  declared modules).
- **Expressions** — the C-like precedence chain `||` → `&&` → comparisons →
  `+ -` → `* / %` → unary → primary, plus value-position arrays and objects.
- **Lexical rules** — identifiers are `[A-Za-z][A-Za-z0-9_]*`; numbers are
  unsigned tokens; strings carry the JSON escape set; only `//` comments
  exist. Words are never keywords: every word is an identifier and is
  special only in its grammar position.

---

## Artifact Types (Rust)

The chumsky parser produces the spanned syntax tree in `ast.rs`; `lower.rs` performs the one traversal that emits both the `phero_ir::ifml` types and the span index. The root artifact:

```rust
pub struct IfmlModel {
    pub format_version: u32,   // always IFML_MODEL_FORMAT_VERSION (= 1)
    pub domains: Vec<DomainDeclaration>,
    pub views: Vec<ViewDeclaration>,
    pub actions: Vec<ActionDeclaration>,
    pub modules: Vec<ModuleDeclaration>,
    pub actors: Vec<ActorDeclaration>,
    pub commands: Vec<CommandDeclaration>,   // CLI surface; subcommands nest
    pub imports: Vec<String>,
}
```

Key element types (all camelCase on the wire, enums `{"type": ..., "value": ...}`-tagged):

```rust
pub struct ViewDeclaration {
    pub name: String,
    pub label: Option<String>,
    pub is_landmark: bool,
    pub is_xor: bool,
    pub is_modal: bool,
    pub params: Vec<ParameterDecl>,
    pub properties: Vec<PropertyAssignment>,
    pub containers: Vec<ContainerDeclaration>,
    pub components: Vec<ComponentDeclaration>,
    pub events: Vec<EventHandler>,
    pub module_uses: Vec<ModuleUse>,
    pub roles: Vec<String>,      // extracted from the property bag
    pub requires: Vec<String>,   // capability requirements
    pub condition: Option<Expression>,
    pub position: Option<Position>,
}

pub enum EventAction {
    Navigate { target: String, binding: Option<ParameterBinding> },
    Refresh { target: String, binding: Option<ParameterBinding> },
    ActionInvocation { name: String, body: Option<ActionBody> },
    Stay,
}

pub enum Expression {
    Ident(String),
    StringLit(String),
    NumLit(f64),
    BoolLit(bool),
    FieldExpr { object: Box<Expression>, field: String },
    BinOp { left: Box<Expression>, op: BinOp, right: Box<Expression> },
    UnaryOp { op: UnaryOp, operand: Box<Expression> },
    Group(Box<Expression>),
    Call { name: String, args: Vec<Expression> },
}

pub enum BinOp { Eq, Ne, Lt, Le, Gt, Ge, RegexMatch, NegRegex, Add, Sub, Mul, Div, Mod, And, Or }
pub enum UnaryOp { Not, Neg }
```

Chart components carry a `ComponentSpec::Chart(ChartSpec)` (inline) or
`ComponentSpec::ChartRef(String)` (modeled-chart placement). `ChartSpec`
holds the closed `ChartKind` vocabulary (`bar | line | pie | radar | metric
| org`), the series pair (`labelField`/`valueFields`), the org payload
(`hierarchyField`, `labelFields`, `detailFields`, `maxDepth`), and
`aggregate: Option<ChartAggregate>` (`sum | avg | count | min | max`).
Every payload field beyond `kind` is `#[serde(default)]` +
`skip_serializing_if` — charts that do not use the new vocabulary serialize
byte-identically to the pre-#80 shape.

`phero_ifml::render_expression(&expr)` renders an expression back to its deterministic DSL source form.

---

## Downstream Consumption

The Phero Lang language server supports the surface: diagnostics (resolver
`E0001`–`E0009` + typed-binding `E0100`–`E0111`, authorization and
decision data included —
the code table is
[docs/DIAGNOSTICS.md](DIAGNOSTICS.md), rendered by `pherolang explain`),
document symbols, hover
(module signatures), go-to-definition on `use` sites (cross-file), semantic
tokens (`textDocument/semanticTokens/full`), and
`textDocument/formatting` backed by the canonical formatter. Semantic tokens
come from the parse-only highlighting path
(`parse_ifml_syntax_recovering`): they need no domain union or import
resolution, so a flow highlights while its imports or bindings are
unresolved. Imports fetch from open editor buffers first, disk second.

The `IfmlModel` artifact is the integration boundary. Consumers `parse_ifml_file` (or deserialize a serialized artifact with `IfmlModel::from_json`) and walk the model to build applications. codegraph, the primary consumer today, projects the artifact into its graph (ViewContainer/ViewComponent/Event/Action/ParameterDefinition/DataBinding/ModuleDefinition nodes, NavigationFlow/DataFlow/HasParameter/... edges) and generates behavior-wired UI plus Playwright tests from it.

### Expression Evaluation

C-like expressions appear in:

| Context | Example | Evaluation |
|---|---|---|
| Filter conditions | `filter: status == "active"` | Target-language predicate |
| Parameter bindings | `navigate("Detail", { id: row.id })` | Route parameter mapping |
| Conditional navigation | `if amount > 10000 -> navigate("Review")` | Branching logic in page controller |
| Default values | `sort: createdAt desc` | Query ordering |

Expression evaluation is **deferred to generation time** — the artifact stores the expression tree, and each consumer translates it to the target language (SQL, Svelte, Rust, etc.). `render_expression` provides a canonical source-form rendering.

---

## Crate Structure

```
crates/phero-ifml/
├── Cargo.toml
├── src/
│   ├── lib.rs            # Public API: parse_ifml, parse_ifml_file, parse_ifml_indexed, compile_ifml_str, check_ifml(+_with_actors/+_with_decisions), expand_ifml, manifest_ifml, format_ifml + re-exports of phero_ir::ifml
│   ├── ast.rs            # Spanned syntax tree (file → declarations/members/expressions)
│   ├── lexer.rs          # logos lexer: no keyword tokens, total error tokens
│   ├── parser/           # chumsky parsers; mod.rs carries the normative EBNF
│   ├── lower.rs          # AST → (IfmlModel, IfmlIndex) in one traversal
│   ├── index.rs          # Span side-table (IfmlIndex): module decls/uses, views, containers, actions, actors, components, commands, auth sites — never serialized
│   ├── resolve.rs        # Cross-file module resolution: IfmlImports bundle, compile_ifml_str (E0001–E0009, the shared Diagnostic record)
│   ├── check/           # Typed expression checking against a domain Model, via phero_expr::DomainTypes; interaction cross-reference, authorization, decision, and lint passes over instantiated-module sites (code table: docs/DIAGNOSTICS.md)
│   ├── expand.rs         # Module-use expansion (opt-in for the IR artifact; the manifest always derives from it)
│   ├── manifest.rs       # The control/route manifest emitter: IfmlCompilation (+ imports, + optional domain union) → phero_ir::ifml_manifest::IfmlManifest
│   └── fmt.rs            # Token-based canonical formatter (the lexer drops comments)
└── tests/
    ├── golden.rs         # Conformance goldens — IR artifacts and manifests (PHERO_UPDATE_FIXTURES=1 regenerates)
    ├── manifest.rs       # The manifest emitter's derivation contract
    ├── resolve.rs        # Resolver conformance tests
    ├── seams.rs          # The shared diagnostic/import/surface-identity seams (SK6)
    ├── patterns.rs       # patterns/ifml library compiles clean; composition + negative cases
    ├── recovery.rs       # Lenient recovery contract: partial model/index + span-accurate E_PARSE
    ├── proptest_recovery.rs # Arbitrary editor text never panics and always terminates
    └── examples.rs       # examples/*/*.ifml compile, type-check, are fmt-canonical, and manifest byte-identically
```

### Module Resolution

`compile_ifml_str(path, text, &imports)` compiles a file plus its `.ifml`
imports in one call. Imports resolve through the `IfmlImports` bundle
(keyed `(importing path, import path)`, provided by the host — the resolver
never touches the filesystem); imports not ending in `.ifml` pass through
untouched. Local modules shadow imported ones; one name exported by two
distinct imported files is ambiguous. Every `use` in the main file — in
views, containers, and module bodies — is resolved and its overrides
validated against the target module's inputs and properties. Results live
in the `IfmlIndex` side-table (`ModuleUseSite::resolved_module`); the
returned `IfmlModel` is the main file's own parse, unchanged, so wire
artifacts stay byte-identical. See `crates/phero-ifml/src/resolve.rs` for the
diagnostic-code table.

Goldens live next to the other conformance fixtures: `tests/conformance/ifml/app.ifml` (canonical source) and `tests/conformance/ifml/app.ifml.json` (committed artifact).

### Typed Bindings

`check_ifml(&compilation, Option<&domain_model>)` type-checks every
type-checkable expression in a compiled model against a domain model —
the union `phero_ir::Model` every domain surface lowers into (`.mox`
declarations, `import sigil` packages, `import schema` classes). It
reuses `phero_expr`'s `DomainTypes`/`TypeChecker` — the same checker that
validates `.mox` operation bodies — so navigation, operations, and the
typing rules of `docs/EXPRESSIONS.md` behave identically.

What is checked, and with which bindings:

| Context | Bindings in scope |
|---|---|
| View/container `if` conditions | the view's `params` |
| Component `filter:` / condition properties | view params + the `data:` entity's features (`with_self`); no self when the component binds a decision service (decision outputs are not typed — see [Decision Data](#decision-data)) |
| Table `column "L" -> expr …` | same as its component; deferred when the table declares no `data:` (no row type to bind — the explicit `Class.feature` column spelling is a generation-time shape) |
| Event guards | same, minus event parameters (untyped — expressions naming them are skipped) |
| Navigation binding pairs | same |
| `use` override expressions | the enclosing view's params; expected type from the module input's `type_ref` |

Every body that declares parameters binds them two ways: as bare idents
(`customerId`) and through the **`params` record** (`params.customerId`) —
the docs-promoted spelling (`navigate("CustomerEdit", { customerId:
params.customerId })`) type-checks: the record is a synthetic class whose
features are the body's typed params (a checking device, not a domain
type; the next parameterized body overwrites it). A param whose type maps
to nothing gets no feature (`params.<field>` defers like the bare ident),
and a param literally named `params` shadows the record — the bare
binding wins. The record exists for view/container/module bodies; command
expressions bind their arguments and options as bare names.

`type_ref` mapping for checking (one shared alias table in
`crates/phero-ifml/src/check/domains.rs`, used by both the checker and the
resolver's override check): `String`/`Uuid` → string, `Int` → int,
`Float` → float, `Boolean` → boolean, `DateTime` → the expression
language's calendar `date` (a checking alias — no time-of-day
reasoning). Generic (identifier) input types stay deferred — expressions
relying on them are skipped, not errors. Bare calls (`today()`,
`score(...)`) type-check against the domain's `Package::functions`
(unknown or ambiguous names are `E0102` diagnostics;
`date("YYYY-MM-DD")` stays the calendar-date constructor), while a call
with an otherwise-untypeable argument stays silently skipped.
Constructs the shared expression language cannot type (`%`, the
`~=`/`!~` regex operators, array/object values) are also skipped
silently: evaluation stays deferred to generation time, as below. Diagnostics `E0100`–`E0103` (unknown data
entity, type mismatch, unknown feature, unknown name) name the site:
`view 'X' component 'orders': …`. With `domains: None` expression
checking is off; the interaction cross-reference pass below still runs.

### Interaction References

Every interaction reference resolves against declarations, union-wide
across the compilation (the main file plus its `.ifml` imports — the same
name scope `data:` entities resolve in), regardless of whether a domain
model is available:

| Reference | Must resolve to | Diagnostic |
|---|---|---|
| `action("X")` | an `action "X" { target: …; }` declaration | `E0104` — `action 'X' is not declared — add action "X" { target: <Entity>; }` |
| `navigate("X")` | a `view "X"` **or container** declaration | `E0105` — `navigate target 'X' is not a declared view or container` |
| `refresh("X")` | a component declaration (any body, imported files included) | `E0106` — `refresh target 'X' is not a declared component` |
| a declaration's `target:` | exactly one class of the domain model (`E0109` — `unknown target entity 'X'` / `ambiguous target entity 'X'`; skipped when no domain model is available) |

Action invocations cannot be inferred from their references: declarations
carry the `target` the lowering needs, so a dangling `action("X")` would
silently lower to neither a UI control nor a route — hence an error, not
a synthesized declaration. Container targets are what make wizard-style
patterns expressible: a module's steps navigate between the module's own
containers. Sites on **instantiated** files are diagnosed — the entry
file instantiates a module by `use`-ing it (transitively through module
composition), and a used module's body references are the app's surface,
so their dangling names are errors with spans in the imported file.
Sibling modules of the same imported file that the app never uses stay
inert: their references belong to their own compilation's report. The
diagnostic span is the referenced string token itself, so editor
squiggles land on the name; the LSP publishes these like every other
checker diagnostic. This is verification register row
**V11** ("interaction references always resolve to declarations",
`docs/VERIFICATION.md`) — `tested`, enforced through both the checker and
the manifest artifact boundary (below), with the graph-level proof
staged behind the kernel gaining node metadata.

### Authorization References

`check_ifml_with_actors(&compilation, domains, actors)` adds the
authorization dimension: every `requires:`/`roles:` name in the main
file — view-level properties and event-level `requires:` prefixes —
resolves bare-name union-wide against the capability and actor tables
of the actor models the host provides:

| Site | Must resolve to | Diagnostic |
|---|---|---|
| `requires: [X, …]` (view property or event prefix) | a `capability X on …` in any provided actors block | `E0107` — `capability 'X' is not declared — add "capability X on <Class>;" to an actors block` |
| `roles: [r, …]` (view property) | an `actor r` in any provided actors block | `E0108` — `role 'r' is not declared — add "actor r;" to an actors block` |

An empty actor slice skips the pass entirely — the same contract as
`domains: None` for expressions. Hosts gather the flow's `.actor`
imports when they exist: the CLI resolves them relative to the
importing file and compiles them like `pherolang check` on the policy (a
missing or non-compiling policy is a clean error naming the resolved
path), the LSP does the same with open documents winning over disk and
degrades to no authorization checking in incomplete workspaces. The
same instantiation rule as the interaction pass applies: a module the
app `use`s contributes its `requires:`/`roles:` sites to the app's
surface; uninstantiated modules keep their names to their own
compilation's report. Spans land on the name tokens, and a view gated
on a capability no policy declares can no longer compile clean — the
#112 defect class one level up. The conformance fixture pair
`tests/conformance/ifml/phero/auth.{mox,actor}` makes `app.ifml`'s
`import "phero/auth.actor"` load-bearing.

### Decision Data

`check_ifml_with_decisions(&compilation, domains, actors, decisions)`
adds the decision dimension: a component's `data:` name resolves
**domain classes first** (unchanged behavior and diagnostics when it
resolves), then — only when the name resolved to no domain class —
against the **decision services** of the provided decision models,
bare-name union-wide across all of them (the `requires:` discipline).

| `data:` name resolves to | Behavior |
|---|---|
| exactly one domain class | unchanged: the class is the component's self type, its features bind expressions |
| several domain classes | unchanged: `E0100` — `ambiguous data entity 'X'` (the decision dimension never rescues domain ambiguity) |
| one decision service | **check-only**: the service's output decisions form the component's data contract — no diagnostic, no IR change |
| one decision service that declares no outputs | `E0110` — `decision service 'X' declares no outputs` (the component has no data contract) |
| decision services of two or more models | `E0111` — `decision service 'X' is declared by two models — 'A' and 'B'` (the declaring models are named by their `decision_model` names) |
| nothing | `E0100` — `unknown data entity 'X'`, with a did-you-mean hint over service names when a decisions slice is provided (`…; did you mean 'AssessRefund'?`) |

The contract is **check-only**: the `IfmlModel` artifact and the
control/route manifest are unchanged — the manifest's `data:`/`target:`
resolution stays domain-only, so a service-named binding rides along as
the bare spelling. Decision outputs are **not typed** in v1: a component
bound to a service gets no self type, so expressions naming the
service's outputs defer (the same deferral family as the decision
*inputs* collected by controls — also not typed in v1). The pass reads
the index's `data_sites` — every component's bare-identifier `data:`
binding, exactly the shape the checker resolves — and applies the same
instantiation reachability rule as the interaction and authorization
passes: a module the app `use`s contributes its sites; uninstantiated
modules keep theirs. An empty decisions slice skips the pass entirely
and keeps every diagnostic byte-identical.

Hosts gather the flow's `.dmn` imports the same way they gather `.actor`
policies: the CLI resolves them relative to the importing file, gathers
each model's own `.mox` imports and `logic table` markdown contents, and
hard-errors on a missing or non-compiling model; the LSP degrades to no
decision checking in incomplete workspaces. The conformance fixture
`tests/conformance/ifml/phero/decisions.dmn` makes `app.ifml`'s
`import "phero/decisions.dmn"` load-bearing, and
`examples/support/support.ifml` binds `data: AssessRefund` to the service
`examples/support/support.dmn` declares.

### Warnings and Lints

Every diagnostic carries a severity class. The resolver and checker
bands (`E0001`–`E0009`, `E0100`–`E0111`) are **errors**: they fail
`pherolang check`/`ir`/`build`/`manifest` and the LSP publishes them as
errors. The lint pass (`check_ifml_with_actors`, after the
authorization pass) adds the `ifml.lints` band — **warnings**
(`W0150`–`W0199`) that render but never fail a compilation; the carrier
decides what they mean (`pherolang check --deny-warnings` turns them into
failures, per-file and in the union integrity pass whose lint channel
they join). Every lint judges **only the main file's** declarations —
imported pattern libraries declare unused modules and actions by
design — while references count anywhere in the compilation (an
uninstantiated module's `action("X")` is still live intent):

| Code | Warns when |
|---|---|
| `W0150` | a main-file `action "X"` declaration no `action("X")` invocation names — `action 'X' is declared but never invoked` |
| `W0151` | a main-file view that is not the entry view (first by position), not `landmark: true`, reached by no `navigate` target, and carries no body content — the empty shell nothing routes to (a content view is intentionally routable: the manifest exports every view as a route) |
| `W0152` | a main-file module declaration shadows an imported module of the same name — resolution deliberately lets the main file win silently; the warning documents it at the winning declaration |
| `W0153` | a main-file component with no `data:` binding that also declares no events, carries no chart, and spells no explicit `Entity.field` table column — a fully static placeholder |
| `W0154` | a main-file view with no domain tie at all — no `data:` binding or explicit entity columns in its body, no `params`, no module composition, no event handlers, no `roles:`/`requires:` gate |

The canonical fixtures, examples, and pattern library are all
warning-silent (`pherolang check` on them renders nothing); `pherolang
explain W0150` renders one entry, and `docs/DIAGNOSTICS.md` is the
generated code table.

### Control/Route Manifest

`pherolang manifest <file.ifml> [-o out.json]` exports the **control/route
manifest** (`phero_ir::ifml_manifest::IfmlManifest`, versioned
independently via `IFML_MANIFEST_FORMAT_VERSION`) — the positive artifact
for what the interaction model lowers to, derived from the compilation
*after* resolution and checking:

- **routes** (`views`): each view with its flags (`landmark`/`xor`/
  `modal`), params, `roles:`/`requires:` gates, and condition;
- **controls**: one entry per component on the **expanded** view tree
  (module `use` sites inlined, overrides applied — pattern-contributed UI
  is indistinguishable from app-authored UI), flattened with its
  container path, carrying the component type, the `data:` entity as
  written plus its resolved `(package, class)` when the host provides
  the domain union and the name resolves uniquely there (a bare spelling
  always rides along), and the form fields / table columns. Resolution is
  **domain-only**: a `data:` that names a decision service (the
  [decision dimension](#decision-data)) rides along as the bare
  spelling with no `resolvedData` — the manifest never encodes decision
  contracts;
- **edges**: every `navigate` / `refresh` / `action(...)` invocation with
  its trigger event, `if` guard, `requires:` gates, and binding pairs;
  nested action-body handlers (`on success`/`on error`) become their own
  edges on the same source; command handlers are edges sourced by the
  invocation path;
- **actions**: the declaration-backed actions with their `target:`
  entities (resolved `(package, class)` included);
- **commands**: the CLI vocabulary flattened to invocation paths
  (`tickets.show`), with typed arguments, options and prompts (defaults
  included), `requires:` gates, and descriptions.

The command checks with the full decisions-aware pass
(`check_ifml_with_decisions`: domains, actors, and decision services), so
a `data:` naming a decision service is in scope and the command exits
cleanly — only `resolvedData` stays domain-only, as above. The actors-only
pass reported `AssessRefund` in `examples/support/support.ifml` as an
unknown domain entity (E0100, issue #283); that regression is pinned by
`crates/phero-cli/tests/cli/manifest.rs`.

The derivation contract is the manifest's reason to exist: **every edge's
target is declaration-backed by construction**. The checker
(`E0104`–`E0106`, instantiated modules included) gates emission, so a
dangling reference is an error before any manifest can exist — consumers
ingest the manifest without re-deriving resolution and without
synthesizing endpoints (the #112 ingest-defect class is structurally
impossible at this boundary; verification row **V11** names it as
enforcement). Emission is deterministic (source order throughout) and
the wire follows the house discipline: camelCase, adjacent tagging,
`#[serde(default)]` + `skip_serializing_if` everywhere — an empty model
serializes as exactly `{"formatVersion":2}`. Golden manifests pin the
semantic product: `tests/conformance/ifml/{app,cli}.manifest.json` and
`examples/artifacts/{cli,ecommerce,support}.manifest.json` (the
`examples_artifacts` precedent).

Declarations resolve one step further, across surfaces: `pherolang check
<dir>` runs the union integrity pass (`phero_driver::check_integrity`,
register row **V12**) over the import-connected application and errors
when a declared action's name binds to no service operation
(ASCII-case-insensitive verb equality — `ApproveRefund` ↔
`approveRefund`), or when a `requires:` name is neither a declared
capability nor a declared actor, or a `roles:` name is not a declared
actor. Checking a flow alone stays per-surface and silent.

### Reference Pattern Library

`patterns/ifml/` ships a domain-agnostic library of 39 reusable modules —
the spectrum of common experiences (feeds, inbox, search, commerce,
workflow, overlays) — organized one file per category:

| File | Modules |
|---|---|
| `navigation.ifml` | Pagination, Tabs, Breadcrumbs, Link |
| `collections.ifml` | DataList, CardGrid, Card, Feed, Timeline, MasterDetail, MediaGallery |
| `data.ifml` | Detail, Summary, Metric, SearchPanel, Calendar, Map |
| `input.ifml` | FormPanel, ReferencePicker, Upload |
| `workflow.ifml` | Wizard, WorkQueue, ApprovalQueue, Progress |
| `communication.ifml` | Inbox, NotificationCenter, CommentThread |
| `commerce.ifml` | Catalogue, ProductDetail, Cart, Checkout |
| `social.ifml` | ReactionBar, SocialItem, ShareAction |
| `overlays.ifml` | Modal, Drawer, Popover, Confirmation, Toast |

Patterns compose: module bodies may contain `use` statements
(`Checkout` composes `Wizard`; `Inbox` composes `SearchPanel` +
`DataList`). They stay domain-agnostic through two conventions:

- **Typed inputs** carry builtin types (`String`, `Int`, `Boolean`) or a
  generic identifier type (`Item`, `Filter`) for entity handles; the
  checker defers the generic ones and type-checks the builtin ones at
  each use site.
- **Slots** are plain module properties (e.g. `Card`'s
  `eyebrow`/`title`/`subtitle`/`body`/`metadata`/`media`/`actions`), so
  a consumer binds them with expressions against its own domain:
  `use "Card" as card { title: product.name; };`.

Import library files by relative path (the CLI and every host resolve
imports relative to the importing file) and instantiate with `use`:

```
import "../patterns/ifml/commerce.ifml";

view "Storefront" {
    params { products: Product };

    use "Catalogue" as catalogue { items: products; };
}
```

Instantiation makes the pattern's body the app's surface: every
reference inside a used module resolves like app-authored code
(`navigate("ItemDetail")` needs the app to declare that view or
container; `action("AddToCart")` needs the action — the
instantiation rule of E0104–E0106 above). App-facing targets stay
deliberately undeclared in the library: declaring what you back is the
consumer's job, and the reference consumers
(`examples/shop/ecommerce.ifml`, `examples/support/support.ifml`) show the
declaration blocks. The library's own wiring resolves internally —
`Wizard`'s steps navigate its own containers, `SearchPanel` refreshes
its own `results` component — and the presentational modules carry no
interaction references at all (`Pagination`'s pager binds page state;
paging is not navigation).

`examples/shop/ecommerce.ifml` and `examples/support/support.ifml` are the reference
consumers.

---

## DSL + Domain Model Symbiosis

The IFML DSL is designed to feel like a *companion* to the `.mox` domain model:

```text
// The domain model defines "Customer" with features: name, email, phone, status
// The IFML DSL references these by name (an excerpt, not a whole file):

component "grid" {
    type: list;
    data: Customer;                // references the domain class
    fields: [name, email, phone];  // references features on Customer
    filter: status == "active";    // expression validated against field types
}
```

The `domain "sales" { schema "sales"; }` declaration at the top of an `.ifml` file establishes which domain provides the entity definitions.

---

## Grammar Authority

The EBNF in
[`crates/phero-ifml/src/parser/mod.rs`](../crates/phero-ifml/src/parser/mod.rs)
is the sole grammar authority for `.ifml`. The parser, the lowering, the
formatter, and the language server all consume the same lexer/parser stack;
no second grammar ships in this repository (there is no Tree-sitter grammar),
so editor features are served by the LSP's one-shot compilation over the same
spans. The grammar-doc tests cross-check the EBNF against the contextual word
tables in both directions: adding a word without documenting it (or the
reverse) fails the suite.

---

## Crate Dependency Graph

```
phero-ifml
  ├── logos (lexer)
  ├── chumsky (parser)
  ├── phero-parser (byte-span token input, recovery combinators, the shared Surface identity)
  ├── phero-diagnostics (the one diagnostic record + the ifml code bands)
  ├── phero-strings (JSON string unescaping)
  ├── phero-expr (typed expression checking)
  └── phero-ir (IfmlModel artifact types, versioned wire format)
```

`phero-ifml` is dependency-clean: no consumer-specific dependencies, so any backend can parse `.ifml` sources or consume `IfmlModel` artifacts directly. The direction that matters is one-way: **`phero-driver` depends on `phero-ifml`**, never the reverse.

### The recorded carve-out (ADR 0005 D6)

`.ifml` stays a separate crate. That is a decision, not debt: the driver
depends on `phero-ifml`, so a shortcut such as "use the driver's diagnostic
type" would either cycle or force a layering rewrite. Its logos lexer, its
chumsky parser, its spanned AST, its `IfmlIndex` and its formatter stay its
own. What it shares are the three seams that live *below* both crates:

| Seam | Shared artifact | Pinned by |
|---|---|---|
| Diagnostics | `phero_diagnostics::Diagnostic<&'static str>` — the crate has no record type of its own; a compilation tags each record `(path, record)` | `tests/seams.rs`, `tests/diagnostic_order.rs` |
| Surface identity | `phero_parser::Surface::Ifml` — one `ALL` entry, `id()`/`extension()` equal `"ifml"`; no local surface enum or mapping | `tests/seams.rs` |
| Import walk | the two-key `IfmlImports` / `walk_ifml_imports` contract: (importing-file identity, import path as written) | `tests/seams.rs`, `tests/resolve.rs` |

The import seam is deliberately **not** the driver's path-keyed walker. The
`.ifml` walk hands the host the importing document's identity and the path
exactly as written, and the bundle is keyed by that pair, while the driver's
gatherers key by the resolved host path. Those are different contracts, not
one contract with a policy flag — which is why
`phero_driver::imports::walker` lists this walk as one of the two it does
not absorb.

**What "aligned" means for a future surface.** A new surface aligns at the
seams below both crates — the shared diagnostic record, the one surface
identity, the shared lexer/parser scaffolding and expression engine, the
import-walk substrate — and never by making a lower crate depend on
`phero-driver`. A surface with a genuinely different import contract (as
`.ifml` has) keeps its own walker and records why in the same place; the
driver's walker does not grow a flag to swallow it.

---

## Testing Strategy

| Scope | Test | Method |
|---|---|---|
| Grammar | Valid `.ifml` files parse without errors | chumsky `parse_ifml` |
| Grammar | Invalid `.ifml` files produce expected errors | strict `Err` + lenient `E0002` diagnostics |
| Grammar | The EBNF and word tables stay in sync | `parser/mod.rs` grammar-doc tests |
| Recovery | Broken documents keep a partial model and index | `tests/recovery.rs` + proptest |
| Artifact | Parse → `to_json_pretty` → golden byte-compare | `tests/conformance/ifml/` golden |
| Artifact | `to_json_pretty` → `from_json` round trip | Golden + unit tests |
| Wire format | `formatVersion` gate, unknown-field tolerance | Unit tests (`phero_ir::ifml`) |
| Expression parsing | C-like expressions produce correct trees | Unit test |
