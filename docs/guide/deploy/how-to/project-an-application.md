---
title: Project an application onto a target
description: Declare an application's components, a profile for the chosen target, and a deployment that layers per-environment overrides on top.
type: how-to
language: deploy
section: how-to
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/deploy/overview.md, docs/guide/deploy/concepts/architecture-profiles-and-deployments.md]
related: [docs/guide/deploy/reference/applications-and-profiles.md]
lastReviewed: 2026-11-21
---

# Project an application onto a target

Use this page when you know an application's components and need to state where
it runs, which settings an environment changes, and which rules no environment
may break. Done means a deployment selects the application and a profile, and
checking the file reports no findings.

## Problem

An application's description and an environment's configuration are usually
tangled: the components are listed in the deployment file, the environment's
values are mixed into them, and a second environment starts as a copy. You need
the application declared once, the target described once, and each environment
to be a small, explicit projection of the two.

## Prerequisites

- The application's components and their connections are known.
- A target is chosen from `standalone`, `dockerCompose`, `kubernetes`, or
  `cloudflareWorkers`.
- The [overview](../overview.md) has been read once.
- [Architecture, profiles, and deployments](../concepts/architecture-profiles-and-deployments.md)
  has been read once, so the three layers are familiar.

## Solution

**1. Declare the logical application.** List the components with their kinds and
only the settings that are true wherever the component runs.

```deploy,fragment
application SupportDesk {
    component api: api {
        port: 18082
        health: "/healthz"
    }
    component db: database {
        storage: volume
    }
    connects api -> db
}
```

A component that hosts an imported `.ddd` design names it with `design "<path>"`
and may add a module, `design "<path>#<Module>"`; a frontend component that runs
an imported `.ifml` flow names it with `flow "<path>"`, optionally with a module
suffix. Both paths must match an import of the file, and the imports come before
the first declaration.

**2. Describe the target once.** Put the target and the settings every
deployment that selects it should inherit into a profile.

```deploy,fragment
profile production_k8s {
    target: kubernetes
    defaults {
        api.runtime: container
        api.replicas: 2
        db.engine: postgres
    }
}
```

Defaults use `component.setting` paths. Keep environment choices here rather
than in component baselines: a baseline belongs to the application, a default
belongs to the profile.

**3. Constrain the profile with policies.** A `require` states what must hold; a
`prohibit` states what must not. Inside the profile body, add the constraints:

```deploy,fragment
require (api.replicas >= 2)
prohibit (db.storage == "local")
```

Policies are evaluated after defaults and overrides, so they constrain every
deployment that selects the profile, not only the values written in the profile.

**4. Declare the deployment.** Join the application and the profile.

```deploy,fragment
deployment staging for SupportDesk {
    use production_k8s
}
```

The deployment name usually names the environment. Several deployments may
select the same application and profile.

**5. Check the projection.**

```sh
pherolang check deploy/support.deploy
```

A missing capability, an engine the target cannot host, or a violated policy is
reported against the deployment. Checking produces no container or manifest; it
either accepts the intent or names what prevents it.

**6. Override only what the environment changes.** Add `configure` entries for
this deployment.

```deploy,fragment
deployment staging for SupportDesk {
    use production_k8s
    configure {
        api.replicas: 3
    }
}
```

An override is the last level of the effective configuration. It can trip a
policy, and that is intentional: the policy is checked against the value the
environment will actually use.

## Example

The finished model: one application, one target profile, and one deployment
whose override raises the replica count.

```deploy
application SupportDesk {
    component api: api {
        port: 18082
        health: "/healthz"
    }
    component db: database {
        storage: volume
    }
    connects api -> db
}

profile production_k8s {
    target: kubernetes
    defaults {
        api.runtime: container
        api.replicas: 2
        db.engine: postgres
    }
    mappings {
        api -> kubernetes.deployment
        database -> external.postgres
    }
    require (api.replicas >= 2)
    prohibit (db.storage == "local")
}

deployment staging for SupportDesk {
    use production_k8s
    configure {
        api.replicas: 3
    }
}
```

`api` takes its port and health path from the component baseline, its runtime
and starting replica count from the profile, and its final replica count from
the deployment. `db` takes its engine and mappings from the profile while
keeping the baseline storage. The projection is accepted because the target
hosts postgres, provides a container runtime, and satisfies both policies.

For a second environment, add another deployment that selects the same profile
with a different override, or a second profile for a different target. The
application declaration does not change.

## Related reference

- [Applications and profiles](../reference/applications-and-profiles.md) — the
  syntax, settings vocabulary, target tables, and every constraint.
- [Architecture, profiles, and deployments](../concepts/architecture-profiles-and-deployments.md)
  — why the layers are separate and how effective settings are computed.
- [The .deploy language reference](../../../DEPLOY.md) — the complete
  engineering reference, including tooling.
