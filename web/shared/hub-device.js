// The emulator's own calculator as a device (web/shared/device.js), for
// saturnus tx (iteration 47; kb/docs/saturnus-tx.md, "Devices"): the live
// calculator of a kept ROM, shared with the emulator page through the
// calculators' hub (web/hub.js), worked through the engine's memory
// commands (web/protocol.md, "The user memory"): `memoryTree`,
// `fetchFile`, `storeFile`, `rename`, `purge`, `createDir`, `backup`,
// `watchMemory`. They run as hidden transactions inside the engine, in
// emulated time, between the emulator page's keys and writes; nothing is
// typed on the calculator, its serial port is not used, and a change made
// on it in the emulator shows here at once (`memoryChanged`).
//
// It takes any object with `request(cmd, args)` and the protocol's
// events as DOM events (`memoryChanged`, `gone`): saturnus tx's HubClient
// (web/shared/calc-hub.js), or the emulator page's own backend in a
// merged app. Besides the device interface it has the parts of
// KermitDevice's surface the transfer page uses (`connection`, `getTree`,
// `putTree`, `backup`, `busy` and `progress` events, `cancelQueued`,
// `refresh`, `disconnect`, `close`). One operation at a time, in a queue
// as KermitDevice's. No free memory (the memory tree has none), no
// restore, no graphics screen and no fast transfer (owner, 2026-10-14).
//
// Events: `change` (detail `{path: null}`: everything, after every
// `memoryChanged`), `busy` (`{label, queued}`), `progress` (`{label,
// done, total}`), `lost` (why: the hub dropped the calculator).

import { HOME } from "./device.js";
import "./models.js";

/** How often, and how far apart, a memory read refused by a write or send in progress (the emulator's) is tried again. */
export const READ_TRIES = 40;
export const READ_RETRY_MS = 250;

/** Whether the engine refused because a write or a send runs. */
const inProgress = (err) => /is in progress \(releaseAll stops it\)/.test(String(err?.message ?? err));

/** The largest file `storeFile` takes (the engine's cap). */
export const MAX_STORE_BYTES = 512 * 1024;

/** A failure in tx's terms (as web/tx/protocol.js's TxFailure): `kind`, the message, `varName` and the kind's fields. */
export class HubFailure extends Error {
  constructor({ kind = "calculator", message = "failed", name = null, ...rest } = {}) {
    super(message);
    this.kind = kind;
    this.varName = name;
    for (const key of ["count", "names", "needed", "free"]) {
      if (rest[key] !== undefined) this[key] = rest[key];
    }
  }
}

/**
 * The engine's refusal `message` in tx's words: the calculator is busy
 * in the emulator (a command line open, typing, another transfer), set
 * up for infrared, too large a file; anything else as it is.
 */
export function refusalText(message) {
  const m = String(message ?? "");
  if (/command line is open/.test(m)) return "The calculator is busy in the emulator: finish or cancel the command line there.";
  if (/typing is in progress/.test(m)) return "The calculator is busy in the emulator: it is typing. Try again when it is done.";
  if (/transfer is in progress|already in progress/.test(m)) return "The calculator is busy with another transfer in the emulator. Try again when it is done.";
  if (/flag -33 is set/.test(m)) return "Flag -33 is set on the calculator (I/O over infrared): clear it in the emulator, then try again.";
  if (/larger than \d+ bytes/.test(m)) return `The file is larger than ${MAX_STORE_BYTES / 1024} KB, the most the emulator's calculator takes.`;
  if (/^the CPU is halted/.test(m)) return `The calculator in the emulator has stopped (${m.replace(/^the CPU is halted: /, "")}): reset it there.`;
  if (m === "no ROM loaded") return "The emulator's calculator is not running.";
  return m;
}

/** A failed request's error as a HubFailure in tx's words. */
function failure(err) {
  if (err instanceof HubFailure) return err;
  const message = String(err?.message ?? err);
  const room = /^a backup needs ([\d.]+) bytes free and the calculator has ([\d.]+)/.exec(message);
  if (room) return new HubFailure({ kind: "noRoom", message, needed: Number(room[1]), free: Number(room[2]) });
  if (message === "closed") return new HubFailure({ kind: "closed", message });
  return new HubFailure({ kind: "calculator", message: refusalText(message) });
}

/** The variables of `path` (`["HOME", ...]`) in the memory tree's `variables`, or null. */
export function directoryOf(variables, path) {
  let vars = variables;
  for (const name of path.slice(1)) {
    vars = vars?.find((v) => v.name === name)?.variables ?? null;
    if (!vars) return null;
  }
  return vars;
}

