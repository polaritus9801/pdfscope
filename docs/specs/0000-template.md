# NNNN — <title>

- **Status:** draft <!-- draft | accepted | implemented | superseded by NNNN -->
- **Author:** <!-- name -->
- **Date:** YYYY-MM-DD
- **Approved by:** <!-- required before status: accepted -->

## Goal

One paragraph. Written from the user's point of view: what can they do afterwards that they cannot
do now?

## Non-goals

Bullet list. What this deliberately does not do. Being explicit here is what keeps the diff small
and the review short.

## Behaviour

Observable outcomes. Cover the unhappy paths: no text layer, unreadable file, model unavailable,
cancelled mid-stream, empty selection.

## Data model

Schema or type changes. If a migration is needed, say whether it is additive and what the rollback
is.

## Interface

Commands, HTTP routes, events, error shapes. Signatures, not prose.

## Interaction design

Required for anything a person can see or touch. Delete this section only if the change has no
interface at all, and say why.

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| … | … | … | … | … | … | … | … | … |

**Alternatives considered.** At least two, and why this one won.

- Chosen: …
- Rejected: … because …

**Discoverability.** How a first-time user finds it without documentation.

**States.** If there are modes, what is legal in each, and what becomes impossible.

**Cost of being wrong.** What the user loses if this misbehaves, and what prevents that.

## Design review

- Reviewed with: … on …
- Decisions: what changed as a result

## Test plan

- Unit: …
- Integration: …
- End-to-end: …

Name the cases. "Tests added" is not a test plan.

## Risks

What could be wrong, how it would show up, and how it would be rolled back.

## Assumptions

Anything that could not be verified while writing this. Mark unverified assumptions explicitly;
they are the first thing a reviewer should check.

## Deviations

Filled in during implementation. Empty at review time is normal.

## Open questions

Questions that must be answered before `status: accepted`.
