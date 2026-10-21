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

  async ask(question, references, regions, onEvent) {
    if (!available) throw new Error("提问只在桌面版可用");
    return new Promise(async (resolve, reject) => {
      const unlisten = await api.event.listen("ask:event", (event) => {
        onEvent(event.payload);
        if (event.payload.kind === "done" || event.payload.kind === "error") {
          unlisten();
          resolve();
        }
      });
      try {
        await api.core.invoke("ask", { question, references, regions });
      } catch (error) {
        unlisten();
        reject(error);
      }
    });
  },
};