/** A tree variable as a listing's entry. */
const entryOf = (v) => ({ name: v.name, kind: v.type, size: v.size, checksum: v.checksum, directory: Array.isArray(v.variables) });

/** How many variables `vars` holds at every level (its directories count too). */
export function countAll(vars) {
  return (vars ?? []).reduce((n, v) => n + 1 + (v.variables ? countAll(v.variables) : 0), 0);
}

export class HubDevice extends EventTarget {
  /**
   * `calc` the hub's connection (`request`, events); `helpers()` the
   * transfer page's name and file helpers (saturnus-tx-web: `file_name`,
   * `folder_name`, `var_name`, `dir_name`, `file_kind`, `name_problem`).
   */
  constructor(calc, { title = "Calculator", helpers }) {
    super();
    this.calc = calc;
    this.title = title;
    this.helpers = helpers;
    this.emulated = true;
    this.model = null;
    this.conn = null;
    /** Never: no restore, no fast transfer here (KermitDevice's states the page asks about). */
    this.restarted = false;
    this.leftServer = false;
    this.queue = [];
    this.running = null;
    /** The memory tree, read once per `memoryChanged`. */
    this.treeRead = null;
    calc.addEventListener("memoryChanged", () => this.refresh(null));
    calc.addEventListener("gone", (e) => {
      this.conn = null;
      this.dispatchEvent(new CustomEvent("lost", { detail: e.detail }));
    });
  }

  /** The calculator (`{model, settings: null, hub: true}`) once connected, else null. */
  get connection() { return this.conn; }

  /** How many operations wait behind the running one. */
  get queued() { return this.queue.length; }

  get home() { return HOME; }

  busyEvent() {
    this.dispatchEvent(new CustomEvent("busy", { detail: { label: this.running?.label ?? null, queued: this.queue.length } }));
  }

  /** The running operation's progress: `done` bytes of `total`. */
  progress(done, total = null) {
    const op = this.running;
    if (!op) return;
    op.done = done;
    if (total !== null) op.total = total;
    this.dispatchEvent(new CustomEvent("progress", { detail: { label: op.label, done, total: op.total ?? null } }));
  }

  /** Run `fn` when the operations before it are done; `label` says what it is, `tag` names it for `cancelQueued`. */
  run(label, fn, { total = null, tag = null } = {}) {
    return new Promise((resolve, reject) => {
      this.queue.push({ label, fn, resolve, reject, total, tag });
      this.busyEvent();
      this.pump();
    });
  }

  async pump() {
    if (this.running || !this.queue.length) return;
    const op = this.queue.shift();
    this.running = op;
    this.busyEvent();
    try {
      op.resolve(await op.fn());
    } catch (err) {
      op.reject(failure(err));
    } finally {
      this.running = null;
      // A change heard while operations ran is told once they are done:
      // the memory is read between them, never inside one of this page's.
      if (!this.queue.length && this.refreshOwed) {
        this.refreshOwed = false;
        this.refresh(null);
      }
      this.busyEvent();
      this.pump();
    }
  }

  /** Drop the operations not started yet (each rejects as `cancelled`); with `tag`, only those of that tag. */
  cancelQueued(tag = null) {
    const dropped = this.queue.filter((op) => tag === null || op.tag === tag);
    this.queue = this.queue.filter((op) => !dropped.includes(op));
    for (const op of dropped) op.reject(new HubFailure({ kind: "cancelled", message: "cancelled" }));
    this.busyEvent();
    return dropped.length;
  }

  /** A command to the calculator; refusals come back in tx's words. */
  async request(cmd, args = {}) {
    try {
      return await this.calc.request(cmd, args);
    } catch (err) {
      throw failure(err);
    }
  }

  /**
   * Show the emulator's calculator of `model` (the kept ROM `rom`, its
   * file name `romName`): the one running, or booted as it was left;
   * then watch its memory. Nothing is typed on it. Resolves to the
   * connection.
   */
  connect({ model, rom, romName = "" }) {
    return this.run("Joining the emulator's calculator…", async () => {
      await this.request("hello");
      const booted = await this.request("attach", { model, rom, romName });
      const got = booted?.model ?? model;
      if (!globalThis.SATURNUS_SERIAL.includes(got)) {
        throw new HubFailure({ message: `The ${globalThis.SATURNUS_MODEL_TITLES?.[got] ?? got} has no user memory to show here.` });
      }
      const watch = await this.request("watchMemory", { on: true });
      if (watch?.supported === false) throw new HubFailure({ message: watch.reason ?? "The calculator's memory cannot be read." });
      this.model = got;
      this.treeRead = null;
      this.conn = { model: got, settings: null, hub: true };
      return this.conn;
    });
  }

