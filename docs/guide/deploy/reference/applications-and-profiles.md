---
title: Applications and profiles
description: The three declarations of a .deploy file, the settings vocabulary, the closed target list, and the rules each projection must satisfy.
type: reference
language: deploy
section: reference
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/deploy/overview.md]
related: [docs/guide/deploy/concepts/architecture-profiles-and-deployments.md, docs/guide/deploy/how-to/project-an-application.md]
lastReviewed: 2026-11-21
---

# Applications and profiles

This page is the authority for the three declarations of a `.deploy` file —
`application`, `profile`, and `deployment` — and for the settings, mappings, and
policies they carry.

## Syntax

Imports come first and name a `.ddd` design or an `.ifml` flow. A binding's path
is written exactly as its import is:

```deploy,fragment
import "support.ddd"
import "support.ifml"
```

An application declares components and their connections:

```deploy,fragment
application SupportDesk {
    component api: api {
        port: 18082
        health: "/healthz"
        design "support.ddd#desk"
    }
    component web: frontend {
        flow "support.ifml"
    }
    component db: database {
        storage: managed
    }
    connects web -> api
    connects api -> db
}
```

A profile selects a target and carries defaults, mappings, and policies:

```deploy,fragment
profile production_k8s {
    target: kubernetes
    defaults {
        api.runtime: container
        api.replicas: 3
        db.engine: postgres
    }
    mappings {
        api -> kubernetes.deployment
        database -> external.postgres
    }
    require (api.replicas >= 3)
    prohibit (db.storage == "local")
}
```

A deployment joins one application to one profile and may override settings:

```deploy,fragment
deployment production for SupportDesk {
    use production_k8s
    configure {
        api.replicas: 4
    }
}
```

The component kinds, their settings, and the value each setting accepts:

| Kind | Settings |
|---|---|
| `api` | `runtime`, `replicas`, `scaling`, `port`, `health`, `cors` |
| `worker` | `runtime`, `replicas`, `port`, `health` |
| `database` | `engine`, `storage`, `runtime`, `port` |
| `queue` | `runtime`, `replicas`, `port` |
| `objectStore` | `storage`, `port` |
| `frontend` | `runtime`, `entrypoint`, `port`, `cors` |

| Setting | Value | Accepted literals |
|---|---|---|
| `runtime` | word | `api` components: `embedded`, `container`, `worker`, `external`. `worker`, `database`, and `queue` components: `embedded`, `container`, `external`. `frontend` components: `static`, `worker`, `container`. |
| `replicas` | integer | A plain integer. |
| `scaling` | word | `singleInstance`, `replicated`, `autoscaled`. |
| `port` | integer | A plain integer. |
| `health` | text | A double-quoted string. |
| `cors` | text | A double-quoted, comma-separated origin list. |
| `engine` | word | `sqlite`, `postgres`, `d1`. |
| `storage` | word | `local`, `volume`, `external`, `managed`. |
| `entrypoint` | text | A double-quoted string. |

The closed target list, with the capabilities each target provides and the
database engines it hosts:

| Target | Capabilities | Engines |
|---|---|---|
| `standalone` | `longRunningProcess`, `persistentFilesystem`, `sqlDatabase`, `backgroundWorker`, `statefulProcess` | `sqlite` |
| `dockerCompose` | The standalone set, plus `container` and `horizontalScaling` | `sqlite`, `postgres` |
| `kubernetes` | The same set as `dockerCompose` | `postgres` |
| `cloudflareWorkers` | `sqlDatabase`, `horizontalScaling`, `edgeExecution` | `d1` |

## Semantics

An `application` is the logical architecture. Component names are unique within
the application, and a `connects` line names two of its components. The
direction of a connection is authored intent; the language does not interpret
it beyond checking both endpoints.

A `component` names a kind, then declares what is true wherever it runs:

- **Baseline settings** use a single-segment path: `port: 18082`.
- **`requires`** adds platform capabilities the component needs beyond the ones
  its kind and settings imply.
- **`env`** declares opaque string pairs, kept verbatim and outside the setting
  vocabulary.
- **`design`** and **`flow`** bind the component to a module of an imported
  design or to an imported flow.

A `profile` is a reusable runtime configuration. Its `target` line is required.
`defaults` entries use two segments — a component name and a setting name — and
apply to every deployment that selects the profile. `mappings` say how a
component kind becomes a platform resource. `require` and `prohibit` state
conditions over component settings.

