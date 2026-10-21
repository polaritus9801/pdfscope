# 0001 — Record architecture decisions

- **Status:** accepted
- **Date:** 2026-10-21
- **Deciders:** z1mLi

## Context

PDFScope has several decisions that are expensive to reverse and easy to forget: the rendering engine,
the UI stack, the packaging model, and the deliberate refusal to build a notes system. Without a
record, each new contributor will re-litigate them from memory, and the reasoning behind a choice
will be lost the moment the person who made it stops answering messages.

Commit messages describe *what* changed. They are a poor place to explain why an alternative was
rejected six months ago.

## Options considered

### Option A — Comments in code

Cheap, but local. A decision about the whole system has no single place to live, and comments rot
silently as the surrounding code is rewritten.

### Option B — A single long design document

Keeps everything together, but becomes stale as a whole: one wrong paragraph casts doubt on the
rest, and diffs are unreviewable.

### Option C — Architecture decision records

One short file per decision, immutable once accepted. Superseded, never edited.

## Decision

We will record every non-obvious, expensive-to-reverse decision as a numbered ADR in `docs/adr/`,
using `0000-template.md`.

An ADR is required when a decision changes a boundary (crate layout, dependency direction), a
technology choice (renderer, database, IPC), or a product stance that constrains future work. It is
not required for routine implementation choices, which belong in the spec.

## Consequences

- New contributors can read the reasoning instead of guessing it.
- Reversing a decision has a visible cost: it needs a new ADR that supersedes the old one.
- Writing an ADR takes about fifteen minutes. That is the price, and it is worth paying only because
  we write them *only* for decisions that are expensive to get wrong.

## References

- Michael Nygard, "Documenting Architecture Decisions".
- [`docs/specs/`](../specs/) — specs describe intent; ADRs describe constraints.
