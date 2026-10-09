// The saved states in this browser: IndexedDB database `saturnus`, store
// `states`, two slots per model. The user's own (Save state, Load state)
// under the model's name, written by the page (backend.js); the
// auto-saved one (iteration 27) under `auto:<model>`, written by the
// Worker whenever the calculator changed and settled (worker.js), and
// restored when the model boots. One never overwrites the other.

const DB_NAME = "saturnus";
const DB_STORE = "states";

/**
 * The models whose saved state holds the ROM: the 49G's state carries its
 * 2 MB flash, which is the ROM. Remove ROMs deletes those states too.
 */
export const ROM_HOLDING_STATES = ["49g"];

/** The key of `model`'s auto-saved state. */
export const autoKey = (model) => `auto:${model}`;

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(DB_STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function dbGet(key) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const req = db.transaction(DB_STORE, "readonly").objectStore(DB_STORE).get(key);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  }).finally(() => db.close());
}

export async function dbPut(key, value) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(DB_STORE, "readwrite");
    tx.objectStore(DB_STORE).put(value, key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  }).finally(() => db.close());
}

export async function dbDelete(key) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(DB_STORE, "readwrite");
    tx.objectStore(DB_STORE).delete(key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  }).finally(() => db.close());
}
