// Release-build overlay baseline for 1000 and 3000 pages.
//
// Usage:
//   cd ui && trunk build --release
//   node scripts/ui-smoke/perf.mjs
//
// Or `just perf-baseline`. It uses headless Chrome over CDP (no npm dependency), generates the
// fixtures in memory, and writes docs/perf/overlay-baseline.json.

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { makePerfPdf } from "./perf-fixture.mjs";

const CHROME =
  process.env.CHROME_PATH ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const HERE = fileURLToPath(new URL(".", import.meta.url));
const DIST = resolve(HERE, "../../ui/dist");
const OUT = resolve(HERE, "../../docs/perf/overlay-baseline.json");
const FAKE_TAURI = readFileSync(join(HERE, "fake-tauri.js"), "utf8");
const COUNTS = (process.env.PERF_PAGES ?? "1000,3000")
  .split(",")
  .map((value) => Number(value.trim()))
  .filter((value) => Number.isInteger(value) && value > 0);
const DEBUG_PORT = Number(process.env.PERF_DEBUG_PORT ?? 9333);

if (!existsSync(CHROME)) throw new Error(`Chrome not found at ${CHROME}`);
if (!existsSync(join(DIST, "index.html"))) throw new Error(`run \`cd ui && trunk build --release\` first`);

const fixtures = new Map(COUNTS.map((count) => [`/fixture-${count}.pdf`, makePerfPdf(count)]));

const MIME = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".pdf": "application/pdf",
  ".wasm": "application/wasm",
};

