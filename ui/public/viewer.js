// The PDF.js boundary.
//
// The document is one vertical column at a fixed base layout. Zoom is a CSS transform on that
// column plus a scaler element that carries the scaled scroll size; pages never reflow while the
// reader pinches. Text layers stay mounted at the base layout and scale with the column, so text
// never disappears while a canvas is re-rendered. Canvases are double-buffered: the old bitmap
// stays visible until the sharp one for the resting scale is ready.
//
// Rendering, scrolling, zoom and selection geometry live here. Application state never does: the UI
// reaches in through `window.PDFScope`, and this module reports through the `window.__pdfscope*`
// callbacks.

import * as pdfjs from "./pdfjs/pdf.min.mjs";

pdfjs.GlobalWorkerOptions.workerSrc = new URL("./pdfjs/pdf.worker.min.mjs", import.meta.url).href;

const WINDOW_MARGIN = 1;
const READING_LINE = 0.35;
const PROGRAMMATIC_SCROLL_MS = 180;
const MIN_SCALE = 0.5;
const MAX_SCALE = 4;
const SETTLE_MS = 120;
const MAX_CANVAS_PIXELS = 32 * 1024 * 1024;
const MAX_CANVAS_DIM = 32767;

/** @type {import("./pdfjs/pdf.min.mjs").PDFDocumentProxy | null} */
let document_ = null;
let pageSizes = [];
let currentPage = 1;
let documentName = "";
let rememberPosition = true;
let defaultZoom = 100;
let pageMode = "continuous";
let toolMode = "pointer";
let borrowedMode = null;
let gestureShift = false;
let pointerHeld = false;
let spreadOffset = false;
let pendingDefaultZoom = 0;
let restorePage = 0;
let initialized = false;

/** Base layout at 100 %: the column never reflows during a zoom. */
let baseWidth = 0;
let baseHeight = 0;
/** The CSS page gap at the reference layout, stored as a fraction of the base width. */
let pageGapRatio = 0;
let baseEpoch = 0;
let baseDirty = true;

/** Visual scale and the scale the latest canvas was rendered for. */
let displayScale = 1;
let targetScale = 1;
let zoomOrigin = null;
let zoomDirect = false;
let zoomFrame = 0;
let zoomLastTime = 0;
let zooming = false;
let settleTimer = 0;
let pendingPanX = 0;
let pendingPanY = 0;
let gesture = null;
let ctrlKeyDown = false;
let lastPointer = null;

/** Base page offsets, rebuilt only when the base layout changes. */
let metrics = [];
let metricsDirty = true;

/** Per-page render state, kept for the lifetime of the document. */
const rendered = new Map();
let desired = new Set();

let updatePromise = null;
let updateAgain = false;
let updateRender = false;
let frame = 0;

let stageBound = null;
let resizeObserver = null;
let resizeTimer = null;
let programmaticUntil = 0;
let lastReportedZoom = null;

function stageElement() {
  return document.querySelector(".stage");
}

function scalerElement() {
  return document.getElementById("pages-scaler");
}

function pagesElement() {
  return document.getElementById("pages");
}

function pageElement(number) {
  return document.getElementById(`page-${number}`);
}

function pageShellOf(node) {
  const element = node instanceof Element ? node : node?.parentElement;
  return element?.closest?.(".page[data-page]") ?? null;
}

let pointerPage = null;

/// The page immediately under the pointer. Unlike CSS :hover this also survives a drag or a
/// borrowed page mode, so exactly one page is lit at all times.
function markPointerPage(clientX, clientY) {
  const page = pageShellOf(document.elementFromPoint(clientX, clientY));
  if (pointerPage === page) return;
  pointerPage?.classList.remove("pointer-page");
  pointerPage = page;
  pointerPage?.classList.add("pointer-page");
}

function clearPointerPage() {
  pointerPage?.classList.remove("pointer-page");
  pointerPage = null;
}

/// Holding a modifier temporarily switches to that mode; releasing it switches back to pointer.
/// This makes the borrowed gesture behave exactly like the sticky mode, not merely look like it.
function syncBorrow(event) {
  if (event.target?.closest?.("input, textarea, select, [contenteditable='true']")) return;

  let mode = "";
  if (event.shiftKey) mode = "text";
  else if (event.metaKey || event.ctrlKey) mode = "pages";
  else if (event.altKey) mode = "box";

  if (borrowedMode) {
    if (mode === borrowedMode) return;
    const previous = borrowedMode;
    borrowedMode = null;
    if (toolMode === previous) applyTool("pointer");
    if (!mode || toolMode !== "pointer") {
      reportBorrow("");
      return;
    }
  } else if (pointerHeld) {
    return;
  }

  if (!mode || toolMode !== "pointer") {
    reportBorrow("");
    return;
  }
  borrowedMode = mode;
  applyTool(mode);
  reportBorrow(mode);
}

function applyTool(mode) {
  window.PDFScope?.setTool(mode);
}

function reportBorrow(mode) {
  window.__pdfscopeToolBorrowed?.(mode || "");
}

function endBorrow() {
  if (!borrowedMode) return;
  const previous = borrowedMode;
  borrowedMode = null;
  if (toolMode === previous) applyTool("pointer");
  reportBorrow("");
}

function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(resolve));
}

function clamp(value, minimum, maximum) {
  return Math.min(Math.max(value, minimum), maximum);
}

let thumbnailObserver = null;
let documentEpoch = 0;
const thumbnailCache = new Map();
const referencePreviewCache = new Map();

async function renderReferencePage(number, width) {
  if (!document_) return "";
  const epoch = documentEpoch;
  const documentAtRequest = document_;
  const key = `${epoch}:${number}:${width}`;
  const cached = referencePreviewCache.get(key);
  if (cached) return cached;
  const page = await documentAtRequest.getPage(number);
  const base = page.getViewport({ scale: 1 });
  const viewport = page.getViewport({ scale: (width * 2) / base.width });
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.ceil(viewport.width));
  canvas.height = Math.max(1, Math.ceil(viewport.height));
  await page.render({ canvasContext: canvas.getContext("2d"), viewport }).promise;
  // A preview started for the previous document must never populate the new document's ruler or
  // reference popover, even when the page number is identical.
  if (epoch !== documentEpoch || documentAtRequest !== document_) return "";
  const url = canvas.toDataURL("image/png");
  referencePreviewCache.set(key, url);
  return url;
}


async function renderThumbnail(element) {
  const number = Number(element.dataset.page);
  if (!number || !document_ || element.dataset.rendered === "true") return;
  element.dataset.rendered = "true";
  const epoch = documentEpoch;
  const documentAtRequest = document_;

  try {
    let url = thumbnailCache.get(number);
    if (!url) {
      const page = await documentAtRequest.getPage(number);
      const base = page.getViewport({ scale: 1 });
      const scale = 200 / base.width;
      const viewport = page.getViewport({ scale });
      element.style.setProperty("--thumb-aspect", `${base.width} / ${base.height}`);
      const canvas = document.createElement("canvas");
      canvas.width = Math.max(1, Math.ceil(viewport.width));
      canvas.height = Math.max(1, Math.ceil(viewport.height));
      const context = canvas.getContext("2d");
      await page.render({ canvasContext: context, viewport }).promise;
      if (epoch !== documentEpoch || documentAtRequest !== document_) return;
      url = canvas.toDataURL("image/png");
      thumbnailCache.set(number, url);
    }
    if (
      epoch !== documentEpoch ||
      documentAtRequest !== document_ ||
      !element.isConnected
    ) {
      return;
    }
    const image = document.createElement("img");
    image.src = url;
    image.alt = `第 ${number} 页`;
    image.draggable = false;
    image.loading = "lazy";
    element.querySelectorAll("img").forEach((existing) => existing.remove());
    element.prepend(image);
  } catch {
    element.dataset.rendered = "failed";
  }
}

function applyPageMode() {
  const pages = pagesElement();
  const paged = pageMode !== "continuous";
  pages?.classList.toggle("paged", paged);
  pages?.classList.toggle("spread", pageMode === "spread");
  markCurrentPage();

  metricsDirty = true;
  if (!paged) {
    resetPageLayers();
    reportZoom(true);
  }

  // Both differ only in layout, so the visual scroll must be re-established after the switch.
  requestAnimationFrame(() => {
    document.querySelector(".stage")?.scrollTo({ top: 0 });
    if (paged) pagedLayout();
    refreshPageRuler();
  });
}

/// Page 1 alone; even pages pair right, odd pages pair left.
function visiblePages() {
  const total = document_?.numPages ?? 0;
  const current = Math.min(Math.max(currentPage, 1), total || 1);
  if (pageMode === "single") return new Set([current]);

  if (spreadOffset) {
    const left = current % 2 === 1 ? current : current - 1;
    return new Set([left, Math.min(left + 1, total)]);
  }

  if (current === 1) return new Set([1]);
  if (current % 2 === 0) return new Set([current, Math.min(current + 1, total)]);
  return new Set([current - 1, current]);
}

/// Scales canvas and text layer as one unit, then resizes the box to match.
///
/// Sizes come from the PDF's own proportions, never from measuring the DOM: a page that was hidden or
/// already scaled would otherwise teach the layout the wrong base.
function pagedLayout() {
  const stage = document.querySelector(".stage");
  if (!stage || pageSizes.length === 0) return;

  const visible = visiblePages();
  const shown = [...document.querySelectorAll(".page[data-page]")].filter((element) =>
    visible.has(Number(element.dataset.page)),
  );
  if (shown.length === 0) return;

  const gap = 24;
  const columns = shown.length;
  const availableWidth = stageContentWidth(stage);
  const availableHeight = stageContentHeight(stage);
  const slotWidth = (availableWidth - (columns - 1) * gap) / columns;

  for (const element of shown) {
    const number = Number(element.dataset.page);
    const size = pageSizes[number - 1];
    if (!size) continue;

    const ratio = size.height / size.width;
    const baseWidth = slotWidth;
    const baseHeight = baseWidth * ratio;
    const scale = Math.max(0.1, Math.min(1, availableHeight / baseHeight));

    element.style.width = `${Math.round(baseWidth * scale)}px`;
    element.style.height = `${Math.round(baseHeight * scale)}px`;

    const layer = element.querySelector(".page-layer");
    if (!layer) continue;
    layer.style.width = `${Math.round(baseWidth)}px`;
    layer.style.height = `${Math.round(baseHeight)}px`;
    layer.style.transformOrigin = "top left";
    layer.style.transform = `scale(${scale})`;

    window.__pagedDebug = { scale, baseWidth, baseHeight, columns, availableWidth, availableHeight };
    // The header should say how much of the real page is shown, not the column's zoom. The callback
    // takes the same shape reportZoom sends: `{ percent, settled }`.
    const shownFraction = (baseWidth * scale) / availableWidth;
    window.__pdfscopeZoom?.({ percent: Math.round(shownFraction * 100), settled: true });
  }
}

function resetPageLayers() {
  for (const element of document.querySelectorAll(".page[data-page]")) {
    element.style.width = "";
    element.style.height = "";
    element.dataset.visible = "";
    const layer = element.querySelector(".page-layer");
    if (layer) {
      layer.style.transform = "";
      layer.style.width = "";
      layer.style.height = "";
    }
  }
}

/// In a spread, the right-hand sheet gets its page bar on the right, away from the spine.
function markSpreadSides() {
  const visible = [...visiblePages()].sort((a, b) => a - b);
  for (const element of document.querySelectorAll(".page[data-page]")) {
    const page = Number(element.dataset.page);
    if (pageMode !== "spread" || !visible.includes(page)) {
      element.dataset.spread = "";
      continue;
    }
    element.dataset.spread = page === visible[1] ? "right" : "left";
  }
}

function markCurrentPage() {
  const visible = visiblePages();
  for (const element of document.querySelectorAll(".page[data-page]")) {
    const page = Number(element.dataset.page);
    element.dataset.current = String(page === currentPage);
    element.dataset.visible = String(visible.has(page));
  }
  if (pageMode !== "continuous") {
    markSpreadSides();
    pagedLayout();
  }
}

function storageKey() {
  return documentName ? `pdfscope:pos:${documentName}` : "";
}

function rememberedPage() {
  if (!rememberPosition) return 0;
  const key = storageKey();
  if (!key) return 0;
  try {
    const value = Number(window.localStorage?.getItem(key));
    return Number.isFinite(value) && value > 0 ? value : 0;
  } catch {
    return 0;
  }
}

function rememberPage(page) {
  if (!rememberPosition) return;
  const key = storageKey();
  if (!key) return;
  try {
    window.localStorage?.setItem(key, String(page));
  } catch {
    /* private mode or quota; remembering a position is best effort */
  }
}


// ── Find in document ────────────────────────────────────────────────────────────────────────────
let findQuery = "";

function clearHighlights(root) {
  for (const mark of root.querySelectorAll("mark.find-hit")) {
    const parent = mark.parentNode;
    if (!parent) continue;
    parent.replaceChild(document.createTextNode(mark.textContent ?? ""), mark);
    parent.normalize();
  }
}

function highlightSpans(pageElement, query) {
  if (!query) return;
  const lowered = query.toLocaleLowerCase();
  for (const span of pageElement.querySelectorAll(".textLayer span")) {
    const text = span.textContent ?? "";
    if (!text || !text.toLocaleLowerCase().includes(lowered)) continue;

    const fragment = document.createDocumentFragment();
    let cursor = 0;
    const haystack = text.toLocaleLowerCase();
    for (;;) {
      const at = haystack.indexOf(lowered, cursor);
      if (at < 0) break;
      if (at > cursor) fragment.appendChild(document.createTextNode(text.slice(cursor, at)));
      const mark = document.createElement("mark");
      mark.className = "find-hit";
      mark.textContent = text.slice(at, at + query.length);
      fragment.appendChild(mark);
      cursor = at + query.length;
    }
    if (cursor < text.length) fragment.appendChild(document.createTextNode(text.slice(cursor)));
    span.replaceChildren(fragment);
  }
}

function applyFindToRendered() {
  for (const pageElement of document.querySelectorAll(".page[data-page]")) {
    clearHighlights(pageElement);
    if (findQuery) highlightSpans(pageElement, findQuery);
  }
}

function clampedPage(number) {
  const total = document_.numPages;
  return clamp(Math.round(Number(number) || 1), 1, total);
}

function stagePadding(stage) {
  const style = getComputedStyle(stage);
  return {
    top: Number.parseFloat(style.paddingTop) || 0,
    right: Number.parseFloat(style.paddingRight) || 0,
    bottom: Number.parseFloat(style.paddingBottom) || 0,
    left: Number.parseFloat(style.paddingLeft) || 0,
  };
}

function stageContentWidth(stage) {
  const style = getComputedStyle(stage);
  const padding = stagePadding(stage);
  const gutter = Number.parseFloat(style.getPropertyValue("--reader-gutter")) || 0;
  return Math.max(200, stage.clientWidth - padding.left - padding.right - gutter * 2);
}

function stageContentHeight(stage) {
  const padding = stagePadding(stage);
  return Math.max(1, stage.clientHeight - padding.top - padding.bottom);
}

function readingOffset() {
  const stage = stageElement();
  return stage ? Math.min(stage.clientHeight * READING_LINE, 240) : 120;
}

// ── Base layout and scaled scroll area ─────────────────────────────────────────────────────────

function setDisplayScale(scale) {
  displayScale = scale;
  const pages = pagesElement();
  if (!pages) return;
  pages.style.setProperty("--view-scale", String(scale));
  pages.style.setProperty("--ui-scale", String(1 / scale));
}

function updateScalerSize() {
  const scaler = scalerElement();
  if (!scaler || baseWidth <= 0 || baseHeight <= 0) return;
  scaler.style.width = `${baseWidth * displayScale}px`;
  scaler.style.height = `${baseHeight * displayScale}px`;
}

/// Measures the one layout the column keeps while zooming. Returns true when the page layout
/// changed and the mounted text layers/canvases must be rebuilt.
function measureBaseLayout(force = false) {
  const stage = stageElement();
  const pages = pagesElement();
  if (!stage || !pages || pages.querySelector(".page[data-page]") === null) return false;

  // Round the base width: a scrollbar appearing can change clientWidth by a fraction of a pixel,
  // and a sub-pixel reflow would move every page by several pixels over a long column.
  const width = Math.round(stageContentWidth(stage));
  const changed = force || baseWidth <= 0 || Math.abs(width - baseWidth) > 1;
  if (!changed) return false;

  // Keep the gap proportional to the page width. During a panel drag the whole column is scaled,
  // so a fixed 16px base gap would visibly grow back to 16px when the new base layout is committed.
  // Capturing the initial CSS gap as a ratio preserves the designed gap at first layout and makes
  // every later reflow match the drag's visual gap.
  if (baseWidth <= 0) {
    pages.style.removeProperty("gap");
    const cssGap = parseFloat(getComputedStyle(pages).gap);
    pageGapRatio = (Number.isFinite(cssGap) && cssGap > 0 ? cssGap : 16) / width;
    pages.style.setProperty("--page-gap", `${cssGap > 0 ? cssGap : 16}px`);
  } else {
    const gap = pageGapRatio > 0 ? pageGapRatio * width : 16;
    pages.style.gap = `${gap.toFixed(2)}px`;
    pages.style.setProperty("--page-gap", `${gap.toFixed(2)}px`);
  }

  baseWidth = width;
  pages.style.width = `${width}px`;
  // `offsetHeight` is the untransformed layout height, which is exactly what the scaler needs.
  baseHeight = pages.offsetHeight;
  baseEpoch += 1;
  metricsDirty = true;
  updateScalerSize();
  schedulePageRulerMeasure();
  return true;
}

