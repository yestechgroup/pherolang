---
title: Deployment models (.deploy)
description: Describe where an application runs — its components, reusable target profiles, and the deployments that project one onto the other.
type: overview
language: deploy
section: start
order: 10
status: stable
docStatus: complete
audience: [application developer]
related: [docs/guide/deploy/concepts/architecture-profiles-and-deployments.md, docs/guide/deploy/reference/applications-and-profiles.md]
lastReviewed: 2026-11-21
---

# Deployment models (.deploy)

A `.deploy` file describes where an application runs. It names the application's
components and how they connect, the targets it can run on, described as
reusable profiles, and the deployments that project the application through a
profile. Phero Lang checks that the projection is possible before any platform
work begins.

## What is it?

A deployment model has three kinds of declaration:

- An `application` is the logical architecture: named components, each with a
  kind and baseline settings, plus `connects` lines that say which component
  talks to which.
- A `profile` is a reusable runtime configuration: one target from a closed
  list, defaults for component settings, mappings from component kinds to
  platform resources, and `require`/`prohibit` policies.
- A `deployment` is one concrete projection: it selects an application and a
  profile and may override individual settings.

The file records intent only. It never defines a container, a manifest, or a
cloud resource; the tooling for that target decides how to realize it. That
boundary is what lets one application description serve several targets without
being copied for each.

## What problems does it solve?

Deployment knowledge usually lives in a pile of environment files, each slightly
different, with the application's shape copied into every one. Adding a
component means editing every environment, the copies drift, and an impossible
combination — a component that needs a local filesystem on a target that has
none, or a database engine the target cannot host — surfaces during a release.

A deployment model puts the application in one place and the environment in
another. Profiles are reused across applications and deployments, so an
environment is described once. Policies are checked against the settings that
result from defaults and overrides, so a per-environment override cannot
silently bypass a rule the profile requires. A target's capabilities are
compared with what each component needs while the model is checked, so an
impossible projection is an error in the model rather than a surprise in
production.

## Key capabilities

- **One logical architecture.** Name components, give each a kind, and state
  the connections between them. Nothing in the application declares where it
  runs.
- **Reusable profiles.** Describe a target once — its defaults, resource
  mappings, and policies — and select it from any deployment.
- **A closed target list.** `standalone`, `dockerCompose`, `kubernetes`, and
  `cloudflareWorkers`. Each target provides a defined set of platform
  capabilities, and the list is fixed by the language.
- **Layered configuration.** For every component, the effective setting is the
  component baseline, then the profile default, then the deployment override.
  The later level wins.
- **Checked policies.** `require` and `prohibit` conditions are typed against
  the application's components and evaluated against each deployment's
  effective settings.
- **Capability and engine validation.** A component's needs — from its kind,
  its settings, and its declared `requires` — are matched against the target,
  and a database engine the target cannot host is refused.
- **Verbatim environment variables.** An `env` block carries opaque name/value
  pairs for the platform, deliberately outside the checked setting vocabulary.
- **Design and flow bindings.** Tie a component to a module of a `.ddd`
  design or to an `.ifml` flow, so the deployment knows which application code
  each component runs.

## A simple example

A support desk with one API, one frontend, and one database, projected through a
Kubernetes profile:

```deploy
application SupportDesk {
    component api: api {
        scaling: autoscaled
        port: 18082
        health: "/healthz"
        env {
            LOG_LEVEL = "info"
        }
    }
    component web: frontend {
        entrypoint: "src/inbox.ts"
    }
    component db: database {
        storage: managed
    }
    connects web -> api
    connects api -> db
}

profile production_k8s {
    target: kubernetes
    defaults {
        api.runtime: container
        api.replicas: 3
        db.engine: postgres
        db.storage: external
    }
    mappings {
        api -> kubernetes.deployment
        database -> external.postgres
    }
    require (api.replicas >= 3)
    prohibit (db.storage == "local")
}

deployment production for SupportDesk {
    use production_k8s
    configure {
        api.replicas: 4
    }
}
```

`api` declares a port, a health path, and one environment variable. The profile
selects the target and supplies the settings every deployment inherits. The
deployment overrides the replica count to 4; the `require` policy is evaluated
after that override and still holds. The `prohibit` policy forbids local
storage, and the database's external storage does not trigger it.

## How it fits into Phero Lang

The deployment model is the last file most projects write, because its bindings
resolve against finished designs and flows:

- [Domain models (.mox)](../mox/overview.md) supply the classes and rules
  behind the application. The deployment model never names a class directly.
- [Application design (.ddd)](../ddd/overview.md) supplies the modules and
  services. A component's `design` binding names the imported design and,
  optionally, the module it hosts.
- [Interaction flows (.ifml)](../ifml/overview.md) supply the screens and
  navigation. A frontend component's `flow` binding names the imported flow.
- [Decision models (.dmn)](../dmn/overview.md) supply decision services the
  application invokes; the deployment says which component runs the calling
  code.
- The deployment model adds the missing dimension: which target each component
  runs on, under which settings and policies.

## Getting started

- [Project an application onto a target](how-to/project-an-application.md)
  walks through the three declarations in order.
- [Applications and profiles](reference/applications-and-profiles.md) is the
  syntax and rules reference.
- The [support-desk deployment model](../../../examples/support/support.deploy)
  is a complete model with one application and two targets.

## Further reading

- [Architecture, profiles, and deployments](concepts/architecture-profiles-and-deployments.md)
  explains why the three layers are separate and how the effective
  configuration is computed.
- [Applications and profiles](reference/applications-and-profiles.md) lists
  every setting, target, and validation rule.
- [The .deploy language reference](../../DEPLOY.md) is the complete engineering
  reference, including target capability tables and tooling.
