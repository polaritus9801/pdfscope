# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the version is `0.x`,
minor releases may contain breaking changes; they are always called out.

## [Unreleased]

### Added

- Workspace layout with one-way crate dependencies: `core`, `proto`, `store`, `ai`, `api`, `server`.
- Domain primitives: content-hash book identity, zero-based page indices that display one-based,
  normalised page rectangles with a validated invariant.
- Credential type whose `Debug` cannot leak the secret.
- SQLite library schema with an in-memory mode for tests.
- `GET /healthz` served by both shells from one router.
- CI: formatting, clippy with warnings denied, tests on Linux/macOS/Windows, documentation build,
  license and advisory checks, and a spec gate for pull requests.
- Pre-commit hook that refuses product code without a spec.
- Repository protocol in `AGENTS.md`, architecture in `docs/ARCHITECTURE.md`, decision records in
  `docs/adr/`, and specification templates in `docs/specs/`.

[Unreleased]: https://github.com/OWNER/verso/commits/main