function rebuildMetrics() {
  const stage = stageElement();
  const pages = pagesElement();
  if (!stage || !pages || baseWidth <= 0 || displayScale <= 0) {
    metrics = [];
    metricsDirty = false;
    return;
  }

  const pagesRect = pages.getBoundingClientRect();
  metrics = Array.from(document.querySelectorAll(".page[data-page]")).map((element) => {
    const rect = element.getBoundingClientRect();
    return {
      top: (rect.top - pagesRect.top) / displayScale,
      height: rect.height / displayScale,
    };
  });
  metricsDirty = false;
}

function pageAtBase(baseY) {
  if (metrics.length === 0) return 1;
  let low = 0;
  let high = metrics.length - 1;
  let found = 0;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (metrics[middle].top <= baseY) {
      found = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  return found + 1;
}

function focusBase() {
  const stage = stageElement();
  const pages = pagesElement();
  if (!stage || !pages) return null;
  const pagesRect = pages.getBoundingClientRect();
  return {
    baseY: (stage.getBoundingClientRect().top + readingOffset() - pagesRect.top) / displayScale,
  };
}

function pageAtFocus() {
  if (metricsDirty) rebuildMetrics();
  if (metrics.length === 0) return currentPage;
  const focus = focusBase();
  return focus ? pageAtBase(focus.baseY) : currentPage;
}

function readAnchor() {
  if (metricsDirty) rebuildMetrics();
  const focus = focusBase();
  if (!focus || metrics.length === 0) return { page: currentPage, fraction: 0 };
  const page = pageAtBase(focus.baseY);
  const metric = metrics[page - 1];
  const fraction = metric
    ? clamp((focus.baseY - metric.top) / Math.max(metric.height, 1), 0, 1)
    : 0;
  return { page, fraction };
}

function syncCurrentPage() {
  if (!document_) return;
  // In paged modes the page is chosen by navigation, never inferred from a scroll position that the
  // paged layout does not use.
  if (pageMode !== "continuous") return;
  const page = pageAtFocus();
  if (page !== currentPage) {
    currentPage = page;
    markCurrentPage();
    rememberPage(page);
    window.__pdfscopePage?.(page);
  }
}

function scrollToAnchor(anchor) {
  const stage = stageElement();
  const page = pageElement(anchor.page);
  if (!stage || !page) return;
  const stageRect = stage.getBoundingClientRect();
  const pageRect = page.getBoundingClientRect();
  const target = stageRect.top + readingOffset();
  const current = pageRect.top + anchor.fraction * pageRect.height;
  stage.scrollTop += current - target;
  currentPage = anchor.page;
  markCurrentPage();
  rememberPage(anchor.page);
  programmaticUntil = performance.now() + PROGRAMMATIC_SCROLL_MS;
  window.__pdfscopePage?.(anchor.page);
}

/// Puts a page's top edge just below the stage's top edge.
function scrollToPageNow(number) {
  const target = clampedPage(number);
  const stage = stageElement();
  const page = pageElement(target);
  if (stage && page) {
    const stageRect = stage.getBoundingClientRect();
    const pageRect = page.getBoundingClientRect();
    stage.scrollTop += pageRect.top - stageRect.top - 16;
  }
  currentPage = target;
  markCurrentPage();
  rememberPage(target);
  programmaticUntil = performance.now() + PROGRAMMATIC_SCROLL_MS;
  window.__pdfscopePage?.(target);
}

function pageVisible(number) {
  const stage = stageElement();
  const page = pageElement(number);
  if (!stage || !page) return false;
  const stageRect = stage.getBoundingClientRect();
  const pageRect = page.getBoundingClientRect();
  return pageRect.bottom > stageRect.top && pageRect.top < stageRect.bottom;
}

// ── Continuous zoom ────────────────────────────────────────────────────────────────────────────

function readingOrigin() {
  const stage = stageElement();
  if (!stage) return { x: 0, y: 0 };
  const rect = stage.getBoundingClientRect();
  // The page column is always centred in the whole stage; the permanent gutters are overlays.
  const x = rect.left + rect.width / 2;
  return { x, y: rect.top + readingOffset() };
}

function resolveOrigin(origin) {
  if (origin && Number.isFinite(origin.x) && Number.isFinite(origin.y)) return origin;
  if (lastPointer) return lastPointer;
  return readingOrigin();
}

function reportZoom(settled) {
  const percent = Math.round(displayScale * 100);
  if (!settled && percent === lastReportedZoom) return;
  lastReportedZoom = percent;
  window.__pdfscopeZoom?.({ percent, settled });
}

function clearSettle() {
  if (settleTimer) {
    clearTimeout(settleTimer);
    settleTimer = 0;
  }
}

function requestZoomFrame() {
  if (zoomFrame) return;
  zoomLastTime = performance.now();
  zoomFrame = requestAnimationFrame(zoomStep);
}

function setZoomTarget(scale, { direct = false, origin = null } = {}) {
  if (!document_) return;
  targetScale = clamp(scale, MIN_SCALE, MAX_SCALE);
  zoomDirect = direct;
  zoomOrigin = resolveOrigin(origin);
  zooming = true;
  clearSettle();
  pagesElement()?.classList.add("zooming");
  requestZoomFrame();
}

function requestZoomBy(steps, options = {}) {
  setZoomTarget(targetScale + 0.25 * Number(steps || 0), {
    direct: false,
    origin: options.origin ?? readingOrigin(),
  });
}

function requestZoomTo(percent, options = {}) {
  setZoomTarget(Number(percent) / 100, {
    direct: options.animate === false,
    origin: options.origin ?? readingOrigin(),
  });
}

function requestFitWidth() {
  setZoomTarget(1, { direct: false, origin: readingOrigin() });
}

/// One rAF owns the visual scale. A pinch is direct; wheel/buttons ease toward the target.
function zoomStep(now) {
  zoomFrame = 0;
  if (!document_) {
    zooming = false;
    return;
  }

  const delta = Math.min(64, Math.max(1, now - zoomLastTime));
  zoomLastTime = now;
  const origin = resolveOrigin(zoomOrigin);

  let nextScale;
  if (zoomDirect) {
    nextScale = targetScale;
  } else {
    const difference = targetScale - displayScale;
    nextScale =
      Math.abs(difference) < 0.0005
        ? targetScale
        : displayScale + difference * (1 - Math.exp(-delta / 45));
  }

  applyDisplayScale(nextScale, origin);
  reportZoom(false);

  if (Math.abs(targetScale - displayScale) > 0.0005) {
    zoomFrame = requestAnimationFrame(zoomStep);
  } else {
    scheduleSettle();
  }
}

function applyPan() {
  const stage = stageElement();
  if (!stage) return;
  if (pendingPanX || pendingPanY) {
    stage.scrollLeft += pendingPanX;
    stage.scrollTop += pendingPanY;
    pendingPanX = 0;
    pendingPanY = 0;
  }
}

/// Applies a new visual scale without reflowing the pages. The point under `origin` is restored by
/// measuring the transformed column before and after, so scrollbar rounding cannot accumulate.
function applyDisplayScale(scale, origin) {
  const stage = stageElement();
  const pages = pagesElement();
  const scaler = scalerElement();
  if (!stage || !pages || !scaler || baseWidth <= 0 || baseHeight <= 0) return;

  if (Math.abs(scale - displayScale) > 1e-7) {
    const before = pages.getBoundingClientRect();
    // Horizontal zoom is always page-centred. Vertical zoom still follows the reading line/pointer.
    // Using one owner for scrollLeft avoids the auto-margin→overflow jump (and its one-frame clamp)
    // that made the page visibly twist as it crossed fit width.
    const pointY = (origin.y - before.top) / displayScale;

    setDisplayScale(scale);
    updateScalerSize();

    // Keep short columns centred during the drag too. Resetting the margin here and restoring it
    // only on release would make a one-page or zoomed-out document jump vertically at pointer-up.
    if (document.body.classList.contains("panel-dragging")) {
      const available = stageContentHeight(stage);
      const scaledHeight = baseHeight * scale;
      scaler.style.marginTop =
        scaledHeight + 0.5 < available ? `${(available - scaledHeight) / 2}px` : "0px";
    } else {
      scaler.style.marginTop = "0px";
    }

    const after = pages.getBoundingClientRect();
    stage.scrollTop += after.top + pointY * displayScale - origin.y;
  }

  applyPan();
  centerHorizontally();
  schedulePageRulerViewport();

  function centerHorizontally() {
    // Auto margins centre the column while it still fits inside the full stage (including the
    // gutters). Once it is wider than the stage, scroll the overflow half-and-half instead.
    // Reading scrollWidth forces the new scroll range to be known before scrollLeft is assigned;
    // otherwise the first frame past the stage width clamps the correction and jitters.
    if (scaler.offsetWidth <= stage.clientWidth + 0.5) {
      stage.scrollLeft = 0;
    } else {
      void stage.scrollWidth;
      stage.scrollLeft = (scaler.offsetWidth - stage.clientWidth) / 2;
    }
  }

  if (!document.body.classList.contains("panel-dragging")) syncCurrentPage();
  scheduleUpdate(false);
}

function scheduleSettle() {
  clearSettle();
  settleTimer = setTimeout(finishZoom, SETTLE_MS);
}

function finishZoom() {
  settleTimer = 0;
  if (!document_) return;
  if (Math.abs(targetScale - displayScale) > 1e-7) {
    applyDisplayScale(targetScale, resolveOrigin(zoomOrigin));
  }
  // The visual state must equal the target even if the last frame stopped inside the easing
  // epsilon. Any residual is sub-pixel; the reading line has already been anchored.
  setDisplayScale(targetScale);
  updateScalerSize();
  zooming = false;
  reportZoom(true);
  centerAtRest();
  refreshPageRuler();
  updateWindow(true).catch(() => {});
  pagesElement()?.classList.remove("zooming");
}

function centerAtRest() {
  const stage = stageElement();
  const scaler = scalerElement();
  if (!stage || !scaler) return;

  const available = stageContentHeight(stage);
  const scaledHeight = baseHeight * displayScale;
  scaler.style.marginTop =
    scaledHeight + 0.5 < available ? `${(available - scaledHeight) / 2}px` : "0px";

  if (scaler.offsetWidth <= stage.clientWidth + 0.5) {
    stage.scrollLeft = 0;
  } else {
    void stage.scrollWidth;
    stage.scrollLeft = (scaler.offsetWidth - stage.clientWidth) / 2;
  }
}

// ── Virtual rendering ─────────────────────────────────────────────────────────────────────────

function scheduleUpdate(render = false) {
  updateRender = updateRender || render;
  if (document.body.classList.contains("panel-dragging")) return;
  if (frame) return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    const shouldRender = updateRender;
    updateRender = false;
    updateWindow(shouldRender).catch(() => {});
  });
}

async function updateWindow(render = false) {
  if (!document_ || !initialized) return;
  if (updatePromise) {
    updateAgain = true;
    updateRender = updateRender || render;
    return updatePromise;
  }

  updatePromise = (async () => {
    let shouldRender = render;
    do {
      updateAgain = false;
      const currentRender = shouldRender || updateRender;
      updateRender = false;
      shouldRender = false;
      await updateWindowOnce(currentRender);
    } while (updateAgain && document_);
  })();

  try {
    await updatePromise;
  } finally {
    updatePromise = null;
  }
}

async function updateWindowOnce(render) {
  if (!document_ || !initialized) return;
  const stage = stageElement();
  const pages = pagesElement();
  if (!stage || !pages) return;

  // Paged modes do not scroll: render exactly the sheets on show, whatever the column metrics say.
  if (pageMode !== "continuous") {
    const visible = visiblePages();
    desired = new Set(visible);
    for (const number of [...rendered.keys()]) {
      if (!visible.has(number)) releasePage(number);
    }
    await Promise.all(
      [...visible].map((number) =>
        ensurePage(number, render).catch((error) => {
          if (error?.name !== "RenderingCancelledException") {
            console.error("PDFScope: page render failed", number, error);
          }
        }),
      ),
    );
    pagedLayout();
    return;
  }

  if (metricsDirty) rebuildMetrics();
  if (metrics.length === 0) {
    desired.clear();
    return;
  }

  const stageRect = stage.getBoundingClientRect();
  const pagesRect = pages.getBoundingClientRect();
  const viewTop = (stageRect.top - pagesRect.top) / displayScale;
  const viewBottom = (stageRect.bottom - pagesRect.top) / displayScale;

  let low = 0;
  let high = metrics.length - 1;
  let first = metrics.length - 1;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (metrics[middle].top + metrics[middle].height > viewTop) {
      first = middle;
      high = middle - 1;
    } else {
      low = middle + 1;
    }
  }

  let last = first;
  while (last + 1 < metrics.length && metrics[last + 1].top < viewBottom) last += 1;

  const from = Math.max(0, first - WINDOW_MARGIN);
  const to = Math.min(metrics.length - 1, last + WINDOW_MARGIN);

  desired = new Set();
  for (let index = from; index <= to; index += 1) desired.add(index + 1);

  for (const number of [...rendered.keys()]) {
    const state = rendered.get(number);
    const layer = pageElement(number)?.querySelector(".page-layer");
    if (!desired.has(number) && (state?.mounted || (layer?.childElementCount ?? 0) > 0)) {
      releasePage(number);
    }
  }

  const jobs = [];
  for (const number of desired) {
    jobs.push(
      ensurePage(number, render).catch((error) => {
        if (error?.name !== "RenderingCancelledException") {
          console.error("PDFScope: page render failed", number, error);
        }
      }),
    );
  }
  await Promise.all(jobs);
}

function ensurePage(number, render) {
  if (!desired.has(number)) return Promise.resolve();
  const shell = pageElement(number);
  if (!shell) return Promise.resolve();

  let state = rendered.get(number);
  if (!state) {
    state = {
      token: 0,
      canvasToken: 0,
      mountTask: null,
      canvasTask: null,
      mounted: false,
      baseEpoch: -1,
      canvasScale: 0,
    };
    rendered.set(number, state);
  }

  if (!state.mounted || state.baseEpoch !== baseEpoch) {
    if (state.mountTask) return state.mountTask.then(() => ensurePage(number, render));
    state.mountTask = mountPage(number, state).finally(() => {
      state.mountTask = null;
    });
    return state.mountTask.then(() => ensurePage(number, render));
  }

  if (render && Math.abs(state.canvasScale - displayScale) > 0.0005) {
    return rerenderCanvas(number, state);
  }
  return Promise.resolve();
}

async function buildTextLayer(number, page, baseViewport, baseScale) {
  const container = document.createElement("div");
  container.className = "textLayer";
  // Fill the page layer instead of freezing the base viewport size. During a panel reflow the old
  // (wider) text layer would otherwise overflow for one frame and flash the stage's horizontal
  // scrollbar. Spans keep their PDF.js coordinates; .textLayer clips them until the next render.
  container.style.setProperty("--scale-factor", String(baseScale));

  const layer = new pdfjs.TextLayer({
    textContentSource: page.streamTextContent(),
    container,
    viewport: baseViewport,
  });
  await layer.render();

  container.dataset.page = String(number);
  container.classList.toggle("empty", container.querySelector("span") === null);
  return container;
}

/// A canvas whose CSS box is the base layout size, but whose backing store is dense enough for the
/// current visual scale. Above the pixel cap it lowers the density rather than allocating more.
function prepareCanvas(page, baseViewport, baseScale, scale) {
  const renderViewport = page.getViewport({ scale: baseScale * scale });
  const ratio = window.devicePixelRatio || 1;
  const widthPixels = Math.max(1, renderViewport.width * ratio);
  const heightPixels = Math.max(1, renderViewport.height * ratio);
  const factor = Math.min(
    1,
    Math.sqrt(MAX_CANVAS_PIXELS / (widthPixels * heightPixels)),
    MAX_CANVAS_DIM / widthPixels,
    MAX_CANVAS_DIM / heightPixels,
  );

  const canvas = document.createElement("canvas");
  canvas.className = "page-canvas";
  canvas.width = Math.max(1, Math.floor(widthPixels * factor));
  canvas.height = Math.max(1, Math.floor(heightPixels * factor));
  // The CSS box follows the page layer (and therefore the base layout). During a panel reflow the
  // old bitmap is briefly stretched into the new box instead of overflowing at its old base size;
  // rerenderCanvas replaces the backing store on the next frame.

  return {
    canvas,
    viewport: renderViewport,
    transform: [ratio * factor, 0, 0, ratio * factor, 0, 0],
  };
}

function drawCanvas(page, prepared) {
  return page.render({
    canvasContext: prepared.canvas.getContext("2d"),
    viewport: prepared.viewport,
    transform: prepared.transform,
  });
}

