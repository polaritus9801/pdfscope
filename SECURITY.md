# Security policy

## Supported versions

PDFScope is pre-1.0. Only the latest release receives security fixes.

## Reporting a vulnerability

Do not open a public issue. Use GitHub's private vulnerability reporting on this repository
("Security" → "Report a vulnerability"), or email the maintainer listed in `Cargo.toml`.

Include:

- what an attacker can do, and what they need to already have;
- the version and platform;
- a reproduction, if one exists.

You will get an acknowledgement within a few days and an assessment within two weeks. If the issue
is accepted, a fix and an advisory are prepared together, and you are credited unless you ask not
to be.

## Threat model

PDFScope is a local application. The relevant assets are the user's documents and their model
credentials.

| Asset | Where it lives | Protection |
|---|---|---|
| PDF files | User's disk | Read only; never uploaded without an explicit action |
| Model API keys | `api-key` file, mode 0600, in the app config directory | Never logged, never written to the library database, redacted in `Debug` |
| Library index | Local SQLite | Rebuildable; contains paths and titles, not document text |
| Document text | Sent to the provider the user configured | Only on an explicit ask; the target endpoint is shown before the first send |

Out of scope for this project: a compromised operating system, a malicious model provider, and the
security of the user's own note-taking application.
