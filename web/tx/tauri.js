// The desktop app's link and folders (kb/iterations/iteration-40-tx-desktop.md):
// what the page uses instead of Web Serial and the File System Access API
// when it runs in saturnus tx's Tauri window (`window.__TAURI__`, as the
// emulator's backend.js picks its backend). The commands are
// crates/saturnus-tx-tauri's; the protocol stays in this page's wasm, the
// native side only moves bytes and files.
//
// - `TauriSerialLink`: a native serial port (`serial_open`,
//   `serial_write`, `serial_close`); the bytes read and the port's loss
//   come on the channel passed to `serial_open`. `serialPorts()` lists
//   the ports with their adapters' names.
// - `NativeDirectory` and `NativeFileHandle`: the computer's files with
//   the handle shape `Folder` (folder.js) runs over, as `MemoryDirectory`
//   has it. A handle is a list of names from the top of the computer
//   (`["Users", "ann"]`; `["C:", "Users"]` on Windows, whose top lists the
//   drives).
// - `computerFolder()`: the right pane's folder in the app, the whole
//   computer as the user may use it, shown at the home folder (owner,
//   2026-10-11: no picker needed to start). `pickNative()` is "Open a
//   folder…" (Tauri's dialog): the pane jumps there; `pickNativeFolder()`
//   a Folder of its own for the aplet server, whose writes carry an
//   `x-serve: 1` header (the app refuses them directly in the home folder;
//   a guard against a bug here, not against a hostile page).
// - Changes are refused outside the home folder and the folders picked in
//   the app's dialog (remembered across runs; the app's scope note):
//   `NoModificationAllowedError` with the app's words, which folder.js
//   shows as they are. `recentNative()`, `usedNative(names)` and
//   `clearRecentNative()` are the pane's "Recent folders".
// - `emulatorRoms()` and `emulatorRom(model)`: the emulator desktop app's
//   ROMs (kept-roms.js), never a path.
// - `saveFile(name, bytes)`: a download, through a save dialog.

import { Folder } from "./folder.js";
import { Link } from "./links.js";

/** Whether the page runs in the desktop app. */
export const isTauri = (win = globalThis) => typeof win.__TAURI__?.core?.invoke === "function";

const core = () => globalThis.__TAURI__.core;

/**
 * A command's failure as the error a browser's handle would throw: the
 * native side says `Name: message` with a DOMException's name (or
 * `TypeError`, or `OsError` for the rest, which folder.js says as it is).
 */
export function nativeError(err) {
  const text = String(err?.message ?? err);
  const m = /^([A-Za-z]+): ([\s\S]*)$/.exec(text);
  if (!m) return new Error(text);
  const [, name, message] = m;
  if (name === "TypeError") return new TypeError(message);
  if (name === "OsError") return Object.assign(new Error(message), { name: "OsError" });
  return new DOMException(message, name);
}

async function call(cmd, args, options) {
  try {
    return await core().invoke(cmd, args, options);
  } catch (err) {
    throw nativeError(err);
  }
}

/** Bytes for a raw request body. */
const bytesOf = (data) => (data instanceof Uint8Array ? data : new Uint8Array(data));

// ------------------------------------------------------------ serial

/** The serial ports: `[{name, label, usb}]`, USB adapters first. */
export const serialPorts = () => core().invoke("serial_ports");

/** A native serial port: one stop bit, no flow control; reopened to change speed or parity. */
export class TauriSerialLink extends Link {
  /** `port` from `serialPorts()`. */
  constructor(port) {
    super();
    this.port = port;
    this.label = port.usb ? "USB serial port" : "serial port";
    this.id = null;
  }

  async open({ baud, parity = "none", dataBits = 8 }) {
    await this.close();
    const channel = new (core().Channel)();
    channel.onmessage = (m) => {
      if (m instanceof ArrayBuffer) {
        if (m.byteLength) this.emitData(new Uint8Array(m));
      } else if (m?.lost) {
        // The native side has let go of the port already; the close the
        // page sends after a loss is harmless.
        this.emitLost(m.lost);
      }
    };
    try {
      this.id = await core().invoke("serial_open", { name: this.port.name, baud, parity, dataBits, onEvent: channel });
    } catch (err) {
      // The native side's words (PORT_IN_USE among them), as an Error.
      throw new Error(String(err?.message ?? err));
    }
  }

