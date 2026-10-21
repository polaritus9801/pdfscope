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
let restorePage = 0;
let initialized = false;

/** Base layout at 100 %: the column never reflows during a zoom. */
let baseWidth = 0;
let baseHeight = 0;
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

function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(resolve));
}

function clamp(value, minimum, maximum) {
  return Math.min(Math.max(value, minimum), maximum);
}

function storageKey() {
  return documentName ? `pdfscope:pos:${documentName}` : "";
}

function rememberedPage() {
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
  const key = storageKey();
  if (!key) return;
  try {
    window.localStorage?.setItem(key, String(page));
  } catch {
    /* private mode or quota; remembering a position is best effort */
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
  const padding = stagePadding(stage);
  return Math.max(200, stage.clientWidth - padding.left - padding.right);
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

  baseWidth = width;
  pages.style.width = `${width}px`;
  // `offsetHeight` is the untransformed layout height, which is exactly what the scaler needs.
  baseHeight = pages.offsetHeight;
  baseEpoch += 1;
  metricsDirty = true;
  updateScalerSize();
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
  const page = pageAtFocus();
  if (page !== currentPage) {
    currentPage = page;
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
  return { x: rect.left + rect.width / 2, y: rect.top + readingOffset() };
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
    const point = {
      x: (origin.x - before.left) / displayScale,
      y: (origin.y - before.top) / displayScale,
    };

    setDisplayScale(scale);
    scaler.style.marginTop = "0px";
    updateScalerSize();

    const after = pages.getBoundingClientRect();
    stage.scrollLeft += after.left + point.x * displayScale - origin.x;
    stage.scrollTop += after.top + point.y * displayScale - origin.y;
  }

  applyPan();

  // When the column is narrower than the stage, auto margins centre it and scrollLeft must be 0.
  if (scaler.offsetWidth <= stageContentWidth(stage) + 0.5) {
    stage.scrollLeft = 0;
  }

  syncCurrentPage();
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

  if (scaler.offsetWidth <= stageContentWidth(stage) + 0.5) {
    stage.scrollLeft = 0;
  }
}

// ── Virtual rendering ─────────────────────────────────────────────────────────────────────────

function scheduleUpdate(render = false) {
  updateRender = updateRender || render;
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
  container.style.width = `${baseViewport.width}px`;
  container.style.height = `${baseViewport.height}px`;
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
  canvas.style.width = `${baseViewport.width}px`;
  canvas.style.height = `${baseViewport.height}px`;

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
  });

  if (typeof ResizeObserver === "undefined" || resizeObserver) return;
  resizeObserver = new ResizeObserver(scheduleResizeCheck);
  resizeObserver.observe(stage);
}

function scheduleResizeCheck() {
  if (!document_ || !initialized) return;
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
  for (const rect of range.getClientRects()) {
    if (rect.width === 0 || rect.height === 0) continue;
    left = Math.min(left, rect.left);
    top = Math.min(top, rect.top);
    right = Math.max(right, rect.right);
    bottom = Math.max(bottom, rect.bottom);
  }
  if (!Number.isFinite(left)) return null;

  const unit = (value) => Math.min(Math.max(value, 0), 1);
  const x = unit((left - box.left) / box.width);
  const y = unit((top - box.top) / box.height);
  const width = unit((right - left) / box.width);
  const height = unit((bottom - top) / box.height);
  if (width <= 0 || height <= 0) return null;

  const text = selection.toString().replace(/\s+/g, " ").trim();
  if (!text) return null;

  const number = Number(startPage.dataset.page);
  if (!number) return null;

  return { page: number, x, y, width, height, text };
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
    const payload = normalisedSelectionRect();
    if (!payload) {
      clearReportedSelection();
      return;
    }
    hadSelection = true;
    window.__pdfscopeSelection?.(payload);
  }, 120);
}

document.addEventListener("selectionchange", reportSelection);
document.addEventListener("mouseup", reportSelection);
document.addEventListener("keyup", reportSelection);

window.addEventListener("keydown", (event) => {
  if (event.key === "Control") ctrlKeyDown = true;
});
window.addEventListener("keyup", (event) => {
  if (event.key === "Control") ctrlKeyDown = false;
});
window.addEventListener("blur", () => {
  ctrlKeyDown = false;
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

function disarmCapture() {
  captureArmed = false;
  capturePage = 0;
  marqueeStart = null;
  clearMarquee();
  stageElement()?.classList.remove("capturing");
}

document.addEventListener("pointerdown", (event) => {
  if (!captureArmed) return;
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
  disarmCapture();

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

window.PDFScope = {
  /** @param {Uint8Array} bytes */
  async load(bytes, name = "") {
    documentName = typeof name === "string" ? name : "";
    stopZoom();
    releaseAll();
    baseWidth = 0;
    baseHeight = 0;
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
    reportZoom(true);
    return document_.numPages;
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
            const index =
              typeof reference === "string"
                ? await document_.getPageIndex(reference)
                : reference;
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
      return;
    }

    scheduleUpdate(false);
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
    requestZoomTo(percent, options);
  },

  zoomBy(steps, options = {}) {
    requestZoomBy(steps, options);
  },

  fitWidth() {
    requestFitWidth();
  },

  beginCapture() {
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
    stageElement()?.classList.add("capturing");
    window.getSelection()?.removeAllRanges();
    window.__pdfscopeCleared?.();
  },

  disarmCapture() {
    disarmCapture();
  },

  clearSelection() {
    hadSelection = false;
    window.getSelection()?.removeAllRanges();
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
