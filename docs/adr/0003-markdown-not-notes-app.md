# 0003 — Verso produces Markdown, but owns no note system

- **Status:** accepted
- **Date:** 2026-10-21
- **Deciders:** polar

## Context

Reading for study produces notes. Every reader that has tried to own them has ended up building an
editor, a sync engine, a conflict resolution model, and a plugin API — and has usually done all four
worse than the tools people already use.

The user's real requirement is weaker and more specific: **when they understand something, they want
to capture it together with where it came from, without breaking their existing habit.**

## Options considered

### Option A — Built-in note editor

- Pros: one app, integrated search, no copy-paste.
- Cons: an editor is a product on its own. Rich text, images, links, search, sync, and version
  history are each larger than the reading features. It also forces a file format on users.

### Option B — Integrate with one specific app (Obsidian plugins, Notion API)

- Pros: deep integration, a flagship feature to advertise.
- Cons: one app's format and release cycle become ours. A plugin must be maintained per app, and
  the integrations rot independently of the reader.

### Option C — Write standard Markdown files into a folder the user chooses

- Pros: works with every editor; the user's notes stay portable; the reader needs no renderer.
- Cons: still requires path configuration, file watching, and a story for what happens when the user
  edits the file underneath us.

### Option D — Produce a portable block on demand, store nothing

- Pros: no configuration, no format, no sync, no watching. The reader stays a reader.
- Cons: weaker "my notes are in the app" feeling; the user must paste.

## Decision

We will **own no notes**. Verso assembles a citation-ready block — quoted text, page number,
question, answer, and (for selections) the cropped image — and puts it on the clipboard as both
Markdown and HTML. Persistence, organisation, and linking belong to whatever the user already uses.

A lightweight in-session history keeps recent exchanges available; it is a convenience, not a file
format.

## Consequences

- No editor, no sync protocol, no file watcher, no format migration — a large amount of surface area
  removed from the project.
- The differentiator becomes the **quality of the assembled block**, which is a much smaller and
  more testable thing to get right than an editor.
- Users who want automatic writing into a vault can be served later by an optional exporter, which
  is additive and does not change this decision.
- This decision is the reason `verso-core` has no filesystem dependency beyond reading the PDF.

## References

- [`docs/ARCHITECTURE.md`](../ARCHITECTURE.md) §6–8.
