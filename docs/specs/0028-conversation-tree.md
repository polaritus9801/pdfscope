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

The editor's “编辑” lives under the composer, right-aligned. The per-answer edit action is removed;
editing always edits the last turn on the active path. Sending it no longer truncates history: it
creates a **sibling node**, and the old path remains reachable.

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

Under the composer, left of “编辑”, a `< n / total >` control appears when the active path contains a
node with siblings. It targets the **deepest such node**, so the control remains available after the
reader continues past the edited turn. Arrows cycle siblings and are disabled while a turn streams.
The control is hidden when no branch exists.

## Acceptance

- During reasoning: the block can be collapsed manually; the first answer token collapses it.
- Completed thinking shows one duration only, in its white header.
- Edit is under the composer; no edit icon remains on answers.
- Editing or regenerating creates a sibling, old turns remain in the tree, and `< n / total >` cycles
  them.
- Switch branch, reload: the selected branch is restored.
- `just check` green.