  /** The memory tree (`{path, variables}`): kept until the memory changes; a read refused while the emulator writes or types is tried again. */
  tree() {
    if (!this.treeRead) {
      const read = async () => {
        for (let i = 1; ; i++) {
          try {
            return await this.calc.request("memoryTree");
          } catch (err) {
            if (!inProgress(err) || i >= READ_TRIES) throw failure(err);
            await new Promise((r) => setTimeout(r, READ_RETRY_MS));
          }
        }
      };
      const p = read();
      this.treeRead = p;
      p.catch(() => {
        if (this.treeRead === p) this.treeRead = null;
      });
    }
    return this.treeRead;
  }

  /** The variables of `path`, or a `notFound` failure. */
  async variables(path) {
    const vars = directoryOf((await this.tree()).variables, path);
    if (!vars) throw new HubFailure({ kind: "notFound", message: `There is no directory ${path.join(" ")} on the calculator.`, name: path.at(-1) });
    return vars;
  }

  /** The listing of `path`: `{path, free: null, entries}` (no free figure: the memory tree has none); read in the queue, between this page's operations. */
  list(path = HOME) {
    return this.run(`Reading ${path[path.length - 1]}…`, () => this.variables(path).then((vars) => ({ path: [...path], free: null, entries: vars.map(entryOf) })));
  }

  /**
   * Forget the memory tree and say so: every listing is read again; while
   * operations run, once they are done. `path` is taken for KermitDevice's
   * sake; the tree is one.
   */
  refresh(path = null) {
    this.treeRead = null;
    if (this.running || this.queue.length) {
      this.refreshOwed = true;
      return;
    }
    this.dispatchEvent(new CustomEvent("change", { detail: { path: path ? [...path] : null } }));
  }

  /** Variable `name` of `path`: `{name, fileName, format, data}`; `text` the calculator's text form. */
  read(path, name, { text = false, size = null, tag = null } = {}) {
    return this.run(`Getting ${name}${text ? " as text" : ""}…`, async () => {
      const data = await this.fetch(path, name, text);
      this.progress(data.length, size ?? data.length);
      const h = await this.helpers();
      const format = text ? "text" : "binary";
      return { name, fileName: h.file_name(name, format), format, data };
    }, { total: size, tag });
  }

  async fetch(path, name, text) {
    const r = await this.request("fetchFile", { dir: path, name, ...(text ? { text: true } : {}) });
    if (!(r?.data instanceof Uint8Array)) throw new HubFailure({ message: `No file came back for ${name}.` });
    return r.data;
  }

  /** Why `name` cannot go into `vars` as a file, or nothing: a directory there, a name in use without `replace`. */
  static checkPut(vars, name, replace) {
    const there = vars.find((v) => v.name === name);
    if (there?.variables) throw new HubFailure({ kind: "isDirectory", message: `${name} is a directory`, name });
    if (there && !replace) throw new HubFailure({ kind: "nameClash", message: `${name} already exists`, name });
    return Boolean(there);
  }

  /** `name` as the calculator takes it, else a `badName` failure. */
  async checkName(name) {
    const h = await this.helpers();
    if (h.name_problem(name)) throw new HubFailure({ kind: "badName", message: `bad name ${name}`, name });
  }

  /** Store `data` as `name` in `path`; resolves to the name stored. */
  async store(path, name, data, { replace = false, asString = false } = {}) {
    await this.checkName(name);
    const h = await this.helpers();
    if (h.file_kind(data) === "other" && !asString) throw new HubFailure({ kind: "notAnObject", message: "not an object" });
    if (data.length > MAX_STORE_BYTES) throw new HubFailure({ message: refusalText(`larger than ${MAX_STORE_BYTES} bytes`) });
    const exists = HubDevice.checkPut(await this.variables(path), name, replace);
    const r = await this.request("storeFile", { dir: path, name, data, replace: exists });
    return r?.name ?? name;
  }

  /**
   * Put `bytes` into variable `name` of `path`; `replace` a variable of
   * that name (`nameClash` otherwise), `asString` store a file that is no
   * object (as a string). Resolves to the name stored.
   */
  write(path, name, bytes, { replace = false, asString = false } = {}) {
    return this.run(`Putting ${name}…`, async () => {
      const stored = await this.store(path, name, bytes, { replace, asString });
      this.refresh(path);
      this.progress(bytes.length, bytes.length);
      return stored;
    }, { total: bytes.length });
  }

