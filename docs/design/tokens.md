# Design tokens

The default theme is **white, lilac, black**. One accent, used sparingly; ink dark enough to read for
hours; a page that stays white.

Two rules:

1. **No raw values outside this file.** A hex code in a component is a bug.
2. **Contrast is a requirement, not a preference.** Body text meets WCAG AA (4.5:1) against its
   surface in both themes. If a token pair cannot meet it, the pair is wrong.

## The lilac ramp

A single hue, ten steps. Everything purple in the product is one of these; nothing is mixed ad hoc.

| Step | Value | Used for |
|---|---|---|
| `--lilac-50` | `#F7F4FC` | panel background, wells |
| `--lilac-100` | `#EFE9F9` | hover on light rows |
| `--lilac-200` | `#E8E0F7` | chip fill, selection tint, borders |
| `--lilac-300` | `#DCCFF3` | focused field border |
| `--lilac-400` | `#CBB8EC` | **location indicator**: in-page tint, scrollbar markers |
| `--lilac-500` | `#B49BE2` | disabled primary, decorative rules |
| `--lilac-600` | `#9A7AD3` | secondary actions |
| `--lilac-700` | `#7F5CBF` | primary action fill (white text, 5.0:1) |
| `--lilac-800` | `#5F4697` | primary hover, headings on tint |
| `--lilac-900` | `#3C2C5E` | text on tint, deep accents |

`#CBB8EC` and `#E8E0F7` are the two the theme is built around: the first is how a location announces
itself without shouting, the second is how an attachment sits in the composer.

## Colour

### Light (default)

| Token | Value | Used for |
|---|---|---|
| `--surface` | `#FFFFFF` | page canvas, panels |
| `--surface-muted` | `--lilac-50` | AI panel background |
| `--surface-sunken` | `--lilac-100` | inputs, pressed states |
| `--border` | `--lilac-200` | hairlines, chip outlines |
| `--border-strong` | `--lilac-300` | focused fields, drag handles |
| `--ink` | `#14101A` | body text |
| `--ink-muted` | `#6B6378` | secondary text, metadata |
| `--ink-faint` | `#9A93A6` | placeholders, disabled, status line |
| `--accent` | `--lilac-700` | primary actions |
| `--accent-hover` | `--lilac-800` | hover on primary actions |
| `--accent-strong` | `--lilac-400` | location tint, scrollbar marker |
| `--accent-tint` | `--lilac-200` | chip fill, selection tint |
| `--accent-soft` | `--lilac-50` | quiet purple surfaces |
| `--accent-ink` | `#FFFFFF` | text on `--accent` |
| `--danger` | `#B23A4E` | destructive actions, errors |
| `--warning` | `#8A5A00` | cost warnings |

### Dark

| Token | Value | Used for |
|---|---|---|
| `--surface` | `#0F0C14` | page surround, panels |
| `--surface-muted` | `#17121F` | AI panel background |
| `--surface-sunken` | `#1F1829` | inputs |
| `--border` | `#2B2337` | hairlines |
| `--border-strong` | `#3D3350` | focused fields |
| `--ink` | `#F4F0FA` | body text |
| `--ink-muted` | `#A79FB5` | secondary text |
| `--ink-faint` | `#6E6679` | placeholders, status line |
| `--accent` | `--lilac-400` | primary actions (dark ink on it) |
| `--accent-hover` | `--lilac-300` | hover |
| `--accent-strong` | `--lilac-500` | location tint, scrollbar marker |
| `--accent-tint` | `#2A2140` | chip fill |
| `--accent-soft` | `#1A1424` | quiet purple surfaces |
| `--accent-ink` | `#14101A` | text on `--accent` |
| `--danger` | `#E4778B` | errors |
| `--warning` | `#D8A54A` | cost warnings |

The page itself stays white in both themes. A PDF is a printed object; tinting it is a bug report.

## Type

| Token | Value |
|---|---|
| `--font-ui` | system stack: `-apple-system, "Segoe UI", "Noto Sans SC", sans-serif` |
| `--font-reading` | user-configurable; defaults to `--font-ui` |
| `--text-xs` | `12px / 1.4` |
| `--text-sm` | `13px / 1.5` |
| `--text-base` | `15px / 1.6` |
| `--text-lg` | `17px / 1.6` |
| `--text-xl` | `20px / 1.4` |
| `--text-2xl` | `24px / 1.3` |
| `--weight-regular` | `400` |
| `--weight-medium` | `500` |
| `--weight-strong` | `600` |

CJK text needs slightly more line height than Latin; `--text-base` is already set for it. Numerals in
the status line use `font-variant-numeric: tabular-nums`.

## Space, radius, elevation

| Token | Value |
|---|---|
| `--space-1` … `--space-8` | `4, 8, 12, 16, 24, 32, 48, 64` px |
| `--radius-sm` | `6px` — controls |
| `--radius-md` | `10px` — panels, cards |
| `--radius-full` | `999px` — chips, pills |
| `--shadow-panel` | `0 1px 2px rgb(20 16 26 / 6%), 0 8px 24px rgb(20 16 26 / 8%)` |
| `--shadow-float` | `0 2px 6px rgb(20 16 26 / 10%), 0 12px 32px rgb(20 16 26 / 14%)` |

## Motion

| Token | Value |
|---|---|
| `--duration-fast` | `120ms` |
| `--duration-base` | `180ms` |
| `--ease` | `cubic-bezier(0.2, 0, 0, 1)` |

Respect `prefers-reduced-motion`: transitions collapse to `0ms`, and nothing relies on motion to
communicate state.

## Applying them

```css
:root {
  --lilac-50: #f7f4fc;
  --lilac-200: #e8e0f7;
  --lilac-400: #cbb8ec;
  --lilac-700: #7f5cbf;
  --surface: #ffffff;
  --ink: #14101a;
}

@media (prefers-color-scheme: dark) {
  :root {
    --surface: #0f0c14;
    --ink: #f4f0fa;
    --accent: var(--lilac-400);
  }
}
```

A theme switch sets `data-theme="light" | "dark" | "system"` on the root element; `system` follows
the media query above.
