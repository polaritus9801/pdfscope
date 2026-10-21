// The PDF.js boundary.
//
// Rendering and text selection live here, behind a small surface: `window.PDFScope` for calls into
// the renderer, and `window.__pdfscopeSelection` for the one thing the renderer reports back. The
// Rust side owns application state and never imports PDF.js or touches a canvas.

import * as pdfjs from "./pdfjs/pdf.min.mjs";

pdfjs.GlobalWorkerOptions.workerSrc = new URL("./pdfjs/pdf.worker.min.mjs", import.meta.url).href;

/** @type {import("./pdfjs/pdf.min.mjs").PDFDocumentProxy | null} */
let document_ = null;
let currentPage = 1;
let currentZoom = 100;
let renderTask = null;

function canvas() {
  return document.getElementById("page-canvas");
}

function textLayerContainer() {
  return document.getElementById("text-layer");
}

/// Positions the text layer over the canvas and fills it with selectable glyphs.
async function paintTextLayer(page, viewport, scale) {
  const container = textLayerContainer();
  if (!container) return;

  await container.replaceChildren();
  container.style.width = `${viewport.width}px`;
  container.style.height = `${viewport.height}px`;
  container.style.setProperty("--scale-factor", String(scale));

  const layer = new pdfjs.TextLayer({
    textContentSource: page.streamTextContent(),
    container,
    viewport,
  });

  await layer.render();

  const hasText = container.querySelector("span") !== null;
  container.classList.toggle("empty", !hasText);
  container.dataset.page = String(currentPage);
}

async function paint() {
  if (!document_) return;

  const target = canvas();
  if (!target) return;

  const page = await document_.getPage(currentPage);
  const base = page.getViewport({ scale: 1 });

  // Fit width, measured on the host rather than the window so the scrollbar is excluded.
  const host = target.parentElement.parentElement;
  const available = Math.max(host.clientWidth - 48, 200);
  const scale = (available / base.width) * (currentZoom / 100);
  const viewport = page.getViewport({ scale });

  const ratio = window.devicePixelRatio || 1;
  target.width = Math.floor(viewport.width * ratio);
  target.height = Math.floor(viewport.height * ratio);
  target.style.width = `${Math.floor(viewport.width)}px`;
  target.style.height = `${Math.floor(viewport.height)}px`;

  if (renderTask) {
    try {
      renderTask.cancel();
    } catch {
      /* a finished task cannot be cancelled; nothing to do */
    }
  }

  renderTask = page.render({
    canvasContext: target.getContext("2d"),
    viewport,
    transform: ratio === 1 ? null : [ratio, 0, 0, ratio, 0, 0],
  });

  try {
    await renderTask.promise;
  } catch (error) {
    if (error?.name !== "RenderingCancelledException") throw error;
  } finally {
    renderTask = null;
  }

  await paintTextLayer(page, viewport, scale);
}

/// Union of the selection's client rects, in coordinates normalised to the rendered page.
function normalisedSelectionRect() {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return null;

  const target = canvas();
  if (!target) return null;
  const box = target.getBoundingClientRect();
  if (box.width === 0 || box.height === 0) return null;

  const range = selection.getRangeAt(0);
  const layer = textLayerContainer();
  if (!layer || !layer.contains(range.commonAncestorContainer)) return null;

  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (const rect of range.getClientRects()) {
    if (rect.width === 0 || rect.height === 0) continue;
    left = Math.min(left, rect.left);
    top = Math.min(top, rect.top);
    right = Math.max(right, rect.right);
    bottom = Math.max(bottom, rect.bottom);
  }
  if (!Number.isFinite(left)) return null;

  const clamp = (value) => Math.min(Math.max(value, 0), 1);
  const x = clamp((left - box.left) / box.width);
  const y = clamp((top - box.top) / box.height);
  const width = clamp((right - left) / box.width);
  const height = clamp((bottom - top) / box.height);
  if (width <= 0 || height <= 0) return null;

  const text = selection.toString().replace(/\s+/g, " ").trim();
  if (!text) return null;

  return { page: currentPage, x, y, width, height, text };
}

let settleTimer = null;

function reportSelection() {
  clearTimeout(settleTimer);
  settleTimer = setTimeout(() => {
    const payload = normalisedSelectionRect();
    if (!payload) return;
    window.__pdfscopeSelection?.(payload);
  }, 120);
}

document.addEventListener("selectionchange", reportSelection);
document.addEventListener("mouseup", reportSelection);
document.addEventListener("keyup", reportSelection);

let resizeTimer = null;
window.addEventListener("resize", () => {
  if (!document_) return;
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => {
    paint().catch(() => {});
  }, 120);
});

window.PDFScope = {
  /** @param {Uint8Array} bytes */
  async load(bytes) {
    if (document_) {
      await document_.destroy();
      document_ = null;
    }
    document_ = await pdfjs.getDocument({ data: bytes }).promise;
    return document_.numPages;
  },

  async render(pageNumber, zoomPercent) {
    if (!document_) return;
    currentPage = Math.min(Math.max(pageNumber, 1), document_.numPages);
    currentZoom = zoomPercent;
    await paint();
  },

  async pageText(pageNumber) {
    if (!document_) return "";
    const page = await document_.getPage(pageNumber);
    const content = await page.getTextContent();
    return content.items.map((item) => item.str).join(" ");
  },

  flash() {
    const page = document.getElementById("page");
    if (!page) return;
    page.classList.remove("flash");
    void page.offsetWidth;
    page.classList.add("flash");
    setTimeout(() => page.classList.remove("flash"), 800);
  },

  clearSelection() {
    window.getSelection()?.removeAllRanges();
  },

  async copy(text) {
    await navigator.clipboard.writeText(text);
  },
};
