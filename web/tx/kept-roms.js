// The ROMs the emulator keeps on this origin (its IndexedDB database
// `saturnus-roms`, web/romstore.js), read and never written: the
// transfer page offers them for its emulated calculator. Opened without
// a version, so the emulator's own upgrades stay its own; a database that
// does not exist yet is not created (the open is aborted), and a schema
// this page does not know reads as no ROMs (kb/docs/saturnus-tx.md, "URL
// and deploy").
//
// In the desktop app (saturnus tx, its own webview origin) the browser's
// store is empty; the ROMs are the emulator desktop app's instead, from
// its settings through the app's `emulator_roms` and `emulator_rom`
// (web/tx/tauri.js; no path reaches the page). Their bytes are checked
// against the SHA-256 the emulator app recorded: a file changed since is
// left out, with a line in the console, as the emulator's slots say
// "changed". They run as private calculators (the two apps share none).

import { ROM_DB } from "../shared/storage-names.js";
import { emulatorRom, emulatorRoms, isTauri } from "./tauri.js";

import "../shared/models.js";

/** The models that have a serial port and a Kermit server, in the page's order (web/shared/models.js). */
export const SERIAL_MODELS = globalThis.SATURNUS_SERIAL;

/** The models that drive transfers themselves (a folder served to them), in the page's order (web/shared/models.js). */
export const APLET_MODELS = globalThis.SATURNUS_APLETS;

/** The models without a serial port (the Pioneers, the 28S and the 19BII), so never a device here (web/shared/models.js). */
export const NO_SERIAL = globalThis.SATURNUS_NO_SERIAL;

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

/** The desktop app's commands for the emulator app's ROMs, or null in a browser. */
const desktopRoms = () => (isTauri() ? { list: emulatorRoms, bytes: emulatorRom } : null);

/** The emulator app's ROMs whose bytes matched their SHA-256, by SHA-256 (the bytes kept for `keptRomBytes`). */
const verified = new Map();

/** Lower-case hex SHA-256 of `bytes`. */
async function sha256Hex(bytes) {
  const d = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return [...d].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * The emulator desktop app's ROMs of `models` (`native`: `{list, bytes}`)
 * as kept rows `[{model, name, sha256, app: true}]`, each one's bytes
 * read and checked. Never throws.
 */
export async function appRoms(native, models = SERIAL_MODELS, { warn = console.warn } = {}) {
  let list;
  try {
    list = await native.list();
  } catch {
    return [];
  }
  const out = [];
  for (const model of models) {
    const r = list.find((x) => x?.model === model);
    if (!r || typeof r.sha256 !== "string") continue;
    const sha256 = r.sha256.toLowerCase();
    if (!verified.has(sha256)) {
      let bytes;
      try {
        bytes = await native.bytes(model);
      } catch {
        continue;
      }
      if ((await sha256Hex(bytes)) !== sha256) {
        warn(`saturnus tx: the emulator app's ${model} ROM (${r.fileName}) changed since it was chosen there; it is left out.`);
        continue;
      }
      verified.set(sha256, bytes);
    }
    out.push({ model, name: typeof r.fileName === "string" ? r.fileName : model, sha256, app: true });
  }
  return out;
}

/**
 * The kept ROMs of `models` (the models with a Kermit server unless
 * told): `[{model, name, sha256}]`, in their order; in the desktop app
 * the emulator app's (`appRoms`). Never throws: a store that cannot be
 * read has none.
 */
export async function keptRoms(idb = globalThis.indexedDB, models = SERIAL_MODELS, native = desktopRoms()) {
  if (native) return appRoms(native, models);
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

/** The bytes of a kept ROM (its SHA-256), or null; in the desktop app one `keptRoms` listed. */
export async function keptRomBytes(sha256, idb = globalThis.indexedDB, native = desktopRoms()) {
  if (native) {
    if (!verified.has(sha256)) await appRoms(native, [...SERIAL_MODELS, ...APLET_MODELS]);
    return verified.get(sha256) ?? null;
  }
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
