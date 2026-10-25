// ProseMirror-backed composer.
//
// @ pills are real inline atom nodes: the document model has no position inside
// them, so the native caret cannot enter. The public Composer API is kept
// compatible with the old contenteditable implementation.

import { Schema } from "prosemirror-model";
import { EditorState, NodeSelection, Plugin, Selection, TextSelection } from "prosemirror-state";
import { Decoration, DecorationSet, EditorView } from "prosemirror-view";
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
const MARKDOWN_KEY = "pdfscope.composer.markdown";

let markdownEnabled = readMarkdownPreference();
let markdownDot = null;
let resizeHandle = null;

function readMarkdownPreference() {
  try {
    return window.localStorage.getItem(MARKDOWN_KEY) === "1";
  } catch {
    return false;
  }
}

function writeMarkdownPreference(value) {
  try {
    window.localStorage.setItem(MARKDOWN_KEY, value ? "1" : "0");
  } catch {
    // The preference is a convenience; the editor still works without it.
  }
}

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
  if (!markdownDot?.isConnected) installMarkdownDot();
  if (!resizeHandle?.isConnected) installComposerHandles();
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

const mathCache = new Map();

function renderFormula(latex, display) {
  const key = `${display ? "D" : "I"}:${latex}`;
  if (mathCache.has(key)) return mathCache.get(key);
  let html = "";
  if (latex.trim() && window.katex?.renderToString) {
    try {
      html = window.katex.renderToString(latex, {
        displayMode: display,
        throwOnError: false,
        strict: "ignore",
        trust: false,
        output: "html",
      });
    } catch {
      html = "";
    }
  }
  mathCache.set(key, html);
  return html;
}

class FormulaWidget {
  constructor(latex, display, from) {
    this.latex = latex;
    this.display = display;
    this.from = from;
  }

  eq(other) {
    return other.latex === this.latex && other.display === this.display && other.from === this.from;
  }

  toDOM() {
    const span = document.createElement("span");
    span.className = this.display ? "md-math md-math-display" : "md-math md-math-inline";
    span.title = "点击编辑 LaTeX";
    const html = renderFormula(this.latex, this.display);
    if (html) {
      span.innerHTML = html;
    } else {
      span.classList.add("md-math-error");
      span.textContent = this.display ? `$$${this.latex}$$` : `$${this.latex}$`;
    }
    span.addEventListener("mousedown", (event) => {
      if (!view) return;
      event.preventDefault();
      event.stopPropagation();
      const pos = Math.min(this.from, view.state.doc.content.size - 1);
      view.dispatch(view.state.tr.setSelection(TextSelection.create(view.state.doc, pos)).scrollIntoView());
      view.focus();
    });
    return span;
  }

  ignoreEvent() {
    return false;
  }
}

function addFormula(latex, display, absoluteFrom, absoluteTo, lineActive, decorations) {
  if (!latex.trim()) return false;
  if (lineActive) {
    decorations.push(
      Decoration.inline(absoluteFrom, absoluteTo, { class: "md-math-source" }),
    );
  } else {
    decorations.push(
      Decoration.inline(absoluteFrom, absoluteTo, { class: "md-math-source-hidden" }),
    );
    decorations.push(
      Decoration.widget(
        absoluteFrom,
        () => new FormulaWidget(latex.trim(), display, absoluteFrom).toDOM(),
        { side: 1 },
      ),
    );
  }
  return true;
}

function syntaxDecoration(from, to, active) {
  if (to <= from) return null;
  return Decoration.inline(from, to, {
    class: active ? "md-syntax" : "md-syntax md-syntax-hidden",
  });
}

