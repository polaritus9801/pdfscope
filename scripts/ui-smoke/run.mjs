// Drives the built UI with a fake Tauri bridge and captures the result.
import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";

const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const PORT = 9224;
const URL = process.argv[2];
const OUT = process.argv[3];

const chrome = spawn(CHROME, [
  "--headless=new",
  "--disable-gpu",
  `--remote-debugging-port=${PORT}`,
  "--window-size=1280,860",
  `--user-data-dir=/tmp/pdfscope-smoke-${Date.now()}`,
  "about:blank",
]);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function targets() {
  for (let attempt = 0; attempt < 60; attempt++) {
    try {
      return await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();
    } catch {
      await sleep(200);
    }
  }
  throw new Error("chrome did not start");
}

const page = (await targets()).find((target) => target.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve) => (ws.onopen = resolve));

let id = 0;
const pending = new Map();
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  if (message.id && pending.has(message.id)) {
    pending.get(message.id)(message);
    pending.delete(message.id);
  }
};

function send(method, params = {}) {
  const messageId = ++id;
  ws.send(JSON.stringify({ id: messageId, method, params }));
  return new Promise((resolve) => pending.set(messageId, resolve));
}

async function evaluate(expression) {
  const result = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  return result.result?.result?.value;
}

await send("Page.enable");
await send("Runtime.enable");
await send("Page.navigate", { url: URL });
await sleep(1500);

const configured = await evaluate(`document.querySelector(".panel-model")?.textContent ?? ""`);
console.log("model label:", configured);

await evaluate(`
  const box = document.querySelector("textarea.ask");
  box.value = "这一节在讲什么？";
  box.dispatchEvent(new Event("input", { bubbles: true }));
  true;
`);

await evaluate(`document.querySelector(".composer-row button").click(); true;`);
await sleep(4000);

const answer = await evaluate(`document.querySelector(".answer-text")?.textContent ?? ""`);
const status = await evaluate(`document.querySelector(".status")?.textContent ?? ""`);
console.log("answer:", JSON.stringify(answer));
console.log("status:", JSON.stringify(status));

const shot = await send("Page.captureScreenshot", { format: "png" });
writeFileSync(OUT, Buffer.from(shot.result.data, "base64"));
console.log("screenshot:", OUT);

ws.close();
chrome.kill();