async function mountPage(number, state) {
  const shell = pageElement(number);
  const layer = shell?.querySelector(".page-layer");
  if (!shell || !layer) return;

  const token = (state.token += 1);
  const epoch = baseEpoch;
  const page = await document_.getPage(number);
  if (state.token !== token || epoch !== baseEpoch || !desired.has(number)) return;

  const pdfWidth = page.getViewport({ scale: 1 }).width;
  const baseScale = baseWidth / pdfWidth;
  const baseViewport = page.getViewport({ scale: baseScale });
  const canvasScale = displayScale;
  const prepared = prepareCanvas(page, baseViewport, baseScale, canvasScale);
  const textLayerPromise = buildTextLayer(number, page, baseViewport, baseScale);

  const task = drawCanvas(page, prepared);
  state.canvasTask = task;
  try {
    await task.promise;
  } finally {
    if (state.canvasTask === task) state.canvasTask = null;
  }

  const textLayer = await textLayerPromise;
  if (state.token !== token || epoch !== baseEpoch || !desired.has(number) || !textLayer) {
    return;
  }

  layer.replaceChildren(prepared.canvas, textLayer);
  if (pageMode !== "continuous") pagedLayout();
  if (findQuery) highlightSpans(layer, findQuery);
  state.mounted = true;
  state.baseEpoch = epoch;
  state.canvasScale = canvasScale;
  shell.dataset.rendered = "true";
}

async function rerenderCanvas(number, state) {
  const shell = pageElement(number);
  const layer = shell?.querySelector(".page-layer");
  if (!shell || !layer || !state.mounted) return;

  if (state.canvasTask) {
    try {
      state.canvasTask.cancel();
    } catch {
      /* a finished task cannot be cancelled */
    }
    state.canvasTask = null;
  }

  const token = (state.canvasToken += 1);
  const epoch = baseEpoch;
  const scale = displayScale;
  const page = await document_.getPage(number);
  if (state.canvasToken !== token || epoch !== baseEpoch || !state.mounted) return;

  const pdfWidth = page.getViewport({ scale: 1 }).width;
  const baseScale = baseWidth / pdfWidth;
  const baseViewport = page.getViewport({ scale: baseScale });
  const prepared = prepareCanvas(page, baseViewport, baseScale, scale);

  const task = drawCanvas(page, prepared);
  state.canvasTask = task;
  try {
    await task.promise;
  } finally {
    if (state.canvasTask === task) state.canvasTask = null;
  }

  if (state.canvasToken !== token || epoch !== baseEpoch || !state.mounted) return;

  const currentLayer = pageElement(number)?.querySelector(".page-layer");
  if (!currentLayer) return;
  const oldCanvas = currentLayer.querySelector("canvas");
  if (oldCanvas) {
    oldCanvas.replaceWith(prepared.canvas);
  } else {
    currentLayer.prepend(prepared.canvas);
  }
  state.canvasScale = scale;
}

function releasePage(number) {
  const state = rendered.get(number);
  if (state) {
    state.token += 1;
    state.canvasToken += 1;
    state.mounted = false;
    state.baseEpoch = -1;
    state.canvasScale = 0;
    if (state.canvasTask) {
      try {
        state.canvasTask.cancel();
      } catch {
        /* a finished task cannot be cancelled */
      }
      state.canvasTask = null;
    }
  }

  const shell = pageElement(number);
  const layer = shell?.querySelector(".page-layer");
  if (!layer) return;

  const selection = window.getSelection();
  if (
    selection &&
    !selection.isCollapsed &&
    selection.anchorNode &&
    layer.contains(selection.anchorNode)
  ) {
    selection.removeAllRanges();
    window.__pdfscopeCleared?.();
  }

  layer.replaceChildren();
  shell.removeAttribute("data-rendered");
}

function releaseAll() {
  for (const number of [...rendered.keys()]) releasePage(number);
  rendered.clear();
  desired.clear();
}

// ── Loading and scrolling ──────────────────────────────────────────────────────────────────────

/// Measures every page's own size. A failure falls back to the previous page's size rather than
/// guessing a standard paper, so a damaged page cannot move the scrollbar.
async function measurePages(count) {
  const sizes = new Array(count);
  let next = 0;

  const worker = async () => {
    while (next < count) {
      const index = next++;
      try {
        const page = await document_.getPage(index + 1);
        const viewport = page.getViewport({ scale: 1 });
        sizes[index] = { width: viewport.width, height: viewport.height };
      } catch {
        sizes[index] = sizes[index - 1] ?? { width: 612, height: 792 };
      }
    }
  };

  await Promise.all(Array.from({ length: Math.min(8, count) }, worker));

  for (let index = 0; index < count; index += 1) {
    sizes[index] ??= sizes[index - 1] ?? { width: 612, height: 792 };
  }
  return sizes;
}

async function waitForShells() {
  const expected = document_?.numPages ?? 0;
  for (let attempt = 0; attempt < 60; attempt += 1) {
    const scaler = scalerElement();
    const shells = document.querySelectorAll(".page[data-page]").length;
    if (scaler && !scaler.classList.contains("hidden") && shells >= expected) return true;
    await nextFrame();
  }
  return false;
}

function handleScroll() {
  if (!document_) return;
  if (captureArmed) disarmCapture();
  if (performance.now() >= programmaticUntil) syncCurrentPage();
  scheduleUpdate(false);
}

function bindStage() {
  const stage = stageElement();
  if (!stage || stageBound === stage) return;
  stageBound?.removeEventListener("scroll", handleScroll);
  stageBound = stage;
  stage.addEventListener("scroll", handleScroll, { passive: true });
  stage.addEventListener("wheel", onWheel, { passive: false });
  stage.addEventListener("gesturestart", onGestureStart, { passive: false });
  stage.addEventListener("gesturechange", onGestureChange, { passive: false });
  stage.addEventListener("gestureend", onGestureEnd, { passive: false });
  stage.addEventListener("pointermove", (event) => {
    lastPointer = { x: event.clientX, y: event.clientY };
    if (toolMode === "pages" || stage.dataset.borrow === "pages") {
      markPointerPage(event.clientX, event.clientY);
    }
  });
  stage.addEventListener("pointerleave", clearPointerPage);

  if (typeof ResizeObserver === "undefined" || resizeObserver) return;
  resizeObserver = new ResizeObserver(scheduleResizeCheck);
  resizeObserver.observe(stage);
}

function scheduleResizeCheck() {
  if (!document_ || !initialized) return;
  if (document.body.classList.contains("panel-dragging")) return;
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(runResizeCheck, 120);
}

function runResizeCheck() {
  if (!document_ || !initialized) return;
  if (zooming || zoomFrame) {
    resizeTimer = setTimeout(runResizeCheck, 160);
    return;
  }

  const stage = stageElement();
  if (!stage) return;
  if (Math.abs(Math.round(stageContentWidth(stage)) - baseWidth) <= 1) {
    centerAtRest();
    return;
  }

  const anchor = readAnchor();
  if (measureBaseLayout(false)) {
    scrollToAnchor(anchor);
    updateWindow(true).catch(() => {});
  } else {
    scrollToAnchor(anchor);
    scheduleUpdate(false);
  }
}

// ── Zoom input ────────────────────────────────────────────────────────────────────────────────

function onWheel(event) {
  if (!document_) return;
  if (!(event.ctrlKey || event.metaKey)) return;
  // Prevent the webview's own page zoom even while a WebKit gesture is in flight.
  event.preventDefault();
  if (gesture) return;

  const stage = stageElement();
  let delta = event.deltaY;
  if (event.deltaMode === 1) delta *= 16;
  else if (event.deltaMode === 2) delta *= stage?.clientHeight ?? 800;

  const factor = Math.exp(-delta / 100);
  if (!Number.isFinite(factor) || factor <= 0) return;

  if (event.deltaX) pendingPanX += event.deltaX;

  // A real ctrl/meta key means a mouse wheel; a synthetic ctrlKey without the key held is a
  // trackpad pinch in Chromium. Pinch follows the fingers; the wheel eases.
  const direct = event.ctrlKey && !ctrlKeyDown && event.deltaMode === 0;

  setZoomTarget(targetScale * factor, {
    direct,
    origin: { x: event.clientX, y: event.clientY },
  });
}

function gestureOrigin(event) {
  if (Number.isFinite(event.clientX) && Number.isFinite(event.clientY)) {
    return { x: event.clientX, y: event.clientY };
  }
  return resolveOrigin(lastPointer);
}

function onGestureStart(event) {
  if (!document_) return;
  event.preventDefault();
  gesture = { startScale: displayScale, origin: gestureOrigin(event) };
  zooming = true;
  clearSettle();
  pagesElement()?.classList.add("zooming");
}

function onGestureChange(event) {
  if (!document_ || !gesture) return;
  event.preventDefault();
  if (!(event.scale > 0)) return;
  const origin = gestureOrigin(event);
  gesture.origin = origin;
  setZoomTarget(gesture.startScale * event.scale, { direct: true, origin });
}

function onGestureEnd(event) {
  if (!gesture) return;
  event.preventDefault();
  gesture = null;
  if (Math.abs(targetScale - displayScale) < 0.0005) scheduleSettle();
  else requestZoomFrame();
}

// ── Selection ──────────────────────────────────────────────────────────────────────────────────

/// Union of the selection's client rects, normalised to the page that contains the selection. The
/// page and its text layer are transformed together, so the normalisation cancels the zoom.
/// A collapsed range at a point, but only when that point lands inside the page's text layer.
function caretInside(layer, clientX, clientY) {
  const document = layer.ownerDocument;
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

  if (!range) return null;
  if (!layer.contains(range.startContainer)) return null;
  return range;
}

/// Absolute character offset of a point inside the text layer.
function offsetAt(layer, node, offset) {
  try {
    const range = document.createRange();
    range.selectNodeContents(layer);
    range.setEnd(node, offset);
    return range.toString().length;
  } catch {
    return null;
  }
}