  async write(bytes) {
    if (this.id === null) throw new Error("the serial port is not open");
    await core().invoke("serial_write", bytesOf(bytes), { headers: { "x-port": String(this.id) } });
  }

  async close() {
    const id = this.id;
    this.id = null;
    if (id !== null) await core().invoke("serial_close", { id }).catch(() => {});
  }
}

// ------------------------------------------------------------ files

function checkName(name) {
  if (typeof name !== "string" || !name || name === "." || name === ".." || /[/\\\0]/.test(name)) {
    throw new TypeError(`Name is not allowed: ${JSON.stringify(name)}`);
  }
}

const pathHeader = (names) => ({ "x-path": encodeURIComponent(JSON.stringify(names)) });

/** A file's contents, read when asked (`getFile()`'s File, the part folder.js uses). */
class NativeFile {
  constructor(names, size, modified, start = 0, end = size) {
    this.names = names;
    this.name = names.at(-1);
    this.lastModified = modified;
    this.start = start;
    this.end = end;
    this.size = Math.max(end - start, 0);
  }

  async arrayBuffer() {
    const data = await call("folder_read", { path: this.names, start: this.start, end: this.end });
    return data instanceof ArrayBuffer ? data : new Uint8Array(data).buffer;
  }

  slice(start = 0, end = this.size) {
    return new NativeFile(this.names, this.size, this.lastModified, this.start + start, this.start + Math.min(end, this.size));
  }
}

/** The headers of a write: the path, and `x-serve` for the aplet server's. */
const writeHeaders = (names, serve) => ({ ...pathHeader(names), ...(serve ? SERVE_HEADER : {}) });

/** The mark of the aplet server's writes (the app refuses them directly in the home folder). */
export const SERVE_HEADER = Object.freeze({ "x-serve": "1" });

/** A file of the computer. `info` its listing entry (size and time), when known; `serve` marks the aplet server's writes. */
export class NativeFileHandle {
  constructor(names, info = null, { serve = false } = {}) {
    this.kind = "file";
    this.names = names;
    this.name = names.at(-1);
    this.info = info;
    this.serve = serve;
  }

  async isSameEntry(other) {
    return other?.kind === "file" && other.names?.join("\0") === this.names.join("\0");
  }

  async getFile() {
    const s = this.info ?? (await call("folder_stat", { path: this.names }));
    if (s.directory) throw new DOMException(`${this.name} is a folder`, "TypeMismatchError");
    return new NativeFile(this.names, s.size ?? 0, s.modified ?? Date.now());
  }

  /** A writable whose bytes replace the file's when closed, all at once (written beside it, then renamed over it). */
  async createWritable() {
    const chunks = [];
    let open = true;
    return {
      write: async (chunk) => {
        if (!open) throw new TypeError("closed");
        const data = chunk?.type === "write" ? chunk.data : chunk;
        chunks.push(data instanceof Uint8Array ? new Uint8Array(data) : new Uint8Array(await new Blob([data]).arrayBuffer()));
      },
      close: async () => {
        if (!open) throw new TypeError("closed");
        open = false;
        const out = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
        let at = 0;
        for (const c of chunks) {
          out.set(c, at);
          at += c.length;
        }
        await call("folder_write", out, { headers: writeHeaders(this.names, this.serve) });
        this.info = null;
      },
      abort: async () => {
        open = false;
      },
    };
  }
}

/**
 * A folder of the computer: `names` from the top; `title` its name in the
 * page (the top has none of its own); `serve` marks the writes under it
 * as the aplet server's.
 */
export class NativeDirectory {
  constructor(names = [], title = names.at(-1) ?? "", { serve = false } = {}) {
    this.kind = "directory";
    this.names = names;
    this.name = title;
    this.serve = serve;
  }

  async isSameEntry(other) {
    return other?.kind === "directory" && other.names?.join("\0") === this.names.join("\0");
  }