  /** Make directory `name` in `path` (`nameClash` for a name in use); resolves to its name. */
  mkdir(path, name) {
    return this.run(`Making ${name}…`, async () => {
      await this.checkName(name);
      if ((await this.variables(path)).some((v) => v.name === name)) throw new HubFailure({ kind: "nameClash", message: `${name} already exists`, name });
      await this.request("createDir", { dir: path, name });
      this.refresh(path);
      return name;
    });
  }

  /** Rename `from` in `path` to `to` (`nameClash` for a name in use); resolves to the new name. */
  rename(path, from, to) {
    return this.run(`Renaming ${from}…`, async () => {
      await this.checkName(to);
      const vars = await this.variables(path);
      if (!vars.some((v) => v.name === from)) throw new HubFailure({ kind: "notFound", message: `${from} is not there`, name: from });
      if (from === to) return to;
      if (vars.some((v) => v.name === to)) throw new HubFailure({ kind: "nameClash", message: `${to} already exists`, name: to });
      await this.request("rename", { dir: path, name: from, to });
      this.refresh(path);
      return to;
    });
  }

  /**
   * Purge `name` of `path`. A directory that holds variables fails with
   * `notEmpty` and their `count` until it is purged with that `count`.
   * Resolves to how many variables went with it.
   */
  purge(path, name, { count = null } = {}) {
    return this.run(`Purging ${name}…`, async () => {
      const v = (await this.variables(path)).find((x) => x.name === name);
      if (!v) throw new HubFailure({ kind: "notFound", message: `${name} is not there`, name });
      const n = v.variables ? countAll(v.variables) : 0;
      if (n > 0 && count !== n) throw new HubFailure({ kind: "notEmpty", message: `${name} holds ${n} variables`, name, count: n });
      // The engine's purge is PGDIR for a directory.
      await this.request("purge", { dir: path, name });
      this.refresh(path);
      return n;
    });
  }

  /**
   * Everything in directory `path`, one fetch per variable, as
   * KermitDevice's: `{dir, items: [{path, fsPath, directory, data?}],
   * skipped: [{path, kind}]}`, a directory's variables before its
   * directories are opened. `skip` the kinds to leave out; `format` "text"
   * fetches the text form.
   */
  getTree(path, { format = "binary", skip = [], size = null } = {}) {
    return this.run(`Getting ${path[path.length - 1]}…`, async () => {
      const h = await this.helpers();
      const items = [];
      const skipped = [];
      let done = 0;
      const todo = [[]];
      while (todo.length) {
        const rel = todo.pop();
        const vars = await this.variables([...path, ...rel]);
        const dirs = [];
        for (const v of vars) {
          const at = [...rel, v.name];
          this.running.label = `Getting ${[...path.slice(1), ...at].join("/")}…`;
          this.busyEvent();
          if (v.variables) {
            items.push({ path: at, fsPath: at.map((n) => h.folder_name(n)), directory: true });
            dirs.push(at);
            continue;
          }
          if (skip.includes(v.type)) {
            skipped.push({ path: at, kind: v.type });
            continue;
          }
          const data = await this.fetch([...path, ...rel], v.name, format === "text");
          // A got file is a binary transfer file or text.
          const fmt = h.file_kind(data) === "text" ? "text" : "binary";
          items.push({ path: at, fsPath: [...rel.map((n) => h.folder_name(n)), h.file_name(v.name, fmt)], directory: false, data });
          done += data.length;
          this.progress(done, size);
        }
        todo.push(...dirs.reverse());
      }
      return { dir: [...path], items, skipped };
    }, { total: size });
  }

  /**
   * Put a tree (`items`: `[{path, data?}]`, file-system names: folders
   * and file names) into `path`: as a new directory `name`, or into
   * `path` itself (`name` null). Variables in use fail with `namesInUse`
   * until `replace`; a file that is no object only with `asString`.
   * Resolves to `{directories, variables}`.
   */
  putTree(path, name, items, { replace = false, asString = false } = {}) {
    const total = items.reduce((n, i) => n + (i.data?.length ?? 0), 0);
    return this.run(`Putting ${name ?? path[path.length - 1]}…`, async () => {
      try {
        return await this.storeTree(path, name, items, { replace, asString, total });
      } finally {
        // Part of a tree may be on the calculator: everything is read again.
        this.refresh(null);
      }
    }, { total });
  }