/// The range between two absolute character offsets of the layer's text.
function rangeBetween(layer, start, end) {
  const range = document.createRange();
  let position = 0;
  let started = false;
  let ended = false;
  const walker = document.createTreeWalker(layer, NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();

  while (node) {
    const length = node.textContent.length;
    if (!started && position + length >= start) {
      range.setStart(node, Math.max(0, Math.min(length, start - position)));
      started = true;
    }
    if (!ended && position + length >= end) {
      range.setEnd(node, Math.max(0, Math.min(length, end - position)));
      ended = true;
      break;
    }
    position += length;
    node = walker.nextNode();
  }

  return started && ended ? range : null;
}

/// Text and normalised boxes for a range inside a page.
function rangeGeometry(page, range) {
  const box = page.getBoundingClientRect();
  if (box.width === 0 || box.height === 0) return null;

  const unit = (value) => Math.min(Math.max(value, 0), 1);
  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  const rects = [];

  for (const line of mergedClientRects(range.getClientRects())) {
    if (line.width === 0 || line.height === 0) continue;
    left = Math.min(left, line.left);
    top = Math.min(top, line.top);
    right = Math.max(right, line.right);
    bottom = Math.max(bottom, line.bottom);
    rects.push({
      x: unit((line.left - box.left) / box.width),
      y: unit((line.top - box.top) / box.height),
      width: unit(line.width / box.width),
      height: unit(line.height / box.height),
    });
  }
  if (!Number.isFinite(left) || rects.length === 0) return null;

  return {
    text: range.toString().replace(/\s+/g, " ").trim(),
    rects,
    rect: {
      x: unit((left - box.left) / box.width),
      y: unit((top - box.top) / box.height),
      width: unit((right - left) / box.width),
      height: unit((bottom - top) / box.height),
    },
  };
}

/// Coalesces overlapping client rects into one box per line.
///
/// A Range can report a line box and, inside it, smaller boxes for the runs of a line (this happens
/// around styled or subscript spans). Painting all of them would draw a second, flatter highlight
/// inside the first.
function mergedClientRects(rects) {
  const merged = [];
  for (const rect of rects) {
    if (!(rect.width > 0) || !(rect.height > 0)) continue;

    let target = null;
    for (const line of merged) {
      const overlap = Math.min(line.bottom, rect.bottom) - Math.max(line.top, rect.top);
      const smaller = Math.min(line.bottom - line.top, rect.height);
      if (smaller > 0 && overlap / smaller > 0.5) {
        target = line;
        break;
      }
    }

    if (!target) {
      merged.push({ left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom });
      continue;
    }
    target.left = Math.min(target.left, rect.left);
    target.top = Math.min(target.top, rect.top);
    target.right = Math.max(target.right, rect.right);
    target.bottom = Math.max(target.bottom, rect.bottom);
  }

  return merged.map((line) => ({
    left: line.left,
    top: line.top,
    right: line.right,
    bottom: line.bottom,
    width: line.right - line.left,
    height: line.bottom - line.top,
  }));
}

function normalisedSelectionRect() {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return null;

  const range = selection.getRangeAt(0);
  const startPage = pageShellOf(range.startContainer);
  const endPage = pageShellOf(range.endContainer);
  if (!startPage || startPage !== endPage) return null;

  const layer = startPage.querySelector(".textLayer");
  if (!layer || !layer.contains(range.commonAncestorContainer)) return null;

  const box = startPage.getBoundingClientRect();
  if (box.width === 0 || box.height === 0) return null;

  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  const unit = (value) => Math.min(Math.max(value, 0), 1);
  const rects = [];
  for (const rect of mergedClientRects(range.getClientRects())) {
    if (rect.width === 0 || rect.height === 0) continue;
    left = Math.min(left, rect.left);
    top = Math.min(top, rect.top);
    const rectRight = rect.right ?? rect.left + rect.width;
    const rectBottom = rect.bottom ?? rect.top + rect.height;
    right = Math.max(right, rectRight);
    bottom = Math.max(bottom, rectBottom);
    rects.push({
      x: unit((rect.left - box.left) / box.width),
      y: unit((rect.top - box.top) / box.height),
      width: unit(rect.width / box.width),
      height: unit(rect.height / box.height),
    });
  }
  if (!Number.isFinite(left)) return null;

  const x = unit((left - box.left) / box.width);
  const y = unit((top - box.top) / box.height);
  const width = unit((right - left) / box.width);
  const height = unit((bottom - top) / box.height);
  if (width <= 0 || height <= 0) return null;

  const text = selection.toString().replace(/\s+/g, " ").trim();
  if (!text) return null;

  const number = Number(startPage.dataset.page);
  if (!number) return null;

  const start = offsetAt(layer, range.startContainer, range.startOffset);
  const end = offsetAt(layer, range.endContainer, range.endOffset);
  if (start === null || end === null) return null;

  return {
    page: number,
    x,
    y,
    width,
    height,
    text,
    rects,
    start: Math.min(start, end),
    end: Math.max(start, end),
  };
}

let settleSelectionTimer = null;
let hadSelection = false;

function clearReportedSelection() {
  if (!hadSelection) return;
  hadSelection = false;
  window.__pdfscopeCleared?.();
}

function reportSelection() {
  clearTimeout(settleSelectionTimer);
  settleSelectionTimer = setTimeout(() => {
    // A selection settles on release, not while the reader is still dragging.
    if (pointerHeld) return;
    const payload = normalisedSelectionRect();
    if (!payload) {
      clearReportedSelection();
      return;
    }
    hadSelection = true;
    window.__pdfscopeSelection?.({
      ...payload,
      auto: toolMode === "text" || gestureShift,
    });
    gestureShift = false;
  }, 120);
}

let pageGesture = null;

/// Paints the live preview for a page gesture: a marquee between the two points and an outline on
/// every page it covers. A click is the same code with a zero-sized box, so there is one path.
function updatePagePreview(clientX, clientY) {
  if (!pageGesture) return;
  pageGesture.lastX = clientX;
  pageGesture.lastY = clientY;
  const stage = stageElement();
  if (!stage) return;

  const stageBox = stage.getBoundingClientRect();
  // Both ends live in the stage's content space, so scrolling moves the box with the pages instead
  // of dragging the starting edge along the viewport.
  const currentX = clientX - stageBox.left + stage.scrollLeft;
  const currentY = clientY - stageBox.top + stage.scrollTop;
  const start = pageGesture.startContent;
  const left = Math.min(start.x, currentX);
  const top = Math.min(start.y, currentY);
  const width = Math.abs(currentX - start.x);
  const height = Math.abs(currentY - start.y);

  let marquee = stage.querySelector(".page-marquee");
  if (!marquee) {
    marquee = document.createElement("div");
    marquee.className = "page-marquee";
    stage.appendChild(marquee);
  }
  marquee.style.left = `${left}px`;
  marquee.style.top = `${top}px`;
  marquee.style.width = `${width}px`;
  marquee.style.height = `${height}px`;
  marquee.classList.toggle("subtract", Boolean(pageGesture.subtract));

  const leftmost = left;
  const rightmost = left + width;
  const topmost = top;
  const bottommost = top + height;

  for (const page of document.querySelectorAll(".page[data-page]")) {
    const box = page.getBoundingClientRect();
    // Compare in content space as well, or scrolled-away pages lose their outline.
    const pageLeft = box.left - stageBox.left + stage.scrollLeft;
    const pageTop = box.top - stageBox.top + stage.scrollTop;
    const pageRight = pageLeft + box.width;
    const pageBottom = pageTop + box.height;
    const covers =
      pageRight >= leftmost &&
      pageLeft <= rightmost &&
      pageBottom >= topmost &&
      pageTop <= bottommost;
    page.classList.toggle("range-preview", covers);
    // The batch inherits the gesture's meaning so the outline can say add (accent) or subtract (white).
    page.classList.toggle("subtract", covers && pageGesture.subtract);
  }
}

function cancelPageGesture() {
  if (!pageGesture) return;
  const { scrollFrame } = pageGesture;
  pageGesture = null;
  if (scrollFrame) cancelAnimationFrame(scrollFrame);
  stageElement()?.classList.remove("page-gesture");
  for (const page of document.querySelectorAll(".page.range-preview")) {
    page.classList.remove("range-preview", "subtract");
  }
  document.querySelector(".page-marquee")?.remove();
  document.removeEventListener("pointermove", onPageMove, true);
  document.removeEventListener("pointerup", onPageUp, true);
  clearPointerPage();
}

function finishPageGesture(clientX, clientY) {
  if (!pageGesture) return;
  const { page: from, startX, startY, subtract, scrollFrame } = pageGesture;
  pageGesture = null;
  if (scrollFrame) cancelAnimationFrame(scrollFrame);
  stageElement()?.classList.remove("page-gesture");

  for (const page of document.querySelectorAll(".page.range-preview")) {
    page.classList.remove("range-preview", "subtract");
  }
  document.querySelector(".page-marquee")?.remove();
  document.removeEventListener("pointermove", onPageMove, true);
  document.removeEventListener("pointerup", onPageUp, true);

  markPointerPage(clientX, clientY);
  const moved = Math.hypot(clientX - startX, clientY - startY) >= 4;
  const hit = pageShellOf(document.elementFromPoint(clientX, clientY));
  const to = hit ? Number(hit.dataset.page) : from;

  if (!moved && to === from) {
    window.__pdfscopeTogglePage?.(from);
  } else if (to !== from) {
    window.__pdfscopePageRange?.(from, to, subtract ? 1 : 0);
  } else {
    window.__pdfscopeTogglePage?.(from);
  }
}

const SCROLL_EDGE = 64;
const SCROLL_STEP = 24;

function autoScrollPages() {
  if (!pageGesture) return;
  const stage = stageElement();
  if (!stage) return;

  const box = stage.getBoundingClientRect();
  const y = pageGesture.lastY;
  let delta = 0;
  if (y < box.top + SCROLL_EDGE) {
    delta = -SCROLL_STEP * Math.min(1, (box.top + SCROLL_EDGE - y) / SCROLL_EDGE);
  } else if (y > box.bottom - SCROLL_EDGE) {
    delta = SCROLL_STEP * Math.min(1, (y - (box.bottom - SCROLL_EDGE)) / SCROLL_EDGE);
  }

  if (delta !== 0) {
    const before = stage.scrollTop;
    stage.scrollTop += delta;
    if (stage.scrollTop !== before) {
      updatePagePreview(pageGesture.lastX, pageGesture.lastY);
    }
  }
  // The page under the pointer can change while the column scrolls under a stationary cursor.
  markPointerPage(pageGesture.lastX, pageGesture.lastY);

  pageGesture.scrollFrame = requestAnimationFrame(autoScrollPages);
}

function onPageMove(event) {
  updatePagePreview(event.clientX, event.clientY);
  markPointerPage(event.clientX, event.clientY);
}

function onPageUp(event) {
  finishPageGesture(event.clientX, event.clientY);
}

document.addEventListener("pointercancel", cancelPageGesture, true);

document.addEventListener(
  "pointerdown",
  (event) => {
    if (event.button !== 0) return;
    if (toolMode !== "pages" && !event.metaKey && !event.ctrlKey) return;
    if (event.target.closest(".page-bar-actions, .page-range-bar, .ref-frame, .ref-handle")) return;
    const page = pageShellOf(event.target);
    if (!page) return;

    event.preventDefault();
    const stage = stageElement();
    if (!stage) return;
    stage.classList.add("page-gesture");
    markPointerPage(event.clientX, event.clientY);
    const stageBox = stage.getBoundingClientRect();
    pageGesture = {
      page: Number(page.dataset.page),
      startX: event.clientX,
      startY: event.clientY,
      startContent: {
        x: event.clientX - stageBox.left + stage.scrollLeft,
        y: event.clientY - stageBox.top + stage.scrollTop,
      },
      // Drag, click and modifier borrow all follow the page the gesture starts on.
      subtract: page.classList.contains("in-scope"),
    };
    updatePagePreview(event.clientX, event.clientY);
    pageGesture.scrollFrame = requestAnimationFrame(autoScrollPages);
    document.addEventListener("pointermove", onPageMove, true);
    document.addEventListener("pointerup", onPageUp, true);
  },
  true,
);

document.addEventListener(
  "pointerdown",
  () => {
    pointerHeld = true;
  },
  true,
);
document.addEventListener(
  "pointerup",
  () => {
    pointerHeld = false;
    reportSelection();
  },
  true,
);

document.addEventListener("selectionchange", reportSelection);
document.addEventListener("mouseup", reportSelection);
document.addEventListener("keyup", reportSelection);

window.addEventListener("keydown", (event) => {
  if (event.key === "Control") ctrlKeyDown = true;
  syncBorrow(event);
});
window.addEventListener("keyup", (event) => {
  if (event.key === "Control") ctrlKeyDown = false;
  syncBorrow(event);
});
window.addEventListener("blur", () => {
  ctrlKeyDown = false;
  endBorrow();
  cancelPageGesture();
  clearPointerPage();
});

// ── Region capture ─────────────────────────────────────────────────────────────────────────────

let captureArmed = false;
let capturePage = 0;
let marqueeStart = null;
let marqueeElement = null;

function normaliseIn(page, clientX, clientY) {
  const box = page?.getBoundingClientRect();
  if (!box || box.width === 0 || box.height === 0) return null;
  const clampValue = (value) => Math.min(Math.max(value, 0), 1);
  return {
    x: clampValue((clientX - box.left) / box.width),
    y: clampValue((clientY - box.top) / box.height),
  };
}

function drawMarquee(page, from, to) {
  if (!marqueeElement) {
    marqueeElement = document.createElement("div");
    marqueeElement.className = "marquee";
    page.appendChild(marqueeElement);
  }
  // The marquee lives in the page's base coordinate system, so use the untransformed layout size.
  const width = page.clientWidth;
  const height = page.clientHeight;
  marqueeElement.style.left = `${Math.min(from.x, to.x) * width}px`;
  marqueeElement.style.top = `${Math.min(from.y, to.y) * height}px`;
  marqueeElement.style.width = `${Math.abs(to.x - from.x) * width}px`;
  marqueeElement.style.height = `${Math.abs(to.y - from.y) * height}px`;
}

function clearMarquee() {
  marqueeElement?.remove();
  marqueeElement = null;
}

/// Renders the page at a scale chosen for the crop, then cuts the rectangle out of it.
async function renderRegion(pageNumber, rect) {
  const page = await document_.getPage(pageNumber);
  const base = page.getViewport({ scale: 1 });
  const cropWidth = Math.max(rect.width * base.width, 1);
  const scale = Math.min(4, Math.max(1, 1600 / cropWidth));
  const viewport = page.getViewport({ scale });

  const full = document.createElement("canvas");
  full.width = Math.floor(viewport.width);
  full.height = Math.floor(viewport.height);
  await page.render({ canvasContext: full.getContext("2d"), viewport }).promise;

  const sx = rect.x * full.width;
  const sy = rect.y * full.height;
  const sw = Math.max(rect.width * full.width, 1);
  const sh = Math.max(rect.height * full.height, 1);

  const cropped = document.createElement("canvas");
  cropped.width = Math.round(sw);
  cropped.height = Math.round(sh);
  cropped.getContext("2d").drawImage(full, sx, sy, sw, sh, 0, 0, cropped.width, cropped.height);

  return cropped.toDataURL("image/png");
}

function reportCapture() {
  window.__pdfscopeCapture?.(captureArmed);
}

function armCapture() {
  if (!document_) return;
  if (zooming || gesture) {
    if (zoomFrame) cancelAnimationFrame(zoomFrame);
    zoomFrame = 0;
    clearSettle();
    targetScale = displayScale;
    zooming = false;
    gesture = null;
    pagesElement()?.classList.remove("zooming");
  }
  captureArmed = true;
  reportCapture();
  stageElement()?.classList.add("capturing");
  window.getSelection()?.removeAllRanges();
  window.__pdfscopeCleared?.();
}

function disarmCapture() {
  captureArmed = false;
  reportCapture();
  capturePage = 0;
  marqueeStart = null;
  clearMarquee();
  stageElement()?.classList.remove("capturing");
}

document.addEventListener(
  "pointerdown",
  (event) => {
    gestureShift = event.shiftKey;

    // Modifier borrowing: Alt draws a box, Cmd/Ctrl clicks a page, without changing the tool.
    if (event.altKey && toolMode === "pointer") {
      const page = pageShellOf(event.target);
      if (page) armCapture();
    }

  },
  true,
);

document.addEventListener("pointerdown", (event) => {
  if (!captureArmed) return;
  // A focused reference owns the pointer; the tool must not start a marquee on top of it.
  if (event.target.closest(".ref-frame")) return;
  const page = pageShellOf(event.target);
  if (!page) return;
  const point = normaliseIn(page, event.clientX, event.clientY);
  if (!point) return;
  capturePage = Number(page.dataset.page);
  marqueeStart = point;
  event.preventDefault();
});

document.addEventListener("pointermove", (event) => {
  if (!captureArmed || !marqueeStart) return;
  const page = pageElement(capturePage);
  if (!page) return;
  const current = normaliseIn(page, event.clientX, event.clientY);
  if (current) drawMarquee(page, marqueeStart, current);
});

document.addEventListener("pointerup", async (event) => {
  if (!captureArmed || !marqueeStart) return;
  const page = pageElement(capturePage);
  const end = normaliseIn(page, event.clientX, event.clientY);
  const start = marqueeStart;
  const pageNumber = capturePage;
  marqueeStart = null;
  clearMarquee();
  // The box tool is sticky: stay armed instead of disarm/re-arm, which would tell Rust to drop the tool.
  if (toolMode === "box") {
    capturePage = 0;
  } else {
    disarmCapture();
  }

  if (!page || !end || !start) return;

  const rect = {
    x: Math.min(start.x, end.x),
    y: Math.min(start.y, end.y),
    width: Math.abs(end.x - start.x),
    height: Math.abs(end.y - start.y),
  };

  const box = page.getBoundingClientRect();
  if (rect.width * box.width < 4 || rect.height * box.height < 4) return;

  try {
    const image = await renderRegion(pageNumber, rect);
    window.__pdfscopeRegion?.({ page: pageNumber, ...rect, image });
  } catch {
    window.__pdfscopeRegion?.({ page: pageNumber, ...rect, image: "" });
  }
});

window.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && captureArmed) disarmCapture();
});

// ── Public boundary ────────────────────────────────────────────────────────────────────────────


const ATTACHMENT_TEXT_LIMIT = 200_000;
const ATTACHMENT_FILE_LIMIT = 25 * 1024 * 1024;
const TEXT_EXTENSIONS = new Set([
  "txt", "md", "markdown", "csv", "tsv", "json", "jsonl", "log", "yaml", "yml",
  "html", "htm", "xml", "toml", "ini", "cfg", "conf", "rs", "ts", "tsx", "js",
  "jsx", "mjs", "cjs", "py", "java", "kt", "c", "h", "cc", "cpp", "hpp", "cs",
  "go", "rb", "php", "swift", "scala", "sh", "bash", "zsh", "fish", "sql",
  "css", "scss", "less", "vue", "svelte", "lua", "pl", "r", "m", "gitignore",
]);

function extensionOf(name) {
  const dot = name.lastIndexOf(".");
  return dot < 0 ? "" : name.slice(dot + 1).toLowerCase();
}

async function readOneAttachment(file) {
  const extension = extensionOf(file.name);

  if (file.type.startsWith("image/")) {
    if (file.size > ATTACHMENT_FILE_LIMIT) {
      throw new Error("图片太大，请换一张小于 25MB 的");
    }
    const data = await readAsDataUrl(file);
    return { kind: "image", name: file.name, data };
  }

  if (file.size > ATTACHMENT_FILE_LIMIT) {
    throw new Error("文件太大，请换一个小于 25MB 的");
  }

  if (extension === "pdf") {
    return { kind: "text", name: file.name, data: await readPdfText(file) };
  }
  if (extension === "docx") {
    return { kind: "text", name: file.name, data: await readOfficeText(file, "docx") };
  }
  if (extension === "pptx") {
    return { kind: "text", name: file.name, data: await readOfficeText(file, "pptx") };
  }
  if (extension === "xlsx") {
    return { kind: "text", name: file.name, data: await readOfficeText(file, "xlsx") };
  }
  if (file.type.startsWith("text/") || TEXT_EXTENSIONS.has(extension)) {
    if (file.size > ATTACHMENT_FILE_LIMIT) {
      throw new Error("文本文件太大，请换一个小于 25MB 的");
    }
    const data = await file.text();
    return { kind: "text", name: file.name, data: clampText(data) };
  }

  throw new Error(`暂不支持 .${extension || "未知"} 文件`);
}

function readAsDataUrl(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

function clampText(text) {
  const characters = [...text];
  if (characters.length <= ATTACHMENT_TEXT_LIMIT) return text;
  return `${characters.slice(0, ATTACHMENT_TEXT_LIMIT).join("")}\n…（已截断）`;
}

/// Extracts the text of an attached PDF; page markers keep the model honest about where it read.
async function readPdfText(file) {
  const data = new Uint8Array(await file.arrayBuffer());
  const pdf = await pdfjsLib.getDocument({ data }).promise;
  const parts = [];
  let length = 0;

  for (let number = 1; number <= pdf.numPages; number += 1) {
    const page = await pdf.getPage(number);
    const content = await page.getTextContent();
    const text = content.items.map((item) => item.str ?? "").join(" ").replace(/\s+/g, " ").trim();
    if (!text) continue;
    const block = `[附件第 ${number} 页]\n${text}`;
    parts.push(block);
    length += block.length;
    if (length >= ATTACHMENT_TEXT_LIMIT) break;
  }

  if (parts.length === 0) throw new Error("这个 PDF 没有可提取的文字（可能是扫描件）");
  return clampText(parts.join("\n\n"));
}

/// Minimal ZIP reader: central directory, stored or raw-deflated entries.
async function readZip(file) {
  const buffer = await file.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);
  let eocd = -1;
  const earliest = Math.max(0, bytes.length - 65557);
  for (let index = bytes.length - 22; index >= earliest; index -= 1) {
    if (view.getUint32(index, true) === 0x06054b50) {
      eocd = index;
      break;
    }
  }
  if (eocd < 0) throw new Error("这个 Office 文件的压缩结构无法识别");

  const count = view.getUint16(eocd + 10, true);
  let cursor = view.getUint32(eocd + 16, true);
  const entries = new Map();

  for (let index = 0; index < count; index += 1) {
    if (view.getUint32(cursor, true) !== 0x02014b50) break;
    const method = view.getUint16(cursor + 10, true);
    const compressedSize = view.getUint32(cursor + 20, true);
    const nameLength = view.getUint16(cursor + 28, true);
    const extraLength = view.getUint16(cursor + 30, true);
    const commentLength = view.getUint16(cursor + 32, true);
    const localOffset = view.getUint32(cursor + 42, true);
    const name = new TextDecoder().decode(bytes.subarray(cursor + 46, cursor + 46 + nameLength));
    entries.set(name, { method, compressedSize, localOffset });
    cursor += 46 + nameLength + extraLength + commentLength;
  }

  const names = [...entries.keys()];
  const read = async (name) => {
    const entry = entries.get(name);
    if (!entry) return null;
    const local = entry.localOffset;
    if (view.getUint32(local, true) !== 0x04034b50) return null;
    const nameLength = view.getUint16(local + 26, true);
    const extraLength = view.getUint16(local + 28, true);
    const start = local + 30 + nameLength + extraLength;
    const data = bytes.subarray(start, start + entry.compressedSize);

    if (entry.method === 0) return new TextDecoder().decode(data);
    if (entry.method === 8) {
      if (typeof DecompressionStream !== "function") {
        throw new Error("当前 webview 不支持解压 Office 文件");
      }
      const stream = new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
      return new TextDecoder().decode(await new Response(stream).arrayBuffer());
    }
    throw new Error("这个 Office 文件使用了不支持的压缩方式");
  };

  return { names, read };
}

function parseXml(text) {
  const document = new DOMParser().parseFromString(text, "application/xml");
  if (document.querySelector("parsererror")) throw new Error("这个 Office 文件的内容无法解析");
  return document;
}

function textsOf(root, tag) {
  return [...root.getElementsByTagNameNS("*", tag)].map((node) => node.textContent ?? "");
}

