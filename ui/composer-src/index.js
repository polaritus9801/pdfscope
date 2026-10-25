// ProseMirror-backed composer.
//
// @ pills are real inline atom nodes: the document model has no position inside
// them, so the native caret cannot enter. The public Composer API is kept
// compatible with the old contenteditable implementation.

import { Schema } from "prosemirror-model";
import { EditorState, NodeSelection, Selection, TextSelection } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { keymap } from "prosemirror-keymap";
import { closeHistory, history, undo, redo } from "prosemirror-history";
import {
  chainCommands,
  deleteSelection,
  joinBackward,
  joinForward,
  splitBlock,
} from "prosemirror-commands";

let root = null;
let view = null;
let notify = null;
let allowMention = true;
let mentionShown = false;
let draftTimer = null;
let partialTimer = null;
let mentionIndex = -1;
let selectionListenerInstalled = false;
let pendingAction = null;

const schema = new Schema({
  nodes: {
    doc: { content: "paragraph+" },
    paragraph: {
      content: "inline*",
      group: "block",
      parseDOM: [{ tag: "p" }],
      toDOM: () => ["p", 0],
    },
    text: { group: "inline" },
    mention: {
      inline: true,
      atom: true,
      group: "inline",
      selectable: true,
      draggable: false,
      attrs: { key: {}, label: {}, orphan: { default: false } },
      parseDOM: [
        {
          tag: "span.mention",
          getAttrs: (dom) => ({
            key: dom.getAttribute("data-key") || "",
            label: dom.getAttribute("data-label") || dom.textContent?.replace(/^@/, "") || "",
            orphan: dom.classList.contains("orphan"),
          }),
        },
      ],
      toDOM: (node) => {
        const attrs = {
          class: node.attrs.orphan ? "mention orphan" : "mention",
          "data-key": node.attrs.key,
          "data-label": node.attrs.label,
          contenteditable: "false",
        };
        if (node.attrs.orphan) attrs.title = "对应的上下文引用已删除";
        return ["span", attrs, `@${node.attrs.label}`];
      },
    },
  },
  marks: {},
});

function emptyDoc() {
  return schema.node("doc", null, [schema.node("paragraph")]);
}

function paragraph(nodes) {
  return schema.node("paragraph", null, nodes);
}

function docFromText(text) {
  const lines = String(text ?? "").replace(/\u200B/g, "").replace(/\r\n?/g, "\n").split("\n");
  return schema.node(
    "doc",
    null,
    lines.map((line) => paragraph(line ? [schema.text(line)] : [])),
  );
}

function docFromParts(parts) {
  const blocks = [];
  let inline = [];

  const pushText = (value) => {
    const lines = String(value).replace(/\u200B/g, "").replace(/\r\n?/g, "\n").split("\n");
    lines.forEach((line, index) => {
      if (index > 0) {
        blocks.push(paragraph(inline));
        inline = [];
      }
      if (line) inline.push(schema.text(line));
    });
  };

  for (const part of parts ?? []) {
    if (part && part.key && part.label) {
      inline.push(
        schema.nodes.mention.create({
          key: String(part.key),
          label: String(part.label),
          orphan: Boolean(part.orphan),
        }),
      );
    } else if (part && typeof part.text === "string") {
      pushText(part.text);
    }
  }
  blocks.push(paragraph(inline));
  return schema.node("doc", null, blocks);
}

function serialiseDoc(doc) {
  let text = "";
  const keys = [];
  doc.forEach((block, _offset, index) => {
    if (index > 0) text += "\n";
    block.forEach((node) => {
      if (node.isText) {
        text += node.text.replace(/\u00a0/g, " ");
      } else if (node.type.name === "mention") {
        text += `@{${node.attrs.key}}`;
        keys.push(node.attrs.key);
      }
    });
  });
  return { text: text.replace(/\u200B/g, "").replace(/\n{3,}/g, "\n\n"), keys };
}

function emit() {
  if (!view) return;
  const payload = serialiseDoc(view.state.doc);
  if (notify) notify(payload.text, payload.keys);
  else window.__pdfscopeComposerInput?.(payload.text, payload.keys);
}

