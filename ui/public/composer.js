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
  return { text: text.replace(/\u200B/g, "").replace(/\n{3,}/g, "\n\n"), keys };
}

function emit() {
  const payload = serialise();
  if (notify) {
    notify(payload.text, payload.keys);
  } else {
    window.__pdfscopeComposerInput?.(payload.text, payload.keys);
  }
}

function placeCaretEnd() {
  if (!root) return;
  const range = document.createRange();
  range.selectNodeContents(root);
  range.collapse(false);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
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
  if (!text.endsWith("@")) {
    window.__pdfscopeMentionClose?.();
    return;
  }
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
  const token = document.createElement("span");
  token.className = "mention";
  token.contentEditable = "false";
  token.dataset.key = key;
  token.textContent = `@${label}`;
  return token;
}

function zeroGuard() {
  return document.createTextNode("\u200B");
}

function afterGuard(token) {
  const next = token?.nextSibling;
  return next && next.nodeType === Node.TEXT_NODE && next.textContent.startsWith("\u200B")
    ? next
    : null;
}

function beforeGuard(token) {
  const previous = token?.previousSibling;
  return previous && previous.nodeType === Node.TEXT_NODE && previous.textContent.endsWith("\u200B")
    ? previous
    : null;
}

function insertToken(key, label) {
  if (!root) return;
  removeTrailingAt();

  const token = makeToken(key, label);
  const before = zeroGuard();
  const after = zeroGuard();

  const range = caretInRoot();
  if (!range) {
    root.append(before, token, after);
  } else {
    range.deleteContents();
    range.insertNode(token);
    token.before(before);
    token.after(after);
  }

  // Invisible zero-width guards give the caret a position outside the pill without a visible space.
  const caret = document.createRange();
  caret.setStart(after, 1);
  caret.collapse(true);
  const selection = window.getSelection();
  selection.removeAllRanges();
  selection.addRange(caret);
  root.focus();
  emit();
}

function attachFiles(files) {
  const input = document.getElementById("attach-input");
  if (!input || !files?.length) return false;
  const transfer = new DataTransfer();
  for (const file of files) transfer.items.add(file);
  input.files = transfer.files;
  input.dispatchEvent(new Event("change", { bubbles: true }));
  return true;
}

// A non-editable pill must never receive the caret inside its label.
document.addEventListener("selectionchange", () => {
  if (!root) return;
  const selection = window.getSelection();
  if (!selection?.anchorNode || !root.contains(selection.anchorNode)) return;
  const anchor =
    selection.anchorNode.nodeType === Node.TEXT_NODE
      ? selection.anchorNode.parentElement
      : selection.anchorNode;
  const token = anchor instanceof Element ? anchor.closest(".mention") : null;
  if (!token) return;
  const range = document.createRange();
  const guard = afterGuard(token);
  if (guard) range.setStart(guard, 1);
  else range.setStartAfter(token);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
});

window.Composer = {
  init(element, callback) {
    root = element;
    notify = callback;

    root.addEventListener("input", () => {
      emit();
      checkMention();
    });
    root.addEventListener("paste", (event) => {
      const clipboard = event.clipboardData;
      const files = [...(clipboard?.files ?? [])];
      if (files.length > 0) {
        event.preventDefault();
        attachFiles(files);
        return;
      }
      event.preventDefault();
      const text = clipboard?.getData("text/plain") ?? "";
      insertText(text);
    });
    root.addEventListener("blur", () => {
      window.__pdfscopeMentionClose?.();
    });
    root.addEventListener("pointerover", (event) => {
      const token = event.target instanceof Element ? event.target.closest(".mention") : null;
      if (token?.dataset.key) window.__pdfscopeMentionHover?.(token.dataset.key);
    });
    root.addEventListener("pointerout", (event) => {
      const token = event.target instanceof Element ? event.target.closest(".mention") : null;
      if (!token?.dataset.key) return;
      if (event.relatedTarget instanceof Node && token.contains(event.relatedTarget)) return;
      window.__pdfscopeMentionLeave?.(token.dataset.key);
    });
    root.addEventListener("click", (event) => {
      const token = event.target instanceof Element ? event.target.closest(".mention") : null;
      if (!token) return;
      if (token.dataset.key) window.__pdfscopeMentionClick?.(token.dataset.key);
      const key = token.dataset.key;
      const place = () => {
        const live = key ? root.querySelector(`.mention[data-key="${key}"]`) : null;
        if (!live) return;
        const liveRect = live.getBoundingClientRect();
        const range = document.createRange();
        if (event.clientX < liveRect.left + liveRect.width / 2) {
          const guard = beforeGuard(live);
          if (guard) range.setStartBefore(guard);
          else range.setStartBefore(live);
        } else {
          const guard = afterGuard(live);
          if (guard) range.setStartAfter(guard);
          else range.setStartAfter(live);
        }
        range.collapse(true);
        const selection = window.getSelection();
        selection?.removeAllRanges();
        selection?.addRange(range);
      };
      root.focus();
      place();
      // The PDF focus/jump triggered by the mention may rerender the panel; reassert after it.
      requestAnimationFrame(() => {
        root.focus();
        place();
      });
    });
  },

  setPlainText(text) {
    if (!root) return;
    root.replaceChildren(document.createTextNode(text ?? ""));
    emit();
    root.focus();
    placeCaretEnd();
    // WebKit may restore its old caret after focus; the next frame wins.
    requestAnimationFrame(placeCaretEnd);
  },

  focusEnd() {
    root?.focus();
    placeCaretEnd();
    requestAnimationFrame(placeCaretEnd);
  },

  focus() {
    root?.focus();
  },

  clear(silent = false) {
    if (!root) return;
    root.replaceChildren();
    if (!silent) emit();
  },

  attachFiles,

  removeToken(key) {
    if (!root) return;
    const token = root.querySelector(`.mention[data-key="${key}"]`);
    if (!token) return;
    // Take the following space with it, so two removals do not leave a double gap.
    const before = beforeGuard(token);
    const after = afterGuard(token);
    token.remove();
    if (before) {
      before.textContent = before.textContent.slice(0, -1);
      if (!before.textContent) before.remove();
    }
    if (after) {
      after.textContent = after.textContent.slice(1);
      if (!after.textContent) after.remove();
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

  markOrphan(key) {
    const token = root?.querySelector(`.mention[data-key="${key}"]`);
    if (!token) return false;
    token.classList.add("orphan");
    token.title = "对应的上下文引用已删除";
    return true;
  },

  unmarkOrphan(key) {
    const token = root?.querySelector(`.mention[data-key="${key}"]`);
    token?.classList.remove("orphan");
    token?.removeAttribute("title");
  },

  focusAfterToken(key) {
    if (!root) return;
    const place = () => {
      const token = root.querySelector(`.mention[data-key="${key}"]`);
      if (!token) return;
      const guard = afterGuard(token);
      const range = document.createRange();
      if (guard) range.setStart(guard, 1);
      else range.setStartAfter(token);
      range.collapse(true);
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
    };
    root.focus();
    place();
    requestAnimationFrame(place);
    setTimeout(place, 40);
    setTimeout(place, 90);
  },

  focus() {
    root?.focus();
  },

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
        nodes.push(zeroGuard(), makeToken(part.key, part.label), zeroGuard());
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

  /// Removes every pill while keeping the typed text, for a context clear-all.
  clearTokens() {
    if (!root) return;
    for (const token of root.querySelectorAll(".mention")) token.remove();
    emit();
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