const server = createServer((request, response) => {
  const url = new URL(request.url, "http://127.0.0.1");
  const fixture = fixtures.get(url.pathname);
  if (fixture) {
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

const sleep = (ms) => new Promise((resolveSleep) => setTimeout(resolveSleep, ms));
const profile = `/tmp/pdfscope-perf-${Date.now()}`;
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
  "--window-size=1440,950",
  `--user-data-dir=${profile}`,
  "about:blank",
]);
chrome.stderr.on("data", () => {});

async function targets() {
  for (let attempt = 0; attempt < 100; attempt += 1) {
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
const results = [];
const failures = [];

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
    return new Promise((resolveSend, reject) =>
      pending.set(id, { resolve: resolveSend, reject }),
    );
  };
  const evaluate = async (expression) => {
    const response = await send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (response.exceptionDetails) {
      throw new Error(
        response.exceptionDetails.exception?.description ??
          response.exceptionDetails.text ??
          "evaluate failed",
      );
    }
    return response.result?.value;
  };
  const waitFor = async (expression, label, timeout = 60_000) => {
    const started = Date.now();
    while (Date.now() - started < timeout) {
      if (await evaluate(expression)) return Date.now() - started;
      await sleep(100);
    }
    throw new Error(`timed out waiting for ${label}`);
  };

  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.addScriptToEvaluateOnNewDocument", {
    source: `localStorage.setItem("pdfscope.composer.markdown", "1");
      localStorage.setItem("pdfscope.page-bar-side", "right");
      localStorage.setItem("pdfscope.scroll-skin", "purple");
      window.__testErrors = [];
      addEventListener("error", (event) => window.__testErrors.push(String(event.message)));
      addEventListener("unhandledrejection", (event) => window.__testErrors.push(String(event.reason)));`,
  });
  await send("Page.addScriptToEvaluateOnNewDocument", { source: FAKE_TAURI });

  for (const count of COUNTS) {
    await send("Page.navigate", { url: APP_URL });
    await sleep(1200);
    const startedAt = await evaluate("performance.now()");
    await evaluate(`(async () => {
      const bytes = await (await fetch("/fixture-${count}.pdf")).arrayBuffer();
      const file = new File([bytes], "perf-${count}.pdf", { type: "application/pdf" });
      const transfer = new DataTransfer();
      transfer.items.add(file);
      const input = document.getElementById("file-input");
      input.files = transfer.files;
      input.dispatchEvent(new Event("change", { bubbles: true }));
      return true;
    })()`);
    await waitFor(
      `document.querySelectorAll(".page[data-page]").length === ${count} &&
       document.querySelector("#page-1[data-rendered]") !== null`,
      `${count} page shells`,
      90_000,
    );
    await sleep(500);
    const load = await evaluate(`performance.now() - ${startedAt}`);

    const dom = await evaluate(`(() => ({
      pageShells: document.querySelectorAll(".page[data-page]").length,
      domNodes: document.querySelectorAll("*").length,
      overlayHits: document.querySelectorAll(".page-overlay-hit").length,
      overlayActions: document.querySelectorAll(".page-overlay-actions").length,
      rangeBars: document.querySelectorAll(".page-range-bar.global").length,
      selectionOutlines: document.querySelectorAll(".page-overlay-solid,.page-overlay-preview").length,
      oldActionHits: document.querySelectorAll(".page-action-hits").length,
      oldPageBars: document.querySelectorAll(".page > .page-range-bar").length,
      errors: window.__testErrors ?? [],
    }))()`);

    const hover = await evaluate(`(async () => {
      const page = document.querySelector("#page-1");
      const hit = document.querySelector(".page-overlay-hit.range-right");
      const pr = page.getBoundingClientRect();
      const hr = hit.getBoundingClientRect();
      const x = hr.left + hr.width / 2;
      const y = pr.top + pr.height / 2;
      const started = performance.now();
      hit.dispatchEvent(new PointerEvent("pointermove", {
        bubbles: true, clientX: x, clientY: y, pointerType: "mouse",
      }));
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      return {
        ms: performance.now() - started,
        activeActions: document.querySelectorAll(".page-overlay-actions.active").length,
        previewOutlines: document.querySelectorAll(".page-overlay-preview").length,
        visibleBars: document.querySelectorAll(".page-range-bar.global.visible").length,
      };
    })()`);
    await evaluate(`document.querySelector(".page-overlay-hit.range-right")
      .dispatchEvent(new PointerEvent("pointerleave", { bubbles: true, pointerType: "mouse" }))`);

    const preview = await evaluate(`(async () => {
      const hit = document.querySelector(".page-overlay-hit.range-right");
      const targetPage = Math.min(${count}, 500);
      const page1 = document.querySelector("#page-1");
      const first = page1.getBoundingClientRect();
      const hr = hit.getBoundingClientRect();
      const x = hr.left + hr.width / 2;
      hit.dispatchEvent(new PointerEvent("pointermove", {
        bubbles: true, clientX: x, clientY: first.top + first.height / 6, pointerType: "mouse",
      }));
      hit.dispatchEvent(new MouseEvent("click", {
        bubbles: true, clientX: x, clientY: first.top + first.height / 6,
      }));
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const stage = document.querySelector(".stage");
      const page = document.querySelector("#page-" + targetPage);
      const pageTop = page.getBoundingClientRect().top - stage.getBoundingClientRect().top + stage.scrollTop;
      stage.scrollTop = Math.max(0, pageTop - 180);
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const targetRect = page.getBoundingClientRect();
      const started = performance.now();
      hit.dispatchEvent(new PointerEvent("pointermove", {
        bubbles: true,
        clientX: x,
        clientY: targetRect.top + targetRect.height * 0.8,
        pointerType: "mouse",
      }));
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      return {
        ms: performance.now() - started,
        targetPage,
        previewOutlines: document.querySelectorAll(".page-overlay-preview").length,
        visibleBars: document.querySelectorAll(".page-range-bar.global.visible").length,
      };
    })()`);

    const commit = await evaluate(`(async () => {
      const started = performance.now();
      window.__pdfscopePageRange(1, Math.min(${count}, 500), 0);
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      return {
        ms: performance.now() - started,
        selectionOutlines: document.querySelectorAll(".page-overlay-solid").length,
        visibleBars: document.querySelectorAll(".page-range-bar.global.visible").length,
      };
    })()`);

    const heap = await send("Runtime.getHeapUsage");
    if (dom.pageShells !== count) failures.push(`${count}: page shells ${dom.pageShells}`);
    if (dom.domNodes >= count * 4) failures.push(`${count}: DOM not compact (${dom.domNodes})`);
    if (dom.overlayHits !== 2 || dom.overlayActions !== 2) {
      failures.push(`${count}: overlay is not a single pair (${dom.overlayHits}/${dom.overlayActions})`);
    }
    if (dom.oldActionHits !== 0 || dom.oldPageBars !== 0) {
      failures.push(`${count}: per-page range UI is still mounted`);
    }
    if (dom.errors.length) failures.push(`${count}: ${dom.errors.join("; ")}`);
    if (hover.activeActions < 1 || hover.previewOutlines < 1) {
      failures.push(`${count}: hover did not reveal the action/preview`);
    }
    if (preview.previewOutlines !== preview.targetPage) {
      failures.push(`${count}: preview outlined ${preview.previewOutlines}/${preview.targetPage}`);
    }
    if (commit.selectionOutlines !== preview.targetPage) {
      failures.push(`${count}: committed ${commit.selectionOutlines}/${preview.targetPage}`);
    }
    results.push({
      pages: count,
      loadMs: Math.round(load),
      dom,
      hover,
      preview,
      commit,
      heapUsedMb: Math.round((heap.usedSize / 1024 / 1024) * 10) / 10,
    });
    console.log(JSON.stringify(results.at(-1)));
  }

  if (failures.length) throw new Error(`overlay baseline failed:\n${failures.join("\n")}`);

  mkdirSync(resolve(HERE, "../../docs/perf"), { recursive: true });
  writeFileSync(
    OUT,
    `${JSON.stringify(
      {
        generatedAt: new Date().toISOString(),
        app: APP_URL,
        browser: "headless Chrome (CDP)",
        viewport: "1440x950",
        counts: COUNTS,
        results,
      },
      null,
      2,
    )}\n`,
  );
  console.log(`wrote ${OUT}`);
} finally {
  try {
    ws?.close();
  } catch {}
  chrome.kill("SIGTERM");
  server.close();
}