function addInlineMarkdown(text, base, lineActive, decorations) {
  const claimed = [];
  const overlaps = (from, to) => claimed.some(([start, end]) => from < end && to > start);
  const patterns = [
    { pattern: /`([^`\n]+)`/g, className: "md-code", before: 1, after: 1 },
    { pattern: /\$\$([^$\n]+)\$\$/g, formula: true, display: true },
    { pattern: /\\\[([^\n]+?)\\\]/g, formula: true, display: true },
    { pattern: /\$([^$\n]+?)\$/g, formula: true, display: false },
    { pattern: /\\\(([^\n]+?)\\\)/g, formula: true, display: false },
    { pattern: /\[([^\]\n]+)\]\(([^)\n]+)\)/g, className: "md-link", link: true },
    { pattern: /\*\*([^*\n]+)\*\*/g, className: "md-strong", before: 2, after: 2 },
    { pattern: /__([^_\n]+)__/g, className: "md-strong", before: 2, after: 2 },
    { pattern: /~~([^~\n]+)~~/g, className: "md-strike", before: 2, after: 2 },
    { pattern: /\*([^*\n]+)\*/g, className: "md-em", before: 1, after: 1 },
    { pattern: /_([^_\n]+)_/g, className: "md-em", before: 1, after: 1 },
  ];

  for (const rule of patterns) {
    rule.pattern.lastIndex = 0;
    let match;
    while ((match = rule.pattern.exec(text))) {
      const start = match.index;
      const end = start + match[0].length;
      if (overlaps(start, end)) {
        rule.pattern.lastIndex = start + 1;
        continue;
      }
      if (rule.formula) {
        if (addFormula(match[1], rule.display, base + start, base + end, lineActive, decorations)) {
          claimed.push([start, end]);
        }
        continue;
      }
      const contentStart = rule.link ? start + 1 : start + rule.before;
      const contentEnd = rule.link ? start + 1 + match[1].length : end - rule.after;
      if (contentEnd <= contentStart) continue;
      const active = lineActive;
      decorations.push(
        Decoration.inline(base + contentStart, base + contentEnd, { class: rule.className }),
      );
      const open = syntaxDecoration(base + start, base + contentStart, active);
      const close = syntaxDecoration(base + contentEnd, base + end, active);
      if (open) decorations.push(open);
      if (close) decorations.push(close);
      claimed.push([start, end]);
    }
  }
}

function buildMarkdownDecorations(state) {
  const decorations = [];
  const selection = state.selection;
  let inFence = false;

  state.doc.forEach((block, pos) => {
    const text = block.textContent;
    const from = pos;
    const to = pos + block.nodeSize;
    const active = selection.to > from && selection.from < to;
    const classes = [];
    const attrs = {};
    let prefixEnd = 0;
    let code = false;
    const fence = /^```/.test(text);

    if (inFence || fence) {
      code = true;
      classes.push("md-code-block");
      if (fence) {
        classes.push("md-code-fence");
        if (inFence) inFence = false;
        else inFence = true;
        if (text.length > 0) {
          decorations.push(
            Decoration.inline(from + 1, from + 1 + text.length, { class: "md-syntax" }),
          );
        }
      }
    } else {
      const heading = /^(#{1,6})\s+/.exec(text);
      if (heading) {
        classes.push("md-heading", `md-h${heading[1].length}`);
        prefixEnd = heading[0].length;
      } else {
        const quote = /^>\s?/.exec(text);
        if (quote) {
          classes.push("md-quote");
          prefixEnd = quote[0].length;
        } else {
          const list = /^(\s*)([-*+]|\d+\.)\s+/.exec(text);
          if (list) {
            classes.push("md-list", /^\d/.test(list[2]) ? "md-list-ol" : "md-list-ul");
            const indent = list[1].replace(/\t/g, "  ").length;
            attrs.style = `--md-list-indent: ${indent}ch`;
            if (list[1].length > 0) {
              decorations.push(
                Decoration.inline(from + 1, from + 1 + list[1].length, {
                  class: "md-list-indent",
                }),
              );
            }
            const markerStart = from + 1 + list[1].length;
            decorations.push(
              Decoration.inline(
                markerStart,
                from + 1 + list[0].length,
                { class: "md-list-marker" },
              ),
            );
          }
        }
      }

      if (prefixEnd) {
        const marker = syntaxDecoration(from + 1, from + 1 + prefixEnd, active);
        if (marker) decorations.push(marker);
      }
    }

    if (classes.length > 0 || Object.keys(attrs).length > 0) {
      decorations.push(
        Decoration.node(from, to, {
          class: classes.join(" "),
          ...attrs,
        }),
      );
    }

    if (!code) {
      let offset = 0;
      block.forEach((child) => {
        if (child.isText) addInlineMarkdown(child.text, from + 1 + offset, active, decorations);
        offset += child.nodeSize;
      });
    }
  });

  return DecorationSet.create(state.doc, decorations);
}

const markdownPreviewPlugin = new Plugin({
  props: {
    decorations(state) {
      return markdownEnabled ? buildMarkdownDecorations(state) : null;
    },
  },
});