async function readOfficeText(file, kind) {
  const zip = await readZip(file);
  const extension = extensionOf(file.name);

  if (kind === "docx") {
    const xml = await zip.read("word/document.xml");
    if (!xml) throw new Error(`${extension} 缺少正文，无法读取`);
    const document = parseXml(xml);
    const paragraphs = [...document.getElementsByTagNameNS("*", "p")]
      .map((paragraph) => textsOf(paragraph, "t").join(""))
      .filter((line) => line.trim());
    return clampText(paragraphs.join("\n"));
  }

  if (kind === "pptx") {
    const slides = zip.names
      .filter((name) => /^ppt\/slides\/slide\d+\.xml$/.test(name))
      .sort((left, right) => slideNumber(left) - slideNumber(right));
    const parts = [];
    for (const [index, name] of slides.entries()) {
      const xml = await zip.read(name);
      if (!xml) continue;
      const document = parseXml(xml);
      const lines = [...document.getElementsByTagNameNS("*", "p")]
        .map((paragraph) => textsOf(paragraph, "t").join(""))
        .filter((line) => line.trim());
      if (lines.length) parts.push(`[附件第 ${index + 1} 张幻灯片]\n${lines.join("\n")}`);
    }
    if (parts.length === 0) throw new Error("这个演示文稿没有可提取的文字");
    return clampText(parts.join("\n\n"));
  }

  // xlsx
  const shared = [];
  const sharedXml = await zip.read("xl/sharedStrings.xml");
  if (sharedXml) {
    const document = parseXml(sharedXml);
    for (const item of document.getElementsByTagNameNS("*", "si")) {
      shared.push(textsOf(item, "t").join(""));
    }
  }

  const sheets = zip.names
    .filter((name) => /^xl\/worksheets\/sheet\d+\.xml$/.test(name))
    .sort();
  const parts = [];
  for (const [index, name] of sheets.entries()) {
    const xml = await zip.read(name);
    if (!xml) continue;
    const document = parseXml(xml);
    const rows = [];
    for (const row of document.getElementsByTagNameNS("*", "row")) {
      const cells = [];
      for (const cell of row.getElementsByTagNameNS("*", "c")) {
        const reference = cell.getAttribute("r") ?? "";
        const type = cell.getAttribute("t") ?? "";
        const value = textsOf(cell, "v")[0] ?? textsOf(cell, "t")[0] ?? "";
        const text = type === "s" ? shared[Number(value)] ?? "" : value;
        if (text.trim()) cells.push(`${reference}: ${text.trim()}`);
      }
      if (cells.length) rows.push(cells.join(" | "));
    }
    if (rows.length) parts.push(`[附件工作表 ${index + 1}]\n${rows.join("\n")}`);
  }
  if (parts.length === 0) throw new Error("这个表格没有可提取的文字");
  return clampText(parts.join("\n\n"));
}

function slideNumber(name) {
  const match = name.match(/slide(\d+)\.xml$/);
  return match ? Number(match[1]) : 0;
}

function panelBodyElement() {
  return document.querySelector(".panel-body");
}

const PANEL_BOTTOM_SLOP = 12;
const PANEL_FOLLOW_SLOP = 8;
const PANEL_ANCHOR_RATIO = 0.35;
const PANEL_ANCHOR_BLOCKS =
  "p, li, pre, blockquote, h1, h2, h3, h4, h5, h6, .turn-question, .answer-text, .think-pill, .turn, .empty-state, .error-card";

// FOLLOW-END state. It is armed by a send and disarmed only by a deliberate upward scroll; while
// armed, every content growth or panel resize keeps the transcript at its end. When disarmed, panel
// resizes anchor the content at the 35% reading line instead.
let panelFollowing = false;
let panelBodyBound = null;
let panelMutation = null;
let panelTranscriptObserver = null;
let panelObservedTranscript = null;
let panelLastScrollTop = 0;

function panelDistanceFromEnd(body) {
  return body.scrollHeight - body.scrollTop - body.clientHeight;
}

function scrollPanelTo(body, top) {
  body.scrollTop = top;
  panelLastScrollTop = body.scrollTop;
}

function keepPanelAtEnd() {
  if (!panelFollowing) return;
  const body = panelBodyElement();
  if (body && panelDistanceFromEnd(body) > 0.5) {
    scrollPanelTo(body, body.scrollHeight);
  }
}

function refreshPanelTranscriptObserver() {
  const transcript = document.getElementById("transcript");
  if (transcript === panelObservedTranscript) return;
  panelTranscriptObserver?.disconnect();
  panelObservedTranscript = transcript;
  if (!transcript || typeof ResizeObserver !== "function") return;
  panelTranscriptObserver = new ResizeObserver(() => {
    if (panelFollowing) keepPanelAtEnd();
  });
  panelTranscriptObserver.observe(transcript);
}

function onPanelScroll() {
  const body = panelBodyBound;
  if (!body) return;
  // Width reflow and auto-scroll both dispatch scroll events. During a panel drag the mode chosen
  // on pointer-down owns the transcript; a transient scroll must not toggle FOLLOW-END/centre midway.
  if (document.body.classList.contains("panel-dragging")) {
    panelLastScrollTop = body.scrollTop;
    return;
  }
  const top = body.scrollTop;
  const distance = panelDistanceFromEnd(body);
  // Only an upward scroll is a detach gesture. Auto-follow moves down; content growth does not move
  // the scroll position at all. Testing distance alone would let a late scroll event from an
  // auto-follow, fired after the next chunk grew the transcript, disable following by accident.
  if (distance > PANEL_BOTTOM_SLOP && top < panelLastScrollTop - 1) {
    panelFollowing = false;
  } else if (distance <= PANEL_FOLLOW_SLOP) {
    panelFollowing = true;
    if (distance > 0.5) keepPanelAtEnd();
  }
  panelLastScrollTop = top;
}

function onPanelWheel(event) {
  // Wheel-up is an explicit reading gesture; detach before the first scroll event so no queued
  // content update can pull the reader back down.
  if (event.deltaY < 0) panelFollowing = false;
}

function bindPanelScroll() {
  const body = panelBodyElement();
  if (!body || panelBodyBound === body) return;
  panelBodyBound = body;
  panelLastScrollTop = body.scrollTop;
  body.addEventListener("scroll", onPanelScroll, { passive: true });
  body.addEventListener("wheel", onPanelWheel, { passive: true });
  panelMutation?.disconnect();
  panelMutation = new MutationObserver(() => {
    refreshPanelTranscriptObserver();
    if (panelFollowing) keepPanelAtEnd();
  });
  panelMutation.observe(body, { childList: true, subtree: true, characterData: true });
  refreshPanelTranscriptObserver();
  if (panelDistanceFromEnd(body) <= PANEL_FOLLOW_SLOP) {
    panelFollowing = true;
  }
}

function armPanelFollow() {
  bindPanelScroll();
  panelFollowing = true;
  keepPanelAtEnd();
}

/// A vertical reading anchor for the transcript at 35% of the viewport (the same reading line as the
/// PDF stage). The anchor is the smallest block under that line plus the fraction into it. A pure
/// scrollTop ratio would drift as text rewraps; a block ratio follows the same paragraph through
/// width changes.
function capturePanelAnchor() {
  if (panelFollowing) return null;
  const body = panelBodyElement();
  if (!body) return null;
  const box = body.getBoundingClientRect();
  const x = box.left + body.clientWidth / 2;
  const y = box.top + body.clientHeight * PANEL_ANCHOR_RATIO;
  const hit = document.elementFromPoint(x, y);
  let element = hit && body.contains(hit) ? hit.closest?.(PANEL_ANCHOR_BLOCKS) : null;
  if (element === body) element = null;

  if (!element) {
    // The centre can land in the gap between two turns, or on the scrollbar/padding.
    let nearest = null;
    let nearestDistance = Infinity;
    for (const turn of body.querySelectorAll(".turn, .empty-state, .error-card")) {
      const rect = turn.getBoundingClientRect();
      const distance = y < rect.top ? rect.top - y : y > rect.bottom ? y - rect.bottom : 0;
      if (distance < nearestDistance) {
        nearest = turn;
        nearestDistance = distance;
      }
    }
    element = nearest;
  }

  if (element) {
    const rect = element.getBoundingClientRect();
    if (rect.height > 0) {
      return { element, ratio: clamp((y - rect.top) / rect.height, 0, 1) };
    }
  }

  const range = Math.max(1, body.scrollHeight - body.clientHeight);
  return { scrollRatio: clamp(body.scrollTop / range, 0, 1) };
}

function restorePanelAnchor(anchor) {
  if (!anchor) return;
  const body = panelBodyElement();
  if (!body) return;
  let top = null;
  if (anchor.element?.isConnected) {
    const rect = anchor.element.getBoundingClientRect();
    if (rect.height > 0) {
      const line = body.getBoundingClientRect().top + body.clientHeight * PANEL_ANCHOR_RATIO;
      top = body.scrollTop + rect.top + anchor.ratio * rect.height - line;
    }
  }
  if (top === null && anchor.scrollRatio !== undefined) {
    top = anchor.scrollRatio * Math.max(1, body.scrollHeight - body.clientHeight);
  }
  if (top !== null) scrollPanelTo(body, Math.max(0, top));
}

// ── OOBE document preview ─────────────────────────────────────────────────────────────────────
//
// Hovering a recent/recommended row for a moment loads that PDF independently of the open document
// and renders the first N pages into a one-row bubble above the row. Recent bytes come from IndexedDB;
// recommended files come through the native bridge. Nothing is retained after the bubble closes.

const PREVIEW_DELAY = 600;
const PREVIEW_PAGE_WIDTH = 78;
const PREVIEW_CACHE_LIMIT = 24;
const previewCache = new Map();
let previewTimer = 0;
let previewToken = 0;
let previewRow = null;
let previewElement = null;

function clearPreview() {
  if (previewTimer) {
    clearTimeout(previewTimer);
    previewTimer = 0;
  }
  previewToken += 1;
  previewRow = null;
  previewElement?.remove();
  previewElement = null;
}

function previewLimit() {
  const width = Math.min(window.innerWidth - 32, 780);
  return Math.max(3, Math.min(12, Math.floor((width - 24) / PREVIEW_PAGE_WIDTH)));
}

async function previewBytes(row) {
  const recentId = row.dataset.previewRecent;
  if (recentId && window.Recents) {
    const record = await window.Recents.load(recentId);
    return record?.bytes ?? null;
  }
  const path = row.dataset.previewPath;
  if (path && window.Bridge?.available) {
    return await window.Bridge.readPdf(path);
  }
  return null;
}

async function renderPreviewPages(bytes, limit) {
  const documentTask = pdfjs.getDocument({ data: new Uint8Array(bytes) });
  const document_ = await documentTask.promise;
  try {
    const pages = [];
    const count = Math.min(document_.numPages, limit);
    for (let number = 1; number <= count; number += 1) {
      const page = await document_.getPage(number);
      const base = page.getViewport({ scale: 1 });
      const viewport = page.getViewport({ scale: (PREVIEW_PAGE_WIDTH * 2) / base.width });
      const canvas = document.createElement("canvas");
      canvas.width = Math.max(1, Math.ceil(viewport.width));
      canvas.height = Math.max(1, Math.ceil(viewport.height));
      await page.render({ canvasContext: canvas.getContext("2d"), viewport }).promise;
      pages.push({
        number,
        url: canvas.toDataURL("image/png"),
        width: canvas.width,
        height: canvas.height,
      });
    }
    return { pages, total: document_.numPages };
  } finally {
    await document_.destroy();
  }
}

function cachedPreview(key, loader) {
  let pending = previewCache.get(key);
  if (!pending) {
    pending = loader().catch((error) => {
      previewCache.delete(key);
      throw error;
    });
    previewCache.set(key, pending);
    if (previewCache.size > PREVIEW_CACHE_LIMIT) {
      const oldest = previewCache.keys().next().value;
      if (oldest !== undefined) previewCache.delete(oldest);
    }
  }
  return pending;
}

function positionPreview(row, element) {
  const rowRect = row.getBoundingClientRect();
  const rect = element.getBoundingClientRect();
  const left = Math.min(
    Math.max(12, rowRect.left + rowRect.width / 2 - rect.width / 2),
    window.innerWidth - rect.width - 12,
  );
  let top = rowRect.top - rect.height - 10;
  if (top < 12) top = rowRect.bottom + 10;
  element.style.left = `${Math.max(12, left)}px`;
  element.style.top = `${top}px`;
}

async function showPreview(row) {
  if (!row.isConnected || !row.matches(":hover")) return;
  const token = previewToken;
  const key = row.dataset.previewRecent
    ? `recent:${row.dataset.previewRecent}`
    : row.dataset.previewPath
      ? `path:${row.dataset.previewPath}`
      : "";
  if (!key) return;
  try {
    const data = await cachedPreview(key, async () => {
      const bytes = await previewBytes(row);
      if (!bytes) throw new Error("preview unavailable");
      return renderPreviewPages(bytes, previewLimit());
    });
    if (token !== previewToken || !row.isConnected) return;

    const bubble = document.createElement("div");
    bubble.className = "oobe-preview";
    bubble.setAttribute("role", "tooltip");
    const strip = document.createElement("div");
    strip.className = "oobe-preview-strip";
    for (const page of data.pages) {
      const figure = document.createElement("figure");
      figure.className = "oobe-preview-page";
      const image = document.createElement("img");
      image.src = page.url;
      image.width = page.width;
      image.height = page.height;
      image.alt = `第 ${page.number} 页`;
      const caption = document.createElement("figcaption");
      caption.textContent = String(page.number);
      figure.append(image, caption);
      strip.append(figure);
    }
    let more = null;
    if (data.total > data.pages.length) {
      more = document.createElement("span");
      more.className = "oobe-preview-more";
      more.textContent = `+${data.total - data.pages.length}`;
      strip.append(more);
    }
    bubble.append(strip);
    document.body.append(bubble);
    if (more) {
      const lastPage = strip.querySelector(".oobe-preview-page:last-of-type");
      if (lastPage) more.style.height = `${lastPage.getBoundingClientRect().height}px`;
    }
    previewElement = bubble;
    positionPreview(row, bubble);
    requestAnimationFrame(() => {
      if (bubble.isConnected) positionPreview(row, bubble);
    });
  } catch {
    // A moved/deleted/corrupt file simply has no preview; the row remains usable.
  }
}

function onPreviewOver(event) {
  const row = event.target instanceof Element ? event.target.closest(".file-row") : null;
  if (!row || row === previewRow) return;
  clearPreview();
  previewRow = row;
  const token = previewToken;
  previewTimer = window.setTimeout(() => {
    previewTimer = 0;
    if (token === previewToken) showPreview(row);
  }, PREVIEW_DELAY);
}

function onPreviewOut(event) {
  const row = event.target instanceof Element ? event.target.closest(".file-row") : null;
  if (!row || row !== previewRow) return;
  if (event.relatedTarget instanceof Node && row.contains(event.relatedTarget)) return;
  clearPreview();
}

function initOobePreview() {
  document.addEventListener("pointerover", onPreviewOver, true);
  document.addEventListener("pointerout", onPreviewOut, true);
  document.addEventListener("scroll", clearPreview, true);
  document.addEventListener("click", clearPreview, true);
  window.addEventListener("resize", clearPreview);
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") clearPreview();
  });
}

function panelElement() {
  return document.querySelector(".panel");
}
// ── Finder drops ─────────────────────────────────────────────────────────────────────────────
//
// Files dropped on the reader open as PDFs; files dropped on the AI panel go through the same
// attachment input as the paperclip button. Tauri's native drag-drop interception is disabled so the
// webview receives real File objects.

function hasFileDrop(event) {
  const types = event.dataTransfer?.types;
  return types ? Array.from(types).includes("Files") : false;
}

function dropNotice(message) {
  document.querySelector(".app-drop-notice")?.remove();
  const notice = document.createElement("div");
  notice.className = "app-drop-notice";
  notice.textContent = message;
  document.body.append(notice);
  window.setTimeout(() => notice.remove(), 3200);
}

function isPdfFile(file) {
  return file.name.toLowerCase().endsWith(".pdf") || file.type === "application/pdf";
}

function openDroppedPdf(file) {
  const input = document.getElementById("file-input");
  if (!input) return false;
  const transfer = new DataTransfer();
  transfer.items.add(file);
  input.value = "";
  input.files = transfer.files;
  input.dispatchEvent(new Event("change", { bubbles: true }));
  return true;
}

let dropCover = null;

const DROP_ICON = `<svg viewBox="0 0 1024 1024" fill="currentColor" aria-hidden="true">
  <path d="M895.5 831.7H128c-35.1 0-63.8 28.7-63.8 63.8 0 35.1 28.7 63.8 63.8 63.8h767.5c35.1 0 63.8-28.7 63.8-63.8 0-35.1-28.7-63.8-63.8-63.8zM213.2 448.2H352v259.2c0 33.3 27 60.4 60.4 60.4h199.4c33.3 0 60.4-27 60.4-60.4V448.2H811c26.7 0 40.7-31.8 22.5-51.5L534.6 73.2c-12.1-13.1-32.9-13.1-45.1 0L190.7 396.7c-18.1 19.7-4.2 51.5 22.5 51.5z"></path>
</svg>`;

function ensureDropCover() {
  if (dropCover?.isConnected) return dropCover;
  const cover = document.createElement("div");
  cover.className = "region-drop-cover";
  cover.innerHTML = `${DROP_ICON}<span></span>`;
  document.body.append(cover);
  dropCover = cover;
  return cover;
}

