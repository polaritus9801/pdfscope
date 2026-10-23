// The Tauri boundary.
//
// The UI calls three functions; nothing else knows whether it is running in the desktop shell. In a
// browser build every call reports that the feature is desktop-only, so the panel can say so instead
// of failing silently.

const api = window.__TAURI__;
const available = Boolean(api && api.core && api.event);

window.Bridge = {
  available,

  async loadSettings() {
    if (!available) return null;
    return api.core.invoke("load_settings");
  },

  async saveSettings(settings) {
    if (!available) throw new Error("设置只在桌面版可用");
    return api.core.invoke("save_settings", { settings });
  },

  async exportMarkdown(filename, content) {
    if (!available) throw new Error("导出文件只在桌面版可用");
    return api.core.invoke("save_export", { filename, content });
  },

  async search(query) {
    if (!available) throw new Error("联网搜索只在桌面版可用");
    return api.core.invoke("web_search", { query });
  },

  async stop() {
    if (!available) return;
    return api.core.invoke("stop_ask");
  },

  async testSettings(settings) {
    if (!available) throw new Error("设置只在桌面版可用");
    return api.core.invoke("test_settings", { settings });
  },

  async ask(question, references, regions, context, history, model, onEvent) {
    if (!available) {
      onEvent({ kind: "error", message: "提问只在桌面版可用" });
      return;
    }

    // Every path must end in an event: a rejected invoke used to leave the panel waiting forever,
    // which looked exactly like the question had never been asked. The silence timer is opt-in now:
    // a model may think for minutes without emitting anything, and the settings decide whether that
    // is tolerated (zero means wait forever).
    const configured = Number(window.PDFScopeRequestTimeout);
    const silenceLimit = Number.isFinite(configured) && configured > 0 ? configured : 0;

    return new Promise((resolve) => {
      let settled = false;
      let unlisten = null;
      let timer = null;

      const settle = (kind, message) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        if (kind) onEvent({ kind, message });
        try {
          unlisten?.();
        } catch {
          // Already detached.
        }
        resolve();
      };

      const arm = () => {
        clearTimeout(timer);
        if (silenceLimit > 0) {
          timer = setTimeout(() => settle("error", "长时间没有响应，已结束"), silenceLimit);
        }
      };

      (async () => {
        try {
          unlisten = await api.event.listen("ask:event", (event) => {
            if (settled) return;
            arm();
            onEvent(event.payload);
            if (["done", "error", "stopped"].includes(event.payload.kind)) {
              settled = true;
              clearTimeout(timer);
              try {
                unlisten?.();
              } catch {
                // Already detached.
              }
              resolve();
            }
          });

          await api.core.invoke("ask", { question, references, regions, context, history, model });
          arm();
        } catch (error) {
          settle("error", String(error?.message ?? error));
        }
      })();
    });
  },
};
