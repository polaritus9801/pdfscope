// A fake Tauri API, enough to drive the real UI in a browser.
//
// The app's `bridge.js` only checks `window.__TAURI__`; providing a convincing fake means the actual
// Leptos components run unmodified, which is what makes this a smoke test rather than a mock.

const listeners = new Set();

function push(payload) {
  for (const listener of listeners) {
    listener({ payload });
  }
}

let answer = "";

window.__TAURI__ = {
  core: {
    async invoke(command, args) {
      switch (command) {
        case "load_settings":
          return {
            preset: "MiMo",
            protocol: "openai",
            base_url: "https://api.xiaomimimo.com/v1",
            model: "mimo-v2.6-flash",
            has_key: true,
          };
        case "save_settings":
          return null;
        case "test_settings":
          return "连接正常 · mimo-v2.6-flash";
        case "stop_ask":
          push({ kind: "stopped" });
          return null;
        case "ask": {
          answer = "";
          const answerText = "这一段讨论的是公共领域的规范有效性。[p.31] 它把合法性建立在交往前提上。";
          setTimeout(() => push({ kind: "reasoning", text: "先想想" }), 60);
          answerText.split("").forEach((character, index) => {
            setTimeout(() => {
              answer += character;
              push({ kind: "text", text: character });
            }, 120 + index * 12);
          });
          setTimeout(
            () => push({ kind: "usage", prompt: 812, completion: 41, cached: 512 }),
            120 + answerText.length * 12,
          );
          setTimeout(() => push({ kind: "done" }), 200 + answerText.length * 12);
          return null;
        }
        default:
          return null;
      }
    },
  },
  event: {
    async listen(name, handler) {
      const wrapped = (event) => handler(event);
      listeners.add(wrapped);
      return async () => listeners.delete(wrapped);
    },
  },
};
