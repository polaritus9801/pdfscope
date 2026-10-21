# Contributing to Verso

Thanks for considering it. This project is small on purpose, and the strongest contribution is
usually the one that removes something.

Before writing code, read [`AGENTS.md`](AGENTS.md). It is the protocol this repository runs on, and
it applies to maintainers as much as to outside contributors. The short version:

1. Open an issue describing the behaviour you want. If it is substantial, expect a spec first.
2. Get the spec accepted **before** implementing.
3. Keep the pull request to one vertical slice.
4. `just check` must pass.

## Setup

```sh
git clone https://github.com/OWNER/verso.git
cd verso
just setup     # configures git hooks, prints the toolchain versions
just check     # fmt + clippy + tests + docs
```

Rust is pinned by [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` will install the right
version automatically. [`just`](https://github.com/casey/just) is optional — every recipe in
[`justfile`](justfile) is a short shell command you can run directly.

## What we are looking for

- **Correctness over coverage.** A test that names a real failure mode beats ten that assert
  getters.
- **Small, reviewable diffs.** If a pull request cannot be read in one sitting, it will be split.
- **Documentation that explains why.** Code comments restating the code are removed in review.
- **Dependency scepticism.** A new crate needs a sentence in the pull request saying why the
  existing ones cannot do it.

## What we are not looking for

- A note editor, a sync service, or a citation manager. Those are deliberate non-goals; see
  [`docs/adr/0003-markdown-not-notes-app.md`](docs/adr/0003-markdown-not-notes-app.md).
- Large refactors with no behaviour change. They are welcome as a separate, scoped discussion.
- Vendored component libraries.

## Commits

```
<type>(<scope>): <imperative summary>

Why this exists and what a reviewer should look at first.

Spec: docs/specs/0002-your-feature.md
```

Types and scopes are listed in [`AGENTS.md`](AGENTS.md) §4. The `Spec:` trailer is enforced by the
`commit-msg` hook; `[trivial]` in the message opts out for genuinely trivial changes.

## Reporting bugs

Include the version (`verso-server --version` or the app's About screen), the operating system, and
the smallest PDF that reproduces the problem. If the file is private, describe its properties —
scanned or text layer, page count, language, encryption — and we will find or synthesise an
equivalent.

Security issues go through [`SECURITY.md`](SECURITY.md), never a public issue.

## License

By contributing you agree that your work is licensed under the [MIT License](LICENSE).