A `deployment` names one application and one profile from the same file. Its
`configure` entries use the same two-segment form as profile defaults and
override them for this deployment only.

**Effective settings.** For every component, a setting's value is taken from the
component baseline, then the profile default, then the deployment override. The
last level that declares the setting wins. Effective settings are computed for
each deployment and are not written back into the application.

**Settings are typed.** An integer setting takes an integer literal, a text
setting takes a double-quoted string, and a word setting takes one of its listed
names written as a bare word. A word outside the list is an error.

**Environment variables are not settings.** An `env` entry is a name and a
quoted string. It never joins the effective settings and cannot be read by a
policy; that separation keeps platform secrets and knobs out of the checked
vocabulary.

**Policies are conditions over settings.** A policy expression reads settings as
`component.setting`, combines them with comparisons and boolean operators
(`&&`, `||`, `!`), and may use integer arithmetic. The condition types as
boolean; one that does not is an error. It is typed against the application being
deployed and evaluated against the deployment's effective settings. An unset
setting referenced by a policy is an error: give the component a baseline or the
profile a default.

**Capabilities are matched to the target.** A component's requirements come from
three places: its kind (a `worker` needs `longRunningProcess`, a `database` needs
`sqlDatabase`), its effective settings (an `sqlite` engine needs
`persistentFilesystem`, a `container` runtime needs `container`), and its
declared `requires`. Every requirement must be provided by the selected target.

**Engines are matched to the target.** A database's effective engine must be one
the target hosts. The check applies to the engine that results from baselines,
defaults, and overrides, so an override cannot move a postgres database onto a
target that only offers D1.

**Nothing is generated.** A deployment model carries authored intent. The
language defines no container, manifest, or cloud-resource output; producing
platform material is a consumer's responsibility.

## Constraints

| Rule | Consequence |
|---|---|
| Imports precede the first declaration and name only `.ddd` or `.ifml` files. | A later `import` is a syntax error; another extension is refused as an unsupported import. |
| A component kind and a mapping kind are one of the six listed kinds. | An unknown kind is an error that lists the known kinds. |
| Application, profile, and deployment names are unique in the file; component names are unique in their application. | A repeated name is an error. |
| A `requires` name is one of the eight platform capabilities. | An unknown capability is an error, so a misspelling cannot become a vacuous requirement. |
| A baseline setting path has one segment; a default or override has two. | A path of the wrong length is an error naming the expected form. |
| A default or override names a component of the deployed application and a setting of that component's kind. | An unknown component or setting is an error naming the profile or deployment. |
| A setting is declared once per level. | A repeated setting is an error naming the declaration. |
| A setting value matches its declared type and, for a word, its accepted list. | A wrong literal kind or an unlisted word is an error naming the setting. |
| An `env` name is non-empty and unique on its component; its value is a quoted string. | An empty name or a duplicate is an error; the value is carried verbatim. |
| A `connects` endpoint names a component of its application. | An unknown endpoint is an error naming the connection. |
| A profile carries a `target:` line naming a target from the closed list. | A missing target is an error; an unknown target is an error listing the four targets. |
| A deployment's `for` names an application of the file and its `use` names a profile of the file. | An unknown application or profile is an error naming the deployment. |
| A `design` or `flow` binding's path matches one of the file's imports as written. | A binding that names no import is refused; the message names the component and the binding. |
| A `#Module` suffix names a module declared by the imported design or flow. | An unknown module is an error that lists the imported model's modules. |
| Every capability a component needs is provided by the target. | An unsupported requirement is an error naming the component, the capability, and the target. |
| A database's effective engine is hosted by the target. | An incompatible engine is an error naming the database, the engine, and the target. |
| Every `require` condition holds and every `prohibit` condition does not, after defaults and overrides. | A violation is an error naming the deployment and the policy. |
| A policy's settings resolve to declared values. | An unknown or unset setting is an error, not an empty comparison. |

## Examples

A complete application with every component kind, a Kubernetes profile, and one
deployment:

