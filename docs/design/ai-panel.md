# The AI panel, visual refresh

**Status:** accepted. Decisions: provider marks are official-style logos with a coloured-monogram
fallback; questions are right-aligned bubbles with flat answers carrying an identity rail; the model
picker is a custom popover.
**Scope:** the AI panel only; the reader and the palette are untouched.

## 1. Why it feels behind

The panel is functionally dense but visually it reads as a form: a row of text, a plain
text field and a stack of small rows. Mature assistants communicate four things the panel barely
does: **who is answering**, **where you are in the task**, **what is attached**, and **what can I do
next**. The missing provider icon is a symptom: there is no visual language for identity.

A second problem is inconsistency. Icons mix stroke widths and sources (inline SVG, text glyphs such
as `✕ ★ ✎`, emoji `✦`), actions are text buttons in some places and icon buttons in others, and the
header, shelf and composer use three different paddings.

## 2. Comparison

| Dimension | Mature assistants | PDFScope today | Target |
|---|---|---|---|
| Identity | provider mark + model, always visible | one muted string `mimo-v2.6-flash` | provider tile + model chip with a menu |
| Empty state | mark, greeting, 3 suggestion cards | one muted line | mark + 3 task cards |
| Message roles | avatar and label, or a distinct bubble | question is plain text, answer is plain Markdown | question bubble, answer with identity rail |
| Reasoning | collapsed "thought for Ns" row | `<details>` with plain text | outlined pill with brain icon and timer |
| Composer | one contained field, tool icons, round send | textarea + separate scope row + plain buttons | contained field, toolbar, circular send/stop |
| Context | attachment cards grouped by kind, clear all | six horizontal chip rows | shelf with header, counts, type icons, clear |
| Actions | icon-only row on hover with tooltips | text actions, some icons | one consistent icon action row |
| States | skeleton, streaming caret, error with retry | empty panel, plain error card | skeleton, caret, structured error |
| Settings | provider cards, connection state | form rows and a select | provider cards with marks and status |
| Icon system | one family, one stroke | mixed SVG + glyph + emoji | one 1.6 px stroke icon set |
| Motion | subtle enter/fade | almost none | 120 ms fades, no layout shift |

## 3. Principles

1. **Identity first.** Every turn knows who produced it; the header always shows the active provider
   and model.
2. **One icon language.** 16/18 px, 1.6 stroke, round caps; no text glyphs; provider marks are the
   only full-colour elements.
3. **Quiet chrome, loud content.** Borders and surfaces recede; the answer and the citation chips are
   the strongest elements.
4. **One primitive per job.** A button, an icon button, a chip and a card. Nothing else.
5. **States are designed, not default.** Hover, focus, streaming, empty, error, disabled, loading.

## 4. Identity: providers and models

- **Header left:** a 28 px provider tile (rounded 8 px) then two lines:
  - `AI 助手` 13 px/600
  - model chip `MiMo v2.6 Flash ▾` 11 px, with a chevron; clicking opens the model menu.
- **Provider marks:** official-style simplified marks for DeepSeek (whale), OpenAI (knot),
  Anthropic (spark), Moonshot/MiMo (moon), Google (four-colour G); a coloured monogram tile is the
  fallback for unknown providers. Search providers reuse the same mark family in Settings.
- **Model menu:** a 280 px popover anchored to the chip: provider rows, model rows with a check,
  a footer `管理模型` opening settings. This replaces the native `<select>`.
- **Turn identity:** a 24 px provider mark beside each answer, with `模型名 · 刚刚` in 11 px muted.
- **Loading/streaming:** the mark gets a soft pulse; no spinner.
- **Settings:** search provider becomes a two-column card list: mark, name, a one-line description and
  a `已配置/未配置` badge.

## 5. Messages

- **Question:** right-aligned bubble, `--accent-soft`, radius 16 px with a 4 px tail, max 86 % width,
  14 px. Hover shows copy and edit icon buttons below the bubble.
- **Answer:** full width, identity rail (mark + model), body 15 px/1.7, 8 px gap; no bubble.
- **Reasoning:** a 28 px outlined pill `思考 12.4 秒` with a brain icon; expands into a muted block.
- **Streaming:** a 2 px blinking caret at the end of the answer; composer's send becomes stop.
- **Actions:** one row of 28 px icon buttons with tooltips: 复制, 复制引用, 重试, 编辑, 分支, 引用此回答.
  They fade in on hover/focus-within and stay visible on touch.