  async storeTree(path, name, items, { replace, asString, total }) {
    const h = await this.helpers();
    if (name !== null) await this.checkName(name);
    // Calculator names: a folder's is the directory's, a file's the variable's.
    const tree = items.map((item) => {
      const last = item.path.length - 1;
      let names;
      try {
        names = item.path.map((p, i) => (i === last && item.data ? h.var_name(p) : h.dir_name(p)));
      } catch {
        throw new HubFailure({ kind: "badName", message: `bad name ${item.path.join("/")}`, name: item.path.at(-1) });
      }
      for (const n of names) if (h.name_problem(n)) throw new HubFailure({ kind: "badName", message: `bad name ${n}`, name: n });
      if (item.data && h.file_kind(item.data) === "other" && !asString) throw new HubFailure({ kind: "notAnObject", message: "not an object" });
      return { path: names, data: item.data ?? null };
    });
    // The directories to have (parents first) and each one's children in the tree's order.
    const key = (p) => p.join("/");
    const dirs = [];
    const children = new Map([["", []]]);
    const addDir = (p) => {
      for (let n = 1; n <= p.length; n++) {
        const k = key(p.slice(0, n));
        if (children.has(k)) continue;
        children.set(k, []);
        children.get(key(p.slice(0, n - 1))).push({ name: p[n - 1] });
        dirs.push(p.slice(0, n));
      }
    };
    const seen = new Set();
    for (const t of tree) {
      if (!t.path.length || seen.has(key(t.path))) throw new HubFailure({ kind: "duplicate", message: `${t.path.join("/")} comes twice`, name: t.path.join("/") });
      seen.add(key(t.path));
      addDir(t.path.slice(0, -1));
      if (t.data) children.get(key(t.path.slice(0, -1))).push({ name: t.path.at(-1), data: t.data });
      else addDir(t.path);
    }
    const root = name === null ? [...path] : [...path, name];
    const top = await this.variables(path);
    if (name !== null) {
      if (top.some((v) => v.name === name)) throw new HubFailure({ kind: "nameClash", message: `${name} already exists`, name });
    } else {
      const inUse = [];
      for (const [k, kids] of children) {
        const at = k ? k.split("/") : [];
        const vars = directoryOf(top, ["HOME", ...at]);
        if (!vars) continue;
        for (const kid of kids) {
          const v = vars.find((x) => x.name === kid.name);
          if (!v) continue;
          const shown = [...at, kid.name].join("/");
          if (kid.data && v.variables) throw new HubFailure({ kind: "isDirectory", message: `${shown} is a directory`, name: shown });
          if (!kid.data && !v.variables) throw new HubFailure({ kind: "notADirectory", message: `${shown} is not a directory`, name: shown });
          if (kid.data) inUse.push(shown);
        }
      }
      if (inUse.length && !replace) throw new HubFailure({ kind: "namesInUse", message: `${inUse.length} names in use`, names: inUse });
    }
    let directories = 0;
    let variables = 0;
    let done = 0;
    if (name !== null) {
      this.running.label = `Putting ${[...path.slice(1), name].join("/")}…`;
      this.busyEvent();
      await this.request("createDir", { dir: path, name });
      directories++;
    }
    // Directory by directory, parents first; the calculator puts a new
    // variable first in its directory, so the children go backwards: a
    // tree got and put back keeps its order.
    for (const parent of [[], ...dirs]) {
      const kids = children.get(key(parent)) ?? [];
      if (!kids.length) continue;
      const at = [...root, ...parent];
      this.treeRead = null;
      const vars = name === null || parent.length ? await this.variables(at) : [];
      for (const kid of [...kids].reverse()) {
        this.running.label = `Putting ${[...at.slice(1), kid.name].join("/")}…`;
        this.busyEvent();
        const there = vars.find((v) => v.name === kid.name);
        if (kid.data) {
          await this.request("storeFile", { dir: at, name: kid.name, data: kid.data, replace: Boolean(there) });
          variables++;
          done += kid.data.length;
          this.progress(done, total);
        } else if (!there) {
          await this.request("createDir", { dir: at, name: kid.name });
          directories++;
        }
      }
    }
    return { directories, variables };
  }

  /** A backup of HOME (one transaction in the engine): `{model, data}`; `noRoom` (with `needed`, `free`) when it does not fit. */
  backup() {
    return this.run("Backing up…", async () => {
      const r = await this.request("backup");
      if (!(r?.data instanceof Uint8Array)) throw new HubFailure({ message: "No backup came back." });
      this.progress(r.data.length, r.data.length);
      return { model: this.model, data: r.data };
    });
  }

  /** Stop watching the memory; the calculator stays as it is (nothing to end: no server runs). */
  disconnect() {
    this.cancelQueued();
    return this.run("Disconnecting…", async () => {
      this.conn = null;
      await this.calc.request("watchMemory", { on: false }).catch(() => {});
      return null;
    });
  }

  /** Leave the calculator (the emulator page goes on showing it). */
  close() {
    this.cancelQueued();
    this.conn = null;
    return this.calc.close();
  }
}
