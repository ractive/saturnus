// The calculators' states: IndexedDB database `saturnus`, store `states`.
// Per model the user's own state (Save state, Load state) under the
// model's name, written by the emulator page (web/backend.js), and the
// auto-saved one (iteration 27) under `auto:<model>`, the calculator the
// emulator and Files both show (iteration 46), written only by the
// calculators' worker (web/hub.js) whenever it changed and settled.

import { STATE_DB as DB_NAME } from "./storage-names.js";

const DB_STORE = "states";

/** The key of `model`'s auto-saved state. */
export const autoKey = (model) => `auto:${model}`;
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
