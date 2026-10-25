// Recent documents are stored as bytes in IndexedDB. The file picker does not expose a path, so
// remembering a name alone would not be enough to reopen a document after a restart.
(() => {
  const DB_NAME = "pdfscope-recents";
  const STORE = "documents";
  const VERSION = 1;
  const LIMIT = 20;
  let dbPromise = null;

  function openDb() {
    if (dbPromise) return dbPromise;
    dbPromise = new Promise((resolve, reject) => {
      const request = indexedDB.open(DB_NAME, VERSION);
      request.onupgradeneeded = () => {
        const db = request.result;
        if (!db.objectStoreNames.contains(STORE)) {
          const store = db.createObjectStore(STORE, { keyPath: "id" });
          store.createIndex("updated", "updated");
        }
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    return dbPromise;
  }

  function finish(transaction) {
    return new Promise((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error);
      transaction.onabort = () => reject(transaction.error);
    });
  }

  async function rows(limit = 50) {
    const db = await openDb();
    return new Promise((resolve, reject) => {
      const transaction = db.transaction(STORE, "readonly");
      const index = transaction.objectStore(STORE).index("updated");
      const result = [];
      const request = index.openCursor(null, "prev");
      request.onsuccess = () => {
        const cursor = request.result;
        if (!cursor || result.length >= limit) return;
        const { id, name, path, size, updated } = cursor.value;
        result.push({ id, name, path: path || "", size, updated });
        cursor.continue();
      };
      transaction.oncomplete = () => resolve(result);
      transaction.onerror = () => reject(transaction.error);
    });
  }

  async function prune() {
    const all = await rows(LIMIT + 20);
    const stale = all.slice(LIMIT);
    if (stale.length === 0) return;
    const db = await openDb();
    const transaction = db.transaction(STORE, "readwrite");
    const store = transaction.objectStore(STORE);
    for (const entry of stale) store.delete(entry.id);
    await finish(transaction);
  }

  window.Recents = {
    async list() {
      try {
        return await rows(LIMIT);
      } catch {
        return [];
      }
    },

    async save(name, bytes, path = "") {
      try {
        const data = bytes instanceof Uint8Array ? bytes.slice() : new Uint8Array(bytes);
        const id = `${path || name}::${data.byteLength}`;
        const db = await openDb();
        const transaction = db.transaction(STORE, "readwrite");
        transaction.objectStore(STORE).put({
          id,
          name,
          path,
          size: data.byteLength,
          updated: Date.now(),
          bytes: data,
        });
        await finish(transaction);
        await prune();
        return true;
      } catch {
        return false;
      }
    },

    async load(id) {
      try {
        const db = await openDb();
        return await new Promise((resolve, reject) => {
          const transaction = db.transaction(STORE, "readonly");
          const request = transaction.objectStore(STORE).get(id);
          request.onsuccess = () => {
            const record = request.result;
            resolve(record ? { name: record.name, bytes: record.bytes } : null);
          };
          request.onerror = () => reject(request.error);
        });
      } catch {
        return null;
      }
    },
  };
})();