function wrapMarkdown(prefix, suffix = prefix) {
  return (state, dispatch) => {
    if (!markdownEnabled) return true;
    const { from, to, empty } = state.selection;
    if (empty) {
      if (dispatch) {
        const tr = state.tr.insertText(prefix + suffix, from);
        tr.setSelection(TextSelection.create(tr.doc, from + prefix.length));
        dispatch(tr);
      }
      return true;
    }
    const before =
      from >= prefix.length ? state.doc.textBetween(from - prefix.length, from, "", "") : "";
    const after =
      to + suffix.length <= state.doc.content.size
        ? state.doc.textBetween(to, to + suffix.length, "", "")
        : "";
    if (before === prefix && after === suffix) {
      if (dispatch) {
        let tr = state.tr.delete(to, to + suffix.length);
        tr = tr.delete(from - prefix.length, from);
        dispatch(tr);
      }
      return true;
    }
    if (dispatch) {
      let tr = state.tr.insertText(suffix, to);
      tr = tr.insertText(prefix, from);
      tr.setSelection(TextSelection.create(tr.doc, from + prefix.length, to + prefix.length));
      dispatch(tr);
    }
    return true;
  };
}

const keymapPlugin = keymap({
  ArrowLeft: moveAcrossMention(-1),
  ArrowRight: moveAcrossMention(1),
  Backspace: chainCommands(deleteAdjacentMention(-1), deleteSelection, joinBackward),
  Delete: chainCommands(deleteAdjacentMention(1), deleteSelection, joinForward),
  "Mod-b": wrapMarkdown("**"),
  "Mod-i": wrapMarkdown("*"),
  "Mod-e": wrapMarkdown("`"),
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

function handleListTab(view, event) {
  if (event.key !== "Tab" || event.metaKey || event.ctrlKey || event.altKey) return false;
  if (document.querySelector(".mention-picker")) return false;
  const selection = view.state.selection;
  if (!selection.empty) return false;
  const $from = selection.$from;
  if ($from.depth !== 1) return false;
  const blockStart = $from.before(1);
  const text = $from.parent.textContent;
  const list = /^(\s*)([-*+]|\d+\.)\s+/.exec(text);
  const previous =
    blockStart > 0 ? view.state.doc.resolve(blockStart - 1).parent.textContent : "";
  const previousList = /^\s*([-*+]|\d+\.)\s+/.test(previous);
  const emptyAfterList = /^\s*$/.test(text) && previousList;
  if (!list && !emptyAfterList) return false;
  const indent = /^\s*/.exec(text)[0].length;

  if (event.shiftKey) {
    if (indent === 0) return false;
    const remove = Math.min(2, indent);
    const tr = view.state.tr.delete(blockStart + 1, blockStart + 1 + remove);
    tr.setSelection(TextSelection.create(tr.doc, Math.max(blockStart + 1, selection.from - remove)));
    view.dispatch(tr.scrollIntoView());
    event.preventDefault();
    return true;
  }

  const tr = view.state.tr.insertText("  ", blockStart + 1);
  tr.setSelection(TextSelection.create(tr.doc, selection.from + 2));
  view.dispatch(tr.scrollIntoView());
  event.preventDefault();
  return true;
}

function handleVerticalArrow(view, event) {
  if (!["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) return false;
  if (event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) return false;
  if (document.querySelector(".mention-picker")) return false;
  const selection = view.state.selection;
  if (!selection.empty) return false;
  const $from = selection.$from;
  if ($from.depth !== 1) return false;
  const blockStart = $from.before(1);
  const blockEnd = $from.after(1);
  const caret = view.coordsAtPos(selection.from);
  if (!caret) return false;
  const lineHeight = parseFloat(getComputedStyle(view.dom).lineHeight) || 20;
  if (event.key === "ArrowUp") {
    const first = view.coordsAtPos(blockStart + 1);
    if (!first || caret.top - first.top >= lineHeight * 0.75 || blockStart <= 0) return false;
    const target = TextSelection.near(view.state.doc.resolve(blockStart - 1), -1);
    view.dispatch(view.state.tr.setSelection(target).scrollIntoView());
    event.preventDefault();
    return true;
  }
  if (event.key === "ArrowLeft") {
    if (selection.from !== blockStart + 1 || blockStart <= 0) return false;
    const target = TextSelection.near(view.state.doc.resolve(blockStart - 1), -1);
    view.dispatch(view.state.tr.setSelection(target).scrollIntoView());
    event.preventDefault();
    return true;
  }
  if (event.key === "ArrowRight") {
    if (selection.from !== blockEnd - 1 || blockEnd >= view.state.doc.content.size) return false;
    const target = TextSelection.near(view.state.doc.resolve(blockEnd + 1), 1);
    view.dispatch(view.state.tr.setSelection(target).scrollIntoView());
    event.preventDefault();
    return true;
  }
  const last = view.coordsAtPos(blockEnd - 1);
  if (!last || last.bottom - caret.bottom >= lineHeight * 0.75) return false;
  if (blockEnd >= view.state.doc.content.size) return false;
  const target = TextSelection.near(view.state.doc.resolve(blockEnd + 1), 1);
  view.dispatch(view.state.tr.setSelection(target).scrollIntoView());
  event.preventDefault();
  return true;
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

function updateMarkdownDot() {
  if (!markdownDot) return;
  markdownDot.classList.toggle("active", markdownEnabled);
  markdownDot.setAttribute("aria-pressed", String(markdownEnabled));
  markdownDot.title = markdownEnabled ? "Markdown 实时预览：开" : "Markdown 实时预览：关";
}

function setMarkdownEnabled(next) {
  if (next === markdownEnabled) {
    updateMarkdownDot();
    return;
  }
  markdownEnabled = Boolean(next);
  writeMarkdownPreference(markdownEnabled);
  if (view) {
    // Force a decoration/plugin pass; the document (and its history) is untouched.
    view.dispatch(
      view.state.tr
        .setSelection(view.state.selection)
        .setMeta("markdownPreview", markdownEnabled),
    );
    view.focus();
  }
  updateMarkdownDot();
}

function installComposerHandles() {
  if (!root) return;
  const card = root.closest(".composer-card");
  if (!card) return;
  let handle = card.querySelector(".composer-resize-handle");
  if (!handle) {
    handle = document.createElement("button");
    handle.type = "button";
    handle.className = "composer-resize-handle";
    handle.tabIndex = 0;
    handle.setAttribute("role", "separator");
    handle.setAttribute("aria-orientation", "horizontal");
    handle.setAttribute("aria-label", "调整输入框高度");
    handle.title = "上下拖动调整高度；双击恢复自动高度";
    card.append(handle);
  }
  resizeHandle = handle;
  if (handle.dataset.bound === "1") return;
  handle.dataset.bound = "1";

  const ask = () => card.querySelector(".ask");
  const reset = () => {
    const target = ask();
    if (target) target.style.removeProperty("--composer-min-height");
    card.classList.remove("composer-resizing");
    document.body.classList.remove("composer-resizing");
  };
  const setHeight = (value) => {
    const target = ask();
    if (!target) return;
    const max = Math.max(56, window.innerHeight * 0.6);
    const next = Math.min(max, Math.max(56, Math.round(value)));
    target.style.setProperty("--composer-min-height", `${next}px`);
    return next;
  };

  handle.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    const target = ask();
    if (!target) return;
    event.preventDefault();
    const startY = event.clientY;
    const startHeight = target.getBoundingClientRect().height;
    let latest = startHeight;
    card.classList.add("composer-resizing");
    document.body.classList.add("composer-resizing");
    handle.setPointerCapture?.(event.pointerId);

    const move = (moveEvent) => {
      latest = setHeight(startHeight + (startY - moveEvent.clientY));
    };
    const finish = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", finish);
      handle.removeEventListener("pointercancel", finish);
      card.classList.remove("composer-resizing");
      document.body.classList.remove("composer-resizing");
      if (latest === undefined || latest <= 57) reset();
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", finish);
    handle.addEventListener("pointercancel", finish);
  });

  handle.addEventListener("dblclick", (event) => {
    event.preventDefault();
    reset();
  });

  handle.addEventListener("keydown", (event) => {
    const target = ask();
    if (!target) return;
    if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      event.preventDefault();
      const current = target.getBoundingClientRect().height;
      if (event.key === "ArrowDown" && current <= 57) {
        reset();
        return;
      }
      setHeight(current + (event.key === "ArrowUp" ? 16 : -16));
    } else if (event.key === "Home" || event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      reset();
    }
  });
}

function installMarkdownDot() {
  if (!root) return;
  const zone = root.closest(".composer-zone");
  if (!zone) return;
  let dot = zone.querySelector(".markdown-dot");
  if (!dot) {
    dot = document.createElement("button");
    dot.type = "button";
    dot.className = "markdown-dot";
    dot.setAttribute("aria-label", "Markdown 实时预览");
    zone.append(dot);
  }
  markdownDot = dot;
  if (dot.dataset.bound !== "1") {
    dot.dataset.bound = "1";
    dot.addEventListener("mousedown", (event) => event.preventDefault());
    dot.addEventListener("click", (event) => {
      event.preventDefault();
      setMarkdownEnabled(!markdownEnabled);
      view?.focus();
    });
  }
  updateMarkdownDot();
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
      plugins: [history(), keymapPlugin, markdownPreviewPlugin],
    });
    view = installView(root, state);
    view.dom.addEventListener("mousedown", handleMouseDown, true);
    view.dom.addEventListener(
      "keydown",
      (event) => {
        if (!view) return;
        if (handleListTab(view, event) || handleVerticalArrow(view, event)) {
          event.preventDefault();
          event.stopPropagation();
        }
      },
      true,
    );
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
    installMarkdownDot();
    installComposerHandles();
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
