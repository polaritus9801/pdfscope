# 0028 — Collapsible thinking and a conversation tree

**Status:** accepted.

## Thinking

- Thinking is a `<details>` in both states.
- While reasoning streams, it starts open and can be collapsed by the reader at any time.
- When the first answer token arrives, the block collapses automatically (and its header switches
  from “思考中…” to “思考 N 秒”). This happens when thinking ends, not when the whole answer ends.
- Opening it again after the answer started is respected; it is not auto-collapsed a second time.
- The header row stays white; only the body below the 1px rule is cream (0027).
- The answer identity rail no longer repeats the duration; the only place seconds appear is the
  thinking header.

## Editing and branches

Every user question bubble has a small icon-only edit control at its lower right. It edits exactly
that turn; there is no edit action under the composer and no text label. Sending it no longer
truncates history: it creates a **sibling node**, and the old path remains reachable.

### Model

A conversation is a tree of turn nodes:

```
TurnNode { id, parent, active_child, turn }
ConversationTree { nodes, active_path, next_id }
```

- `active_path` is the path currently shown as `Conversation.turns`; `turns` stays as a serialized
  cache for compatibility.
- A new follow-up appends a child of the active leaf.
- Edit and regenerate create a sibling at the same parent and select it; the old node and its whole
  subtree stay in `nodes`.
- Switching a branch sets the parent's `active_child` and follows that sibling's `active_child`
  chain, so the old continuation reappears.
- Old conversations without a tree are converted from their linear `turns` on load.
- Tree and active path are saved with the conversation and restored on reload.

### Navigation

Under the same question bubble, immediately left of its edit icon, a `< n / total >` control appears
when that node has siblings. It switches the versions of that specific question, follows the chosen
sibling's active continuation and therefore keeps the old branch reachable. It is per node, hidden
when the node has no alternative and while a turn streams.

## Acceptance

- During reasoning: the block can be collapsed manually; the first answer token collapses it.
- Completed thinking shows one duration only, in its white header.
- Each question bubble carries an icon-only edit at its lower right; no edit icon remains on answers
  and nothing edit-related sits under the composer.
- Editing or regenerating creates a sibling; the bubble's `< n / total >` (left of its edit icon)
  cycles that node's alternatives and the old continuation remains in the tree.
- Switch branch, reload: the selected branch is restored.
- `just check` green.
