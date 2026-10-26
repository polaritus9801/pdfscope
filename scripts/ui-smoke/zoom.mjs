// Headless zoom/panel smoke for the rebuilt reader.
//
// Usage:
//   cd ui && trunk build
//   node scripts/ui-smoke/zoom.mjs
//
// It needs only the built UI and Chrome (CHROME_PATH or a standard install). The checks cover the
// scroll-anchored zoom contract: no transform placement, no release transaction, pointer scroll,
// native horizontal overflow, and the unchanged 020 panel-drag path.

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { makePerfPdf } from "./perf-fixture.mjs";

const candidates = [
  process.env.CHROME_PATH,
  process.env.CHROME_BIN,
  "/usr/bin/google-chrome-stable",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
].filter(Boolean);
const CHROME = candidates.find((path) => existsSync(path));
if (!CHROME) throw new Error("Chrome not found; set CHROME_PATH");
const HERE = fileURLToPath(new URL(".", import.meta.url));
const DIST = resolve(HERE, "../../ui/dist");
const FAKE_TAURI = readFileSync(join(HERE, "fake-tauri.js"), "utf8");
if (!existsSync(join(DIST, "index.html"))) throw new Error("run `cd ui && trunk build` first");
mkdirSync("/tmp", { recursive: true });

const PAGE_COUNT = Number(process.env.ZOOM_PAGES ?? 200);
const fixture = makePerfPdf(PAGE_COUNT);
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
const DEBUG_PORT = 9400 + (process.pid % 500);
const profile = `/tmp/pdfscope-zoom-${Date.now()}`;
const sleep = (ms) => new Promise((resolveSleep) => setTimeout(resolveSleep, ms));
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
  "about:blank",
]);

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
const checks = [];
const failures = [];
function check(condition, label, detail = "") {
  if (condition) {
    checks.push(label);
    console.log("ok  ", label);
  } else {
    failures.push(`${label}${detail ? ` — ${detail}` : ""}`);
    console.log("FAIL", label, detail);
  }
}