function updateDropCover(target) {
  const panel = target?.closest?.(".panel") ?? null;
  const stage = target?.closest?.(".stage") ?? null;
  const zone = panel ? "panel" : stage ? "stage" : null;
  if (!zone) {
    hideDropCover();
    return;
  }
  const region = panel ?? stage;
  const rect = region.getBoundingClientRect();
  const cover = ensureDropCover();
  cover.dataset.zone = zone;
  cover.style.left = `${rect.left}px`;
  cover.style.top = `${rect.top}px`;
  cover.style.width = `${rect.width}px`;
  cover.style.height = `${rect.height}px`;
  cover.querySelector("span").textContent = zone === "panel" ? "上传文件" : "打开文件";
}

function hideDropCover() {
  dropCover?.remove();
  dropCover = null;
}

function initFileDrops() {
  document.addEventListener("dragover", (event) => {
    if (!hasFileDrop(event)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
    const target = event.target instanceof Element ? event.target : null;
    updateDropCover(target);
  });

  document.addEventListener("dragleave", (event) => {
    if (event.relatedTarget === null) hideDropCover();
  });

  document.addEventListener("drop", (event) => {
    if (!hasFileDrop(event)) return;
    event.preventDefault();
    hideDropCover();
    const files = [...(event.dataTransfer?.files ?? [])];
    if (files.length === 0) return;
    const target = event.target instanceof Element ? event.target : null;
    if (target?.closest(".panel")) {
      window.Composer?.attachFiles?.(files);
      return;
    }
    if (target?.closest(".stage")) {
      const pdf = files.find(isPdfFile);
      if (!pdf) {
        dropNotice("阅读器只支持打开 PDF；其他文件请拖到右侧 AI 面板");
        return;
      }
      openDroppedPdf(pdf);
      return;
    }
    dropNotice("把 PDF 拖到左侧阅读器，或把附件拖到右侧 AI 面板");
  });

  document.addEventListener("dragend", () => hideDropCover());
}



let panelDrag = null;
let panelDragFrame = 0;

/// Starts a smooth divider drag: the panel width is written directly once per frame and the stage
/// keeps the same content point in its centre, without a Leptos re-render per pointer event.
function beginPanelResize() {
  const panel = panelElement();
  const stage = stageElement();
  if (!panel || !stage || panelDrag) return;
  // A pending window-resize check must not re-anchor the page in the middle of this drag.
  clearTimeout(resizeTimer);
  bindPanelScroll();
  const body = panelBodyElement();
  if (body && panelDistanceFromEnd(body) <= PANEL_FOLLOW_SLOP) {
    panelFollowing = true;
  }
  panelDrag = {
    panel,
    stage,
    anchorX: stage.scrollLeft + stage.clientWidth / 2,
    anchorY: stage.scrollTop + stage.clientHeight / 2,
    // A fixed PDF anchor for the whole drag. Recomputing the on-screen point every frame lets
    // sub-pixel scroll corrections be rounded away, so a slow 1px/frame drag accumulates drift.
    pdfAnchor: readAnchor(),
    clientX: 0,
    width: panel.getBoundingClientRect().width,
    baseScale: displayScale,
    baseWidth,
  };
  document.body.classList.add("panel-dragging");
  document.documentElement.classList.add("resizing");
  window.addEventListener("pointermove", onPanelResizeMove, true);
  window.addEventListener("pointerup", finishPanelResize, true);
  window.addEventListener("blur", finishPanelResize, true);
}

function onPanelResizeMove(event) {
  if (!panelDrag) return;
  event.preventDefault();
  panelDrag.clientX = event.clientX;
  if (panelDragFrame) return;
  panelDragFrame = requestAnimationFrame(applyPanelResize);
}

/// Re-applies the drag's fixed base-layout anchor to the reading line. The correction is absolute
/// to that anchor, not to the current (possibly already rounded) scroll position, so error cannot
/// accumulate across slow frames.
function restorePdfDragAnchor(anchor) {
  if (!anchor || !panelDrag) return;
  const stage = panelDrag.stage;
  const page = pageElement(anchor.page);
  if (!stage || !page) return;
  const stageRect = stage.getBoundingClientRect();
  const pageRect = page.getBoundingClientRect();
  const target = stageRect.top + readingOffset();
  const current = pageRect.top + anchor.fraction * pageRect.height;
  stage.scrollTop += current - target;
}

function applyPanelResize() {
  panelDragFrame = 0;
  if (!panelDrag) return;
  const width = Math.min(720, Math.max(320, window.innerWidth - panelDrag.clientX));
  const panelAnchor = capturePanelAnchor();
  panelDrag.width = width;
  panelDrag.panel.style.width = `${width}px`;
  if (panelFollowing) {
    keepPanelAtEnd();
  } else {
    restorePanelAnchor(panelAnchor);
  }

  // Reflow the stage continuously: keep the 100% layout, but visually scale the column so the page
  // width follows the divider instead of snapping only when the drag ends.
  const stage = panelDrag.stage;
  if (panelDrag.baseWidth > 0) {
    const content = stageContentWidth(stage);
    const scale = Math.min(
      MAX_SCALE,
      Math.max(MIN_SCALE, (panelDrag.baseScale * content) / panelDrag.baseWidth),
    );
    const stageBox = stage.getBoundingClientRect();
    applyDisplayScale(scale, {
      x: stageBox.left + stage.clientWidth / 2,
      y: stageBox.top + readingOffset(),
    });
    restorePdfDragAnchor(panelDrag.pdfAnchor);
  } else {
    stage.scrollLeft = Math.max(0, panelDrag.anchorX - stage.clientWidth / 2);
  }
}

function finishPanelResize(event) {
  if (!panelDrag || panelDrag.releasing) return 0;
  panelDrag.releasing = true;

  // Stop responding to further pointer moves, but keep the drag styling until the final frame has
  // been painted. Take the pointer-up coordinate so the divider lands where the pointer was.
  window.removeEventListener("pointermove", onPanelResizeMove, true);
  if (event && Number.isFinite(event.clientX)) {
    panelDrag.clientX = event.clientX;
  }

  if (!panelDragFrame) {
    panelDragFrame = requestAnimationFrame(applyPanelResize);
  }

  // Let the pending drag frame apply the final width and scale and reach the screen, then commit
  // the base-layout reflow on the following frame. Both frames now have the same visual size; the
  // old code committed in the same frame as the pending pointer move, which flashed.
  requestAnimationFrame(() => {
    requestAnimationFrame(commitPanelResize);
  });
  return 0;
}

function commitPanelResize() {
  if (!panelDrag) return 0;
  if (panelDragFrame) {
    cancelAnimationFrame(panelDragFrame);
    panelDragFrame = 0;
  }
  // Use the last width that was actually applied and painted. Applying a still-pending pointer
  // position here would make the divider (and the PDF) take one unpainted step on pointer-up.

  // Capture the reading-line anchor while the drag scale is still applied. Reflowing the base
  // layout first and resetting the visual scale second would make the two frames disagree and
  // produce the old release jump.
  const anchor = panelDrag.pdfAnchor ?? readAnchor();
  const panelAnchor = capturePanelAnchor();
  const width = Math.round(panelDrag.width);
  const finalScale = panelDrag.baseScale;
  panelDrag.panel.style.width = `${width}px`;
  if (panelFollowing) {
    keepPanelAtEnd();
  } else {
    restorePanelAnchor(panelAnchor);
  }

  window.removeEventListener("pointermove", onPanelResizeMove, true);
  window.removeEventListener("pointerup", finishPanelResize, true);
  window.removeEventListener("blur", finishPanelResize, true);
  document.body.classList.remove("panel-dragging");
  document.documentElement.classList.remove("resizing");
  panelDrag = null;

  // Reflow the fit-width base layout to the final stage width, then put the same PDF point back
  // on the reading line at the restored zoom. The order matters: the anchor is in base-layout
  // coordinates, so it survives both the reflow and the scale reset.
  const reflowed = measureBaseLayout(false);
  setDisplayScale(finalScale);
  targetScale = finalScale;
  updateScalerSize();
  scrollToAnchor(anchor);
  centerAtRest();
  reportZoom(true);

  if (reflowed) {
    updateWindow(true).catch(() => {});
  } else {
    scheduleUpdate(false);
  }
  window.__pdfscopePanelResized?.(width);
  return width;
}

window.PDFScope = {
  /** @param {Uint8Array} bytes */
  async load(bytes, name = "") {
    documentName = typeof name === "string" ? name : "";
    documentEpoch += 1;
    referencePreviewCache.clear();
    if (pageRuler) {
      hidePageRulerHover();
      pageRuler.sectionToken += 1;
      pageRuler.sections = [];
      pageRuler.sectionsDocument = null;
      pageRuler.marks = [];
      pageRuler.element.hidden = true;
    }
    stopZoom();
    releaseAll();
    baseWidth = 0;
    baseHeight = 0;
    pageGapRatio = 0;
    baseEpoch += 1;
    baseDirty = true;
    metrics = [];
    metricsDirty = true;
    initialized = false;
    currentPage = 1;
    displayScale = 1;
    targetScale = 1;
    lastReportedZoom = null;
    desired.clear();

    const pages = pagesElement();
    if (pages) {
      pages.style.setProperty("--view-scale", "1");
      pages.style.setProperty("--ui-scale", "1");
      pages.style.removeProperty("gap");
      pages.style.removeProperty("--page-gap");
      pages.classList.remove("zooming");
    }

    if (document_) {
      try {
        await document_.destroy();
      } catch {
        /* the old document is already gone */
      }
      document_ = null;
    }

    document_ = await pdfjs.getDocument({ data: bytes }).promise;
    pageSizes = await measurePages(document_.numPages);
    restorePage = rememberedPage();
    pendingDefaultZoom = defaultZoom;
    thumbnailCache.clear();
    thumbnailObserver?.disconnect();
    thumbnailObserver = null;
    reportZoom(true);
    return document_.numPages;
  },

  /// Loads a bundled document through the same path as a file picker.
  async openSample(url) {
    const response = await fetch(url);
    const blob = await response.blob();
    const file = new File([blob], "示例.pdf", { type: "application/pdf" });
    const transfer = new DataTransfer();
    transfer.items.add(file);
    const input = document.getElementById("file-input");
    if (!input) return false;
    input.files = transfer.files;
    input.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  },

  /// A small page image for a reference whose page is outside the reader's viewport.
  async previewPage(number, width = 180) {
    if (!document_) return "";
    const page = Math.max(1, Math.min(Number(number) || 1, document_.numPages));
    try {
      return await renderReferencePage(page, width);
    } catch {
      return "";
    }
  },

  /// `single` shows one page at a time; anything else keeps the continuous column.
  setPageMode(mode) {
    pageMode = ["single", "spread"].includes(mode) ? mode : "continuous";
    applyPageMode();
  },

  /// pointer | text | box | pages. Text and box tools stay active until pointer is chosen.
  setTool(name) {
    if (borrowedMode && name !== borrowedMode) borrowedMode = null;
    toolMode = ["text", "box", "pages"].includes(name) ? name : "pointer";
    if (stageElement()) {
      stageElement().dataset.tool = toolMode;
      delete stageElement().dataset.borrow;
    }
    clearPointerPage();
    if (toolMode === "pages" && lastPointer) {
      // Switching while the pointer already rests on a page must light it without a move.
      markPointerPage(lastPointer.x, lastPointer.y);
    }
    if (toolMode === "box") {
      armCapture();
    } else {
      disarmCapture();
    }
  },

  /// Shifts the spread pairing by one page: page 1 pairs with 2 instead of standing alone.
  setSpreadOffset(value) {
    spreadOffset = Boolean(value);
    if (pageMode === "spread") {
      markCurrentPage();
    }
  },

  /// The zoom a freshly opened document starts at, as a percentage of fit width.
  setDefaultZoom(percent) {
    const value = Number(percent);
    defaultZoom = Number.isFinite(value) && value >= 50 && value <= 400 ? value : 100;
  },

  /// Turns the remembered reading position on or off.
  setRememberPosition(value) {
    rememberPosition = Boolean(value);
  },

  /// Renders a thumbnail for every `.thumb[data-page]` that scrolls into view. Cached per page.
  thumbnails() {
    const targets = document.querySelectorAll(".thumb[data-page]:not([data-queued])");
    if (targets.length === 0) return;

    if (!thumbnailObserver) {
      thumbnailObserver = new IntersectionObserver((entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          void renderThumbnail(entry.target);
        }
      }, { rootMargin: "200px" });
    }

    for (const target of targets) {
      const number = Number(target.dataset.page);
      const size = pageSizes[number - 1];
      if (size?.width > 0 && size?.height > 0) {
        target.style.setProperty("--thumb-aspect", `${size.width} / ${size.height}`);
      }
      target.dataset.queued = "1";
      thumbnailObserver.observe(target);
    }
  },

  /// Arms FOLLOW-END after a send and binds transcript growth observers.
  panelSent() {
    armPanelFollow();
  },

  /// Recent documents live in IndexedDB so they can be reopened after a restart.
  recentsList() {
    return window.Recents ? window.Recents.list() : Promise.resolve([]);
  },

  recentsSave(name, bytes, path = "") {
    return window.Recents ? window.Recents.save(name, bytes, path) : Promise.resolve(false);
  },

  recentsLoad(id) {
    return window.Recents ? window.Recents.load(id) : Promise.resolve(null);
  },

  nativeOpenAvailable() {
    return Boolean(window.Bridge?.available);
  },

  pickPdf() {
    return window.Bridge?.pickPdf?.() ?? Promise.resolve(null);
  },

  readPdf(path) {
    return window.Bridge?.readPdf?.(path) ?? Promise.reject(new Error("桌面版不可用"));
  },

  scanRecommended(recentDirs) {
    return window.Bridge?.scanRecommended?.(recentDirs) ?? Promise.resolve([]);
  },

  /// The reader's preferred panel width, remembered between launches.
  loadPanelWidth() {
    try {
      const value = Number(window.localStorage.getItem("pdfscope.panelWidth"));
      return Number.isFinite(value) && value > 0 ? value : null;
    } catch {
      return null;
    }
  },

  savePanelWidth(width) {
    try {
      window.localStorage.setItem("pdfscope.panelWidth", String(Math.round(width)));
    } catch {
      // Preference lost, layout still works.
    }
  },

  beginPanelResize,
  finishPanelResize,
  panelWidth() {
    return Math.round(panelElement()?.getBoundingClientRect().width ?? 0);
  },

  /// Opens a citation in the system browser.
  openUrl(url) {
    window.open(url, "_blank", "noopener");
  },

  /// Reads the attachments chosen in a file input: images, documents and text.
  async readAttachments(inputId) {
    const input = document.getElementById(inputId);
    const files = [...(input?.files ?? [])];
    const out = [];
    if (input) input.value = "";

    for (const file of files) {
      try {
        const attachment = await readOneAttachment(file);
        if (attachment) out.push(attachment);
      } catch (error) {
        out.push({
          kind: "error",
          name: file.name,
          message: error instanceof Error ? error.message : "这个文件读不出来",
        });
      }
    }
    return out;
  },

  /// The conversation library of one document.  /// The conversation library of one document.
  loadConversations(key) {
    try {
      return window.localStorage.getItem(`pdfscope.conversations.${key}`);
    } catch {
      return null;
    }
  },

  saveConversations(key, json) {
    try {
      window.localStorage.setItem(`pdfscope.conversations.${key}`, json);
    } catch {
      // Quota or privacy mode: the conversation still lives in memory.
    }
  },

  /// The transcript of one document, persisted in the webview's local storage.
  loadTranscript(key) {
    try {
      return window.localStorage.getItem(`pdfscope.transcript.${key}`);
    } catch {
      return null;
    }
  },

  saveTranscript(key, json) {
    try {
      window.localStorage.setItem(`pdfscope.transcript.${key}`, json);
    } catch {
      // Storage may be full or disabled; losing persistence is better than losing the answer.
    }
  },

  clearTranscript(key) {
    try {
      window.localStorage.removeItem(`pdfscope.transcript.${key}`);
    } catch {
      // Nothing to clear.
    }
  },

  /// The text the reader has selected inside one element, if the selection starts there.
  selectedTextWithin(id) {
    const element = document.getElementById(id);
    const selection = window.getSelection();
    if (!element || !selection || selection.isCollapsed || !selection.anchorNode) return "";
    if (!element.contains(selection.anchorNode)) return "";
    return selection.toString().trim();
  },

  /// The document point under the centre of the stage, in the stage's content coordinates.
  captureStageAnchor() {
    const stage = stageElement();
    if (!stage) return null;
    return {
      x: stage.scrollLeft + stage.clientWidth / 2,
      y: stage.scrollTop + stage.clientHeight / 2,
    };
  },

  /// Puts that same content point back under the centre, after the stage has been resized.
  restoreStageAnchor(x, y) {
    const stage = stageElement();
    if (!stage || !Number.isFinite(x) || !Number.isFinite(y)) return;
    requestAnimationFrame(() => {
      stage.scrollLeft = Math.max(0, x - stage.clientWidth / 2);
      stage.scrollTop = Math.max(0, y - stage.clientHeight / 2);
    });
  },

  /// Scrolls a page thumbnail to the middle of its container, naturally clamped at the ends.
  centerThumb(containerId, pageNumber) {
    const container = document.getElementById(containerId);
    const target = container?.querySelector(`[data-page="${pageNumber}"]`);
    target?.scrollIntoView({ block: "center", inline: "nearest" });
  },

  /// Text covered by a normalised rectangle, read from the rendered text layer.
  async textInRect(pageNumber, rect) {
    const page = pageElement(pageNumber);
    if (!page || !rect) return { text: "", rects: [] };

    const layer = page.querySelector(".textLayer");
    const box = page.getBoundingClientRect();
    if (!layer || box.width === 0 || box.height === 0) return { text: "", rects: [] };

    const left = box.left + rect.x * box.width;
    const top = box.top + rect.y * box.height;
    const right = left + rect.width * box.width;
    const bottom = top + rect.height * box.height;

    // Character boundaries, not span boxes: a caret sits between two characters, so the range
    // between the two corners is exactly the text the rectangle covers.
    const start = caretInside(layer, left + 1, top + 2);
    const end = caretInside(layer, right - 1, bottom - 2);

    if (start && end) {
      const range = document.createRange();
      const ordered =
        start.compareBoundaryPoints(Range.START_TO_START, end) <= 0
          ? [start, end]
          : [end, start];
      range.setStart(ordered[0].startContainer, ordered[0].startOffset);
      range.setEnd(ordered[1].startContainer, ordered[1].startOffset);

      const text = range.toString().replace(/\s+/g, " ").trim();
      if (text) {
        const unit = (value) => Math.min(Math.max(value, 0), 1);
        const rects = [];
        for (const line of range.getClientRects()) {
          if (line.width === 0 || line.height === 0) continue;
          rects.push({
            x: unit((line.left - box.left) / box.width),
            y: unit((line.top - box.top) / box.height),
            width: unit(line.width / box.width),
            height: unit(line.height / box.height),
          });
        }
        return { text, rects };
      }
    }

    return { text: "", rects: [] };
  },

  /// Starts dragging one end of a passage. The renderer owns the gesture: it reads character offsets
  /// from the caret under the pointer, rebuilds the Range, and reports the final geometry on release.
  beginGripDrag(pageNumber, id, start, end, side, clientX, clientY) {
    const page = pageElement(pageNumber);
    const layer = page?.querySelector(".textLayer");
    if (!page || !layer || end <= start) return;

    const total = layer.textContent.length;
    const fixed = side === "start" ? end : start;
    let moving = side === "start" ? start : end;
    let began = false;

    const geometryFor = () => {
      const startOffset = Math.min(fixed, moving);
      const endOffset = Math.max(fixed, moving);
      const range = rangeBetween(layer, startOffset, endOffset);
      if (!range) return null;
      const geometry = rangeGeometry(page, range);
      if (!geometry) return null;
      return { ...geometry, start: startOffset, end: endOffset };
    };

    const report = () => {
      const geometry = geometryFor();
      if (!geometry) return;
      if (!began) {
        began = true;
        // One undo entry per gesture, taken before the first change is applied.
        window.__pdfscopePassageBegin?.(id);
      }
      window.__pdfscopePassageEdited?.({
        id,
        page: pageNumber,
        start: geometry.start,
        end: geometry.end,
        text: geometry.text,
        rect: geometry.rect,
        rects: geometry.rects,
      });
    };

    const move = (event) => {
      const caret = caretInside(layer, event.clientX, event.clientY);
      if (caret) {
        const offset = offsetAt(layer, caret.startContainer, caret.startOffset);
        if (offset !== null) moving = Math.max(0, Math.min(total, offset));
      }
      report();
      event.preventDefault();
    };

    const up = (event) => {
      document.removeEventListener("pointermove", move, true);
      document.removeEventListener("pointerup", up, true);
      document.removeEventListener("pointercancel", up, true);
      document.body.classList.remove("grip-dragging");
      report();
      event.preventDefault();
    };

    document.body.classList.add("grip-dragging");
    document.addEventListener("pointermove", move, true);
    document.addEventListener("pointerup", up, true);
    document.addEventListener("pointercancel", up, true);
  },

  /// Re-crops a region after its rectangle changed.
  async regionImage(pageNumber, rect) {
    return renderRegion(pageNumber, rect);
  },

  /// Counts matches for `query` across the document and highlights the rendered pages.
  async find(query) {
    const needle = String(query ?? "").trim();
    findQuery = needle;

    if (!needle || !document_) {
      applyFindToRendered();
      return { total: 0, pages: [] };
    }

    const lowered = needle.toLocaleLowerCase();
    const pages = [];
    let total = 0;

    for (let number = 1; number <= document_.numPages; number += 1) {
      const page = await document_.getPage(number);
      const content = await page.getTextContent();
      const text = content.items.map((item) => item.str ?? "").join(" ").toLocaleLowerCase();
      let from = 0;
      let hits = 0;
      for (;;) {
        const at = text.indexOf(lowered, from);
        if (at < 0) break;
        hits += 1;
        from = at + lowered.length;
      }
      if (hits > 0) {
        pages.push(number);
        total += hits;
      }
    }

    applyFindToRendered();
    return { total, pages };
  },

  /// Removes every find highlight.
  clearFind() {
    findQuery = "";
    applyFindToRendered();
  },

  /// The document's table of contents, flattened: `[{ title, page, depth }, …]`.
  async outline() {
    if (!document_) return [];

    const items = await document_.getOutline();
    const flat = [];

    const walk = async (nodes, depth) => {
      for (const node of nodes ?? []) {
        let page = 0;
        try {
          const destination =
            typeof node.dest === "string" ? await document_.getDestination(node.dest) : node.dest;
          const reference = destination?.[0];
          if (reference !== undefined) {
            // PDF.js exposes destinations as page indices, named strings, or page references
            // ({num, gen}). getPageIndex resolves all three; treating the object as a number
            // silently turned every outline entry into page 0.
            const index =
              typeof reference === "number"
                ? reference
                : await document_.getPageIndex(reference);
            page = Number(index) + 1;
          }
        } catch {
          /* a broken destination simply has no page */
        }

        flat.push({ title: String(node.title ?? ""), page, depth });
        await walk(node.items, depth + 1);
      }
    };

    await walk(items, 0);
    return flat;
  },

  /// Base sizes for Rust's page shells: `[{ width, height }, …]` at scale 1.
  pageSizes() {
    return pageSizes.map(({ width, height }) => ({ width, height }));
  },

  async render(pageNumber) {
    if (pendingDefaultZoom > 0) {
      setZoomTarget(pendingDefaultZoom / 100, { direct: true });
      pendingDefaultZoom = 0;
    }
    if (!document_) return;
    const target = clampedPage(restorePage > 0 ? restorePage : pageNumber);
    restorePage = 0;

    const shells = await waitForShells();
    if (!shells) return;

    bindStage();
    if (baseDirty || baseWidth <= 0) {
      baseDirty = false;
      measureBaseLayout(true);
      setDisplayScale(1);
      updateScalerSize();
    }

    if (target !== currentPage || !initialized) {
      scrollToPageNow(target);
      initialized = true;
      reportZoom(true);
      centerAtRest();
      await updateWindow(true);
      refreshPageRuler();
      return;
    }

    scheduleUpdate(false);
    refreshPageRuler();
  },

  async pageText(pageNumber) {
    if (!document_) return "";
    const page = await document_.getPage(clampedPage(pageNumber));
    const content = await page.getTextContent();
    return content.items.map((item) => item.str).join(" ");
  },

  flash(pageNumber) {
    if (!document_) return;
    const target = clampedPage(pageNumber ?? currentPage);
    if (target !== currentPage || !pageVisible(target)) scrollToPageNow(target);

    let attempts = 0;
    const apply = () => {
      const page = pageElement(target);
      if (!page || page.clientWidth === 0) {
        attempts += 1;
        if (attempts < 30) requestAnimationFrame(apply);
        return;
      }
      page.classList.remove("flash");
      void page.offsetWidth;
      page.classList.add("flash");
      setTimeout(() => page.classList.remove("flash"), 800);
    };
    requestAnimationFrame(apply);
    scheduleUpdate(false);
  },

  zoomTo(percent, options = {}) {
    if (pageMode !== "continuous") return;
    requestZoomTo(percent, options);
  },

  zoomBy(steps, options = {}) {
    requestZoomBy(steps, options);
  },

  fitWidth() {
    if (pageMode !== "continuous") return;
    requestFitWidth();
  },

  beginCapture() {
    armCapture();
  },

  disarmCapture() {
    disarmCapture();
  },

  clearSelection() {
    hadSelection = false;
    // The click that adds a reference can finish after this call and make the browser paint the old
    // native selection again. Clear now, on the next frame, and once more just after the click's
    // default action so no second (flatter) highlight can survive under the passage mark.
    const clear = () => {
      window.getSelection()?.removeAllRanges();
      document.getSelection()?.removeAllRanges();
    };
    clear();
    requestAnimationFrame(clear);
    setTimeout(clear, 60);
    // Even if the browser paints the old selection again, it cannot show as a second layer. The
    // quiet window ends on the next press, or after a second and a half at the latest.
    document.body.classList.add("selection-quiet");
    const release = () => {
      document.body.classList.remove("selection-quiet");
      document.removeEventListener("pointerdown", release, true);
    };
    document.addEventListener("pointerdown", release, true);
    setTimeout(release, 1500);
    window.__pdfscopeCleared?.();
  },

  async copy(text) {
    await navigator.clipboard.writeText(text);
  },
};

