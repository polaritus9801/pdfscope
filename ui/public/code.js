// Mature syntax highlighting and copy controls for every rendered code block.
// highlight.js is bundled locally so the desktop reader never needs a network request.
(() => {
  const COPY_ICON = '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="11" height="11" rx="2"></rect><path d="M5 15V5a2 2 0 0 1 2-2h8"></path></svg>';
  const CHECK_ICON = '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12l5 5 9-10"></path></svg>';
  let scheduled = false;

  async function write(text) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      const area = document.createElement("textarea");
      area.value = text;
      area.style.position = "fixed";
      area.style.opacity = "0";
      document.body.append(area);
      area.select();
      const ok = document.execCommand("copy");
      area.remove();
      return ok;
    }
  }

  function enhance(root = document) {
    for (const code of root.querySelectorAll("pre > code")) {
      if (!code.dataset.highlighted || code.dataset.highlighted === "no") {
        if (window.hljs) {
          try { window.hljs.highlightElement(code); } catch { /* unknown language */ }
        }
      }
      const pre = code.parentElement;
      if (!pre) continue;
      let frame = pre.parentElement?.classList.contains("code-frame") ? pre.parentElement : null;
      if (!frame) {
        frame = document.createElement("div");
        frame.className = "code-frame";
        pre.before(frame);
        frame.append(pre);
      }
      if (frame.querySelector(":scope > .code-copy")) continue;
      const button = document.createElement("button");
      button.type = "button";
      button.className = "code-copy";
      button.setAttribute("aria-label", "复制代码");
      button.innerHTML = COPY_ICON;
      button.addEventListener("click", async (event) => {
        event.preventDefault();
        event.stopPropagation();
        const ok = await write(code.textContent ?? "");
        button.innerHTML = ok ? CHECK_ICON : COPY_ICON;
        button.classList.toggle("is-copied", ok);
        window.setTimeout(() => {
          button.innerHTML = COPY_ICON;
          button.classList.remove("is-copied");
        }, 1300);
      });
      frame.append(button);
    }
  }

  function schedule() {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(() => {
      scheduled = false;
      enhance(document);
    });
  }

  window.PdfScopeCode = { enhance };
  window.addEventListener("DOMContentLoaded", schedule);
  new MutationObserver(schedule).observe(document.body, {
    childList: true,
    subtree: true,
  });
})();

// A reply can be quoted partially, with the same compact bubble used for PDF text selections.
(() => {
  let bar = null;
  let quote = "";

  function hide() {
    bar?.remove();
    bar = null;
    quote = "";
  }

  function currentAnswerSelection() {
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed || selection.rangeCount === 0) return null;
    const range = selection.getRangeAt(0);
    const node = range.commonAncestorContainer;
    const element = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
    const answer = element?.closest(".answer-text");
    if (!answer) return null;
    const text = selection.toString().replace(/\s+/g, " ").trim();
    if (!text) return null;
    const rect = range.getBoundingClientRect();
    if (!rect || rect.width === 0 || rect.height === 0) return null;
    return { text, rect };
  }

  function show() {
    const found = currentAnswerSelection();
    if (!found) {
      hide();
      return;
    }
    hide();
    quote = found.text;
    bar = document.createElement("div");
    bar.className = "selection-bar answer-selection-bar";
    bar.innerHTML =
      '<span class="selection-page">回答</span>' +
      '<span class="selection-quote"></span>' +
      '<button type="button" class="button button-primary selection-action">加入引用</button>' +
      '<button type="button" class="button selection-action">复制</button>';
    bar.querySelector(".selection-quote").textContent =
      found.text.length > 32 ? `${found.text.slice(0, 32)}…` : found.text;
    const left = Math.min(
      Math.max(found.rect.left + found.rect.width / 2, 120),
      window.innerWidth - 120,
    );
    const above = found.rect.top > 80;
    bar.style.left = `${left}px`;
    bar.style.top = `${above ? found.rect.top : found.rect.bottom}px`;
    bar.classList.toggle("below", !above);
    bar.addEventListener("pointerdown", (event) => event.preventDefault());
    const buttons = bar.querySelectorAll("button");
    buttons[0].addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (quote) window.__pdfscopeAnswerQuote?.(quote);
      window.getSelection()?.removeAllRanges();
      hide();
    });
    buttons[1].addEventListener("click", async (event) => {
      event.preventDefault();
      event.stopPropagation();
      try {
        await navigator.clipboard.writeText(quote);
      } catch {
        /* clipboard denied */
      }
      hide();
    });
    document.body.append(bar);
  }

  document.addEventListener("mouseup", () => {
    requestAnimationFrame(show);
  });
  document.addEventListener("selectionchange", () => {
    if (window.getSelection()?.isCollapsed) hide();
  });
  window.addEventListener("scroll", hide, true);
  window.addEventListener("resize", hide);
})();
