# AGENTS.md

Operating protocol for anyone — human or AI — changing this repository.

Read this file fully before touching the tree. Then read, in order:

1. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — how the system is shaped.
2. [`docs/specs/`](docs/specs/) — the spec for the area you are about to change.
3. [`docs/adr/`](docs/adr/) — decisions already made; do not silently reverse them.

If those documents contradict each other, stop and ask. Do not pick the convenient one.

---

## 0. Non-negotiables

1. **No product code without an approved spec.** Implementation begins only after a file in
   `docs/specs/` has `status: accepted`. This is enforced by a git hook and by CI.
2. **Never learn a requirement and write code in the same turn.** First turn: understand, question,
   write the spec. Later: implement.
3. **No silent scope expansion.** If you notice something else worth fixing, note it in the spec's
   `Out of scope` or open an issue. Do not "just fix it" in the same change.
4. **Every commit compiles, lints, and passes tests.** `just check` is the floor, not the goal.
5. **Docs change with the code.** A behaviour change without a doc/test update is an incomplete
   change.
6. **Prefer deletion.** The best contribution to this repository is usually fewer lines.
7. **One vertical slice per commit.** No drive-by refactors, no reformatting unrelated files.
8. **Read before you write.** Never assume an API from memory: open the code, the crate source, or
   the upstream docs. Guessing is how a codebase rots.

---

## 1. The loop

Six phases. Do not skip forward. It is always acceptable to go back.

### P0 — Clarify

Ask questions. Do not propose solutions, do not write code.

- What is the user-visible behaviour that must exist afterwards?
- What must *not* change?
- What is the smallest version that would still be worth shipping?

Ask 3–5 precise questions at a time. A question the code can answer is not a question — go read
the code instead.

### P1 — Research

Read the code and the ecosystem before designing.

- Which existing modules does this touch? List the files.
- Which dependency already solves part of this? Prefer composition over new code.
- Has an ADR already decided something here?
- What breaks if this changes? Name the callers.

Output of this phase goes in the spec, not in a chat message that will be forgotten.

### P2 — Spec

Write `docs/specs/NNNN-slug.md` from [`docs/specs/0000-template.md`](docs/specs/0000-template.md).
Set `status: draft`. Then **stop and ask for approval.**

A spec is not approved because it looks reasonable. It is approved when the human says so
explicitly ("approved", "批准", "go ahead"). At that point — and only then — flip `status: accepted`
in the first commit that implements it.

### P3 — Slice

Break the spec into the smallest vertical slices that each produce something observable. State the
slice you are doing before doing it. If a slice needs more than one commit, say why.

### P4 — Implement

- Test first where the behaviour is testable in isolation; test with the change otherwise.
- Keep the diff readable. A reviewer should be able to follow it once, top to bottom.
- Leave the tree better: no dead code, no commented-out blocks, no `TODO` without an issue link.

### P5 — Verify

- `just check` — format, clippy, tests.
- `just doc` — public API documentation builds without warnings.
- UI changes: attach a screenshot or recording. "It works" is not evidence.
- Behaviour changes: show the test that fails without the change.

### P6 — Land

- Update `CHANGELOG.md` under `## [Unreleased]`.
- Record any deviation from the spec in its `Deviations` section.
- If a non-obvious decision was made, add an ADR. If one was reversed, supersede it — never edit
  history.

---

## 2. Approval gate

| State | Who sets it | May implementation start? |
|---|---|---|
| `draft` | agent | No |
| `accepted` | agent, after explicit human approval | Yes |
| `implemented` | agent, at P6 | — |
| `superseded` | agent, when replaced | — |

The gate exists because most bad code in this repository will come from a plausible-sounding design
that was never actually agreed to. If you are tempted to skip it, ask yourself what would go wrong
if the spec were wrong — that is the cost the gate is buying down.

**Trivial exception.** Changes under 10 lines that do not alter behaviour (typos, comments,
dependency bumps, formatting) may skip P2, but the commit must carry `[trivial]`. Everything else
needs a spec.

---

## 3. Writing a spec

Required sections — a spec missing any of them is not ready for review:

- **Goal** — one paragraph, user-visible.
- **Non-goals** — what this deliberately does not do. The most important section.
- **Behaviour** — observable outcomes, including error paths.
- **Data model** — schema or type changes, with migration notes.
- **Interface** — command/HTTP signatures, events, error shapes.
- **Test plan** — unit, integration, end-to-end; name the cases.
- **Risks** — what could be wrong, and the rollback.
- **Assumptions** — anything you could not verify.
- **Deviations** — filled during implementation.

Keep it under two pages. A spec nobody reads protects nobody.

---

## 4. Commit protocol

```
<type>(<scope>): <imperative summary, lower case, no period>

Why this change exists. What the old behaviour was, what it is now, and
what a reviewer should look at first.

Spec: docs/specs/0002-slug.md
```

- Types: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `build`, `ci`, `chore`, `revert`.
- Scope: crate or area — `core`, `store`, `ai`, `api`, `desktop`, `ui`, `ci`, `docs`.
- The `Spec:` trailer is required unless the change is `[trivial]`.
- One logical change per commit. If the message needs "and", split the commit.

---

## 5. Definition of done

A change is done when all of the following are true:

- [ ] Spec accepted, and this change is inside its scope.
- [ ] `just check` passes locally.
- [ ] New behaviour has a test that fails without the change.
- [ ] Error paths are handled, not `unwrap()`ed.
- [ ] Public items have doc comments.
- [ ] `CHANGELOG.md` updated.
- [ ] Deviations recorded in the spec.
- [ ] No new dependency without a note in the PR explaining why the existing ones cannot do it.

---

## 6. Architecture rules

- **Dependency direction is one-way:** `core` ← `store` / `ai` ← `api` ← `server` / `desktop`.
  `core` depends on nothing framework-shaped. It must never import `tauri`, `axum`, or `sqlx`.
- **`core` is pure.** Types and rules only: no file system, no network, no clock, no randomness
  beyond injected values. If it needs to know the time, it takes a `DateTime` argument.
- **Errors are typed at library edges.** `thiserror` in library crates; `anyhow` only in binaries
  and tests. Add context at the boundary, not deep inside.
- **No `unwrap`/`expect` outside tests.** Clippy denies it; the lint is not advisory.
- **No blocking I/O in async.** Use `tokio::fs` or `spawn_blocking`; say why in a comment.
- **Migrations are append-only.** Never edit a shipped migration; add a new one.
- **Public API is deliberate.** Adding a `pub` item is a commitment; keep fields private and expose
  what the callers actually need.
- **Feature flags stay off by default.** A flag that is always on is a branch that should be deleted.

---

## 7. Security and privacy

- API keys live in the OS keychain and never enter logs, error messages, panic output, or git.
  A type that holds a secret must implement a redacting `Debug`.
- PDF content is sent to a model only on an explicit user action. Nothing is uploaded in the
  background, ever.
- No telemetry by default, and none added without an ADR.
- File paths are untrusted input: canonicalize before use, and respect symlink policy.
- Dependency additions are reviewed for license and maintenance, not just convenience.

---

## 8. When you are stuck

- Prefer one precise question over a confident guess.
- If guessing is unavoidable, write the assumption into the spec, mark it, and say so in the final
  message. An assumption that is written down is recoverable; one that is not is a future bug.
- If the spec turns out to be wrong mid-implementation, stop, record the deviation, and ask. Do not
  quietly implement the better idea you just had.

---

## 9. Anti-patterns (this repository says no)

- Specs written after the code, to satisfy the gate.
- "While I was in there" changes.
- Abstractions with one implementation.
- Configuration added for a hypothetical user.
- Error messages that say "something went wrong".
- Tests that assert on log output or internal call counts instead of behaviour.
- Comments that restate the code; comments that explain *why* are the only ones allowed.
- Large PRs that cannot be reviewed in one sitting. Split them.

---

## 10. Map

```
crates/
  pdfscope-core     pure domain types and rules
  pdfscope-proto    wire types shared by client and server
  pdfscope-store    SQLite persistence and migrations
  pdfscope-ai       model providers, streaming, redaction
  pdfscope-api      HTTP/SSE surface over core
  pdfscope-server   the web binary
  pdfscope-desktop  Tauri shell (arrives with its own spec)
ui/                 Leptos front end (arrives with its own spec)
docs/specs/         what we are going to build
docs/adr/           what we already decided
```
