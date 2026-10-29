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
      if (!pre || pre.querySelector(":scope > .code-copy")) continue;
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
      pre.append(button);
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
