# 0036 — Mention picker and attachment ownership

**Status:** accepted.

## Floating picker

The `@` mention picker is absolutely positioned above the composer:

```css
.mention-picker {
  position: absolute;
  right: var(--space-3);
  left: var(--space-3);
  bottom: calc(100% + 6px);
  max-height: min(240px, 42vh);
}
```

Opening it therefore does not change the composer/context-shelf height or push the transcript
upward. It floats over the context shelf and scrolls internally.

## Token insertion

`makeToken(key, label)` now creates the pill directly:

```html
<span class="mention" contenteditable="false" data-key="p:3">@p.3</span>
```

The previous implementation called `makeToken` from inside itself, so choosing a picker row
overflowed the stack and never inserted a visible token.

## Attachments own references

The context shelf is the owner of references, not the inline token.

- Deleting an `@` pill from the editor removes only that text occurrence. The corresponding passage,
  region, page, image, file or answer stays in the context shelf and is still sent.
- Removing a reference from the shelf explicitly removes its token, so the editor never keeps an
  orphan.
- Clear-context still clears both the shelf and all tokens.

## Answer time

`.turn-time` is 11px (previously 10px); layout and colour are unchanged.

## Acceptance

- Typing `@` floats the picker without changing composer height.
- Choosing a picker item inserts a visible `@...` pill.
- Deleting that pill leaves the shelf count/reference intact.
- Removing the shelf reference also removes the pill.
- Answer completion time renders at 11px.