  async getDirectoryHandle(name, { create = false } = {}) {
    checkName(name);
    const names = [...this.names, name];
    if (create) await call("folder_mkdir", { path: names });
    else if (!(await call("folder_stat", { path: names })).directory) throw new DOMException(`${name} is a file`, "TypeMismatchError");
    return new NativeDirectory(names, names.at(-1), { serve: this.serve });
  }

  async getFileHandle(name, { create = false } = {}) {
    checkName(name);
    const names = [...this.names, name];
    if (create) {
      await call("folder_create", { path: names }, this.serve ? { headers: SERVE_HEADER } : undefined);
      return new NativeFileHandle(names, null, { serve: this.serve });
    }
    const s = await call("folder_stat", { path: names });
    if (s.directory) throw new DOMException(`${name} is a folder`, "TypeMismatchError");
    return new NativeFileHandle(names, s, { serve: this.serve });
  }

  async removeEntry(name, { recursive = false } = {}) {
    checkName(name);
    await call("folder_remove", { path: [...this.names, name], recursive });
  }

  /** Rename `from` to `to` in place (folder.js uses it where it is there instead of a copy and a delete). */
  async renameEntry(from, to) {
    checkName(from);
    checkName(to);
    await call("folder_rename", { dir: this.names, from, to });
  }

  async *entries() {
    for (const e of await call("folder_list", { path: this.names })) {
      const names = [...this.names, e.name];
      yield [e.name, e.directory ? new NativeDirectory(names, e.name, { serve: this.serve }) : new NativeFileHandle(names, e, { serve: this.serve })];
    }
  }

  async *keys() {
    for await (const [name] of this.entries()) yield name;
  }

  async *values() {
    for await (const [, handle] of this.entries()) yield handle;
  }

  [Symbol.asyncIterator]() {
    return this.entries();
  }
}

/** The name of the computer's top in the page's paths and its crumbs. */
export const COMPUTER_TITLE = "This computer";

/**
 * The computer's files as one Folder (kind "native"), its top the root
 * (the drives on Windows), shown first at the home folder: `start` is
 * that path, `pathOf(names)` turns a place's names into a path of it.
 */
export async function computerFolder() {
  const folder = new Folder(new NativeDirectory([], COMPUTER_TITLE), { kind: "native", title: COMPUTER_TITLE });
  folder.pathOf = (names) => [COMPUTER_TITLE, ...names];
  folder.start = folder.pathOf(await call("folder_home"));
  return folder;
}

/** The user's home folder, as names from the top. */
export const homeNames = () => call("folder_home");

/** A folder chosen in the app's dialog, as names from the top; null when cancelled. */
export const pickNative = () => call("folder_pick");

/** A folder chosen in the app's dialog as a Folder of its own (kind "picked") for the aplet server, its writes marked; or null. */
export async function pickNativeFolder() {
  const names = await pickNative();
  if (!names) return null;
  const title = names.at(-1) ?? COMPUTER_TITLE;
  const folder = new Folder(new NativeDirectory(names, title, { serve: true }), { kind: "picked", title });
  folder.remembered = false;
  return folder;
}

/** The pane's "Recent folders": `[{names, picked, used}]`, newest first (those that are there). */
export const recentNative = () => call("folder_recent");

/** Report folder `names` shown in the pane: to the top of the recent folders (never a folder the app may change by this). */
export const usedNative = (names) => call("folder_used", { path: names });

/** Forget the recent folders (and the picked ones' right to be changed). */
export const clearRecentNative = () => call("folder_recent_clear");

/** The emulator desktop app's ROMs: `[{model, fileName, sha256}]`. */
export const emulatorRoms = () => call("emulator_roms");

/** The bytes of the emulator desktop app's ROM of `model`. */
export async function emulatorRom(model) {
  return new Uint8Array(await call("emulator_rom", { model }));
}

/** Save `bytes` (or a Blob) as `name` where the user says (a save dialog); resolves to whether it was saved. */
export async function saveFile(name, bytes) {
  const data = bytes instanceof Blob ? new Uint8Array(await bytes.arrayBuffer()) : bytesOf(bytes);
  return core().invoke("save_file", data, { headers: { "x-name": encodeURIComponent(name) } });
}
