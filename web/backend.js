// The two backends of the page (web/protocol.md): WorkerBackend runs the
// wasm core in a Web Worker, TauriBackend talks to the Tauri app's native
// core. Same interface; nothing else in the page knows which one it has.
// Each is an EventTarget that dispatches the protocol's events (`frame`,
// `keys`, `status`, `error`, `memoryChanged`) as CustomEvents with the message as `detail`.

import { ROM_HOLDING_STATES, autoKey, dbDelete, dbGet, dbPut } from "./states.js";

export { ROM_HOLDING_STATES };

const PROTOCOL = 1;

// ------------------------------------------------------------ common

/** The command methods both backends share; `request` and `send` differ. */
class Backend extends EventTarget {
  constructor() {
    super();
    /** Static data per model, fetched once. */
    this.cache = new Map();
  }

  dispatch(msg) {
    if (msg && typeof msg.type === "string" && msg.type !== "reply") {
      this.dispatchEvent(new CustomEvent(msg.type, { detail: msg }));
    }
  }

  async cached(cmd, model) {
    const key = `${cmd}:${model}`;
    if (!this.cache.has(key)) {
      const p = this.request(cmd, { model });
      this.cache.set(key, p);
      p.catch(() => this.cache.delete(key));
    }
    return this.cache.get(key);
  }

  /** `{protocol, host, models}`. */
  hello() { return this.request("hello"); }
  /** The skin JSON of `model` (with its letters and typing rules). */
  skin(model) { return this.cached("skin", model); }
  keyDown(key) { this.send("keyDown", { key }); }
  keyUp(key) { this.send("keyUp", { key }); }
  keyUpAll() { this.send("keyUpAll"); }
  typeLetter(letter) { this.send("typeLetter", { letter }); }
  typeKeys(keys) { this.send("typeKeys", { keys }); }
  releaseAll() { this.send("releaseAll"); }
  setSpeed(speed) { this.send("setSpeed", { speed }); }
  pause(paused) { return this.request("pause", { paused }); }
  reset() { return this.request("reset"); }
  visibility(hidden) { this.send("visibility", { hidden }); }
  stats() { return this.request("stats"); }
  /** Ask for `memoryChanged` events (or stop); `{supported, reason}`. */
  watchMemory(on) { return this.request("watchMemory", { on }); }
  /** `{path, variables}`: the current directory and HOME's tree. */
  memoryTree() { return this.request("memoryTree"); }
  /** The stack's typed levels, level 1 first. */
  stack() { return this.request("stack"); }
  /** `{system, user, set}`. */
  flags() { return this.request("flags"); }
  /** The typed object at `address` (a variable's, from `memoryTree`). */
  objectAt(address) { return this.request("objectAt", { address }); }
  /** Type `text` at the cursor (or start a command line); `{typed, keys, emulatedMs, commandLine}`. */
  insert(text) { return this.request("insert", { text }); }
  /** Type `text`, then ENTER; also `{closed, error, running}`. */
  run(text) { return this.request("run", { text }); }
  /** Clear the command line being edited, then type `text`. */
  replace(text) { return this.request("replace", { text }); }
  /** `{active, text, cursor}` of the command line, from RAM. */
  commandLine() { return this.request("commandLine"); }

  // The writes (protocol.md, "The user memory, written"): each one hidden
  // Kermit transaction with the calculator's server. `dir` is a path
  // (`["HOME", "D"]`).
  /** Purge variable `name` of `dir` (a directory with everything in it). */
  purge(dir, name) { return this.request("purge", { dir, name }); }
  /** Rename variable `name` of `dir` to `to`. */
  rename(dir, name, to) { return this.request("rename", { dir, name, to }); }
  /** Make `dir` the calculator's current directory. */
  changeDir(dir) { return this.request("changeDir", { dir }); }
  /** Set (`on`) or clear flag `flag` (negative: a system flag). */
  setFlag(flag, on) { return this.request("setFlag", { flag, on }); }
  /** The text to edit of a variable (`{dir, name}`) or a stack level (`{level}`): `{text}`. */
  editText(where) { return this.request("editText", where); }
  /**
   * Compile `text` on the calculator into a variable (`{dir, name}`) or a
   * stack level (`{level}`), if `was` is still what is there: `{emulatedMs,
   * keys, error?}`, `error` the calculator's own (nothing changed).
   */
  storeText({ text, was = null, ...where }) { return this.request("storeText", { ...where, text, ...(was === null ? {} : { was }) }); }