function hasMention(doc) {
  let found = false;
  doc.descendants((node) => {
    if (found) return false;
    if (node.type.name === "mention") found = true;
    return !found;
  });
  return found;
}

function syncEmpty() {
  if (!root || !view) return;
  const empty = view.state.doc.textContent === "" && !hasMention(view.state.doc);
  root.classList.toggle("is-editor-empty", empty);
}

function textBeforeSelection(state) {
  if (!state.selection.empty) return "";
  return state.doc.textBetween(0, state.selection.from, "\n", "\n");
}

function triggerAtSelection(state) {
  return textBeforeSelection(state).endsWith("@");
}

function syncMention(docChanged) {
  if (!view || !root) return;
  if (!allowMention) {
    if (mentionShown) {
      mentionShown = false;
      window.__pdfscopeMentionClose?.();
    }
    return;
  }
  const picker = Boolean(document.querySelector(".mention-picker"));
  const at = triggerAtSelection(view.state);
  if (at && docChanged && !picker) {
    mentionShown = true;
    window.__pdfscopeMentionOpen?.();
    return;
  }
  if (!at && (mentionShown || (docChanged && picker))) {
    mentionShown = false;
    window.__pdfscopeMentionClose?.();
  }
}

function dispatchTransaction(transaction) {
  if (!view) return;
  const next = view.state.apply(transaction);
  view.updateState(next);
  syncEmpty();
  if (transaction.docChanged && !transaction.getMeta("silent")) emit();
  syncMention(transaction.docChanged && !transaction.getMeta("programmatic"));
}

function placeEnd() {
  if (!view) return;
  const selection = Selection.atEnd(view.state.doc);
  view.dispatch(view.state.tr.setSelection(selection).scrollIntoView());
  view.focus();
}

function replaceDoc(doc, silent = false) {
  if (!view) return;
  const tr = view.state.tr.replaceWith(0, view.state.doc.content.size, doc.content);
  tr.setMeta("addToHistory", false);
  tr.setMeta("programmatic", true);
  if (silent) tr.setMeta("silent", true);
  view.dispatch(tr.scrollIntoView());
}

function insertPlainText(text) {
  if (!view) return;
  const value = String(text ?? "").replace(/\r\n?/g, "\n");
  if (!value) return;
  view.dispatch(view.state.tr.insertText(value).scrollIntoView());
}

function makeMention(key, label, orphan = false) {
  return schema.nodes.mention.create({
    key: String(key),
    label: String(label || key),
    orphan: Boolean(orphan),
  });
}

function mentionPositions(key = null) {
  const positions = [];
  view?.state.doc.descendants((node, pos) => {
    if (node.type.name !== "mention") return;
    if (key === null || node.attrs.key === key) positions.push(pos);
  });
  return positions;
}

// Delete an atom immediately before/after a collapsed caret. This runs before
// ProseMirror's normal delete commands, so the pill is always removed whole.
function deleteAdjacentMention(direction) {
  return (state, dispatch) => {
    const selection = state.selection;
    if (!selection.empty) return false;
    const pos = selection.from;
    const node =
      direction < 0
        ? pos > 0
          ? state.doc.nodeAt(pos - 1)
          : null
        : state.doc.nodeAt(pos);
    if (!node || node.type.name !== "mention") return false;
    if (dispatch) {
      const from = direction < 0 ? pos - 1 : pos;
      const tr = state.tr.delete(from, from + node.nodeSize).scrollIntoView();
      dispatch(tr);
    }
    return true;
  };
}

function moveAcrossMention(direction) {
  return (state, dispatch) => {
    const selection = state.selection;
    const node =
      selection instanceof NodeSelection && selection.node.type.name === "mention"
        ? selection.node
        : null;
    if (node) {
      const target = direction < 0 ? selection.from : selection.to;
      if (dispatch) {
        dispatch(
          state.tr
            .setSelection(TextSelection.create(state.doc, target))
            .scrollIntoView(),
        );
      }
      return true;
    }
    if (!selection.empty) return false;
    const pos = selection.from;
    const adjacent =
      direction < 0
        ? pos > 0
          ? state.doc.nodeAt(pos - 1)
          : null
        : state.doc.nodeAt(pos);
    if (!adjacent || adjacent.type.name !== "mention") return false;
    const target = direction < 0 ? pos - 1 : pos + 1;
    if (dispatch) {
      dispatch(
        state.tr
          .setSelection(TextSelection.create(state.doc, target))
          .scrollIntoView(),
      );
    }
    return true;
  };
}

