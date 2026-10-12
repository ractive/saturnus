// The ROMs the emulator keeps on this origin (its IndexedDB database
// `saturnus-roms`, web/romstore.js), read and never written: the
// transfer page offers them for its emulated calculator. Opened without
// a version, so the emulator's own upgrades stay its own; a database that
// does not exist yet is not created (the open is aborted), and a schema
// this page does not know reads as no ROMs (kb/docs/saturnus-tx.md, "URL
// and deploy").

import { ROM_DB } from "../shared/storage-names.js";

/** The models that have a serial port and a Kermit server, in the page's order. */
export const SERIAL_MODELS = ["48sx", "48gx", "49g"];

/** The models that drive transfers themselves (a folder served to them), in the page's order. */
export const APLET_MODELS = ["38g", "39g", "40g"];

/** The Pioneer models: no serial port, so never a device here (web/shared/models.js). */
import "../shared/models.js";
export const PIONEERS = globalThis.SATURNUS_PIONEERS;

function promised(req) {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

/** The database, or null when there is none (or no IndexedDB). */
function openExisting(idb) {
  return new Promise((resolve) => {
    let req;
    try {
      req = idb.open(ROM_DB);
    } catch {
      resolve(null);
      return;
    }
    let created = false;
    req.onupgradeneeded = () => {
      // It did not exist: abort, so it is not created by this page.
      created = true;
      req.transaction?.abort();
    };
    req.onsuccess = () => {
      if (created) {
        req.result.close();
        resolve(null);
      } else resolve(req.result);
    };
    req.onerror = () => resolve(null);
    req.onblocked = () => resolve(null);
  });
}

/**
 * The kept ROMs of `models` (the models with a Kermit server unless
 * told): `[{model, name, sha256}]`, in their order. Never throws: a store
 * that cannot be read has none.
 */
export async function keptRoms(idb = globalThis.indexedDB, models = SERIAL_MODELS) {
  if (!idb) return [];
  const db = await openExisting(idb);
  if (!db) return [];
  try {
    if (!db.objectStoreNames.contains("slots")) return [];
    const store = db.transaction("slots", "readonly").objectStore("slots");
    const [keys, values] = await Promise.all([promised(store.getAllKeys()), promised(store.getAll())]);
    return slotsOf(keys, values, models);
  } catch {
    return [];
  } finally {
    db.close();
  }
}

/** The ROM rows of the slot store's keys and values (pure; tested). */
export function slotsOf(keys, values, models = SERIAL_MODELS) {
  const out = [];
  for (const model of models) {
    const i = keys.indexOf(model);
    const v = i >= 0 ? values[i] : null;
    if (v && typeof v.sha256 === "string" && v.sha256) out.push({ model, name: typeof v.name === "string" ? v.name : model, sha256: v.sha256 });
  }
  return out;
}

/** The bytes of a kept ROM (its SHA-256), or null. */
export async function keptRomBytes(sha256, idb = globalThis.indexedDB) {
  if (!idb) return null;
  const db = await openExisting(idb);
  if (!db) return null;
  try {
    if (!db.objectStoreNames.contains("images")) return null;
    const v = await promised(db.transaction("images", "readonly").objectStore("images").get(sha256));
    if (v instanceof Uint8Array) return v;
    if (v instanceof ArrayBuffer) return new Uint8Array(v);
    if (v?.buffer instanceof ArrayBuffer) return new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
    return null;
  } catch {
    return null;
  } finally {
    db.close();
  }
}
