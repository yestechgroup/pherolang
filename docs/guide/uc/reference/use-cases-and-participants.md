---
title: Use cases and participants
description: The statement forms of a .uc file — subject, actors, occurrences, messages, successions, steps, and realizes bindings.
type: reference
language: uc
section: reference
order: 10
status: stable
docStatus: complete
audience: [application developer]
prerequisites: [docs/guide/uc/overview.md]
related: [docs/guide/uc/concepts/use-case-patterns.md]
lastReviewed: 2026-11-21
---

# Use cases and participants

This page states the forms a `.uc` file accepts, what each one means, and every
rule a scenario is checked against.

## Syntax

A file is `import` declarations followed by `usecase` declarations. This is the
whole shape, with the optional parts bracketed:

```uc,fragment
/// The objective of the pattern, as a doc comment above the declaration.
usecase <Name> {
    subject <name>: <Type> {
        occurrence <name>;
        occurrence <message>.<source|target>;
        perform <name>: <operation> requires [ <Capability> ] when ( <condition> );
    }
    actor <name>
    actor <name>: <Type> {
        occurrence <name>;
    }
    message <name> of <Type> from <participant>.<anchor> to <participant>.<anchor>;
    first <node> then <node>;
    realizes <alias>.<Service>.<operation>;
}
```

After its name, each construct may drop the parts it does not need. A step may
omit the operation, in which case the step name is the operation name, and it
may omit `requires` and `when`. A message may omit its payload and either
endpoint. A scenario declares `realizes` only when it is implemented by an
application service.

The four kinds of import, one line each:

```uc,fragment
import "support.mox"
import "support.ddd" as support
import "support.actor"
import "support.evt"
```

A message may omit its payload and one or both endpoints; an end it omits is
owned by a participant's occurrence declaration instead:

```uc,fragment
message escalate of Ticket;
occurrence escalate.source;
occurrence escalate.target;

message promptHandover of HandoverPrompt from Drone.recipientVerified
    to Recipient.promptAcknowledged;

realizes handover.HandoverService.releaseParcel;
```

Statement lines — `occurrence`, `perform`, `message`, `first … then …`, and
`realizes` — end with `;`. Participant lines and the `usecase` block do not;
`pherolang fmt` writes the canonical `;` on every statement line whether or not
the source had one.

## Semantics

**Imports.** An import names a file the project provides. The kind of the
imported file decides which of the four sources it belongs to: a domain model
supplies types, an application design supplies service operations, an actor
policy supplies principals and capabilities, and an event contract supplies
payload names. Imported domain models are compiled together, and every type a
scenario names resolves in that union. An `as` clause is allowed only on a
design import; a `realizes` line that names a design uses that alias.

**Subject.** `subject <name>: <Type>` names the classifier whose behavior the
scenario specifies. Its type is stored resolved against the imported domains.
At most one subject is declared, and only the subject performs steps. A
scenario with no subject declares actors only.

**Actors.** `actor <name>` is a policy principal: the name must match an actor
declared in an imported actor policy, and only such participants take part in
capability checks. `actor <name>: <Type>` is a domain-typed participant: an
instance of an imported class that takes part in the scenario but contributes
nothing to capability checks.

**Occurrences.** `occurrence <name>;` declares a lifetime point the participant
witnesses. `occurrence <message>.<source|target>;` declares that the
participant owns that end of the named message, which must be declared in the
same use case. Occurrence names are unique within one participant. An
occurrence carries no payload, channel, or subscription; it is a point in this
scenario, never an event in an event contract.

**Messages.** `message <name> (of <Type>)? (from <endpoint>)? (to <endpoint>)?;`
declares one communication. An endpoint is `participant.anchor`, where the
anchor names an occurrence or a step declared by that participant; when a name
matches both, the occurrence is meant. An endpoint is always
`participant.anchor`: a bare participant name is not an endpoint, because a
message end is itself a temporal point and must attach to one. A message is sent
at the end of its source anchor and received at the start of its target anchor.
Each end is bound exactly once: an inline endpoint on the message, or exactly
one participant declaring `occurrence <message>.<side>` — never both, never
neither. A payload name resolves as a domain type first (primitives are not
payload types), then as an event name across the imported event contracts.

**Steps.** `perform <name>: <operation>;` names a step the subject performs,
with the operation resolving on the subject's class and along that class's
`extends` chain. `requires [ <Capability> … ]` states capabilities that a
participating policy actor must hold. `when ( <condition> )` constrains the step
with an expression over the subject; the condition is typed as boolean with the
subject's class as the self type. The guard is written last, before the `;`.