const keymapPlugin = keymap({
  ArrowLeft: moveAcrossMention(-1),
  ArrowRight: moveAcrossMention(1),
  Backspace: chainCommands(deleteAdjacentMention(-1), deleteSelection, joinBackward),
  Delete: chainCommands(deleteAdjacentMention(1), deleteSelection, joinForward),
  "Mod-z": undo,
  "Shift-Mod-z": redo,
  "Mod-y": redo,
});

function attachFiles(files) {
  const input = document.getElementById("attach-input");
  if (!input || !files?.length) return false;
  const transfer = new DataTransfer();
  for (const file of files) transfer.items.add(file);
  input.files = transfer.files;
  input.dispatchEvent(new Event("change", { bubbles: true }));
  return true;
}

function handlePaste(editorView, event) {
  const clipboard = event.clipboardData;
  const files = [...(clipboard?.files ?? [])];
  if (files.length > 0) {
    event.preventDefault();
    attachFiles(files);
    return true;
  }
  const text = clipboard?.getData("text/plain");
  if (text === undefined) return false;
  event.preventDefault();
  insertPlainText(text);
  return true;
}

function handleDrop(event) {
  if (!view) return;
  const data = event.dataTransfer;
  if (!data) return;
  const files = [...(data.files ?? [])];
  if (files.length > 0) return;
  const text = data.getData("text/plain");
  if (!text) return;
  event.preventDefault();
  event.stopPropagation();
  const found = view.posAtCoords({ left: event.clientX, top: event.clientY });
  if (found) {
    view.dispatch(
      view.state.tr.setSelection(TextSelection.create(view.state.doc, found.pos)),
    );
  }
  const [key, ...rest] = text.split("\n");
  if (/^(p:|r:|page:|a:|file:|img:)/.test(key)) {
    window.Composer.insertToken(key, rest.join("\n") || key);
  } else {
    insertPlainText(text);
  }
}

function handleNativePaste(event) {
  const files = [...(event.clipboardData?.files ?? [])];
  if (files.length === 0) return;
  event.preventDefault();
  event.stopPropagation();
  attachFiles(files);
}

function handleDragOver(event) {
  const data = event.dataTransfer;
  if (!data) return;
  if ([...(data.files ?? [])].length > 0) return;
  if (data.types?.includes("text/plain")) event.preventDefault();
}

function handleMouseDown(event) {
  if (!view) return;
  const target = event.target instanceof Element ? event.target.closest(".mention") : null;
  if (!target) return;
  const key = target.dataset.key;
  const rect = target.getBoundingClientRect();
  const domPos = view.posAtDOM(target, 0);
  const before = event.clientX < rect.left + rect.width / 2;
  const position = before ? domPos : domPos + 1;
  const selection = TextSelection.create(view.state.doc, position);
  view.dispatch(view.state.tr.setSelection(selection));
  event.preventDefault();
  event.stopPropagation();
  view.focus();
  if (key) window.__pdfscopeMentionClick?.(key);
}

function handlePointerOver(event) {
  const target = event.target instanceof Element ? event.target.closest(".mention") : null;
  if (target?.dataset.key) window.__pdfscopeMentionHover?.(target.dataset.key);
}

function handlePointerOut(event) {
  const target = event.target instanceof Element ? event.target.closest(".mention") : null;
  if (!target?.dataset.key) return;
  if (event.relatedTarget instanceof Node && target.contains(event.relatedTarget)) return;
  window.__pdfscopeMentionLeave?.(target.dataset.key);
}

function installView(element, state) {
  return new EditorView(element, {
    state,
    dispatchTransaction,
    editorProps: {
      attributes: {
        "aria-label": "问题",
        "data-placeholder": "问点什么…",
        spellcheck: "false",
      },
      handlePaste,
    },
  });
}

