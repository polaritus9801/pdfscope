// Drives the continuous reader in headless Chrome.
//
// Never kill Chrome by a broad pattern from around this script: it shares the machine with the
// reader's own browser. Kill only the process this script spawned, which it already does.
//
// The fixture is generated here (40 text pages by default, one landscape page) so the test has no
// binary dependency. `READER_PAGES=400` reuses the same checks at the spec's acceptance size. It verifies the four paths that share the scroll model — selection, box capture,
// reference tints and [p.N] citation jumps — and writes the screenshots named in the spec.
//
// Usage: node scripts/ui-smoke/reader.mjs [out-dir]

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const HERE = fileURLToPath(new URL(".", import.meta.url));
const DIST = resolve(HERE, "../../ui/dist");
const OUT = resolve(process.argv[2] ?? join(HERE, "../../docs/images"));
const FAKE_TAURI = readFileSync(join(HERE, "fake-tauri.js"), "utf8");
const PAGE_COUNT = Number(process.env.READER_PAGES ?? 40);
const CITATION_PAGE = 31;

if (!existsSync(CHROME)) throw new Error(`Chrome not found at ${CHROME}`);
if (!existsSync(join(DIST, "index.html"))) throw new Error(`run \`just ui\` first: ${DIST} is empty`);
mkdirSync(OUT, { recursive: true });

const MIME = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".pdf": "application/pdf",
  ".wasm": "application/wasm",
};

// ── Fixture ────────────────────────────────────────────────────────────────────────────────────

function pageBox(number) {
  return number === 20 ? [842, 595] : [612, 792];
}

function escapePdf(text) {
  return text.replace(/([\\()])/g, "\\$1");
}

function contentFor(number) {
  const [, height] = pageBox(number);
  const lines = [
    "BT",
    "/F1 24 Tf",
    `72 ${height - 72} Td`,
    `(PDFScope reader fixture) Tj`,
    "0 -36 Td",
    "/F1 14 Tf",
    `(Page ${number} of ${PAGE_COUNT}) Tj`,
    "0 -30 Td",
    "/F1 11 Tf",
    "(This page exists so the continuous column has text to select.) Tj",
    "0 -18 Td",
    "(The quick brown fox jumps over the lazy dog.) Tj",
    "0 -18 Td",
    "(Reference tint and citation geometry stay normalised to this page.) Tj",
    "ET",
  ];
  return lines.join("\n");
}