```deploy
application SupportDesk {
    component api: api {
        replicas: 2
        port: 18082
        health: "/healthz"
        cors: "https://desk.example.com"
        env {
            DATABASE_URL = "postgres://db/support"
        }
    }
    component web: frontend {
        runtime: static
        entrypoint: "dist/index.html"
    }
    component search: worker {
        requires longRunningProcess
    }
    component inbox: queue {
        runtime: container
    }
    component archive: objectStore {
        storage: volume
    }
    component db: database {
        storage: managed
    }
    connects web -> api
    connects api -> db
    connects api -> inbox
    connects inbox -> search
    connects search -> archive
}

profile production_k8s {
    target: kubernetes
    defaults {
        api.runtime: container
        api.replicas: 3
        db.engine: postgres
    }
    mappings {
        api -> kubernetes.deployment
        worker -> kubernetes.job
        queue -> kubernetes.statefulset
        objectStore -> external.s3
        database -> external.postgres
    }
    require (api.replicas >= 3)
    prohibit (db.storage == "local")
}

deployment production for SupportDesk {
    use production_k8s
    configure {
        api.replicas: 5
    }
}
```

The `api` baseline of 2 replicas is replaced by the profile default of 3 and
then by the deployment override of 5. The `search` worker's intrinsic
`longRunningProcess` requirement is provided by the target, and so are the
container runtime `inbox` declares and the database service `db` needs. The
postgres engine comes from the profile, and the managed storage baseline keeps
the `prohibit` policy satisfied.

One application can also be projected onto targets with different capabilities:

```deploy
application NotesDesk {
    component web: frontend {
        runtime: worker
        entrypoint: "src/worker.ts"
    }
    component api: api {
        runtime: worker
        port: 8787
    }
    component db: database {
        storage: external
    }
    connects web -> api
    connects api -> db
}

profile edge {
    target: cloudflareWorkers
    defaults {
        db.engine: d1
    }
}

profile laptop {
    target: standalone
    defaults {
        api.runtime: embedded
        db.engine: sqlite
        db.storage: local
    }
}

profile compose {
    target: dockerCompose
    defaults {
        api.runtime: container
        db.engine: postgres
    }
}

deployment edgeLive for NotesDesk {
    use edge
}

deployment local for NotesDesk {
    use laptop
}

deployment composeLive for NotesDesk {
    use compose
}
```

`edgeLive` runs on the edge target with a D1 database and a worker runtime.
`local` runs the same application on a standalone target with sqlite and a local
filesystem. `composeLive` runs it in containers with postgres. Each deployment
has its own effective settings; none of them changes the application.

## Diagnostics

No diagnostic codes are allocated for `.deploy`; each failure is reported as a
message naming the declaration it concerns.

| Message | Meaning |
|---|---|
| `unknown component kind '<kind>'` | The kind is outside the six-word vocabulary. |
| `unknown capability '<name>'` | A `requires` name is outside the eight-word vocabulary. |
| `unknown setting '<name>' for kind '<kind>'` | The setting is not in that kind's vocabulary. |
| `setting '<name>' expects an integer` (or `string`, or `word`) | The literal does not match the setting's type. |
| `invalid <setting> '<value>'` | A word setting's value is outside its accepted list. |
| `profile '<name>' declares no target: line` | A profile must select a target. |
| `unknown deployment target '<name>'` | The target is outside the four-word list. |
| `component '<name>' binds unknown design "<path>"` | A binding names no import of the file (`flow` is the twin message). |
| `component '<name>' requires capability '<capability>', which target '<target>' does not provide` | The projection needs more than the target offers. |
| `database '<name>' engine '<engine>' is not available on target '<target>'` | The target does not host the effective engine. |
| `policy violated in deployment '<name>': require (<condition>)` | A `require` does not hold; `prohibit` is the twin message. |
| `cannot evaluate policy condition: component '<name>' has no effective setting '<setting>'` | A policy reads a setting no level declares. |

## Compatibility

- A compiled deployment model is self-describing, emitted as JSON with its own
  `formatVersion`, versioned independently of the domain model.
- `pherolang artifact check` validates an emitted model from the file alone.
- The semantic core is closed and the platform surface is open. Kinds and
  targets are fixed words; capability names, setting paths, policy expressions,
  and mapping resources are carried as authored text, so a new platform resource
  needs no language change.
- Policy expressions are carried verbatim between their parentheses; the model
  never reformats them.
- Resolution is validation-only. Every name, setting, and binding is kept as
  authored, and effective settings are computed for checking rather than
  serialized back.
- The model describes intent. No container or manifest output is part of this
  release; the tooling for a target consumes the model and decides how to
  realize it.
