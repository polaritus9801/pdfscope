// The PDF.js boundary.
//
// Everything about rendering lives here, behind three functions on `window.PDFScope`. The Rust side
// owns application state and calls them; it never imports PDF.js and never touches a canvas.
//
// This is a boundary, not a convenience: replacing the renderer later means rewriting this file and
// nothing else.

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

async function paint() {
  if (!document_) return;
  const canvas = document.getElementById("page-canvas");
  if (!canvas) return;

  const page = await document_.getPage(currentPage);
  const base = page.getViewport({ scale: 1 });

  // Fit width, measured on the host rather than the window so the scrollbar is excluded.
  const host = canvas.parentElement;
  const available = Math.max(host.clientWidth - 48, 200);
  const scale = (available / base.width) * (currentZoom / 100);
  const viewport = page.getViewport({ scale });

  const ratio = window.devicePixelRatio || 1;
  canvas.width = Math.floor(viewport.width * ratio);
  canvas.height = Math.floor(viewport.height * ratio);
  canvas.style.width = `${Math.floor(viewport.width)}px`;
  canvas.style.height = `${Math.floor(viewport.height)}px`;

  if (renderTask) {
    try {
      renderTask.cancel();
    } catch {
      /* a finished task cannot be cancelled; nothing to do */
    }
  }

  renderTask = page.render({
    canvasContext: canvas.getContext("2d"),
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
}

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
};