try {
  const target = (await targets()).find((entry) => entry.type === "page");
  ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolveOpen) => (ws.onopen = resolveOpen));
  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (!message.id || !pending.has(message.id)) return;
    const { resolve: resolveMessage, reject } = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) reject(new Error(JSON.stringify(message.error)));
    else resolveMessage(message.result);
  };
  const send = (method, params = {}) => {
    const id = (nextId += 1);
    ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolveMessage, reject) => pending.set(id, { resolve: resolveMessage, reject }));
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
  async function waitFor(expression, label, timeout = 15_000) {
    const started = Date.now();
    while (Date.now() - started < timeout) {
      if (await evaluate(expression)) return;
      await sleep(100);
    }
    throw new Error(`timed out waiting for ${label}`);
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
  await sleep(900);
  await evaluate(`(async () => {
    const bytes = await (await fetch("/fixture.pdf")).arrayBuffer();
    const file = new File([bytes], "zoom-fixture.pdf", { type: "application/pdf" });
    const transfer = new DataTransfer();
    transfer.items.add(file);
    const input = document.getElementById("file-input");
    input.files = transfer.files;
    input.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`);
  await waitFor(
    `document.querySelectorAll(".page[data-page]").length === ${PAGE_COUNT} &&
     document.querySelector("#page-1[data-rendered=\\"true\\"]") !== null`,
    "the zoom fixture",
    20_000,
  );
  await sleep(250);

  const initial = await evaluate(`(() => {
    const stage = document.querySelector(".stage");
    const pages = document.querySelector("#pages");
    const rect = pages.getBoundingClientRect();
    return {
      width: rect.width,
      left: rect.left,
      scrollLeft: stage.scrollLeft,
      scale: Number.parseFloat(getComputedStyle(pages).getPropertyValue("--view-scale")) || 1,
      overflowX: getComputedStyle(stage).overflowX,
      customBar: !!document.querySelector(".stage-hscroll"),
      ruler: !!document.querySelector(".page-ruler"),
    };
  })()`);
  check(initial.overflowX === "auto", "horizontal overflow uses the native scrollbar");
  check(!initial.customBar, "no custom horizontal scrollbar");
  check(initial.ruler, "the page ruler exists");

  await evaluate(`(async () => {
    const stage = document.querySelector(".stage");
    for (let index = 0; index < 10; index += 1) {
      stage.dispatchEvent(new WheelEvent("wheel", {
        bubbles: true, cancelable: true, ctrlKey: true, deltaY: -6, deltaMode: 0,
        clientX: 800, clientY: 420,
      }));
      await new Promise((resolveFrame) => requestAnimationFrame(resolveFrame));
    }
    return true;
  })()`);
  await waitFor(
    `!document.querySelector("#pages").classList.contains("zooming")`,
    "the wheel zoom settle",
    10_000,
  );
  const wheel = await evaluate(`(() => {
    const stage = document.querySelector(".stage");
    const pages = document.querySelector("#pages");
    const style = getComputedStyle(pages);
    return {
      scale: Number.parseFloat(style.getPropertyValue("--view-scale")),
      scrollLeft: stage.scrollLeft,
      scrollWidth: stage.scrollWidth,
      clientWidth: stage.clientWidth,
      pageOffsetX: style.getPropertyValue("--page-offset-x").trim(),
      pageOffsetY: style.getPropertyValue("--page-offset-y").trim(),
    };
  })()`);
  check(wheel.scale > 1.2, "wheel zoom advances", String(wheel.scale));
  check(wheel.scrollWidth > wheel.clientWidth, "zoomed page has horizontal overflow");
  check(wheel.scrollLeft > 0, "wheel zoom follows the pointer horizontally", String(wheel.scrollLeft));
  check(!wheel.pageOffsetX && !wheel.pageOffsetY, "zoom leaves no transform placement", `${wheel.pageOffsetX}/${wheel.pageOffsetY}`);

  // Return to fit before the gesture contract test.
  await evaluate(`(() => {
    const button = [...document.querySelectorAll("button")].find((node) => node.textContent.trim() === "适应宽度");
    button?.click();
    return !!button;
  })()`);
  await waitFor(
    `!document.querySelector("#pages").classList.contains("zooming") &&
     Number.parseFloat(getComputedStyle(document.querySelector("#pages")).getPropertyValue("--view-scale")) <= 1.001`,
    "fit width",
    10_000,
  );
  await sleep(250);

  const beforeGesture = await evaluate(`(() => {
    const page = document.querySelector("#pages").getBoundingClientRect();
    const stage = document.querySelector(".stage");
    return { left: page.left, top: page.top, width: page.width, scrollLeft: stage.scrollLeft, scrollTop: stage.scrollTop };
  })()`);
  await evaluate(`(() => {
    const stage = document.querySelector(".stage");
    const fire = (type, scale) => {
      const event = new Event(type, { bubbles: true, cancelable: true });
      Object.defineProperties(event, {
        clientX: { value: 800 }, clientY: { value: 420 }, scale: { value: scale },
      });
      stage.dispatchEvent(event);
    };
    fire("gesturestart", 1);
    fire("gesturechange", 1.18);
    fire("gestureend", 1.18);
    return true;
  })()`);
  await waitFor(
    `!document.querySelector("#pages").classList.contains("zooming")`,
    "the pinch settle",
    10_000,
  );
  await sleep(350);
  const afterGesture = await evaluate(`(() => {
    const page = document.querySelector("#pages").getBoundingClientRect();
    const stage = document.querySelector(".stage");
    return { left: page.left, top: page.top, width: page.width, scrollLeft: stage.scrollLeft, scrollTop: stage.scrollTop };
  })()`);
  check(Math.abs(afterGesture.left - beforeGesture.left) > 1, "pinch changes the page position");
  check(Math.abs(afterGesture.width - beforeGesture.width) > 1, "pinch changes the page size");

  // Return to fit for the panel contract. The divider path is defined at fit width; testing it while
  // the page is still pinched would mix two independent scroll states.
  await evaluate(`(() => {
    const button = [...document.querySelectorAll("button")].find((node) => node.textContent.trim() === "适应宽度");
    button?.click();
    return !!button;
  })()`);
  await waitFor(
    `!document.querySelector("#pages").classList.contains("zooming") &&
     Number.parseFloat(getComputedStyle(document.querySelector("#pages")).getPropertyValue("--view-scale")) <= 1.001`,
    "fit width before panel drag",
    10_000,
  );
  await sleep(250);

  // The panel path must still scale the page continuously and leave a stable left gutter.
  const beforePanel = await evaluate(`(() => {
    const stage = document.querySelector(".stage");
    return { clientWidth: stage.clientWidth, pageLeft: document.querySelector("#pages").getBoundingClientRect().left };
  })()`);
  await evaluate(`(() => {
    const original = Element.prototype.getBoundingClientRect;
    window.__rectCalls = 0;
    window.__restoreRect = () => { Element.prototype.getBoundingClientRect = original; };
    Element.prototype.getBoundingClientRect = function () {
      window.__rectCalls += 1;
      return original.call(this);
    };
    return true;
  })()`);
  await evaluate(`window.PDFScope.beginPanelResize()`);
  const panelRectCalls = [];
  for (const x of [800, 700, 600]) {
    await evaluate(`window.__rectCalls = 0; window.dispatchEvent(new PointerEvent("pointermove", {
      bubbles: true, clientX: ${x}, clientY: 300, pointerId: 1, pointerType: "mouse",
    }))`);
    await sleep(70);
    panelRectCalls.push(await evaluate(`window.__rectCalls`));
  }
  await evaluate(`window.__restoreRect()`);
  const duringPanel = await evaluate(`(() => {
    const stage = document.querySelector(".stage");
    return { clientWidth: stage.clientWidth, pageLeft: document.querySelector("#pages").getBoundingClientRect().left };
  })()`);
  await evaluate(`window.PDFScope.finishPanelResize()`);
  await sleep(350);
  const afterPanel = await evaluate(`(() => {
    const stage = document.querySelector(".stage");
    return { clientWidth: stage.clientWidth, pageLeft: document.querySelector("#pages").getBoundingClientRect().left };
  })()`);
  check(afterPanel.clientWidth < beforePanel.clientWidth, "panel drag narrows the stage");
  check(Math.abs(duringPanel.pageLeft - beforePanel.pageLeft) < 2, "panel drag keeps the PDF left gutter");
  check(Math.abs(afterPanel.pageLeft - beforePanel.pageLeft) < 2, "panel release keeps the PDF left gutter");
  const maxPanelRectCalls = Math.max(...panelRectCalls);
  check(maxPanelRectCalls < PAGE_COUNT / 2, "panel drag layout reads stay bounded", String(maxPanelRectCalls));

  // At high zoom, the divider path must hold the page's horizontal centre on the stage content
  // centre instead of preserving an arbitrary pointer scroll offset.
  await evaluate(`(() => {
    const button = [...document.querySelectorAll("button")].find((node) => node.textContent.trim() === "适应宽度");
    button?.click();
    return !!button;
  })()`);
  await waitFor(
    `!document.querySelector("#pages").classList.contains("zooming") &&
     Number.parseFloat(getComputedStyle(document.querySelector("#pages")).getPropertyValue("--view-scale")) <= 1.001`,
    "fit width before high-zoom panel drag",
    10_000,
  );
  await sleep(200);
  await evaluate(`(async () => {
    const stage = document.querySelector(".stage");
    for (let index = 0; index < 28; index += 1) {
      stage.dispatchEvent(new WheelEvent("wheel", {
        bubbles: true, cancelable: true, ctrlKey: true, deltaY: -5, deltaMode: 0,
        clientX: 800, clientY: 420,
      }));
      await new Promise((resolveFrame) => requestAnimationFrame(resolveFrame));
    }
    return true;
  })()`);
  await waitFor(
    `!document.querySelector("#pages").classList.contains("zooming")`,
    "high-zoom settle",
    10_000,
  );
  const highZoom = await evaluate(`(() => {
    const pages = document.querySelector("#pages");
    return Number.parseFloat(getComputedStyle(pages).getPropertyValue("--view-scale"));
  })()`);
  check(highZoom > 1.5, "high-zoom panel fixture is overflowing horizontally", String(highZoom));
  const anchorFraction = await evaluate(`(() => {
    const stage = document.querySelector(".stage");
    const pages = document.querySelector("#pages");
    const style = getComputedStyle(stage);
    const stageRect = stage.getBoundingClientRect();
    const pageRect = pages.getBoundingClientRect();
    const contentWidth = stage.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
    const centre = stageRect.left + parseFloat(style.paddingLeft) + contentWidth / 2;
    return (centre - pageRect.left) / pageRect.width;
  })()`);
  await evaluate(`window.PDFScope.beginPanelResize()`);
  const panelLeft = await evaluate(`document.querySelector(".panel").getBoundingClientRect().left`);
  await evaluate(`window.dispatchEvent(new PointerEvent("pointermove", {
    bubbles: true, clientX: ${panelLeft - 100}, clientY: 300, pointerId: 1, pointerType: "mouse",
  }))`);
  await sleep(180);
  const centeredDuring = await evaluate(`(() => {
    const stage = document.querySelector(".stage");
    const pages = document.querySelector("#pages");
    const style = getComputedStyle(stage);
    const stageRect = stage.getBoundingClientRect();
    const pageRect = pages.getBoundingClientRect();
    const contentWidth = stage.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
    return {
      anchorX: pageRect.left + ${anchorFraction} * pageRect.width,
      stageCenter: stageRect.left + parseFloat(style.paddingLeft) + contentWidth / 2,
    };
  })()`);
  await evaluate(`window.PDFScope.finishPanelResize()`);
  await sleep(350);
  const centeredAfter = await evaluate(`(() => {
    const stage = document.querySelector(".stage");
    const pages = document.querySelector("#pages");
    const style = getComputedStyle(stage);
    const stageRect = stage.getBoundingClientRect();
    const pageRect = pages.getBoundingClientRect();
    const contentWidth = stage.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
    return {
      anchorX: pageRect.left + ${anchorFraction} * pageRect.width,
      stageCenter: stageRect.left + parseFloat(style.paddingLeft) + contentWidth / 2,
    };
  })()`);
  check(Math.abs(centeredDuring.anchorX - centeredDuring.stageCenter) < 2, "high-zoom panel keeps the viewport anchor during drag", JSON.stringify(centeredDuring));
  check(Math.abs(centeredAfter.anchorX - centeredAfter.stageCenter) < 2, "high-zoom panel keeps the viewport anchor after release", JSON.stringify(centeredAfter));

  const errors = await evaluate(`window.__testErrors ?? []`);
  check(errors.length === 0, "no uncaught page errors", JSON.stringify(errors));

  console.log("\nchecks:", checks.length, "failures:", failures.length);
  if (failures.length > 0) {
    for (const failure of failures) console.error("FAILURE:", failure);
    process.exitCode = 1;
  }
} finally {
  try { ws?.close(); } catch { /* disconnected */ }
  chrome.kill();
  server.close();
}