function mentionItems() {
  return [...document.querySelectorAll(".mention-picker .mention-item")];
}

window.Mention = {
  reset() {
    mentionIndex = -1;
    for (const item of mentionItems()) item.classList.remove("active");
  },
  step(delta) {
    const items = mentionItems();
    if (items.length === 0) return;
    mentionIndex = (mentionIndex + delta + items.length) % items.length;
    items.forEach((item, index) => item.classList.toggle("active", index === mentionIndex));
    items[mentionIndex].scrollIntoView({ block: "nearest" });
  },
  choose() {
    const items = mentionItems();
    const item = items[mentionIndex >= 0 ? mentionIndex : 0];
    item?.click();
  },
};

window.Composer = {
  init(element, callback) {
    root = element;
    notify = callback;
    if (view) {
      if (view.dom.parentElement === element) return;
      view.destroy();
      view = null;
    }
    root.setAttribute("tabindex", "-1");
    const state = EditorState.create({
      schema,
      doc: emptyDoc(),
      plugins: [history(), keymapPlugin],
    });
    view = installView(root, state);
    view.dom.addEventListener("mousedown", handleMouseDown, true);
    view.dom.addEventListener("pointerover", handlePointerOver, true);
    view.dom.addEventListener("pointerout", handlePointerOut, true);
    root.addEventListener("drop", handleDrop, true);
    root.addEventListener("dragover", handleDragOver, true);
    root.addEventListener("paste", handleNativePaste, true);
    syncEmpty();

    const onFocus = (event) => {
      if (event.target === root) view?.focus();
    };
    const onBlur = () => {
      mentionShown = false;
      window.__pdfscopeMentionClose?.();
    };
    root.addEventListener("focus", onFocus);
    view.dom.addEventListener("blur", onBlur);
    if (!selectionListenerInstalled) {
      selectionListenerInstalled = true;
      document.addEventListener("selectionchange", () => {
        if (!view) return;
        requestAnimationFrame(() => {
          if (view?.hasFocus()) syncMention(false);
        });
      });
    }
    root.addEventListener("mousedown", (event) => {
      if (event.target === root) {
        event.preventDefault();
        view?.focus();
      }
    });
    if (pendingAction) {
      const action = pendingAction;
      pendingAction = null;
      action();
    }
  },

  setPlainText(text) {
    if (!view) {
      pendingAction = () => window.Composer.setPlainText(text);
      return;
    }
    replaceDoc(docFromText(text));
    placeEnd();
  },

  focusEnd() {
    placeEnd();
  },

  focus() {
    view?.focus();
  },

  clear(silent = false) {
    if (!view) {
      pendingAction = () => window.Composer.clear(silent);
      return;
    }
    replaceDoc(emptyDoc(), Boolean(silent));
  },

  attachFiles,

  removeToken(key) {
    if (!view) return;
    const positions = mentionPositions(key);
    if (positions.length === 0) return;
    let tr = closeHistory(view.state.tr);
    for (const pos of positions.reverse()) tr = tr.delete(pos, pos + 1);
    view.dispatch(tr.scrollIntoView());
  },

  insertNewline() {
    if (!view) return;
    splitBlock(view.state, view.dispatch);
    view.focus();
  },

  insertToken(key, label) {
    if (!view) return;
    const state = view.state;
    const from = state.selection.from;
    const tr = closeHistory(state.tr);
    if (state.selection.empty && triggerAtSelection(state)) tr.delete(from - 1, from);
    const node = makeMention(key, label);
    tr.replaceSelectionWith(node);
    tr.setSelection(TextSelection.near(tr.doc.resolve(tr.selection.from)));
    view.dispatch(tr.scrollIntoView());
    view.focus();
  },

  markOrphan(key) {
    if (!view) return false;
    const positions = mentionPositions(key);
    if (positions.length === 0) return false;
    let tr = view.state.tr;
    for (const pos of positions) {
      const node = view.state.doc.nodeAt(pos);
      if (node) tr = tr.setNodeMarkup(pos, null, { ...node.attrs, orphan: true });
    }
    tr.setMeta("silent", true);
    tr.setMeta("addToHistory", false);
    view.dispatch(tr);
    return true;
  },

  unmarkOrphan(key) {
    if (!view) return;
    const positions = mentionPositions(key);
    if (positions.length === 0) return;
    let tr = view.state.tr;
    for (const pos of positions) {
      const node = view.state.doc.nodeAt(pos);
      if (node) tr = tr.setNodeMarkup(pos, null, { ...node.attrs, orphan: false });
    }
    tr.setMeta("silent", true);
    tr.setMeta("addToHistory", false);
    view.dispatch(tr);
  },

  focusAfterToken(key) {
    if (!view) return;
    const pos = mentionPositions(key)[0];
    if (pos === undefined) return;
    const selection = TextSelection.create(view.state.doc, pos + 1);
    view.dispatch(view.state.tr.setSelection(selection).scrollIntoView());
    view.focus();
  },

  isAtStart() {
    if (!view) return false;
    const selection = view.state.selection;
    return selection.empty && selection.from === 1;
  },

  isAtEnd() {
    if (!view) return false;
    const selection = view.state.selection;
    return selection.empty && selection.from === view.state.doc.content.size - 1;
  },

  setRich(parts) {
    if (!view) {
      pendingAction = () => window.Composer.setRich(parts);
      return;
    }
    replaceDoc(docFromParts(parts));
  },

  loadDraft(key) {
    try {
      return window.localStorage.getItem(`pdfscope.draft.${key}`) ?? "";
    } catch {
      return "";
    }
  },

  saveDraftSoon(key, text) {
    clearTimeout(draftTimer);
    draftTimer = setTimeout(() => {
      try {
        window.localStorage.setItem(`pdfscope.draft.${key}`, text);
      } catch {
        // A draft is a convenience; losing it is acceptable.
      }
    }, 300);
  },

  removeDraft(key) {
    clearTimeout(draftTimer);
    try {
      window.localStorage.removeItem(`pdfscope.draft.${key}`);
    } catch {
      // Nothing to remove.
    }
  },

  savePartialSoon(key, json) {
    clearTimeout(partialTimer);
    partialTimer = setTimeout(() => {
      try {
        window.localStorage.setItem(`pdfscope.partial.${key}`, json);
      } catch {
        // A recovery copy is best-effort.
      }
    }, 800);
  },

  loadPartial(key) {
    try {
      return window.localStorage.getItem(`pdfscope.partial.${key}`) ?? "";
    } catch {
      return "";
    }
  },

  clearPartial(key) {
    clearTimeout(partialTimer);
    try {
      window.localStorage.removeItem(`pdfscope.partial.${key}`);
    } catch {
      // Nothing to clear.
    }
  },

  clearTokens() {
    if (!view) return;
    const positions = mentionPositions();
    if (positions.length === 0) return;
    let tr = closeHistory(view.state.tr);
    for (const pos of positions.reverse()) tr = tr.delete(pos, pos + 1);
    view.dispatch(tr.scrollIntoView());
  },

  loadPromptHistory() {
    try {
      return window.localStorage.getItem("pdfscope.prompts");
    } catch {
      return null;
    }
  },

  savePromptHistory(json) {
    try {
      window.localStorage.setItem("pdfscope.prompts", json);
    } catch {
      // History is a convenience; losing it is acceptable.
    }
  },

  insertTokenAt(clientX, clientY, key, label) {
    if (!view) return;
    const found = view.posAtCoords({ left: clientX, top: clientY });
    if (found) {
      view.dispatch(
        view.state.tr.setSelection(TextSelection.create(view.state.doc, found.pos)),
      );
    } else {
      placeEnd();
    }
    window.Composer.insertToken(key, label);
  },

  setMentionsAllowed(value) {
    allowMention = Boolean(value);
    if (!allowMention && mentionShown) {
      mentionShown = false;
      window.__pdfscopeMentionClose?.();
    }
  },

  mentionStep(delta) {
    window.Mention.step(delta);
  },
  mentionChoose() {
    window.Mention.choose();
  },
  mentionReset() {
    window.Mention.reset();
  },
};