function stopZoom() {
  if (zoomFrame) cancelAnimationFrame(zoomFrame);
  zoomFrame = 0;
  clearSettle();
  zooming = false;
  gesture = null;
  zoomOrigin = null;
  pendingPanX = 0;
  pendingPanY = 0;
  pagesElement()?.classList.remove("zooming");
}

// ── Page ruler ────────────────────────────────────────────────────────────────────────────────
//
// A custom vertical scrollbar that shows page boundaries, top-level outline chapters, the current
// page and the viewport window. Page geometry is read from the already-rendered page elements; ticks
// are drawn on one canvas, so scrolling only updates the thumb and the current-page line.
let pageRuler = null;

function initPageRuler() {
  if (pageRuler) return;
  const shell = document.querySelector(".stage-shell");
  if (!shell) {
    setTimeout(initPageRuler, 50);
    return;
  }

  const element = document.createElement("div");
  element.className = "page-ruler";
  element.tabIndex = 0;
  element.setAttribute("role", "scrollbar");
  element.setAttribute("aria-orientation", "vertical");
  element.setAttribute("aria-label", "页面标尺");
  element.setAttribute("aria-valuemin", "1");
  element.setAttribute("aria-controls", "pages");
  element.innerHTML = `
    <canvas aria-hidden="true"></canvas>
    <div class="page-ruler-thumb" hidden></div>
    <div class="page-ruler-hover-line" hidden></div>
    <div class="page-ruler-tooltip" hidden>
      <div class="page-ruler-tooltip-page"></div>
      <div class="page-ruler-tooltip-section"></div>
    </div>
    <div class="page-ruler-preview" hidden><img alt="" draggable="false"></div>
  `;
  shell.append(element);

  pageRuler = {
    element,
    canvas: element.querySelector("canvas"),
    thumb: element.querySelector(".page-ruler-thumb"),
    hoverLine: element.querySelector(".page-ruler-hover-line"),
    tooltip: element.querySelector(".page-ruler-tooltip"),
    tooltipPage: element.querySelector(".page-ruler-tooltip-page"),
    tooltipSection: element.querySelector(".page-ruler-tooltip-section"),
    preview: element.querySelector(".page-ruler-preview"),
    previewImage: element.querySelector(".page-ruler-preview img"),
    marks: [],
    sections: [],
    sectionsDocument: null,
    sectionToken: 0,
    trackHeight: 0,
    total: 1,
    hoverPage: 0,
    previewTimer: 0,
    previewToken: 0,
    drag: null,
    stage: null,
    measureFrame: 0,
    viewportFrame: 0,
  };
  pageRuler.ctx = pageRuler.canvas.getContext("2d");
  document.documentElement.classList.add("ruler-ready");

  element.addEventListener("pointerdown", onPageRulerPointerDown);
  element.addEventListener("pointermove", onPageRulerPointerMove);
  element.addEventListener("pointerup", onPageRulerPointerUp);
  element.addEventListener("pointercancel", onPageRulerPointerUp);
  element.addEventListener("pointerleave", () => {
    if (!pageRuler?.drag) hidePageRulerHover();
  });
  element.addEventListener("keydown", onPageRulerKeyDown);
  element.addEventListener(
    "wheel",
    (event) => {
      if (!pageRuler?.stage) return;
      event.preventDefault();
      pageRuler.stage.scrollTop += event.deltaY;
    },
    { passive: false },
  );

  if (typeof ResizeObserver === "function") {
    const observer = new ResizeObserver(() => schedulePageRulerMeasure());
    observer.observe(shell);
    const stage = stageElement();
    const pages = pagesElement();
    if (stage) observer.observe(stage);
    if (pages) observer.observe(pages);
    pageRuler.observer = observer;
  }
  window.addEventListener("resize", schedulePageRulerMeasure);

  bindPageRulerStage();
  refreshPageRuler();
}

function bindPageRulerStage() {
  const stage = stageElement();
  if (!pageRuler || !stage || pageRuler.stage === stage) return;
  pageRuler.stage = stage;
  stage.addEventListener("scroll", schedulePageRulerViewport, { passive: true });
}

function refreshPageRuler() {
  if (!pageRuler) return;
  bindPageRulerStage();
  schedulePageRulerMeasure();
  if (document_ && pageRuler.sectionsDocument !== document_) {
    void loadPageRulerSections();
  }
}

async function loadPageRulerSections() {
  if (!pageRuler || !document_ || !window.PDFScope?.outline) return;
  const documentAtRequest = document_;
  const token = ++pageRuler.sectionToken;
  let sections = [];
  try {
    sections = (await window.PDFScope.outline()) ?? [];
  } catch {
    sections = [];
  }
  if (
    !pageRuler ||
    document_ !== documentAtRequest ||
    token !== pageRuler.sectionToken
  ) {
    return;
  }
  pageRuler.sections = Array.isArray(sections) ? sections : [];
  pageRuler.sectionsDocument = documentAtRequest;
  drawPageRuler();
}

function schedulePageRulerMeasure() {
  if (!pageRuler || pageRuler.measureFrame) return;
  pageRuler.measureFrame = requestAnimationFrame(() => {
    pageRuler.measureFrame = 0;
    measurePageRuler();
  });
}

function measurePageRuler() {
  const ruler = pageRuler;
  if (!ruler) return;
  bindPageRulerStage();
  const stage = stageElement();
  const pages = pagesElement();
  const pageElements = pages ? [...pages.querySelectorAll(".page[data-page]")] : [];
  if (!stage || !pages || pageElements.length === 0 || !document_) {
    ruler.element.hidden = true;
    return;
  }

  ruler.element.hidden = false;
  ruler.stage = stage;
  ruler.trackHeight = Math.max(1, stage.clientHeight);
  ruler.element.style.height = `${ruler.trackHeight}px`;

  const dpr = Math.min(2, window.devicePixelRatio || 1);
  ruler.canvas.width = Math.max(1, Math.round(16 * dpr));
  ruler.canvas.height = Math.max(1, Math.round(ruler.trackHeight * dpr));
  ruler.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  const stageRect = stage.getBoundingClientRect();
  const pagesRect = pages.getBoundingClientRect();
  const pagesTop = pagesRect.top - stageRect.top + stage.scrollTop;
  ruler.total = Math.max(1, stage.scrollHeight);
  const maxScroll = Math.max(0, ruler.total - stage.clientHeight);
  ruler.marks = pageElements
    .map((page) => {
      const number = Number(page.dataset.page);
      const rect = page.getBoundingClientRect();
      const top = pagesTop + (rect.top - pagesRect.top);
      // The page tick is placed where the viewport must be positioned to centre this page in the
      // reader. That makes the tick line up with the middle of the scrollbar thumb, not its top.
      const centerScroll = clamp(
        top + rect.height / 2 - stage.clientHeight / 2,
        0,
        maxScroll,
      );
      return {
        number,
        top,
        height: rect.height,
        centerScroll,
        centerFraction: clamp(
          (centerScroll + stage.clientHeight / 2) / ruler.total,
          0,
          1,
        ),
      };
    })
    .filter((mark) => Number.isFinite(mark.number) && mark.number > 0)
    .sort((a, b) => a.number - b.number);
  ruler.element.setAttribute("aria-valuemax", String(document_.numPages || ruler.marks.length));
  drawPageRuler();
  updatePageRulerViewport();
}

function schedulePageRulerViewport() {
  if (!pageRuler || pageRuler.viewportFrame) return;
  pageRuler.viewportFrame = requestAnimationFrame(() => {
    pageRuler.viewportFrame = 0;
    updatePageRulerViewport();
  });
}

