// The composer editor.
//
// A textarea cannot hold inline objects, so the question field is a contenteditable element whose
// attachment tokens are non-editable pills. This module owns only the DOM side: serialisation,
// token insertion, paste and the `@` trigger. The interface owns the attachments themselves.

let root = null;
let notify = null;
let allowMention = true;

function serialise() {
  if (!root) return { text: "", keys: [] };
  let text = "";
  const keys = [];

  const walk = (node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      text += node.textContent.replace(/\u00a0/g, " ");
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return;
    if (node.dataset && node.dataset.key) {
      text += `@{${node.dataset.key}}`;
      keys.push(node.dataset.key);
      return;
    }
    if (node.tagName === "BR") {
      text += "\n";
      return;
    }
    for (const child of node.childNodes) walk(child);
    // Chromium wraps a line in a <div> after Enter; keep the break in the text.
    if (node.tagName === "DIV" && node.nextSibling) text += "\n";
  };

  for (const child of root.childNodes) walk(child);
  return { text: text.replace(/\n{3,}/g, "\n\n"), keys };
}

function emit() {
  const payload = serialise();
  if (notify) {
    notify(payload.text, payload.keys);
  } else {
    window.__pdfscopeComposerInput?.(payload.text, payload.keys);
  }
}

function caretInRoot() {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || !root) return null;
  const range = selection.getRangeAt(0);
  return root.contains(range.commonAncestorContainer) ? range : null;
}

function textBeforeCaret() {
  const range = caretInRoot();
  if (!range) return "";
  const probe = range.cloneRange();
  probe.setStart(root, 0);
  return probe.toString();
}

function removeTrailingAt() {
  const range = caretInRoot();
  if (!range || !range.collapsed) return;
  const text = textBeforeCaret();
  if (!text.endsWith("@")) return;

  const node = range.startContainer;
  if (node.nodeType !== Node.TEXT_NODE || range.startOffset === 0) return;
  const before = node.textContent.slice(0, range.startOffset);
  if (!before.endsWith("@")) return;
  // Only a mention trigger: start of the field, or preceded by whitespace.
  const preceding = before.slice(0, -1);
  if (preceding.length > 0 && !/\s$/.test(preceding)) return;

  const next = before.slice(0, -1) + node.textContent.slice(range.startOffset);
  node.textContent = next;
  const offset = before.length - 1;
  range.setStart(node, offset);
  range.collapse(true);
  const selection = window.getSelection();
  selection.removeAllRanges();
  selection.addRange(range);
}

function checkMention() {
  if (!allowMention) return;
  const text = textBeforeCaret();
  if (!text.endsWith("@")) return;
  const before = text.slice(0, -1);
  if (before.length > 0 && !/\s$/.test(before)) return;
  window.__pdfscopeMentionOpen?.();
}

function insertText(text) {
  const range = caretInRoot();
  if (!range) {
    root.appendChild(document.createTextNode(text));
  } else {
    range.deleteContents();
    const node = document.createTextNode(text);
    range.insertNode(node);
    range.setStartAfter(node);
    range.collapse(true);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
  }
  root.focus();
  emit();
}

function makeToken(key, label) {
  const token = makeToken(key, label);
  return token;
}

function insertToken(key, label) {
  if (!root) return;
  removeTrailingAt();

  const token = makeToken(key, label);

  const range = caretInRoot();
  if (!range) {
    root.appendChild(token);
  } else {
    range.deleteContents();
    range.insertNode(token);
    range.setStartAfter(token);
    range.collapse(true);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
  }

  // A plain space after the pill keeps typing natural and keeps the caret out of the token.
  const space = document.createTextNode(" ");
  token.after(space);
  const after = document.createRange();
  after.setStart(space, 1);
  after.collapse(true);
  const selection = window.getSelection();
  selection.removeAllRanges();
  selection.addRange(after);

  root.focus();
  emit();
}

