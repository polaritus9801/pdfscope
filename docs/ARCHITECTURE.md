# Architecture

PDFScope is a local-first PDF reader with an AI beside the page. This document describes the shape of
the system, the reasoning behind it, and the rules that keep it that shape.

Decisions with alternatives considered live in [`adr/`](adr/). This file states the current design;
the ADRs record why.

---

## 1. Principles

1. **The page is the unit of work.** Every feature — asking, citing, citing back, remembering —
   is addressed by a book and a page. Anything that erases the page (whole-document embeddings,
   text dumps) is a last resort, not the default.
2. **Local by default.** A user's library never leaves the machine unless they explicitly send
   something to a model. No accounts, no telemetry.
3. **Boring where it counts.** Rendering, persistence, and transport use the most widely deployed
   implementations available rather than the most interesting one.
4. **Boundaries before features.** A small number of crates with one-way dependencies is worth more
   than any single feature.
5. **Delete before you add.** Every abstraction needs a second caller before it is introduced.

---

## 2. Shape

```
┌──────────────────────────────────────────────────────────────────┐
│  pdfscope-desktop (Tauri 2)              pdfscope-server (axum)        │
│  window, dialogs, keychain            HTTP + SSE, static assets  │
└───────────────┬───────────────────────────────┬──────────────────┘
                │                               │
                └───────────┬───────────────────┘
                            │
                     ┌──────▼──────┐
                     │  pdfscope-api  │  requests, SSE events, errors
                     └──────┬──────┘
              ┌─────────────┼─────────────┐
        ┌─────▼─────┐ ┌─────▼─────┐ ┌─────▼─────┐
        │pdfscope-store│ │ pdfscope-ai  │ │pdfscope-proto│
        └─────┬─────┘ └─────┬─────┘ └─────┬─────┘
              └─────────────┼─────────────┘
                     ┌──────▼──────┐
                     │ pdfscope-core  │  types and rules, no I/O
                     └─────────────┘
```

The front end (Leptos, compiled to WebAssembly) talks only to `pdfscope-api` — over Tauri IPC in the
desktop build, over HTTP in the web build. It does not know which one it is talking to.

### Crates

| Crate | Owns | Must not |
|---|---|---|
| `pdfscope-core` | Anchors, page ranges, context assembly, prompt construction | Import `tauri`, `axum`, `sqlx`, or touch I/O |
| `pdfscope-proto` | Wire types shared by client and server | Contain logic |
| `pdfscope-store` | SQLite schema, migrations, queries | Know about HTTP or models |
| `pdfscope-ai` | Provider clients, streaming, secret handling | Know about PDFs or storage |
| `pdfscope-api` | Routing, request validation, SSE framing | Contain domain rules |
| `pdfscope-server` | Composition, config, lifecycle | Contain domain rules |
| `pdfscope-desktop` | Window, dialogs, keychain, IPC bridge | Contain domain rules |

The same `pdfscope-api` handlers serve both shells. If a feature works in one shell and not the other,
the bug is in the shell, not the API.

---

## 3. The document pipeline

```
PDF file
   │
   ├─ text layer present ──► PDF.js extracts per-page text + glyph boxes
   │
   └─ scan / no text layer ─► page rendered to image ─► OCR (TSV: text + boxes)
                                          └──────────► vision model (text only, no boxes)
   │
   ▼
page text + boxes, keyed by (book, page)
   │
   ├─ selection  ──► context window (this page ± N) ──► model ──► answer + citations
   ├─ rectangle  ──► cropped image ──────────────────► vision model ──► answer
   └─ page range ──► per-page summaries ─────────────► model ──► chapter summary
```

**Why PDF.js for extraction:** it is the only widely deployed implementation that exposes a text
layer with per-item geometry, which is what makes selection, highlighting, and citation resolution
possible at all. Rendering is a commodity; geometry is not.

**Why OCR outputs boxes:** an answer that cannot be located on the page is a rumour. Boxes let a
citation become a highlight.

---

## 4. Anchors

The single most failure-prone part of the product, and therefore the most constrained.

An anchor is stored as:

| Field | Purpose |
|---|---|
| `book` | Content hash of the PDF — identity survives moves and renames |
| `page` | Zero-based page index; displayed as +1 |
| `rect` | Normalised coordinates (0..1) so zoom never matters |
| `quote` | The selected text, normalised whitespace |
| `context_before` / `context_after` | ~20 characters each, for re-anchoring |
| `page_hash` | Hash of the page's extracted text at anchor time |

Resolution tries, in order: exact quote match on the same page → quote + context → search the
neighbouring pages → give up and mark `needs_review`. It never silently lands on the wrong text.

`rect` is a *hint*. Text is the source of truth. This is why the app degrades gracefully when a PDF
is replaced with a different edition, and why it never claims more confidence than it has.

---

## 5. Asking

Three scopes, and no more:

| Scope | Context sent | Uses |
|---|---|---|
| Selection | The selection plus surrounding paragraph | "What does this mean?" |
| Page | The page (text, or its rendered image) | "What is this table saying?" |
| Range | Pages `a..=b`, chunked, each chunk labelled with its page | "Summarise this chapter" |

Each chunk carries a page label in the prompt, and the response schema asks for citations as page
numbers. This is what makes answers navigable rather than merely plausible. The full design is in
`docs/specs/0002-*` (planned).

---

## 6. Data

SQLite, one file per installation, migrations append-only.

- The **library** is an index, not a source of truth: it can be rebuilt by rescanning the disk.
- **Annotations** live beside the document, keyed by content hash and page.
- **Notes** are not stored at all by default. PDFScope produces Markdown for the user to paste into
  whatever they already use. It deliberately owns no note format.

That last point is a product decision with architectural weight: it removes an editor, a sync
protocol, and a file format from the system.

---

## 7. Front end

Leptos (client-side rendering, compiled to WebAssembly) with design tokens and headless component
primitives. The PDF view is a thin wrapper over PDF.js; the UI is not.

Rules:

- No inline styles. Tokens only.
- Every interactive element is reachable by keyboard and announces its state.
- No component library is vendored wholesale; primitives are copied in and owned.
- The visual language is calm and dense: this is a reading tool, not a dashboard.

### Interface modules (`ui/src`)

One module, one reason to change:

| Module | Owns |
|---|---|
| `main.rs` | The shell: application state, the renderer callbacks, header, rail, stage. |
| `model.rs` | `Passage`, `Region`, `Selection`, `PageSize`, the `window.__pdfscope*` payload parsers. |
| `page.rs` | One page shell: reference frames, drag maths, page bar, pointer handling. |
| `scope_ui.rs` | The page-manager dialog: thumbnails, range drags, clear. |
| `settings.rs` | The settings sheet: form state, load once, save, test. |
| `panel.rs` | The conversation: transcript, reasoning, composer, history, answer actions. |
| `viewer.js` | PDF.js behind `window.PDFScope`: rendering, zoom, selection, capture, find, thumbnails. |

`main.rs` composes the modules; it does not own their state. The renderer boundary is the same in
both directions: calls go through `window.PDFScope`, reports come back as `window.__pdfscope*`.

---

## 8. What is deliberately absent

| Not doing | Instead |
|---|---|
| A note editor | Copy a citation-ready block into the user's editor |
| A sync service | Files on disk; the user's own sync if any |
| A citation manager | Import a path; keep metadata minimal |
| Whole-library embedding search | Page-scoped retrieval first; revisit with evidence |
| Plugin system | Wait for a third-party need that actually exists |

Each of these has been considered and rejected for now. Reversal is allowed — through an ADR, not
through a commit.