function updatePageRulerViewport() {
  const ruler = pageRuler;
  if (!ruler || !ruler.stage) return;
  const stage = ruler.stage;
  const track = ruler.trackHeight || stage.clientHeight;
  const scrollHeight = Math.max(1, stage.scrollHeight);
  const maxScroll = Math.max(0, scrollHeight - stage.clientHeight);
  const thumbHeight = Math.min(track, Math.max(22, (stage.clientHeight / scrollHeight) * track));
  // The thumb's centre follows the viewport centre, matching the page-centre ticks.
  const thumbCenter =
    ((stage.scrollTop + stage.clientHeight / 2) / scrollHeight) * track;
  const thumbTop = clamp(thumbCenter - thumbHeight / 2, 0, Math.max(0, track - thumbHeight));
  ruler.thumb.style.height = `${thumbHeight}px`;
  ruler.thumb.style.top = `${thumbTop}px`;
  ruler.thumb.hidden = maxScroll <= 1;
  ruler.element.setAttribute("aria-valuenow", String(currentPage));
  ruler.element.setAttribute("aria-valuetext", `第 ${currentPage} 页`);
}

function rulerSectionFor(page) {
  if (!pageRuler) return null;
  let best = null;
  for (const section of pageRuler.sections) {
    if (!section || !(section.page > 0) || section.page > page) continue;
    if (!best || section.page >= best.page) best = section;
  }
  return best;
}

function pageMarkAtY(y) {
  const ruler = pageRuler;
  if (!ruler || ruler.marks.length === 0) return null;
  let best = ruler.marks[0];
  for (const mark of ruler.marks) {
    if (mark.centerFraction * ruler.trackHeight <= y) best = mark;
    else break;
  }
  return best;
}

function pageRulerColors() {
  const style = getComputedStyle(pageRuler.element);
  return {
    border: style.getPropertyValue("--border-strong").trim() || "#cbb8ec",
    faint: style.getPropertyValue("--ink-faint").trim() || "#8c7aa2",
  };
}

function pageMajorStep(count) {
  const target = 16;
  const raw = count / target;
  const power = 10 ** Math.floor(Math.log10(Math.max(1, raw)));
  const normalized = raw / power;
  const nice = normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 5 ? 5 : 10;
  return Math.max(1, Math.round(nice * power));
}

function drawPageRuler() {
  const ruler = pageRuler;
  if (!ruler?.ctx || ruler.marks.length === 0) return;
  const height = ruler.trackHeight;
  if (height <= 0) return;
  const colors = pageRulerColors();
  const context = ruler.ctx;
  context.clearRect(0, 0, 16, height);

  // One bookmark per page; the shallowest entry wins and determines the bar length.
  const bookmarks = new Map();
  for (const section of ruler.sections) {
    if (!section || !(section.page > 0)) continue;
    const current = bookmarks.get(section.page);
    if (!current || section.depth < current.depth) {
      bookmarks.set(section.page, {
        depth: Number(section.depth) || 0,
        title: String(section.title ?? ""),
      });
    }
  }
  const hasBookmarks = bookmarks.size > 0;
  const depths = [...bookmarks.values()].map((bookmark) => bookmark.depth);
  const depthMin = hasBookmarks ? Math.min(...depths) : 0;
  const depthMax = hasBookmarks ? Math.max(...depths) : 0;
  const depthSpan = Math.max(1, depthMax - depthMin);
  const majorStep = pageMajorStep(ruler.marks.length);
  const maxTicks = Math.max(80, Math.min(500, Math.floor(height / 2)));
  const sampleStep = Math.max(1, Math.ceil(ruler.marks.length / maxTicks));
  const longest = 16;
  const shortest = 8;

  context.lineWidth = 1;
  context.strokeStyle = colors.faint;
  context.fillStyle = colors.faint;
  for (let index = 0; index < ruler.marks.length; index += 1) {
    const mark = ruler.marks[index];
    const bookmark = bookmarks.get(mark.number);
    const major = !hasBookmarks && (mark.number === 1 || mark.number % majorStep === 0);
    if (!bookmark && !major && index % sampleStep !== 0) continue;
    const y = Math.min(height - 1, Math.max(0, mark.centerFraction * height));
    if (bookmark) {
      const ratio = Math.min(1, Math.max(0, (bookmark.depth - depthMin) / depthSpan));
      const length = Math.round(longest - ratio * (longest - shortest));
      context.globalAlpha = 0.62;
      context.fillRect(16 - length, y - 1, length, 2);
    } else {
      const length = major ? longest : shortest;
      context.globalAlpha = major ? 0.58 : 0.34;
      context.beginPath();
      context.moveTo(16 - length, y + 0.5);
      context.lineTo(16, y + 0.5);
      context.stroke();
    }
  }
  context.globalAlpha = 1;
}

function hidePageRulerHover() {
  if (!pageRuler) return;
  pageRuler.hoverPage = 0;
  pageRuler.previewToken += 1;
  if (pageRuler.previewTimer) {
    clearTimeout(pageRuler.previewTimer);
    pageRuler.previewTimer = 0;
  }
  pageRuler.hoverLine.hidden = true;
  pageRuler.tooltip.hidden = true;
  pageRuler.preview.hidden = true;
}

function updatePageRulerHover(event) {
  const ruler = pageRuler;
  if (!ruler || ruler.drag || ruler.marks.length === 0) return;
  const rect = ruler.element.getBoundingClientRect();
  const y = clamp(event.clientY - rect.top, 0, ruler.trackHeight);
  const mark = pageMarkAtY(y);
  if (!mark) {
    hidePageRulerHover();
    return;
  }
  const tickY = Math.min(
    ruler.trackHeight - 1,
    Math.max(0, mark.centerFraction * ruler.trackHeight),
  );
  ruler.hoverLine.style.top = `${tickY}px`;
  ruler.hoverLine.hidden = false;

  if (mark.number === ruler.hoverPage && !ruler.tooltip.hidden) {
    ruler.tooltip.style.top = `${Math.max(
      4,
      Math.min(y + 14, ruler.trackHeight - ruler.tooltip.offsetHeight - 4),
    )}px`;
    return;
  }

  ruler.hoverPage = mark.number;
  ruler.previewToken += 1;
  if (ruler.previewTimer) clearTimeout(ruler.previewTimer);
  ruler.preview.hidden = true;

  const section = rulerSectionFor(mark.number);
  ruler.tooltipPage.textContent = `第 ${mark.number} 页`;
  ruler.tooltipSection.textContent = section?.title ?? "";
  ruler.tooltipSection.hidden = !section?.title;
  ruler.tooltip.hidden = false;
  ruler.tooltip.style.top = `${Math.max(
    4,
    Math.min(y + 14, ruler.trackHeight - ruler.tooltip.offsetHeight - 4),
  )}px`;

  const token = ruler.previewToken;
  ruler.previewTimer = setTimeout(async () => {
    ruler.previewTimer = 0;
    const url = await window.PDFScope?.previewPage?.(mark.number, 140);
    if (!url || token !== ruler.previewToken || ruler.hoverPage !== mark.number) return;
    ruler.previewImage.src = url;
    ruler.previewImage.alt = `第 ${mark.number} 页`;
    ruler.preview.hidden = false;
    const placePreview = () => {
      if (
        token !== ruler.previewToken ||
        ruler.hoverPage !== mark.number ||
        ruler.preview.hidden
      ) {
        return;
      }
      const previewHeight = ruler.preview.offsetHeight;
      let previewTop = y - previewHeight - 14;
      if (previewTop < 4) {
        previewTop = y + 14 + ruler.tooltip.offsetHeight + 8;
      }
      ruler.preview.style.top = `${Math.max(
        4,
        Math.min(previewTop, ruler.trackHeight - previewHeight - 4),
      )}px`;
    };
    if (ruler.previewImage.decode) {
      ruler.previewImage.decode().then(placePreview).catch(placePreview);
    }
    requestAnimationFrame(placePreview);
  }, 160);
}

function rulerScrollToPage(mark) {
  const stage = pageRuler?.stage;
  if (!stage || !mark) return;
  stage.scrollTo({ top: Math.max(0, mark.centerScroll), behavior: "auto" });
}

function onPageRulerPointerDown(event) {
  const ruler = pageRuler;
  if (!ruler) return;
  event.preventDefault();
  ruler.element.focus({ preventScroll: true });
  hidePageRulerHover();
  if (ruler.thumb.contains(event.target)) {
    const stage = ruler.stage;
    if (!stage || ruler.thumb.hidden) return;
    ruler.drag = { pointerId: event.pointerId };
    ruler.thumb.setPointerCapture(event.pointerId);
    return;
  }
  const rect = ruler.element.getBoundingClientRect();
  const mark = pageMarkAtY(clamp(event.clientY - rect.top, 0, ruler.trackHeight));
  rulerScrollToPage(mark);
}

function onPageRulerPointerMove(event) {
  const ruler = pageRuler;
  if (!ruler) return;
  if (ruler.drag) {
    const stage = ruler.stage;
    if (!stage) return;
    const rect = ruler.element.getBoundingClientRect();
    const y = clamp(event.clientY - rect.top, 0, ruler.trackHeight);
    const contentCenter = (y / ruler.trackHeight) * stage.scrollHeight;
    const maxScroll = Math.max(0, stage.scrollHeight - stage.clientHeight);
    stage.scrollTop = clamp(contentCenter - stage.clientHeight / 2, 0, maxScroll);
    return;
  }
  updatePageRulerHover(event);
}

function onPageRulerPointerUp(event) {
  const ruler = pageRuler;
  if (!ruler?.drag) return;
  if (ruler.thumb.hasPointerCapture(event.pointerId)) {
    ruler.thumb.releasePointerCapture(event.pointerId);
  }
  ruler.drag = null;
}

function onPageRulerKeyDown(event) {
  const ruler = pageRuler;
  const stage = ruler?.stage;
  if (!stage) return;
  const viewport = stage.clientHeight;
  let top = null;
  switch (event.key) {
    case "ArrowDown":
      top = stage.scrollTop + 64;
      break;
    case "ArrowUp":
      top = stage.scrollTop - 64;
      break;
    case "PageDown":
      top = stage.scrollTop + viewport * 0.9;
      break;
    case "PageUp":
      top = stage.scrollTop - viewport * 0.9;
      break;
    case "Home":
      top = 0;
      break;
    case "End":
      top = stage.scrollHeight;
      break;
    default:
      return;
  }
  event.preventDefault();
  stage.scrollTo({ top: Math.max(0, top), behavior: "auto" });
}

initOobePreview();

initFileDrops();

initPageRuler();

// ── Offscreen reference thumbnails ─────────────────────────────────────────────────────────────
//
// Hovering a page-bearing reference highlights the PDF through Rust. If that page is outside the
// reader viewport, no scroll happens (by design), so show a small page image next to the popover.

let refPreviewElement = null;
let refPreviewTimer = 0;
let refPreviewToken = 0;

function hideReferencePreview() {
  if (refPreviewTimer) {
    clearTimeout(refPreviewTimer);
    refPreviewTimer = 0;
  }
  refPreviewToken += 1;
  refPreviewElement?.remove();
  refPreviewElement = null;
}

function referencePageVisible(number) {
  const page = document.querySelector(`.page[data-page="${number}"]`);
  const stage = document.querySelector(".stage");
  if (!page || !stage) return false;
  const pageBox = page.getBoundingClientRect();
  const stageBox = stage.getBoundingClientRect();
  return pageBox.bottom > stageBox.top && pageBox.top < stageBox.bottom;
}

async function showReferencePreview(row, number) {
  const token = refPreviewToken;
  const url = await window.PDFScope?.previewPage?.(number, 180);
  if (!url || token !== refPreviewToken || !row.matches(":hover")) return;
  const box = document.createElement("div");
  box.className = "ref-page-preview";
  box.title = `第 ${number} 页`;
  const image = document.createElement("img");
  image.src = url;
  image.alt = `第 ${number} 页`;
  box.append(image);
  document.body.append(box);
  refPreviewElement = box;
  const target = row.getBoundingClientRect();
  const left = target.left - box.offsetWidth - 10;
  box.style.left = `${Math.max(8, left)}px`;
  box.style.top = `${Math.max(8, Math.min(target.top, window.innerHeight - box.offsetHeight - 8))}px`;
  if (left < 8) box.style.left = `${Math.min(window.innerWidth - box.offsetWidth - 8, target.right + 10)}px`;
}

let referencePlacementFrame = 0;

function placeReferencePopovers() {
  if (referencePlacementFrame) cancelAnimationFrame(referencePlacementFrame);
  referencePlacementFrame = requestAnimationFrame(() => {
    referencePlacementFrame = 0;
    const panel = document.querySelector(".panel");
    if (!panel) return;
    const panelBox = panel.getBoundingClientRect();
    for (const popover of document.querySelectorAll(".question-refs-popover")) {
      const badge = popover.closest(".question-refs")?.querySelector(".question-refs-badge");
      if (!badge) continue;
      const capsule = badge.getBoundingClientRect();
      // Start at the capsule's left edge and extend right; only flip left when the panel edge is
      // reached. The width is capped so a narrow panel simply truncates the row labels.
      const width = Math.min(300, Math.max(180, panelBox.width - 16));
      popover.style.width = `${width}px`;
      let left = capsule.left;
      if (left + width > panelBox.right - 8) left = panelBox.right - width - 8;
      if (left < panelBox.left + 8) left = panelBox.left + 8;
      const height = popover.getBoundingClientRect().height;
      let top = capsule.bottom + 6;
      if (top + height > window.innerHeight - 8) {
        top = Math.max(8, capsule.top - height - 6);
      }
      popover.style.left = `${Math.round(left)}px`;
      popover.style.top = `${Math.round(top)}px`;
      popover.classList.add("positioned");
    }
  });
}

function initReferencePlacement() {
  document.addEventListener(
    "click",
    (event) => {
      if (event.target instanceof Element && event.target.closest(".question-refs-badge")) {
        window.setTimeout(placeReferencePopovers, 0);
      }
    },
    true,
  );
  document.addEventListener("scroll", placeReferencePopovers, true);
  window.addEventListener("resize", placeReferencePopovers);
  const observer = new MutationObserver(() => {
    if (document.querySelector(".question-refs-popover")) placeReferencePopovers();
  });
  observer.observe(document.body, { childList: true, subtree: true });
}

initReferencePlacement();

function initReferencePreviews() {
  document.addEventListener("pointerover", (event) => {
    const row = event.target instanceof Element ? event.target.closest(".refs-row[data-ref-page], .mention-item[data-ref-page]") : null;
    if (!row) return;
    hideReferencePreview();
    const number = Number(row.dataset.refPage);
    if (!number || referencePageVisible(number)) return;
    const token = refPreviewToken;
    refPreviewTimer = window.setTimeout(() => {
      refPreviewTimer = 0;
      if (token === refPreviewToken) showReferencePreview(row, number);
    }, 120);
  });
  document.addEventListener("pointerout", (event) => {
    const row = event.target instanceof Element ? event.target.closest(".refs-row[data-ref-page], .mention-item[data-ref-page]") : null;
    if (!row) return;
    if (event.relatedTarget instanceof Node && row.contains(event.relatedTarget)) return;
    hideReferencePreview();
  });
  document.addEventListener("scroll", hideReferencePreview, true);
  document.addEventListener("click", hideReferencePreview, true);
  window.addEventListener("resize", hideReferencePreview);
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") hideReferencePreview();
  });
}

initReferencePreviews();

// ── @ picker placement ─────────────────────────────────────────────────────────────────────────
//
// The picker is fixed above the caret, not above the whole composer. It first tries to centre on
// the @; at the panel edges it shifts inward instead of escaping the AI column.

function placeMentionPicker() {
  const picker = document.querySelector(".mention-picker");
  const panel = document.querySelector(".panel");
  const editor = document.querySelector("#ask-editor");
  if (!picker || !panel || !editor) return;
  const panelBox = panel.getBoundingClientRect();
  const editorBox = editor.getBoundingClientRect();
  let caret = null;
  const selection = window.getSelection();
  if (selection?.rangeCount && editor.contains(selection.anchorNode)) {
    const range = selection.getRangeAt(0).cloneRange();
    range.collapse(true);
    const rect = range.getBoundingClientRect();
    if (rect && (rect.width > 0 || rect.height > 0)) caret = rect;
  }
  const width = Math.min(300, Math.max(180, panelBox.width - 16));
  picker.style.width = `${width}px`;
  const anchorLeft = caret ? caret.left : editorBox.left;
  const anchorRight = caret ? caret.right : editorBox.left + 1;
  const anchorTop = caret ? caret.top : editorBox.top;
  let left = (anchorLeft + anchorRight) / 2 - width / 2;
  left = Math.max(panelBox.left + 8, Math.min(left, panelBox.right - width - 8));
  const height = picker.getBoundingClientRect().height;
  const top = Math.max(8, anchorTop - height - 8);
  picker.style.left = `${Math.round(left)}px`;
  picker.style.top = `${Math.round(top)}px`;
  picker.classList.add("positioned");
}

function initMentionPickerPlacement() {
  const observer = new MutationObserver(() => {
    if (document.querySelector(".mention-picker")) requestAnimationFrame(placeMentionPicker);
  });
  observer.observe(document.body, { childList: true, subtree: true });
  window.addEventListener("resize", placeMentionPicker);
}

initMentionPickerPlacement();

