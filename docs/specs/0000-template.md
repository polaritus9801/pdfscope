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
