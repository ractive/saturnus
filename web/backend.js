// The two backends of the page (web/protocol.md): WorkerBackend runs the
// wasm core in a Web Worker, TauriBackend talks to the Tauri app's native
// core. Same interface; nothing else in the page knows which one it has.
// Each is an EventTarget that dispatches the protocol's events (`frame`,
// `keys`, `status`, `error`, `memoryChanged`) as CustomEvents with the message as `detail`.

const PROTOCOL = 1;
const DB_NAME = "saturnus";
const DB_STORE = "states";

// ------------------------------------------------------------ IndexedDB

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(DB_STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function dbGet(key) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const req = db.transaction(DB_STORE, "readonly").objectStore(DB_STORE).get(key);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  }).finally(() => db.close());
}

async function dbPut(key, value) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(DB_STORE, "readwrite");
    tx.objectStore(DB_STORE).put(value, key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  }).finally(() => db.close());
}

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
  /** The plain key grid of `model`. */
  layout(model) { return this.cached("layout", model); }
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
}

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
