# 0002 — Anchoring a selection to a page

- **Status:** draft
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** —

## Goal

A reader can select text in a PDF, ask a question about it, and get an answer that cites the page
the selection came from — and later find that selection again on the same page, even after the
document has been reopened, the window resized, or the file replaced with a different edition of the
same text.

## Non-goals

- No highlighting UI yet. This spec produces the anchor and its resolution, not the paint.
- No notes, no Markdown export, no Obsidian integration.
- No OCR. Pages without a text layer are out of scope until 0004.
- No cross-page selections. A selection belongs to one page; a caller that needs more asks about a
  page range instead.

## Behaviour

**Capture.** A selection produces an anchor:

| Field | Rule |
|---|---|
| `book` | SHA-256 of the file, computed once at import |
| `page` | Zero-based index of the page containing the selection |
| `rect` | Union of the selection's client rects, converted to normalised page coordinates |
| `quote` | Selected text with whitespace collapsed to single spaces, trimmed |
| `context_before` / `context_after` | Up to 20 characters of the page text on each side, whitespace-collapsed |
| `page_hash` | Hash of the page's extracted text at capture time |

**Resolve.** Given an anchor and a freshly extracted page, resolution proceeds in order and stops at
the first success:

1. `page_hash` matches and the `quote` occurs exactly once on the page → `Exact`, rect updated.
2. `page_hash` matches and the `quote` occurs more than once → disambiguate with the surrounding
   context; if still ambiguous → `Ambiguous`.
3. `page_hash` differs and the `quote` occurs exactly once → `Relocated`, rect updated from the new
   text position.
4. Otherwise → `NeedsReview`, rect left as captured.

A resolution never returns a position for text it did not match. The four outcomes are part of the
public type, because "we are not sure" is information the UI must show.

**Edge cases.** Empty selection → refuse at capture. Selection spanning a line break → whitespace
collapsed, not rejected. Selection containing only whitespace → refuse. A quote longer than one page
→ refuse; the caller should use a page range.

## Data model

No library schema change. Anchors are not persisted in this slice; they are returned to the caller
so the persistence layer can be specified separately with its retention rules.

## Interface

```rust
pub struct Anchor {
    pub book: BookId,
    pub page: PageIndex,
    pub rect: Rect,
    pub quote: String,
    pub context_before: String,
    pub context_after: String,
    pub page_hash: PageHash,
}

pub enum Resolution {
    Exact { rect: Rect },
    Relocated { rect: Rect },
    Ambiguous { matches: u16 },
    NeedsReview,
}

pub fn capture(page: &PageText, selection: &SelectedText) -> CoreResult<Anchor>;
pub fn resolve(anchor: &Anchor, page: &PageText) -> Resolution;
```

`PageText` and `SelectedText` are inputs owned by `pdfscope-core`; their construction from PDF.js output
belongs to the desktop layer and is specified separately.

## Test plan

- Capture: single line; multi-line; leading and trailing whitespace; empty → error.
- Resolve: exact match; same text twice on a page → `Ambiguous`; moved paragraph → `Relocated`;
  reworded paragraph → `NeedsReview`.
- Property: for a random quote from a random page, `capture` then `resolve` is always `Exact`.
- Normalisation: CRLF, ideographic spaces (U+3000), and PDF ligatures each collapse identically in
  quote and page text.

## Risks

- **Whitespace and glyph normalisation is where anchoring quietly breaks.** Mitigation: normalisation
  is one function, used by both capture and resolution, and tested against real CJK and Latin pages.
- **`rect` is a hint.** The UI must not assume it is current until a resolution says so.
- **Hash collisions are irrelevant** for this use (SHA-256 over page text), and the design does not
  depend on them.

## Assumptions

- PDF.js exposes per-item text with transform matrices on all three WebViews. Unverified on
  WebKitGTK; the first test of the capture layer must be on Linux.
- `unicode-normalization` is sufficient for the ligature cases we care about. To be confirmed against
  a real book with `ﬁ`/`ﬂ` ligatures.

## Deviations

*(empty)*

## Open questions

1. Should a selection that spans a column break be one anchor or several? Proposal: one anchor with a
   rect union, because the quote is contiguous even when the layout is not.
2. Where does the page hash come from — the extracted text at import, or recomputed on each open?
   Proposal: computed at import and cached, recomputed lazily when a resolution needs it.