  // The ROM slots (protocol.md, "ROM slots"): the host remembers the ROM
  // of each model. Each resolves to the slots, `romSlots`'s result.
  romSlots() { return this.request("romSlots"); }
  /**
   * Boot `model` from its remembered ROM, with the state the host kept for
   * it (the calculator as it was left); adds `booted`.
   */
  bootModel(model) { return this.request("bootModel", { model }); }
  /** Boot `model` cold, forgetting its kept state (the user's saved state stays); adds `booted`. */
  startFresh(model) { return this.request("bootModel", { model, fresh: true }); }
  /** Take offer `offer` (an id from the slots) as `model`'s ROM and boot it. */
  takeOffer(model, offer) { return this.request("chooseRom", { model, offer }); }
  /** Forget `model`'s ROM, or every ROM. */
  forgetRom(model = null) { return this.request("forgetRom", model ? { model } : {}); }
  /** Whether the last model boots when the page opens. */
  romSettings(bootLast) { return this.request("romSettings", { bootLast }); }
}

/** `bytes` as base64, for the hosts that carry *bytes* fields in JSON. */
export function base64(bytes) {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(bin);
}

/** Largest file the page reads for a ROM (an unpacked 49G); a larger one is sent empty, so it is "not a ROM". */
const MAX_ROM_FILE = 4 * 1024 * 1024;

// ------------------------------------------------------------ Worker

/** The wasm core in a Web Worker (`worker.js`). */
export class WorkerBackend extends Backend {
  constructor() {
    super();
    this.host = "worker";
    /** The ROM comes from a file input in the page. */
    this.romSource = "file";
    this.nextId = 1;
    this.waiting = new Map();
    this.worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
    this.worker.onmessage = (e) => this.receive(e.data);
    this.worker.onerror = (e) => {
      const message = e.message || "the worker failed to start";
      for (const { reject } of this.waiting.values()) reject(new Error(message));
      this.waiting.clear();
      this.dispatch({ type: "error", message });
    };
  }

  receive(msg) {
    if (msg?.type === "reply") {
      const w = this.waiting.get(msg.id);
      if (!w) return;
      this.waiting.delete(msg.id);
      if (msg.ok) w.resolve(msg.result);
      else w.reject(new Error(msg.error));
      return;
    }
    this.dispatch(msg);
  }

  request(cmd, args = {}, transfer = []) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      this.worker.postMessage({ v: PROTOCOL, id, cmd, ...args }, transfer);
    });
  }

  send(cmd, args = {}) {
    this.worker.postMessage({ v: PROTOCOL, cmd, ...args });
  }

  /** Boot from the File `file`; resolves to `{model, romName}`. */
  async boot({ model, file }) {
    const rom = new Uint8Array(await file.arrayBuffer());
    return this.request("boot", { model, rom, romName: file.name }, [rom.buffer]);
  }

  /**
   * Choose ROM files for `model` (a FileList or array of Files): each is
   * identified and assigned to its model's slot; `{booted, notice, ...slots}`.
   */
  async chooseRom(model, files) {
    const list = await Promise.all([...files].map(async (f) => ({
      name: f.name,
      rom: f.size > MAX_ROM_FILE ? new Uint8Array(0) : new Uint8Array(await f.arrayBuffer()),
    })));
    return this.request("chooseRom", { model, files: list }, list.map((f) => f.rom.buffer));
  }

  /**
   * Store the File `file` as variable `name` in `dir`; `{name, emulatedMs,
   * keys}` (`name` as the calculator stored it). The page has the file.
   */
  async storeFile(dir, name, file) {
    const data = new Uint8Array(await file.arrayBuffer());
    return this.request("storeFile", { dir, name, data }, [data.buffer]);
  }

  /** Fetch variable `name` of `dir`: `{name, size, data}` (`data` a Uint8Array, for the page to save). */
  fetchFile(dir, name) { return this.request("fetchFile", { dir, name }); }

  /** Whether a saved state exists for `model`. */
  async hasState(model) {
    try {
      return Boolean(await dbGet(model));
    } catch {
      return false;
    }
  }

  /** Save into the model's IndexedDB slot; resolves to a status message. */
  async saveState(model) {
    const { state, cycles } = await this.request("saveState");
    await dbPut(model, { state, saved: Date.now(), cycles });
    return `state saved ${new Date().toLocaleTimeString()}`;
  }

  /**
   * Forget `model`'s ROM, or every ROM, and the saved states that hold a
   * copy of it (`ROM_HOLDING_STATES`), the user's and the auto-saved one
   * (the Worker has stopped keeping it when it replies).
   */
  async forgetRom(model = null) {
    const r = await super.forgetRom(model);
    for (const m of ROM_HOLDING_STATES) {
      if (model === null || model === m) {
        await dbDelete(m);
        await dbDelete(autoKey(m));
      }
    }
    return r;
  }

  /** Load the model's IndexedDB slot; resolves to a status message. */
  async loadState(model) {
    const rec = await dbGet(model);
    if (!rec) return "no saved state for this model";
    await this.request("loadState", { state: rec.state });
    return `state from ${new Date(rec.saved).toLocaleString()} loaded`;
  }
}

