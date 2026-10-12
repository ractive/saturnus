// The calculators' states, shared by the emulator (/saturnus/emulator/)
// and saturnus tx (/saturnus/transfer/), which live on the same origin:
// IndexedDB database `saturnus`, store `states`. Per model the user's own
// state (Save state, Load state) under the model's name, written by the
// emulator page (web/backend.js); the auto-saved one (iteration 27) under
// `auto:<model>`, the calculator both pages run (iteration 41), written by
// the Worker that runs it whenever it changed and settled; and
// `holder:<model>`, who runs that calculator now (`{token, who}`), written
// by the Worker that took its lock (web/shared/calc-lock.js). An auto
// save is written only while the holder record still names the writer:
// a page whose lock was taken (a frozen tab woken up) never writes over
// the calculator another page runs now.

import { STATE_DB as DB_NAME } from "./storage-names.js";

const DB_STORE = "states";

/** The key of `model`'s auto-saved state. */
export const autoKey = (model) => `auto:${model}`;
/** The key of who runs `model`'s calculator now. */
export const holderKey = (model) => `holder:${model}`;

/** The text a page shows when a state was saved by a newer build than its own. */
export const TOO_NEW = "This calculator was saved by a newer version of saturnus. Reload this page to use it.";

/**
 * The format version of `state` (the u16 after the `SATURNUS` magic,
 * crates/saturnus-core/src/state.rs), or null when it is not a state.
 */
export function stateFormatOf(state) {
  if (!state || state.length < 10) return null;
  const magic = "SATURNUS";
  for (let i = 0; i < magic.length; i++) {
    if (state[i] !== magic.charCodeAt(i)) return null;
  }
  return state[8] | (state[9] << 8);
}

/** Whether `state` comes from a newer build than one that reads `format`. */
export function tooNew(state, format) {
  const f = stateFormatOf(state);
  return f !== null && f > format;
}

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

/** Say that `who` ("emulator" or "tx") runs `model`'s calculator now; resolves to the token its writes carry. */
export async function claim(model, who) {
  const token = `${who}:${Date.now().toString(36)}:${Math.random().toString(36).slice(2)}`;
  await dbPut(holderKey(model), { token, who });
  return token;
}

/** Who runs `model`'s calculator (`"emulator"`, `"tx"`), or null when unknown. */
export async function holderOf(model) {
  try {
    const h = await dbGet(holderKey(model));
    return typeof h?.who === "string" ? h.who : null;
  } catch {
    return null;
  }
}

/**
 * Write `record` as `model`'s auto-saved state if `token` still holds it
 * (checked and written in one transaction); resolves to whether it was
 * written.
 */
export async function putIfHolder(model, record, token) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(DB_STORE, "readwrite");
    const store = tx.objectStore(DB_STORE);
    let wrote = false;
    const req = store.get(holderKey(model));
    req.onsuccess = () => {
      if (req.result?.token !== token) return;
      store.put(record, autoKey(model));
      wrote = true;
    };
    tx.oncomplete = () => resolve(wrote);
    tx.onerror = () => reject(tx.error);
  }).finally(() => db.close());
}
