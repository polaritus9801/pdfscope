# 0001 — Repository foundation

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi (direct request: "把这个基建任务做好，给这个项目打一个非常好的地基")

## Goal

Make the repository ready for product work: a workspace whose boundaries are enforced by the
compiler, a CI pipeline that rejects regressions, a written protocol for how changes are proposed
and accepted, and enough documented context that a new contributor — human or model — can make a
correct change without archaeology.

After this spec, a contributor can open the repository, run one command, and get a green build; and
can read one file to know how work is supposed to proceed.

## Non-goals

- **No product behaviour.** No PDF parsing, no OCR, no model calls, no UI. Those arrive with their
  own specs (0002 and later).
- **No desktop shell yet.** `verso-desktop` and `ui/` are mapped and documented but not created;
  creating them without their spec would violate the gate this spec establishes.
- **No release pipeline.** Tagging and signing need the desktop app to exist first.
- **No plugin system, no sync, no account model.**

## Behaviour

- `cargo check --workspace` succeeds from a clean clone with a stable toolchain.
- `just check` runs format, clippy, and tests, and fails on any of them.
- `just setup` configures the repository git hooks.
- A commit that changes crate sources without a staged spec is rejected by the hook, unless the
  message contains `[trivial]`.
- CI runs the same checks on Linux, macOS, and Windows, plus license/advisory checks and a
  documentation build.

## Data model

One SQLite migration, additive, defining the minimum the library needs:

- `books(id, sha256 UNIQUE, path, title, page_count, added_at)`
- `reading_progress(book_id PRIMARY KEY, last_page, updated_at)`

Both tables are an index over the disk, not a source of truth: deleting the database loses nothing
that cannot be rebuilt.

## Interface

Public surfaces created here, deliberately minimal:

- `verso_core::{BookId, PageIndex, Rect, CoreError}` — types only.
- `verso_proto::HealthResponse` — the one wire type both shells share today.
- `verso_ai::{ProviderConfig, Secret}` — configuration plus a type whose `Debug` redacts.
- `verso_api::router(state)` — serves `GET /healthz`.
- `verso-server` — a binary that binds `PORT` (default 8787) and serves the router.

No other public API is added.

## Test plan

- `verso-core`: page-index display is one-based, `Rect` rejects out-of-range values, serde round-trip.
- `verso-ai`: `Secret` debug output never contains the secret; config reject empty model.
- `verso-store`: an in-memory database migrates, inserts a book, and reads it back.
- `verso-api`: `GET /healthz` returns 200 and a JSON body with the version.
- Hook: a scripted check that a fake staged change without a spec is refused.

## Risks

- **Chosen dependencies age quickly.** Mitigation: `Cargo.lock` is committed, `cargo deny` runs in
  CI, and Dependabot keeps versions moving.
- **The gate can annoy on small changes.** Mitigation: the `[trivial]` exemption and a documented
  `--no-verify` escape hatch for emergencies.
- **Workspace boundaries can be documented but not enforced.** Mitigation: `verso-core` carries no
  framework dependency, so an accidental import fails to compile rather than passing review.

## Assumptions

- Rust stable ≥ 1.85 is available on contributors' machines; `rust-toolchain.toml` pins it.
- SQLite is bundled by `sqlx`, so no system package is required on any target.
- `just` is available or installable; the underlying commands are also documented verbatim.

## Deviations

- The gate is implemented as a `commit-msg` hook rather than `pre-commit`. Git passes the commit
  message file only to `commit-msg`, and without it the `[trivial]` exemption cannot be evaluated —
  the first implementation rejected every trivial commit, which the end-to-end check caught.
- `sqlx` needs its `macros` feature for the embedded `migrate!` macro, even though no compile-time
  query macro is used. Enabled deliberately; the alternative (a runtime migration path) would break
  when the desktop binary runs from an app bundle.
- `rustfmt.toml` omits `imports_granularity`: it is a nightly-only option, and a repository that
  cannot be formatted with its own pinned toolchain is a repository that fights itself.

## Open questions

None blocking.