// ------------------------------------------------------------ Tauri

/**
 * The Tauri app's native core: commands through `invoke("command")`,
 * events on the `saturnus` event (`window.__TAURI__`, withGlobalTauri).
 */
export class TauriBackend extends Backend {
  constructor(tauri) {
    super();
    this.host = "tauri";
    /** The ROM and states come from native file dialogs. */
    this.romSource = "dialog";
    this.tauri = tauri;
    /**
     * Messages are numbered within this page's session: the app runs each
     * invoke as its own task and puts them back in this order, so a keyUp
     * never overtakes its keyDown.
     */
    this.session = crypto.randomUUID();
    this.seq = 0;
    this.listening = tauri.event.listen("saturnus", (e) => this.dispatch(e.payload));
  }

  async request(cmd, args = {}) {
    // Numbered when called, in call order, before any await.
    const msg = { v: PROTOCOL, cmd, ...args, session: this.session, seq: this.seq++ };
    await this.listening;
    return this.tauri.core.invoke("command", { msg });
  }

  send(cmd, args = {}) {
    this.request(cmd, args).catch((err) => this.dispatch({ type: "error", message: String(err) }));
  }

  /** Boot from a ROM the app asks for in a file dialog; `null` if cancelled. */
  boot({ model }) {
    return this.request("boot", { model });
  }

  /** Choose `model`'s ROM in the app's file dialog; `null` if cancelled. */
  chooseRom(model) {
    return this.request("chooseRom", { model });
  }

  /**
   * Download `model`'s ROM from hpcalc.org after the app's confirmation
   * (it keeps the file in its data folder and boots it); `null` if
   * cancelled.
   */
  downloadRom(model) {
    return this.request("downloadRom", { model });
  }

  /**
   * Store a file as variable `name` in `dir`: the File `file` (dropped on
   * the page; its bytes travel as base64), or without one a file the app
   * asks for in a dialog (named after it); `null` if cancelled.
   */
  async storeFile(dir, name, file) {
    if (!file) return this.request("storeFile", { dir });
    const data = base64(new Uint8Array(await file.arrayBuffer()));
    return this.request("storeFile", { dir, name, data });
  }

  /** Fetch variable `name` of `dir` into a file the app asks for: `{name, size, file}`, `null` if cancelled. */
  fetchFile(dir, name) { return this.request("fetchFile", { dir, name }); }

  /** States are files: loading is always offered. */
  async hasState() {
    return true;
  }

  async saveState() {
    const r = await this.request("saveState");
    return r ? `state saved to ${r.path}` : "";
  }

  async loadState() {
    const r = await this.request("loadState");
    return r ? `state from ${r.path} loaded` : "";
  }
}

/** The backend for this host. */
export function createBackend() {
  return window.__TAURI__ ? new TauriBackend(window.__TAURI__) : new WorkerBackend();
}