**Successions.** `first <node> then <node>;` states that the first node
completes before the second begins. `start` and `done` are the scenario's
implicit initial and final nodes. A node is a step name, one end of a message
(`m.source` or `m.target`), an occurrence or step qualified by its participant
(`participant.x`), or a bare message name, which means that message's receiving
end. A bare occurrence name must be unique across the scenario's participants.

**Realization.** `realizes <alias>.<Service>.<operation>;` names the
application service operation that implements the scenario. The alias must name
an aliased design import, the service must exist in that design, and the
operation must exist on that service.

## Constraints

| Rule | Consequence when broken |
|---|---|
| Imports come before the first use case. | An import after a use case is reported and dropped. |
| At most one subject per use case. | A second `subject` line is reported; the first is kept. |
| A bare actor names an actor of an imported actor policy. | With no actor policy provided, a bare actor is always an error. |
| Steps belong to the subject. | A step on an actor is reported, naming the step and the actor. |
| A step's operation exists on the subject's class, walking `extends`. | An unknown operation is reported. A class imported from a Rune model carries no class operations, so such a subject cannot take steps. |
| A step guard parses and types as boolean over the subject. | The step is reported; the scenario does not compile. |
| Every message end is bound exactly once. | Zero bindings is an unbound-end error; two bindings are a bound-more-than-once error. |
| An endpoint names a declared participant and one of its occurrences or steps. | An endpoint naming anything else is reported. |
| A payload resolves in the imported domains first, then by event name. | An unknown name is reported; a name found in more than one event contract is reported as ambiguous. |
| Every succession node resolves within the scenario. | An unresolvable node is reported, and the message says how to qualify it. |
| With a succession declared, the graph is acyclic and every step and message end is reachable from `start`, with `done` reachable. | A cycle, an unreachable step, or an unreachable message end is reported, listing the offending nodes. |
| Participants, steps, and messages share one namespace per use case; occurrences are unique per participant; use case names are unique per file. | A duplicate name is reported. |
| Every required capability exists and is held by a participating policy actor. | A missing capability is reported, and so is a capability the participants are only forbidden to hold. |
| Two required capabilities must not form a pair of a `never_both` constraint in the imported policy. | A scenario requiring both is reported. |
| `realizes` names an aliased design import, an existing service, and an existing operation on it. | Each miss is reported naming what was not found. |
| An `as` clause appears only on a design import. | An alias on any other import is reported. |

A use case with no successions makes no ordering promise and skips the graph
checks. Every error drops the compiled model; warnings do not.

## Examples

A subject, a domain-typed actor, and one fully anchored message:

```uc,fixture=examples/support
import "support.mox"

usecase ClarifyTicket {
    subject Ticket: Ticket {
        occurrence clarificationRequested;
    }
    actor Reporter: User {
        occurrence clarificationSent;
    }
    message requestInfo of Ticket from Ticket.clarificationRequested to Reporter.clarificationSent;
    first start then requestInfo.source;
    first requestInfo.source then requestInfo;
    first requestInfo then done;
}
```

A subject-only scenario whose step carries a boolean guard:

```uc,fixture=examples/support
import "support.mox"

usecase ApproveRefund {
    subject Ticket: Ticket {
        perform approve: approveRefund when (refundCents > 0);
    }
    first start then approve;
    first approve then done;
}
```

A message whose two ends are owned by one participant, so the message needs no
inline endpoint:

```uc,fixture=examples/support
import "support.mox"

usecase EscalateInternal {
    subject Ticket: Ticket {
        occurrence escalate.source;
        occurrence escalate.target;
    }
    message escalate of Ticket;
}
```

The last example declares no succession, so it promises no order: the two ends
exist, and the scenario says nothing about when they happen.

## Diagnostics

None. Each check reports a message naming the offending construct and its use
case rather than an allocated code; the exact wording is part of the
[engineering reference's validation contract](../../../USECASES.md).

## Compatibility

- The compiled model is versioned. The current version is 2, and a reader
  rejects a document with any other version.
- Member names are camelCase, and empty collections are omitted, so adding a
  member later leaves models that do not use it unchanged.
- Message names, succession references, and `realizes` bindings are kept as
  authored. Participant types are stored resolved to their package-qualified
  name.
- A participant with no type name is a policy principal, not an instance of a
  domain class. That is the signal a consumer uses to tell the two actor forms
  apart.
- A step's `when` guard is carried as the trimmed interior of its parentheses,
  written out verbatim, and omitted when the step has no guard.
- A message end records which kind of anchor it matched: an occurrence, a step,
  or one end owned by the message itself.
