---
title: Architecture, profiles, and deployments
description: How an application's logical architecture, its reusable target profiles, and its deployments compose into one checked projection.
type: concept
language: deploy
section: concepts
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/deploy/overview.md]
related: [docs/guide/deploy/how-to/project-an-application.md, docs/guide/deploy/reference/applications-and-profiles.md]
lastReviewed: 2026-11-21
---

# Architecture, profiles, and deployments

A deployment model separates three questions that environment files usually mix
together: what the application is made of, which runtime constraints it runs
under, and which concrete projection an environment uses. Keeping them apart is
what makes one application deployable to several targets without copying its
description.

## Overview

The three layers are three declarations:

| Layer | Declaration | Question it answers |
|---|---|---|
| Architecture | `application` | Which components exist, of which kinds, and how do they connect? |
| Profile | `profile` | Which target, which defaults and mappings, and which policies? |
| Deployment | `deployment` | Which application, through which profile, with which overrides? |

Architecture is environment-free. A component body holds only what is true of
that component wherever it runs: its kind, its baseline settings, the
capabilities it needs, and any bindings to a design or a flow.

A profile is application-independent. It names a target and the constraints that
follow from that target. It can be selected by deployments of different
applications.

A deployment is the join. It names one application and one profile of the same
file, and it may override settings for that environment.

None of the three describes a container, a manifest, or a cloud resource. The
language stops at intent; turning intent into platform material is the job of
the tooling for that target.

## Why does it matter?

The usual alternative is one configuration file per environment. The
application's shape is copied into each one, so a new component means editing
every environment, and the copies drift until no two environments describe the
same system. Policy is worse: a rule enforced by a deployment script can be
bypassed by the next override, and nobody notices until an audit.

Separating the layers removes both problems. The application is described once.
An environment is a profile that can be reused, reviewed, and versioned on its
own. The effective configuration is computed from explicit levels rather than
scattered defaults, and every policy is evaluated against the result, after
overrides. A component that needs something the target cannot provide is refused
while the model is checked, not when a release is attempted.

The cost of ignoring the separation is not only repetition. It is that no single
place can answer "what does this environment actually run?" — the answer is
spread across files that disagree.

## How it works

**Three levels, one effective configuration.** For each component, the value of
a setting comes from the first level that declares it:

1. the component baseline in the `application` body;
2. the selected profile's `defaults` entry for that component;
3. the deployment's `configure` entry.

A later level wins. The deployment alone owns the result: two deployments of the
same application through the same profile may differ entirely, and neither
changes the other. A policy that reads a setting no level declares is an error,
not an empty value, so an incomplete configuration cannot pass by accident.

**A profile selects one target.** The target list is closed:

| Target | What it provides |
|---|---|
| `standalone` | Long-running processes, a persistent filesystem, a relational database, background workers, stateful processes. |
| `dockerCompose` | The standalone set, plus containers and horizontal scaling. |
| `kubernetes` | The same set as `dockerCompose`. |
| `cloudflareWorkers` | A relational database, horizontal scaling, and edge execution. |

**Target capabilities are provider-defined.** Every component kind brings
intrinsic requirements: a `worker` needs a long-running process, and a
`database` needs a relational database service. Settings add conditional
requirements: an engine of `sqlite` needs a persistent filesystem, and a runtime
of `container` needs container support. A component may also declare extra
capabilities with `requires`. Each requirement is compared with what the selected
target provides. Declaring a capability the target lacks is refused; so is using
a kind whose intrinsic requirements the target cannot meet.

**Engines are matched to targets.** A database declares an engine — `sqlite`,
`postgres`, or `d1` — and each target hosts a fixed subset. A postgres database
is never silently mapped onto an edge target that only offers D1; the
projection is refused with the engine and the target named.

**Policies run on the result.** `require` and `prohibit` conditions read
component settings such as `api.replicas` and `db.storage`. They are typed
against the components and settings of the application being deployed, then
evaluated against that deployment's effective settings. A `require` that does
not hold and a `prohibit` that does are both errors, including when the cause is
an override.

**Mappings and environment variables stay authored.** A mapping says how a
component kind becomes a platform resource, such as an API becoming a
deployment. An `env` block carries opaque name/value pairs for the platform.
Both are carried as written; neither is part of the checked setting vocabulary.

**No platform material is produced.** The language defines the projection, not
its realization. Nothing here emits a container definition or a manifest; that
is deliberately left to a consumer of the model.

## Example

One application, one profile, and one deployment, with all three configuration
levels visible:

```deploy
application LibraryDesk {
    component api: api {
        port: 8080
    }
    component db: database {
        storage: volume
    }
    connects api -> db
}

profile local {
    target: standalone
    defaults {
        api.runtime: embedded
        api.replicas: 1
        db.engine: sqlite
        db.storage: local
    }
}

deployment developer for LibraryDesk {
    use local
    configure {
        api.replicas: 2
    }
}
```

`api` has no baseline replica count. The profile supplies 1, and the deployment
overrides it to 2 — so this deployment runs two API instances without changing
the application or the profile. `db` takes both its engine and its storage from
the profile. The projection checks because `standalone` hosts sqlite and
provides the persistent filesystem that an sqlite database needs.

## Relationship to other concepts

- The **domain model** ([the .mox overview](../../mox/overview.md)) declares the
  classes behind the application; the deployment model does not reference them
  directly.
- An **application design** ([the .ddd overview](../../ddd/overview.md)) is
  bound to a component with `design`, naming the imported design and,
  optionally, a module of it. The deployment model names which component hosts
  which module instead of restating services.
- An **interaction flow** ([the .ifml overview](../../ifml/overview.md)) is
  bound to a frontend component with `flow "<path>"`.
- A **decision model** ([the .dmn overview](../../dmn/overview.md)) exposes
  decision services the application invokes; the deployment decides which
  component runs the invoking code.
- The **deployment target** is the vocabulary word the profile selects. Its
  capabilities and hosted engines are defined by the language, never authored in
  the file.

## Further reading

- [Project an application onto a target](../how-to/project-an-application.md)
  applies the three layers in order.
- [Applications and profiles](../reference/applications-and-profiles.md) states
  the syntax, the setting vocabulary, and every constraint.
- [The .deploy language reference](../../../DEPLOY.md) documents the target
  capability tables, resolution rules, and tooling.