window.Composer = {
  init(element, callback) {
    root = element;
    notify = callback;

    root.addEventListener("input", () => {
      emit();
      checkMention();
    });
    root.addEventListener("paste", (event) => {
      event.preventDefault();
      const text = event.clipboardData?.getData("text/plain") ?? "";
      insertText(text);
    });
    root.addEventListener("blur", () => {
      window.__pdfscopeMentionClose?.();
    });
  },

  setPlainText(text) {
    if (!root) return;
    root.replaceChildren(document.createTextNode(text ?? ""));
    emit();
    root.focus();
  },

  focus() {
    root?.focus();
  },

  clear(silent = false) {
    if (!root) return;
    root.replaceChildren();
    if (!silent) emit();
  },

  removeToken(key) {
    if (!root) return;
    const token = root.querySelector(`.mention[data-key="${key}"]`);
    if (!token) return;
    // Take the following space with it, so two removals do not leave a double gap.
    const next = token.nextSibling;
    token.remove();
    if (next && next.nodeType === Node.TEXT_NODE && next.textContent.startsWith(" ")) {
      next.textContent = next.textContent.slice(1);
    }
    emit();
  },

  insertNewline() {
    insertText("\n");
  },

  focus() {
    root?.focus();
  },

  insertToken,

  /// Whether the caret sits at the very start of the field.
  isAtStart() {
    const range = caretInRoot();
    if (!range || !range.collapsed) return false;
    const probe = document.createRange();
    probe.setStart(root, 0);
    probe.setEnd(range.startContainer, range.startOffset);
    return probe.toString().length === 0;
  },

  /// Whether the caret sits at the very end of the field.
  isAtEnd() {
    const range = caretInRoot();
    if (!range || !range.collapsed) return false;
    const probe = document.createRange();
    probe.setStart(range.endContainer, range.endOffset);
    probe.setEnd(root, root.childNodes.length);
    return probe.toString().length === 0;
  },

  /// Rebuilds the field from ordered parts: `{ text }` or `{ key, label }`.
  setRich(parts) {
    if (!root) return;
    const nodes = [];
    for (const part of parts ?? []) {
      if (part && part.key && part.label) {
        nodes.push(makeToken(part.key, part.label));
        nodes.push(document.createTextNode(" "));
      } else if (part && typeof part.text === "string" && part.text.length > 0) {
        nodes.push(document.createTextNode(part.text));
      }
    }
    root.replaceChildren(...nodes);
    emit();
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

  /// Persists an in-flight answer so a crash does not lose it.
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

  /// Inserts a pill at a viewport point, for a chip dropped into the field.
  insertTokenAt(clientX, clientY, key, label) {
    if (!root) return;
    const document = root.ownerDocument;
    let range = null;
    if (typeof document.caretRangeFromPoint === "function") {
      range = document.caretRangeFromPoint(clientX, clientY);
    } else if (typeof document.caretPositionFromPoint === "function") {
      const position = document.caretPositionFromPoint(clientX, clientY);
      if (position) {
        range = document.createRange();
        range.setStart(position.offsetNode, position.offset);
        range.collapse(true);
      }
    }
    if (range && root.contains(range.commonAncestorContainer)) {
      const selection = window.getSelection();
      selection.removeAllRanges();
      selection.addRange(range);
    }
    insertToken(key, label);
  },

  setMentionsAllowed(value) {
    allowMention = Boolean(value);
  },
};


// Keyboard navigation for the mention picker (which is rendered by the interface).
let draftTimer = null;
let partialTimer = null;
let mentionIndex = -1;

function mentionItems() {
  return [...document.querySelectorAll(".mention-picker .mention-item")];
}

window.Mention = {
  // The interface reaches all composer commands through one object.

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

window.Composer.mentionStep = (delta) => window.Mention.step(delta);
window.Composer.mentionChoose = () => window.Mention.choose();
window.Composer.mentionReset = () => window.Mention.reset();
