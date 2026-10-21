# Design tokens

The default theme is **white, lilac, black** — a quiet surface, one accent, and ink dark enough to
read for hours. Everything in the interface is built from these tokens; no component invents a
colour, a radius, or a spacing value.

Two rules:

1. **No raw values outside this file.** A hex code in a component is a bug.
2. **Contrast is a requirement, not a preference.** Body text meets WCAG AA (4.5:1) against its
   surface in both themes. If a token pair cannot meet it, the pair is wrong.

## Colour

### Light (default)

| Token | Value | Used for |
|---|---|---|
| `--surface` | `#FFFFFF` | page canvas, panels |
| `--surface-muted` | `#F7F4FC` | AI panel background, wells |
| `--surface-sunken` | `#EFE9F8` | input fields, pressed states |
| `--border` | `#E6DFF2` | hairlines, chip outlines |
| `--border-strong` | `#D3C8E8` | focused fields, drag handles |
| `--ink` | `#14101A` | body text |
| `--ink-muted` | `#6B6378` | secondary text, metadata |
| `--ink-faint` | `#9A93A6` | placeholders, disabled |
| `--accent` | `#6F4FBF` | primary actions, active chip border |
| `--accent-hover` | `#5C3FA6` | hover on primary actions |
| `--accent-soft` | `#EDE7FA` | chip fill, selection tint |
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
| `--ink-faint` | `#6E6679` | placeholders |
| `--accent` | `#A98BEF` | primary actions |
| `--accent-hover` | `#BBA3F5` | hover |
| `--accent-soft` | `#241B36` | chip fill |
| `--accent-ink` | `#14101A` | text on `--accent` |
| `--danger` | `#E4778B` | errors |
| `--warning` | `#D8A54A` | cost warnings |

The page itself stays white in both themes. A PDF is a printed object; tinting it is not a feature,
it is a bug report.

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

CJK text needs slightly more line height than Latin; `--text-base` is already set for it.

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
  --surface: #ffffff;
  --ink: #14101a;
  --accent: #6f4fbf;
  /* … */
}

@media (prefers-color-scheme: dark) {
  :root {
    --surface: #0f0c14;
    --ink: #f4f0fa;
    --accent: #a98bef;
  }
}
```

A theme switch sets `data-theme="light" | "dark" | "system"` on the root element; `system` follows
the media query above.