- **Empty state:** provider mark 40 px, `从这页开始问`, three suggestion cards: 总结本页 / 解释选中段落 /
  和上一轮对比. Cards are keyboard focusable.

## 6. Composer and attachments

```
┌ field ───────────────────────────────────────────┐
│ 问点什么…                                         │
│  @   📎   🌐   全文·本页·引用            (↑)     │
└──────────────────────────────────────────────────┘
  上下文 · 6 项                             清空
  ▤ p.20 "coordinate frames…"  ▤ p.31 …           ← horizontal
  [thumb] [thumb]                                  ← images
  p.12–14 ×   ✦ 回答 3 ×                           ← pages/answers
```

- The field is one card (radius 14, border, focus ring 3 px accent at 12 %), min 56 px, max 180 px.
- The toolbar row is inside the card: `@`, attach, web (accent when on), scope segmented control on
  the left; token estimate and a 32 px circular send button on the right.
- Send: arrow-up icon; disabled when empty; becomes a stop square while streaming.
- Shelf sits above the field, with a header (`上下文 · N 项`, `清空`) and groups; each group scrolls
  horizontally and uses a type icon. Page ranges collapse to `p.12–14`.
- Scope control keeps three icons but gets a raised active segment, a count badge and keyboard focus;
  no text labels in the control, counts in the tooltip.

## 7. Icon set

One family, drawn on a 24 grid, 1.6 stroke, round caps:

`plus, history, chevron-down, collapse, expand, attach, globe, at, send, stop, copy, retry, edit,
branch, quote, trash, pin, search, close, check, page, pages, image, file, link, sparkle, brain,
settings, book`

Provider marks are 24 grid filled shapes and are the only coloured icons. Implement as a Rust
`Icon` component (name, size) backed by one match, so no page carries ad-hoc SVG anymore.

## 8. Type, colour, space

- Text: 22/16/15/13/12/11; body 15/1.7; labels 11/500 uppercase tracking 0.04em.
- Colour: existing lilac palette; provider marks are the only saturated colour outside accents.
- Space: 4/8/12/16/24; header 52 px; message gap 20 px; composer padding 12 px.
- Borders: 1 px `--border`; only the focused field and the active scope segment get a stronger edge.
- Radii: 6 controls, 10 cards, 14 composer, 16 bubbles, full chips.

## 9. States and motion

- Skeleton: three shimmering lines for the first token; no spinner.
- Error: a card with icon, one-line cause, `重试` and `打开设置` when it is a key problem.
- Disabled: 40 % opacity, no pointer events, tooltip explains why.
- Focus: 2 px accent ring outside the element, never removed.
- Motion: 120 ms `ease` for fades and colour; no width/height animation; respects
  `prefers-reduced-motion`.

## 10. Phases

| Phase | Work | Risk |
|---|---|---|
| P0 Foundations | `Icon` component; provider marks; header rebuild; composer card | Low: markup/CSS only |
| P1 Messages | identity rail, question bubble, reasoning pill, action row, empty state | Medium: transcript markup |
| P2 Shelf | shelf header, grouped cards, clear all, page ranges | Low |
| P3 States | skeleton, error card, focus rings, motion | Low |
| P4 Settings | provider cards, model menu, status badges | Medium |

Each phase is its own commit, `just check` green, no behaviour change outside the panel. A manual
checklist covers light/dark at 320/420/720 px widths.

## 11. Decisions

1. **Provider marks:** official-style logos for DeepSeek, OpenAI, Anthropic, MiMo and Google, with a
   coloured monogram tile for anything unknown.
2. **Message layout:** right-aligned question bubble, full-width answers with an identity rail.
3. **Header menu:** a custom popover grouping providers and models, with a check and 管理模型; the
   native `<select>` is removed.

## 12. First slice

P0: an `Icon` component with one stroke language, provider marks, the 52 px header (tile, title,
model chip, history/new/collapse), and the contained composer card with its toolbar and circular
send/stop. Behaviour is unchanged; only markup and CSS move.