function makePdf() {
  const objects = [];
  const add = (body) => {
    objects.push(body);
    return objects.length;
  };

  add("<< /Type /Catalog /Pages 2 0 R >>");
  add(""); // pages, filled after the kids are known
  add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");

  const kids = [];
  for (let number = 1; number <= PAGE_COUNT; number += 1) {
    const [width, height] = pageBox(number);
    const content = contentFor(number);
    const contentObject = add(
      `<< /Length ${Buffer.byteLength(content)} >>\nstream\n${content}\nendstream`,
    );
    const pageObject = add(
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${width} ${height}] /Resources << /Font << /F1 3 0 R >> >> /Contents ${contentObject} 0 R >>`,
    );
    kids.push(`${pageObject} 0 R`);
  }
  objects[1] = `<< /Type /Pages /Kids [${kids.join(" ")}] /Count ${PAGE_COUNT} >>`;

  let pdf = "%PDF-1.7\n%\xE2\xE3\xCF\xD3\n";
  const offsets = [0];
  objects.forEach((body, index) => {
    offsets[index + 1] = Buffer.byteLength(pdf, "latin1");
    pdf += `${index + 1} 0 obj\n${body}\nendobj\n`;
  });

  const xref = Buffer.byteLength(pdf, "latin1");
  pdf += `xref\n0 ${objects.length + 1}\n`;
  pdf += "0000000000 65535 f \n";
  for (let index = 1; index <= objects.length; index += 1) {
    pdf += `${String(offsets[index]).padStart(10, "0")} 00000 n \n`;
  }
  pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(pdf, "latin1");
}

// ── Static server ──────────────────────────────────────────────────────────────────────────────

const fixture = makePdf();
const server = createServer((request, response) => {
  const url = new URL(request.url, "http://127.0.0.1");
  if (url.pathname === "/fixture.pdf") {
    response.writeHead(200, { "content-type": "application/pdf" });
    response.end(fixture);
    return;
  }

  const requested = normalize(url.pathname === "/" ? "/index.html" : url.pathname);
  const path = join(DIST, requested);
  if (!path.startsWith(DIST)) {
    response.writeHead(403).end();
    return;
  }
  try {
    const body = readFileSync(path);
    response.writeHead(200, { "content-type": MIME[extname(path)] ?? "application/octet-stream" });
    response.end(body);
  } catch {
    response.writeHead(404).end();
  }
});

await new Promise((resolveListen) => server.listen(0, "127.0.0.1", resolveListen));
const { port } = server.address();
const APP_URL = `http://127.0.0.1:${port}/`;
console.log("serving", APP_URL);

// ── Chrome / CDP ───────────────────────────────────────────────────────────────────────────────

const DEBUG_PORT = 9225;
const profile = `/tmp/pdfscope-reader-${Date.now()}`;
// Extra flags let the same suite run under accessibility settings, e.g.
// SMOKE_FLAGS="--force-prefers-reduced-motion" node scripts/ui-smoke/reader.mjs
const extraFlags = (process.env.SMOKE_FLAGS ?? "").split(" ").filter(Boolean);

const chrome = spawn(CHROME, [
  "--headless=new",
  "--no-sandbox",
  "--disable-gpu",
  "--disable-software-rasterizer",
  "--disable-dev-shm-usage",
  "--disable-crash-reporter",
  "--disable-breakpad",
  "--disable-extensions",
  "--no-first-run",
  "--no-default-browser-check",
  `--remote-debugging-port=${DEBUG_PORT}`,
  "--window-size=1280,860",
  `--user-data-dir=${profile}`,
  ...extraFlags,
  "about:blank",
]);

const sleep = (ms) => new Promise((resolveSleep) => setTimeout(resolveSleep, ms));

async function targets() {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try {
      return await (await fetch(`http://127.0.0.1:${DEBUG_PORT}/json`)).json();
    } catch {
      await sleep(200);
    }
  }
  throw new Error("chrome did not start");
}

let ws;
let nextId = 0;
const pending = new Map();

try {
  const target = (await targets()).find((entry) => entry.type === "page");
  ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolveOpen) => (ws.onopen = resolveOpen));
  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (!message.id || !pending.has(message.id)) return;
    const { resolve, reject } = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) reject(new Error(JSON.stringify(message.error)));
    else resolve(message.result);
  };

  const send = (method, params = {}) => {
    const id = (nextId += 1);
    ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
  };

  async function evaluate(expression) {
    const response = await send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (response.exceptionDetails) {
      const detail = response.exceptionDetails;
      throw new Error(detail.exception?.description ?? detail.text ?? "evaluate failed");
    }
    return response.result?.value;
  }

  async function waitFor(expression, label, timeout = 12_000) {
    const started = Date.now();
    while (Date.now() - started < timeout) {
      if (await evaluate(expression)) return;
      await sleep(100);
    }
    throw new Error(`timed out waiting for ${label}`);
  }

  const failures = [];
  const checks = [];
  function check(condition, label, detail = "") {
    if (condition) {
      checks.push(label);
      console.log("ok  ", label);
    } else {
      failures.push(`${label}${detail ? ` — ${detail}` : ""}`);
      console.log("FAIL", label, detail);
    }
  }

  async function shot(name) {
    const result = await send("Page.captureScreenshot", { format: "png" });
    const path = join(OUT, name);
    writeFileSync(path, Buffer.from(result.data, "base64"));
    console.log("shot", path);
    return path;
  }

  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.addScriptToEvaluateOnNewDocument", {
    source: `window.__testErrors = [];
      addEventListener("error", (event) => window.__testErrors.push(String(event.message)));
      addEventListener("unhandledrejection", (event) => window.__testErrors.push(String(event.reason)));`,
  });
  await send("Page.addScriptToEvaluateOnNewDocument", { source: FAKE_TAURI });
  await send("Page.navigate", { url: APP_URL });
  await sleep(1200);

  // Load the fixture through the real file input, the same way a file picker would.
  await evaluate(`(async () => {
    const bytes = await (await fetch("/fixture.pdf")).arrayBuffer();
    const file = new File([bytes], "reader-fixture.pdf", { type: "application/pdf" });
    const transfer = new DataTransfer();
    transfer.items.add(file);
    const input = document.getElementById("file-input");
    input.files = transfer.files;
    input.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`);

  await waitFor(
    `document.querySelectorAll('.page[data-page]').length === ${PAGE_COUNT} &&
     document.querySelector('#page-1[data-rendered="true"]') !== null &&
     document.querySelector('#page-2[data-rendered="true"]') !== null &&
     document.querySelector('.page-total')?.textContent?.includes('${PAGE_COUNT}')`,
    `the first page and ${PAGE_COUNT} shells`,
    20_000,
  );
  // The renderer finishes its initial window asynchronously; let that settle before scrolling.
  await sleep(300);

  const initial = await evaluate(`(() => {
    const stage = document.querySelector('.stage');
    return {
      pages: document.querySelectorAll('.page[data-page]').length,
      rendered: [...document.querySelectorAll('.page[data-rendered]')].map((page) => Number(page.dataset.page)),
      total: document.querySelector('.page-total')?.textContent?.trim(),
      scrollTop: stage.scrollTop,
      gap: getComputedStyle(document.querySelector('.pages')).rowGap,
      height1: document.querySelector('#page-1').getBoundingClientRect().height,
      height20: document.querySelector('#page-20').getBoundingClientRect().height,
      scrollHeight: document.querySelector('.pages').scrollHeight,
    };
  })()`);
  console.log("initial", initial);
  check(initial.pages === PAGE_COUNT, "one shell per page");
  check(initial.rendered.includes(1), "page 1 has a canvas");
  check(initial.rendered.length <= 4, "the initial render window is bounded", JSON.stringify(initial.rendered));
  check(initial.height1 !== initial.height20, "a landscape page keeps its own height");
  check(initial.gap === "16px", "pages are separated by the spacing token", initial.gap);
  await shot("reader-01-continuous.png");

  // Single page: one sheet, sitting at the top of the reading area.
  await evaluate(`window.PDFScope.setPageMode("single")`);
  await waitFor(
    `(() => {
       const pages = [...document.querySelectorAll('.page')].filter(
         (node) => getComputedStyle(node).display !== 'none',
       );
       if (pages.length !== 1) return false;
       const stage = document.querySelector('.stage').getBoundingClientRect();
       const top = pages[0].getBoundingClientRect().top;
       return top >= stage.top - 1 && top <= stage.top + 40;
     })()`,
    "one page at the top in single mode",
  );
  check(true, "single mode shows one page at the top");
  await shot("reader-01b-single.png");

  await evaluate(`window.PDFScope.setPageMode("continuous")`);
  await waitFor(
    `[...document.querySelectorAll('.page')].filter(
       (node) => getComputedStyle(node).display !== 'none',
     ).length > 1`,
    "the column returns",
  );
  check(true, "continuous mode restores the column");

  // Spread: at page 2 the facing pair (2, 3) is on show, side by side.
  await evaluate(`window.PDFScope.setPageMode("spread")`);
  await evaluate(`window.PDFScope.render(2)`);
  await waitFor(
    `(() => {
       const shown = [...document.querySelectorAll('.page')].filter(
         (node) => getComputedStyle(node).display !== 'none',
       );
       if (shown.length !== 2) return false;
       const [left, right] = shown
         .map((node) => ({ page: Number(node.dataset.page), rect: node.getBoundingClientRect() }))
         .sort((a, b) => a.rect.x - b.rect.x);
       return left.page === 2 && right.page === 3 && right.rect.x > left.rect.x;
     })()`,
    "the facing pair at page 2",
  );
  check(true, "spread shows pages 2 and 3 side by side");
  await shot("reader-01c-spread.png");

  // Offset: the cover pairs with page 2 instead of standing alone.
  await evaluate(`window.PDFScope.setSpreadOffset(true)`);
  await evaluate(`window.PDFScope.render(1)`);
  await waitFor(
    `(() => {
       const shown = [...document.querySelectorAll('.page')].filter(
         (node) => getComputedStyle(node).display !== 'none',
       );
       if (shown.length !== 2) return false;
       const pages = shown.map((node) => Number(node.dataset.page)).sort((a, b) => a - b);
       return pages[0] === 1 && pages[1] === 2;
     })()`,
    "the offset pair at page 1",
  );
  check(true, "offset pairs page 1 with page 2");
  await evaluate(`window.PDFScope.setSpreadOffset(false)`);

  await evaluate(`window.PDFScope.setPageMode("continuous")`);

  // At 50 %, two pages are visible at once: the column, not a single-page canvas.
  await evaluate(`(() => {
    const minus = [...document.querySelectorAll('.bar-right button')].find((node) => node.textContent.trim() === '−');
    minus.click();
    minus.click();
    return true;
  })()`);
  await waitFor(
    `!document.getElementById('pages').classList.contains('zooming') &&
     Number.parseFloat(getComputedStyle(document.getElementById('pages')).getPropertyValue('--view-scale') || '1') <= 0.501 &&
     document.querySelector('#page-1').getBoundingClientRect().width < 500`,
    "the column at 50 %",
  );
  const multi = await evaluate(`(() => ({
    pageField: document.querySelector('.page-field')?.value,
    rendered: [...document.querySelectorAll('.page[data-rendered]')].map((node) => Number(node.dataset.page)).sort((a, b) => a - b),
    page2: document.querySelector('#page-2 canvas') !== null,
    scrollHeight: document.querySelector('.pages').scrollHeight,
  }))()`);
  console.log("multi-page", multi);
  check(multi.pageField === "1", "zooming out keeps the current page", multi.pageField);
  check(multi.page2, "at 50 % the next page is already rendered");
  check(multi.rendered.length <= 4, "the window is still bounded at 50 %", JSON.stringify(multi.rendered));
  await shot("reader-02-multi-page.png");

  await evaluate(`(() => {
    const button = [...document.querySelectorAll('.bar-right button')].find((node) => node.textContent.trim() === '适应宽度');
    button.click();
    return true;
  })()`);
  await waitFor(
    `!document.getElementById('pages').classList.contains('zooming') &&
     Number.parseFloat(getComputedStyle(document.getElementById('pages')).getPropertyValue('--view-scale') || '1') >= 0.999 &&
     document.querySelector('#page-1').getBoundingClientRect().width > 760`,
    "the column back at fit width",
  );
  // `render` restores the reading line after the new layout settles; do not race it.
  await sleep(300);

  // Scroll deep into the column by moving the page, not by changing the page state.
  await evaluate(`(() => {
    const stage = document.querySelector('.stage');
    const page = document.querySelector('#page-18');
    stage.scrollTop = page.offsetTop - 16;
    return true;
  })()`);
  await waitFor(
    `document.querySelector('#page-18[data-rendered="true"]') !== null &&
     document.querySelector('.page-field')?.value === '18'`,
    "page 18 after scrolling",
  );

  const scrolled = await evaluate(`(() => {
    const stage = document.querySelector('.stage');
    return {
      rendered: [...document.querySelectorAll('.page[data-rendered]')].map((page) => Number(page.dataset.page)).sort((a, b) => a - b),
      pageField: document.querySelector('.page-field')?.value,
      page1Canvas: document.querySelector('#page-1 canvas') !== null,
      page18Canvas: document.querySelector('#page-18 canvas') !== null,
      scrollTop: stage.scrollTop,
      scrollHeight: document.querySelector('.pages').scrollHeight,
    };
  })()`);
  console.log("scrolled", scrolled);
  check(scrolled.rendered.includes(18), "page 18 is rendered after scrolling");
  check(!scrolled.page1Canvas, "page 1 released its canvas when it left the window");
  check(scrolled.rendered.length <= 5, "the scroll window stays bounded", JSON.stringify(scrolled.rendered));
  check(scrolled.pageField === "18", "the page field follows the scroll", scrolled.pageField);
  check(
    scrolled.scrollHeight === initial.scrollHeight,
    "mounting and releasing canvases does not change the column height",
    `${initial.scrollHeight} -> ${scrolled.scrollHeight}`,
  );
  await evaluate(`(() => {
    const badge = document.createElement('div');
    badge.id = 'test-window-badge';
    badge.textContent = 'rendered: ' + ${JSON.stringify(scrolled.rendered)}.join(', ');
    Object.assign(badge.style, {
      position: 'fixed', right: '24px', bottom: '24px', zIndex: 99,
      padding: '8px 12px', borderRadius: '6px', background: '#14101a', color: '#fff',
      font: '12px -apple-system, sans-serif', opacity: '0.85',
    });
    document.body.appendChild(badge);
    return true;
  })()`);
  await shot("reader-03-scroll-window.png");
  await evaluate(`document.getElementById('test-window-badge')?.remove(); true;`);

  // ── Selection and reference tint ─────────────────────────────────────────────────────────────

  await evaluate(`(() => {
    const page = document.querySelector('#page-18');
    const span = page.querySelector('.textLayer span');
    if (!span || !span.firstChild) throw new Error('page 18 has no selectable text');
    const range = document.createRange();
    range.setStart(span.firstChild, 0);
    range.setEnd(span.firstChild, Math.min(28, span.firstChild.length));
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange', { bubbles: true }));
    document.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
    return true;
  })()`);
  await waitFor(
    `document.querySelector('#page-18 .selection-bar') !== null`,
    "the selection bar on page 18",
  );

  const bar = await evaluate(`(() => {
    const page = document.querySelector('#page-18');
    const bar = page.querySelector('.selection-bar');
    const tint = page.querySelector('.ref-tint');
    return {
      page: page.querySelector('.selection-page')?.textContent,
      quote: page.querySelector('.selection-quote')?.textContent,
      otherPage: document.querySelector('.page:not([data-page="18"]) .selection-bar') !== null,
      tintBefore: tint !== null,
    };
  })()`);
  console.log("selection", bar);
  check(bar.page === "p.18", "the selection bar reports page 18", bar.page);
  check(!bar.otherPage, "the bar exists only on the selected page");
  await shot("reader-04-selection.png");

  await evaluate(`document.querySelector('#page-18 .selection-bar .button-primary').click(); true;`);
  await waitFor(
    `document.querySelector('#page-18 .ref-tint') !== null &&
     document.querySelectorAll('.ref-row').length >= 1`,
    "the reference tint and chip",
  );

  const tinted = await evaluate(`(() => ({
    page18: document.querySelectorAll('#page-18 .ref-tint').length,
    page19: document.querySelectorAll('#page-19 .ref-tint').length,
    chip: document.querySelector('.ref-row .ref-page')?.textContent,
    bar: document.querySelector('#page-18 .selection-bar') !== null,
  }))()`);
  console.log("tinted", tinted);
  check(tinted.page18 === 1, "the reference tint lands on page 18");
  check(tinted.page19 === 0, "the tint does not leak to another page");
  check(tinted.chip === "p.18", "the chip carries page 18", tinted.chip);
  check(!tinted.bar, "adding the reference dismisses the selection bar");

  // Scrolling away releases the canvas but must not release the page's tint.
  await evaluate(`(() => {
    const stage = document.querySelector('.stage');
    const page = document.querySelector('#page-30');
    stage.scrollTop = page.offsetTop - 16;
    return true;
  })()`);
  await waitFor(
    `document.querySelector('.page-field')?.value === '30' &&
     document.querySelector('#page-30[data-rendered="true"]') !== null`,
    "page 30 after scrolling",
  );
  const away = await evaluate(`(() => ({
    page18Canvas: document.querySelector('#page-18 canvas') !== null,
    page18Tint: document.querySelectorAll('#page-18 .ref-tint').length,
    page30Tint: document.querySelectorAll('#page-30 .ref-tint').length,
  }))()`);
  console.log("away", away);
  check(!away.page18Canvas, "page 18 released its canvas");
  check(away.page18Tint === 1, "page 18 kept its reference tint after release");
  check(away.page30Tint === 0, "page 30 did not inherit the tint");

  // Typing a page scrolls to it, and the tint is visible on the page it belongs to.
  await evaluate(`(() => {
    const field = document.querySelector('.page-field');
    field.value = '18';
    field.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  })()`);
  await waitFor(
    `document.querySelector('.page-field')?.value === '18' &&
     document.querySelector('#page-18[data-rendered="true"]') !== null`,
    "page 18 after typing the page number",
  );
  const returned = await evaluate(`(() => ({
    page18Canvas: document.querySelector('#page-18 canvas') !== null,
    page18Tint: document.querySelectorAll('#page-18 .ref-tint').length,
    scrollTop: document.querySelector('.stage').scrollTop,
  }))()`);
  console.log("returned", returned);
  check(returned.page18Canvas, "page 18 remounted its canvas");
  check(returned.page18Tint === 1, "the tint survived the scroll away and back");
  await shot("reader-05-reference-tint.png");

  // ── Box capture ──────────────────────────────────────────────────────────────────────────────

  await evaluate(`document.querySelector('button[title^="框选"]').click(); true;`);
  await waitFor(`document.querySelector('.stage').classList.contains('capturing')`, "capture armed");

  const marqueePoint = await evaluate(`(() => {
    const box = document.querySelector('#page-18').getBoundingClientRect();
    return { left: box.left, top: box.top, width: box.width, height: box.height };
  })()`);
  const startX = marqueePoint.left + marqueePoint.width * 0.16;
  const startY = marqueePoint.top + marqueePoint.height * 0.08;
  const endX = marqueePoint.left + marqueePoint.width * 0.55;
  const endY = marqueePoint.top + marqueePoint.height * 0.28;

  await evaluate(`(() => {
    const page = document.querySelector('#page-18');
    const pointer = (type, x, y) => new PointerEvent(type, {
      bubbles: true, clientX: x, clientY: y, pointerId: 7, pointerType: 'mouse', button: 0,
    });
    page.dispatchEvent(pointer('pointerdown', ${startX}, ${startY}));
    document.dispatchEvent(pointer('pointermove', ${endX}, ${endY}));
    return document.querySelector('#page-18 .marquee') !== null;
  })()`);
  check(
    await evaluate(`document.querySelector('#page-18 .marquee') !== null`),
    "the marquee is drawn inside page 18",
  );
  await shot("reader-06-marquee.png");

  await evaluate(`(() => {
    const pointer = (type, x, y) => new PointerEvent(type, {
      bubbles: true, clientX: x, clientY: y, pointerId: 7, pointerType: 'mouse', button: 0,
    });
    document.dispatchEvent(pointer('pointerup', ${endX}, ${endY}));
    return true;
  })()`);
  await waitFor(
    `document.querySelector('.ref-row .ref-thumb') !== null &&
     document.querySelector('.stage').classList.contains('capturing') === false`,
    "the captured region chip",
  );

  const region = await evaluate(`(() => ({
    thumb: document.querySelector('.ref-row .ref-thumb')?.src?.startsWith('data:image/png'),
    chip: [...document.querySelectorAll('.ref-row .ref-page')].map((node) => node.textContent),
    page18Tints: document.querySelectorAll('#page-18 .ref-tint').length,
    capturing: document.querySelector('.stage').classList.contains('capturing'),
  }))()`);
  console.log("region", region);
  check(region.thumb === true, "the region is a PNG data URL");
  check(region.chip.includes("p.18"), "the region chip carries page 18", JSON.stringify(region.chip));
  check(region.page18Tints >= 2, "the region adds a tint on page 18");
  check(region.capturing === false, "capture disarms after the drag");
  const regionImage = await evaluate(`document.querySelector('.ref-row .ref-thumb')?.src ?? ""`);
  if (regionImage.startsWith("data:image/png;base64,")) {
    writeFileSync(join(OUT, "reader-region-crop.png"), Buffer.from(regionImage.split(",")[1], "base64"));
  }
  await shot("reader-07-region.png");

  const regionsBeforeCancel = await evaluate(`document.querySelectorAll('.ref-row .ref-thumb').length`);
  await evaluate(`document.querySelector('button[title^="框选"]').click(); true;`);
  await waitFor(`document.querySelector('.stage').classList.contains('capturing')`, "capture armed again");
  await evaluate(`(() => {
    const page = document.querySelector('#page-18');
    const box = page.getBoundingClientRect();
    page.dispatchEvent(new PointerEvent('pointerdown', {
      bubbles: true, clientX: box.left + 100, clientY: box.top + 100,
      pointerId: 8, pointerType: 'mouse', button: 0,
    }));
    document.dispatchEvent(new PointerEvent('pointermove', {
      bubbles: true, clientX: box.left + 260, clientY: box.top + 240,
      pointerId: 8, pointerType: 'mouse', button: 0,
    }));
    document.querySelector('.stage').scrollTop += 60;
    return true;
  })()`);
  await waitFor(
    `document.querySelector('.stage').classList.contains('capturing') === false`,
    "scrolling cancels capture",
  );
  await evaluate(`(() => {
    const page = document.querySelector('#page-18');
    const box = page.getBoundingClientRect();
    document.dispatchEvent(new PointerEvent('pointerup', {
      bubbles: true, clientX: box.left + 260, clientY: box.top + 240,
      pointerId: 8, pointerType: 'mouse', button: 0,
    }));
    return true;
  })()`);
  await sleep(350);
  const cancelled = await evaluate(`({
    thumbs: document.querySelectorAll('.ref-row .ref-thumb').length,
    marquee: document.querySelector('.marquee') !== null,
    capturing: document.querySelector('.stage').classList.contains('capturing'),
  })`);
  console.log("cancelled capture", cancelled);
  check(
    cancelled.thumbs === regionsBeforeCancel,
    "scrolling during a capture adds no region",
    JSON.stringify(cancelled),
  );
  check(!cancelled.marquee, "scrolling during a capture removes the marquee");
  check(cancelled.capturing === false, "scrolling during a capture disarms it");

  // ── [p.N] citation jump ──────────────────────────────────────────────────────────────────────

  await evaluate(`(() => {
    const box = document.querySelector('textarea.ask');
    box.value = '这一页在讲什么？';
    box.dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector('.composer-row button').click();
    return true;
  })()`);
  await waitFor(
    `document.querySelector('.citation[data-page="${CITATION_PAGE}"]') !== null`,
    "the citation in the answer",
    20_000,
  );
  await evaluate(`document.querySelector('.citation[data-page="${CITATION_PAGE}"]').click(); true;`);
  await waitFor(
    `document.querySelector('.page-field')?.value === '${CITATION_PAGE}' &&
     document.querySelector('#page-${CITATION_PAGE}[data-rendered="true"]') !== null`,
    "page 31 after clicking the citation",
  );
  await sleep(80);

  const citation = await evaluate(`(() => {
    const stage = document.querySelector('.stage');
    const page = document.querySelector('#page-${CITATION_PAGE}');
    const box = page.getBoundingClientRect();
    const stageBox = stage.getBoundingClientRect();
    return {
      pageField: document.querySelector('.page-field')?.value,
      flash: page.classList.contains('flash'),
      visible: box.top < stageBox.bottom && box.bottom > stageBox.top,
      topOffset: Math.round(box.top - stageBox.top),
      rendered: [...document.querySelectorAll('.page[data-rendered]')].map((node) => Number(node.dataset.page)),
    };
  })()`);
  console.log("citation", citation);
  check(citation.pageField === String(CITATION_PAGE), "the page field follows the citation", citation.pageField);
  check(citation.flash, "the citation flashes the target page");
  check(citation.visible, "the target page is in the viewport");
  check(
    citation.rendered.includes(CITATION_PAGE),
    "the citation's page is rendered",
    JSON.stringify(citation.rendered),
  );
  await shot("reader-08-citation.png");

  // ── Scroll to the end and back: the window is reused, not accumulated ───────────────────────

  await evaluate(`(() => {
    const stage = document.querySelector('.stage');
    stage.scrollTop = stage.scrollHeight;
    return true;
  })()`);
  await waitFor(
    `document.querySelector('.page-field')?.value === '${PAGE_COUNT}' &&
     document.querySelector('#page-${PAGE_COUNT}[data-rendered="true"]') !== null`,
    "the last page",
  );
  const end = await evaluate(`(() => ({
    pageField: document.querySelector('.page-field')?.value,
    rendered: [...document.querySelectorAll('.page[data-rendered]')].map((node) => Number(node.dataset.page)),
    page1Canvas: document.querySelector('#page-1 canvas') !== null,
  }))()`);
  console.log("end", end);
  check(end.pageField === String(PAGE_COUNT), "the page indicator reaches the last page", end.pageField);
  check(end.rendered.includes(PAGE_COUNT), "the last page is rendered");
  check(!end.page1Canvas, "page 1 stayed released at the end");

  await evaluate(`(() => {
    const stage = document.querySelector('.stage');
    stage.scrollTop = 0;
    return true;
  })()`);
  await waitFor(
    `document.querySelector('.page-field')?.value === '1' &&
     document.querySelector('#page-1[data-rendered="true"]') !== null`,
    "page 1 after scrolling back",
  );
  const back = await evaluate(`(() => ({
    pageField: document.querySelector('.page-field')?.value,
    page1Canvas: document.querySelector('#page-1 canvas') !== null,
    page40Canvas: document.querySelector('#page-${PAGE_COUNT} canvas') !== null,
  }))()`);
  console.log("back", back);
  check(back.pageField === "1", "the page indicator returns to 1", back.pageField);
  check(back.page1Canvas, "page 1 remounted");
  check(!back.page40Canvas, `page ${PAGE_COUNT} was released`);

  // ── Zoom keeps the reading line; only the window re-renders ─────────────────────────────────

  await evaluate(`(() => {
    const stage = document.querySelector('.stage');
    stage.scrollTop = document.querySelector('#page-18').offsetTop - 16;
    return true;
  })()`);
  await waitFor(
    `document.querySelector('.page-field')?.value === '18' &&
     document.querySelector('#page-18[data-rendered="true"]') !== null`,
    "page 18 for the zoom check",
  );

  const readingLine = `(() => {
    const stage = document.querySelector('.stage');
    const page = document.querySelector('#page-18');
    const stageBox = stage.getBoundingClientRect();
    const box = page.getBoundingClientRect();
    const focus = stage.scrollTop + Math.min(stage.clientHeight * 0.35, 240);
    const pageTop = box.top - stageBox.top + stage.scrollTop;
    return {
      fraction: (focus - pageTop) / box.height,
      pageWidth: page.getBoundingClientRect().width,
      pageField: document.querySelector('.page-field')?.value,
      rendered: [...document.querySelectorAll('.page[data-rendered]')].map((node) => Number(node.dataset.page)).sort((a, b) => a - b),
      page1Canvas: document.querySelector('#page-1 canvas') !== null,
    };
  })()`;

  const beforeZoom = await evaluate(readingLine);
  const animation = await evaluate(`(() => {
    const pages = document.getElementById('pages');
    const page = document.querySelector('#page-18');
    const textLayer = page.querySelector('.textLayer');
    const canvas = page.querySelector('canvas');
    const scales = [];
    window.__zoomCanvas = canvas;
    window.__zoomTextLayer = textLayer;
    [...document.querySelectorAll('.bar-right button')].find((node) => node.textContent.trim() === '+')?.click();
    return new Promise((resolve) => {
      let frames = 0;
      const step = () => {
        const scale = Number.parseFloat(getComputedStyle(pages).getPropertyValue('--view-scale') || '1');
        scales.push(scale);
        frames += 1;
        if (scale >= 1.249 || frames >= 80) {
          resolve({
            scales,
            textSame: page.querySelector('.textLayer') === textLayer,
            canvasSame: page.querySelector('canvas') === canvas,
            label: document.querySelector('.zoom')?.textContent,
          });
          return;
        }
        requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    });
  })()`);
  await waitFor(
    `Number.parseFloat(getComputedStyle(document.getElementById('pages')).getPropertyValue('--view-scale') || '1') >= 1.249 &&
     document.querySelector('#page-18 canvas') !== window.__zoomCanvas`,
    "page 18 re-rendered at the new zoom",
  );
  await sleep(200);
  const afterZoom = await evaluate(readingLine);
  const zoomSettled = await evaluate(`(() => ({
    textSame: document.querySelector('#page-18 .textLayer') === window.__zoomTextLayer,
    scales: ${JSON.stringify(animation.scales)}.map((value) => value.toFixed(3)),
  }))()`);
  console.log("zoom", { beforeZoom, afterZoom, animation: { ...animation, scales: undefined }, settled: zoomSettled });
  check(animation.scales.length >= 4, "the button animates over multiple frames");
  check(
    new Set(animation.scales.map((scale) => scale.toFixed(3))).size >= 4,
    "the button does not jump in a single paint",
    JSON.stringify(animation.scales.map((scale) => scale.toFixed(3))),
  );
  check(animation.textSame, "the text layer stays mounted during the button animation");
  check(animation.canvasSame, "the old canvas stays visible during the button animation");
  check(animation.label === "125%", "the zoom label follows the animation", animation.label);
  check(afterZoom.pageField === "18", "zoom keeps the current page", afterZoom.pageField);
  check(
    Math.abs(afterZoom.fraction - beforeZoom.fraction) < 0.03,
    "zoom keeps the reading line on the same fraction of the page",
    `${beforeZoom.fraction.toFixed(3)} -> ${afterZoom.fraction.toFixed(3)}`,
  );
  check(afterZoom.rendered.length <= 5, "zoom re-renders only the window", JSON.stringify(afterZoom.rendered));
  check(!afterZoom.page1Canvas, "zoom did not wake a released page");
  check(zoomSettled.textSame, "the text layer survives the settled re-render");
  await shot("reader-09-zoom.png");

  // ── Zoom-out re-centres a page that is no longer wider than the stage ─────────────────────────

  await evaluate(`(() => {
    const stage = document.querySelector('.stage');
    stage.scrollLeft = 120;
    return stage.scrollLeft > 0;
  })()`);
  await evaluate(`(() => {
    const button = [...document.querySelectorAll('.bar-right button')].find((node) => node.textContent.trim() === '适应宽度');
    button.click();
    return true;
  })()`);
  await waitFor(
    `!document.getElementById('pages').classList.contains('zooming') &&
     Number.parseFloat(getComputedStyle(document.getElementById('pages')).getPropertyValue('--view-scale') || '1') >= 0.999 &&
     document.querySelector('#page-18').getBoundingClientRect().width < ${afterZoom.pageWidth - 10}`,
    "page 18 back at fit width",
  );
  await sleep(300);
  const recentered = await evaluate(`(() => {
    const stage = document.querySelector('.stage');
    const page = document.querySelector('#page-18');
    const stageBox = stage.getBoundingClientRect();
    const pageBox = page.getBoundingClientRect();
    return {
      scrollLeft: stage.scrollLeft,
      centerError: Math.abs((pageBox.left + pageBox.width / 2) - (stageBox.left + stageBox.width / 2)),
      scale: Number.parseFloat(getComputedStyle(document.getElementById('pages')).getPropertyValue('--view-scale') || '1'),
    };
  })()`);
  console.log("recenter", recentered);
  check(recentered.scrollLeft === 0, "zooming back to fit width clears horizontal scroll", String(recentered.scrollLeft));
  check(recentered.centerError < 1.5, "the page is centred after zoom-out", recentered.centerError.toFixed(2));

  // ── A pinch/wheel stream: text stays, the old canvas stays, the pointer is anchored ──────────

  const stream = await evaluate(`(async () => {
    const stage = document.querySelector('.stage');
    const pages = document.getElementById('pages');
    const page = document.querySelector('#page-18');
    const textLayer = page.querySelector('.textLayer');
    const canvas = page.querySelector('canvas');
    const spanCount = textLayer.querySelectorAll('span').length;
    const box = page.getBoundingClientRect();
    const fx = 0.35;
    const fy = 0.28;
    const px = Math.round(box.left + box.width * fx);
    const py = Math.round(box.top + box.height * fy);
    const samples = [];
    const frameTimes = [];
    let previous = performance.now();
    window.__streamCanvas = canvas;
    window.__streamTextLayer = textLayer;
    for (let index = 0; index < 8; index += 1) {
      stage.dispatchEvent(new WheelEvent('wheel', {
        bubbles: true, cancelable: true, ctrlKey: true, deltaMode: 0,
        deltaY: -8, clientX: px, clientY: py,
      }));
      await new Promise((resolve) => requestAnimationFrame(resolve));
      const now = performance.now();
      frameTimes.push(now - previous);
      previous = now;
      const rect = page.getBoundingClientRect();
      samples.push({
        scale: Number.parseFloat(getComputedStyle(pages).getPropertyValue('--view-scale') || '1'),
        dx: rect.left + rect.width * fx - px,
        dy: rect.top + rect.height * fy - py,
        textSame: page.querySelector('.textLayer') === textLayer,
        spans: textLayer.querySelectorAll('span').length,
        canvasSame: page.querySelector('canvas') === canvas,
        label: document.querySelector('.zoom')?.textContent,
      });
    }
    return { samples, frameTimes, spanCount, px, py };
  })()`);
  console.log("zoom stream", {
    frames: stream.samples.length,
    scale: [stream.samples[0]?.scale, stream.samples[stream.samples.length - 1]?.scale],
    spans: stream.spanCount,
    anchor: stream.samples.map((sample) => [sample.dx.toFixed(1), sample.dy.toFixed(1)]),
    frameMax: Math.max(...stream.frameTimes).toFixed(1),
    frameAverage: (stream.frameTimes.reduce((sum, value) => sum + value, 0) / stream.frameTimes.length).toFixed(1),
  });
  await shot("reader-10-zoom-gesture.png");
  check(stream.samples.length === 8, "the wheel stream was sampled every frame");
  check(
    stream.samples.every((sample) => sample.textSame && sample.spans === stream.spanCount),
    "text never disappears and the text layer is never rebuilt during the stream",
  );
  check(
    stream.samples.every((sample) => sample.canvasSame),
    "the canvas is not replaced during the stream",
  );
  check(
    stream.samples.every((sample) => Math.abs(sample.dx) < 3 && Math.abs(sample.dy) < 3),
    "the content under the pointer stays anchored",
    JSON.stringify(stream.samples.map((sample) => [sample.dx.toFixed(1), sample.dy.toFixed(1)])),
  );
  check(
    stream.samples[stream.samples.length - 1].scale > stream.samples[0].scale + 0.5,
    "the wheel stream changes scale continuously",
    `${stream.samples[0].scale.toFixed(2)} -> ${stream.samples[stream.samples.length - 1].scale.toFixed(2)}`,
  );
  check(
    Math.max(...stream.frameTimes) < 50,
    "no wheel frame blocks for more than 50 ms",
    `${Math.max(...stream.frameTimes).toFixed(1)} ms`,
  );

  await waitFor(
    `document.querySelector('#page-18 canvas') !== window.__streamCanvas &&
     document.querySelector('#page-18 .textLayer') === window.__streamTextLayer`,
    "the settled canvas after the stream",
    10_000,
  );
  const streamSettled = await evaluate(`(() => ({
    scale: Number.parseFloat(getComputedStyle(document.getElementById('pages')).getPropertyValue('--view-scale') || '1'),
    textSame: document.querySelector('#page-18 .textLayer') === window.__streamTextLayer,
    canvasWidth: document.querySelector('#page-18 canvas')?.width,
  }))()`);
  console.log("zoom stream settled", streamSettled);
  check(streamSettled.textSame, "the text layer is unchanged after the stream settles");
  check(streamSettled.canvasWidth > 1000, "the settled canvas has the new backing resolution", String(streamSettled.canvasWidth));
  await shot("reader-11-zoom-settled.png");

  // Selection still works at a transformed scale, and the bar keeps its readable size.
  await evaluate(`(() => {
    const page = document.querySelector('#page-18');
    const stageElement = document.querySelector('.stage');
    stageElement.scrollLeft = 0;
    const stage = stageElement.getBoundingClientRect();
    const span = [...page.querySelectorAll('.textLayer span')].find((node) => {
      const rect = node.getBoundingClientRect();
      return rect.left >= stage.left && rect.right > stage.left &&
        rect.top >= stage.top + 90 && rect.bottom < stage.bottom - 60;
    }) ?? [...page.querySelectorAll('.textLayer span')].find((node) => {
      const rect = node.getBoundingClientRect();
      return rect.left >= stage.left && rect.right > stage.left &&
        rect.bottom > stage.top && rect.top < stage.bottom;
    });
    if (!span) throw new Error('no visible text span at 190 %');
    const range = document.createRange();
    range.setStart(span.firstChild, 0);
    range.setEnd(span.firstChild, Math.min(24, span.firstChild.length));
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange', { bubbles: true }));
    document.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
    return true;
  })()`);
  await waitFor(`document.querySelector('#page-18 .selection-bar') !== null`, "selection at 190 %");
  const zoomSelection = await evaluate(`(() => {
    const bar = document.querySelector('#page-18 .selection-bar');
    const box = bar.getBoundingClientRect();
    return {
      page: document.querySelector('#page-18 .selection-page')?.textContent,
      height: box.height,
      width: box.width,
      scale: Number.parseFloat(getComputedStyle(document.getElementById('pages')).getPropertyValue('--view-scale') || '1'),
      stage: document.querySelector('.stage').getBoundingClientRect().toJSON(),
      box: box.toJSON(),
    };
  })()`);
  console.log("zoom selection", zoomSelection);
  check(zoomSelection.page === "p.18", "selection still reports the right page at zoom", zoomSelection.page);
  check(zoomSelection.height < 50, "the selection bar is counter-scaled at zoom", zoomSelection.height.toFixed(1));
  check(
    zoomSelection.box.left < zoomSelection.stage.right &&
      zoomSelection.box.right > zoomSelection.stage.left &&
      zoomSelection.box.top < zoomSelection.stage.bottom &&
      zoomSelection.box.bottom > zoomSelection.stage.top,
    "the selection bar is visible at zoom",
  );
  await shot("reader-12-zoom-selection.png");
  await evaluate(`window.PDFScope.clearSelection(); true;`);
  await waitFor(`document.querySelector('#page-18 .selection-bar') === null`, "cleared zoom selection");

  // Region capture keeps its page-space rectangle at a transformed scale.
  const regionsBefore = await evaluate(`document.querySelectorAll('.ref-row .ref-thumb').length`);
  await evaluate(`document.querySelector('button[title^="框选"]').click(); true;`);
  await waitFor(`document.querySelector('.stage').classList.contains('capturing')`, "capture at zoom armed");
  const regionPoints = await evaluate(`(() => {
    const page = document.querySelector('#page-18');
    const stage = document.querySelector('.stage');
    const box = page.getBoundingClientRect();
    const stageBox = stage.getBoundingClientRect();
    const startX = Math.max(box.left + 40, stageBox.left + 120);
    const startY = Math.max(box.top + 40, stageBox.top + 160);
    const endX = Math.min(box.right - 40, startX + 220);
    const endY = Math.min(box.bottom - 40, startY + 150);
    const event = (type, x, y) => new PointerEvent(type, {
      bubbles: true, clientX: x, clientY: y, pointerId: 9, pointerType: 'mouse', button: 0,
    });
    page.dispatchEvent(event('pointerdown', startX, startY));
    document.dispatchEvent(event('pointermove', endX, endY));
    document.dispatchEvent(event('pointerup', endX, endY));
    return { startX, startY, endX, endY };
  })()`);
  await waitFor(
    `document.querySelectorAll('.ref-row .ref-thumb').length > ${regionsBefore}`,
    "region at zoom",
  );
  const zoomRegion = await evaluate(`({
    thumbs: document.querySelectorAll('.ref-row .ref-thumb').length,
    capturing: document.querySelector('.stage').classList.contains('capturing'),
    image: document.querySelectorAll('.ref-row .ref-thumb')[${regionsBefore}]?.src?.startsWith('data:image/png'),
  })`);
  console.log("zoom region", zoomRegion, regionPoints);
  check(zoomRegion.thumbs > regionsBefore, "box capture works at zoom");
  check(zoomRegion.image === true, "the zoom-time region is a PNG");
  check(zoomRegion.capturing === false, "capture disarms at zoom");

  await evaluate(`(() => {
    const button = [...document.querySelectorAll('.bar-right button')].find((node) => node.textContent.trim() === '适应宽度');
    button.click();
    return true;
  })()`);
  await waitFor(
    `!document.getElementById('pages').classList.contains('zooming') &&
     Number.parseFloat(getComputedStyle(document.getElementById('pages')).getPropertyValue('--view-scale') || '1') >= 0.999`,
    "fit width after the stream",
  );
  await sleep(300);

  // WKWebView path: gesturestart/gesturechange/gestureend, with cumulative scale.
  const gesture = await evaluate(`(async () => {
    const stage = document.querySelector('.stage');
    const page = document.querySelector('#page-18');
    const textLayer = page.querySelector('.textLayer');
    const box = page.getBoundingClientRect();
    const px = Math.round(box.left + box.width * 0.4);
    const py = Math.round(box.top + box.height * 0.35);
    const make = (type, scale) => {
      const event = new Event(type, { bubbles: true, cancelable: true });
      Object.defineProperties(event, {
        scale: { value: scale },
        clientX: { value: px },
        clientY: { value: py },
      });
      return event;
    };
    stage.dispatchEvent(make('gesturestart', 1));
    for (const scale of [1.05, 1.15, 1.3, 1.45]) {
      stage.dispatchEvent(make('gesturechange', scale));
      await new Promise((resolve) => requestAnimationFrame(resolve));
    }
    stage.dispatchEvent(make('gestureend', 1.45));
    await new Promise((resolve) => setTimeout(resolve, 500));
    return {
      scale: Number.parseFloat(getComputedStyle(document.getElementById('pages')).getPropertyValue('--view-scale') || '1'),
      label: document.querySelector('.zoom')?.textContent,
      textSame: page.querySelector('.textLayer') === textLayer,
    };
  })()`);
  console.log("gesture", gesture);
  check(gesture.scale >= 1.44, "gesturechange updates the scale", gesture.scale.toFixed(2));
  check(gesture.label === "145%", "the label follows the gesture", gesture.label);
  check(gesture.textSame, "the text layer survives a gesture");

  await evaluate(`(() => {
    const button = [...document.querySelectorAll('.bar-right button')].find((node) => node.textContent.trim() === '适应宽度');
    button.click();
    return true;
  })()`);
  await waitFor(
    `Number.parseFloat(getComputedStyle(document.getElementById('pages')).getPropertyValue('--view-scale') || '1') >= 0.999`,
    "fit width after the gesture",
  );
  await sleep(300);

  // A physical ctrl/meta key means a mouse wheel: the same scale, but eased over frames.
  const mouseZoom = await evaluate(`(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Control' }));
    const stage = document.querySelector('.stage');
    const pages = document.getElementById('pages');
    const page = document.querySelector('#page-18');
    const textLayer = page.querySelector('.textLayer');
    const canvas = page.querySelector('canvas');
    const box = page.getBoundingClientRect();
    const px = Math.round(box.left + box.width * 0.35);
    const py = Math.round(box.top + box.height * 0.3);
    const scales = [];
    let textSame = true;
    let canvasSame = true;
    stage.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true, cancelable: true, ctrlKey: true, deltaMode: 0,
      deltaY: -30, clientX: px, clientY: py,
    }));
    for (let index = 0; index < 24; index += 1) {
      await new Promise((resolve) => requestAnimationFrame(resolve));
      scales.push(Number.parseFloat(getComputedStyle(pages).getPropertyValue('--view-scale') || '1'));
      textSame = textSame && page.querySelector('.textLayer') === textLayer;
      canvasSame = canvasSame && page.querySelector('canvas') === canvas;
    }
    window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Control' }));
    return {
      scales,
      textSame,
      canvasSame,
      label: document.querySelector('.zoom')?.textContent,
      textLayer,
    };
  })()`);
  console.log("mouse zoom", {
    range: [mouseZoom.scales[0], mouseZoom.scales[mouseZoom.scales.length - 1]],
    distinct: new Set(mouseZoom.scales.map((value) => value.toFixed(3))).size,
  });
  check(
    new Set(mouseZoom.scales.map((value) => value.toFixed(3))).size >= 4,
    "mouse-wheel zoom eases over frames",
  );
  check(mouseZoom.textSame, "the text layer stays mounted during mouse-wheel zoom");
  check(mouseZoom.canvasSame, "the canvas stays visible during mouse-wheel zoom");
  check(mouseZoom.label === "135%", "the label follows the eased mouse zoom", mouseZoom.label);
  await waitFor(
    `!document.getElementById('pages').classList.contains('zooming')`,
    "the mouse-wheel zoom settles",
  );
  await evaluate(`(() => {
    const button = [...document.querySelectorAll('.bar-right button')].find((node) => node.textContent.trim() === '适应宽度');
    button.click();
    return true;
  })()`);
  await waitFor(
    `!document.getElementById('pages').classList.contains('zooming') &&
     Number.parseFloat(getComputedStyle(document.getElementById('pages')).getPropertyValue('--view-scale') || '1') >= 0.999`,
    "fit width after mouse zoom",
  );
  await sleep(300);

  // A stage resize re-measures the base layout and remounts the visible text/canvas once.
  const beforeResize = await evaluate(`({
    width: document.querySelector('#page-18').getBoundingClientRect().width,
    field: document.querySelector('.page-field')?.value,
  })`);
  await evaluate(`document.querySelector('.panel-toggle').click(); true;`);
  await waitFor(
    `document.querySelector('#page-18').getBoundingClientRect().width > ${beforeResize.width + 100} &&
     document.querySelector('.page-field')?.value === '${beforeResize.field}'`,
    "the page reflows after the panel collapses",
  );
  await sleep(500);
  const collapsed = await evaluate(`(() => {
    const page = document.querySelector('#page-18');
    const text = page.querySelector('.textLayer');
    return {
      width: page.getBoundingClientRect().width,
      textWidth: text?.getBoundingClientRect().width,
      field: document.querySelector('.page-field')?.value,
      rendered: page.dataset.rendered === 'true',
    };
  })()`);
  console.log("resize collapsed", collapsed);
  check(collapsed.field === beforeResize.field, "the page indicator survives a resize", collapsed.field);
  check(Math.abs(collapsed.width - collapsed.textWidth) < 2, "the text layer reflows with the page", `${collapsed.width} / ${collapsed.textWidth}`);
  check(collapsed.rendered, "the page is rendered after the resize");

  await evaluate(`document.querySelector('.panel-expand').click(); true;`);
  await waitFor(
    `document.querySelector('#page-18').getBoundingClientRect().width < ${beforeResize.width + 20} &&
     document.querySelector('.page-field')?.value === '${beforeResize.field}'`,
    "the page reflows after the panel expands",
  );
  await sleep(400);

  // ── ⌘↑ / ⌘↓ ──────────────────────────────────────────────────────────────────────────────────

  await evaluate(`document.getElementById('app-root').dispatchEvent(new KeyboardEvent('keydown', {
    key: 'ArrowDown', metaKey: true, bubbles: true, cancelable: true,
  })); true;`);
  await waitFor(
    `document.querySelector('.page-field')?.value === '${PAGE_COUNT}' &&
     document.querySelector('#page-${PAGE_COUNT}[data-rendered="true"]') !== null`,
    "the end of the document with ⌘↓",
  );
  check(true, "⌘↓ jumps to the last page");

  await evaluate(`document.getElementById('app-root').dispatchEvent(new KeyboardEvent('keydown', {
    key: 'ArrowUp', metaKey: true, bubbles: true, cancelable: true,
  })); true;`);
  await waitFor(
    `document.querySelector('.page-field')?.value === '1' &&
     document.querySelector('#page-1[data-rendered="true"]') !== null`,
    "the start of the document with ⌘↑",
  );
  check(true, "⌘↑ jumps to the first page");

  // The toast/status must not have thrown; surface any uncaught console error.
  const consoleErrors = await evaluate(`window.__testErrors ?? []`);
  check(consoleErrors.length === 0, "no uncaught page errors", JSON.stringify(consoleErrors));

  console.log("\nchecks:", checks.length, "failures:", failures.length);
  if (failures.length > 0) {
    for (const failure of failures) console.error("FAILURE:", failure);
    process.exitCode = 1;
  }
  console.log("screenshots:", OUT);
} finally {
  try {
    ws?.close();
  } catch {
    /* already disconnected */
  }
  chrome.kill();
  server.close();
}
