// A calculator in Kermit server mode on any link, as a Device
// (web/shared/device.js): connect, list, read (get), write (put), mkdir,
// rename, purge, a directory's tree got and put, the memory, backup,
// restore, the graphics screen, the fast transfer (XModem, the 48GX and
// 49G), disconnect, over the protocol Worker
// (protocol.js). One operation at a
// time, as the link is half duplex; more wait in a queue (`queued`), and
// those not started yet can be cancelled. Listings are kept until the
// page changes that directory or asks again (`refresh`); a change made on
// the calculator's keyboard is not seen until then (the server is busy
// serving).
//
// Events: `change` (`{path}`; no path: everything), `busy` (`{label,
// queued}`, label null when idle), `progress` (`{label, done, total}`),
// `search` (the speed search: `{type: "listening" | "found", settings}`),
// `connection` (the calculator or null), `lost` (why).
//
// An emulated calculator's server can end behind the page's back (ATTN
// in the emulator, which shows the same calculator). With `recover` (the
// emulated calculator's `startServer`, gentle: it presses no key and types
// nothing into a calculator in use), an operation that hears nothing from
// it for `silentMs` is stopped, the server started again and the session
// synced (`sync`: late packets of the stopped one skipped); then a read
// runs once more, and a write (put, mkdir, rename, purge, a tree put) is
// never run again: its directory is read again and it fails, saying so.
// `suspectServer` (ATTN pressed in the emulator) has the server checked
// before the next operation. A calculator on a cable has no `recover`:
// its Kermit timeouts and retries end a request.

import { HOME, pathKey } from "../shared/device.js";
import { TxFailure } from "./protocol.js";

/** An emulated calculator's longest silence in an operation before its server is checked. */
export const SILENT_MS = 15_000;

/** Operations whose silence is no stall: the speed search, a restore's restart, the fast transfer's wait for the user. */
const UNWATCHED = new Set(["connect", "restore", "xmodem"]);

/** What a write stopped by a server that ended says: it is never run again on its own. */
export const WRITE_STOPPED = "The calculator's server stopped during the operation; check the list and try again.";

export class KermitDevice extends EventTarget {
  /**
   * `client` a ProtocolClient; `title` the place's name ("HP 48GX on USB
   * serial port"); `recover()` starts an emulated calculator's server
   * again, resolving to whether SERVER was typed (null: a real calculator,
   * nothing to recover); `silentMs` as `SILENT_MS`; `timers` and `now`
   * the clock (tests).
   */
  constructor(client, { title = "Calculator", emulated = false, recover = null, silentMs = SILENT_MS, timers = globalThis, now = () => Date.now() } = {}) {
    super();
    this.client = client;
    this.title = title;
    this.emulated = emulated;
    this.recover = recover;
    this.silentMs = silentMs;
    this.timers = timers;
    this.now = now;
    /** The server may have ended (keys pressed elsewhere, a failed recovery): checked before the next operation. */
    this.suspect = false;
    /** Counts `suspectServer` calls: a check clears `suspect` only when none came while it ran. */
    this.suspicions = 0;
    /** When the link last carried a byte from the calculator. */
    this.heard = now();
    client.link?.addEventListener?.("data", () => {
      this.heard = this.now();
    });
    this.listings = new Map();
    this.queue = [];
    this.running = null;
    this.lost = null;
    /** A restore restarted the calculator: not connected until `connect` again. */
    this.restarted = false;
    /** A fast transfer ended server mode: not connected until `connect` again. */
    this.leftServer = false;
    this.forward = (type) => (e) => this.dispatchEvent(new CustomEvent(type, { detail: e.detail }));
    client.addEventListener("search", this.forward("search"));
    client.addEventListener("connection", this.forward("connection"));
    client.addEventListener("lost", (e) => {
      this.lost = e.detail;
      this.dispatchEvent(new CustomEvent("lost", { detail: e.detail }));
    });
    client.addEventListener("progress", (e) => this.progress(e.detail));
    // A tree's next variable: the status names it.
    client.addEventListener("item", (e) => {
      const op = this.running;
      const path = e.detail?.path;
      if (!op || !Array.isArray(path)) return;
      op.label = `${op.verb ?? "Getting"} ${path.slice(1).join("/") || path.join("/")}…`;
      this.busyEvent();
    });
  }

  /** The connected calculator (`{model, settings, version, userMode}`) or null. */
  get connection() { return this.client.connection; }

  /** How many operations wait behind the running one. */
  get queued() { return this.queue.length; }

  busyEvent() {
    this.dispatchEvent(new CustomEvent("busy", { detail: { label: this.running?.label ?? null, queued: this.queue.length } }));
  }

  /** Progress of the running operation: bytes so far of the expected total. */
  progress(e) {
    const op = this.running;
    if (!op) return;
    if (e.total !== undefined) {
      op.done = e.sent;
      op.total = e.total;
    } else if (e.received !== undefined) {
      op.done = (op.done ?? 0) + e.received;
    } else if (e.done !== undefined) {
      op.done = e.done;
    }
    this.dispatchEvent(new CustomEvent("progress", { detail: { label: op.label, done: op.done ?? 0, total: op.total ?? null } }));
  }

  /**
   * Run `fn` when the operations before it are done; `label` says what it
   * is ("Getting PRG…"); `kind` "read" for one that only reads (run again
   * after a restarted server), "connect", "restore" or "xmodem" for one
   * that may be silent, "disconnect"; anything else is a write, never run
   * again; `path` the directory it changes.
   */
  run(label, fn, { total = null, tag = null, verb = null, kind = null, path = null } = {}) {
    return new Promise((resolve, reject) => {
      this.queue.push({ label, fn, resolve, reject, total, tag, verb, kind, path });
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
      op.resolve(await this.attempt(op));
    } catch (err) {
      op.reject(err);
    } finally {
      this.running = null;
      this.busyEvent();
      this.pump();
    }
  }

  /** ATTN was pressed on the emulated calculator elsewhere: its server is checked before the next operation. */
  suspectServer() {
    if (!this.recover) return;
    this.suspect = true;
    this.suspicions++;
  }

  /**
   * Run `op`; an emulated calculator's operation is watched: silent too
   * long, it is stopped and the server started again; a read then runs
   * once more (a second silence fails it), a write fails.
   */
  async attempt(op) {
    if (!this.recover || UNWATCHED.has(op.kind) || !this.client.connection) return op.fn();
    if (op.kind === "disconnect") {
      try {
        return await this.watched(op.fn);
      } catch (err) {
        // Server mode is what a disconnect ends: no server, nothing to end.
        if (err?.stalled) return null;
        throw err;
      }
    }
    if (this.suspect) await this.restartServer();
    try {
      return await this.watched(op.fn);
    } catch (err) {
      if (!err?.stalled) throw err;
    }
    await this.restartServer({ stalled: true });
    if (op.kind !== "read") {
      // A write may have been done, or half done: never sent again on its own.
      this.refresh(op.path);
      throw new TxFailure({ kind: "calculator", message: WRITE_STOPPED });
    }
    try {
      return await this.watched(op.fn);
    } catch (err) {
      if (!err?.stalled) throw err;
      this.suspectServer();
      throw new TxFailure({ kind: "calculator", message: "The emulated calculator does not answer, though its server was started again." });
    }
  }

  /** Run `fn`, stopped (rejecting with `stalled`) once the calculator was silent for `silentMs`. */
  async watched(fn) {
    this.heard = this.now();
    let stopped = false;
    const timer = this.timers.setInterval(() => {
      if (this.now() - this.heard < this.silentMs) return;
      stopped = true;
      this.timers.clearInterval(timer);
      this.client.abort();
    }, Math.max(this.silentMs / 4, 10));
    try {
      return await fn();
    } catch (err) {
      // Only the abort's own rejection is the stall; a failure that came first is the operation's.
      if (stopped && err?.kind === "aborted") throw Object.assign(new Error("stalled"), { stalled: true });
      throw err;
    } finally {
      this.timers.clearInterval(timer);
    }
  }

  /**
   * The emulated calculator's server started again (asked first whether
   * it runs; nothing pressed or typed into a calculator in use), then the
   * session synced when SERVER was typed or an operation was stopped (the
   * stray answers skipped); when SERVER was typed, every listing is read
   * again (the memory may have changed meanwhile). The connection stays:
   * the calculator's flag -35 is still the session's (a connect would
   * take it for the user's).
   */
  async restartServer({ stalled = false } = {}) {
    const asked = this.suspicions;
    let typed;
    try {
      typed = await this.recover();
      if (typed || stalled) await this.client.request({ type: "sync" });
    } catch (err) {
      this.suspectServer();
      throw new TxFailure({ kind: "calculator", message: `The emulated calculator left server mode, and its server did not start again: ${err?.message ?? err}` });
    }
    // ATTN pressed again while it was checked: checked again before the next operation.
    if (this.suspicions === asked) this.suspect = false;
    if (typed) this.refresh(null);
  }

  /** Drop the operations not started yet (each rejects as `cancelled`); with `tag`, only those of that tag. */
  cancelQueued(tag = null) {
    const dropped = this.queue.filter((op) => tag === null || op.tag === tag);
    this.queue = this.queue.filter((op) => !dropped.includes(op));
    for (const op of dropped) op.reject(new TxFailure({ kind: "cancelled", message: "cancelled" }));
    this.busyEvent();
    return dropped.length;
  }

  /** Find the calculator and connect: `{find, baud, parity}`. */
  connect(options = {}) {
    return this.run("Looking for the calculator…", async () => {
      const r = await this.client.request({ type: "connect", find: options.find ?? true, baud: options.baud ?? 9600, parity: options.parity ?? "none" });
      this.restarted = false;
      this.leftServer = false;
      this.suspect = false;
      this.listings.clear();
      return r.connection;
    }, { kind: "connect" });
  }

  /** The listing of `path`: kept, or read (once, however many ask). */
  list(path = HOME) {
    const key = pathKey(path);
    if (!this.listings.has(key)) {
      const p = this.run(`Reading ${path[path.length - 1]}…`, () => this.client.request({ type: "list", dir: path }), { kind: "read" });
      this.listings.set(key, p);
      p.catch(() => {
        if (this.listings.get(key) === p) this.listings.delete(key);
      });
    }
    return this.listings.get(key);
  }

  /** Forget the listing of `path` (none: all) and say so. */
  refresh(path = null) {
    if (path) this.listings.delete(pathKey(path));
    else this.listings.clear();
    this.dispatchEvent(new CustomEvent("change", { detail: { path: path ? [...path] : null } }));
  }

  /** Variable `name` of `path`: `{name, fileName, format, data}`; `text` asks for the calculator's text form. `size` (bytes) for the progress; `tag` names it for `cancelQueued`. */
  read(path, name, { text = false, size = null, tag = null } = {}) {
    const label = `Getting ${name}${text ? " as text" : ""}…`;
    return this.run(label, async () => {
      const r = await this.client.request({ type: "get", dir: path, name, format: text ? "text" : "binary" });
      return { name: r.name, fileName: r.fileName, format: r.format, data: r.data };
    }, { total: size, tag, kind: "read" });
  }

  /**
   * Put `bytes` into variable `name` of `path`; `replace` a variable of
   * that name, `asString` store a file that is no object as a string.
   * Resolves to the name stored; the directory's listing is read again.
   */
  write(path, name, bytes, { replace = false, asString = false } = {}) {
    return this.run(`Putting ${name}…`, async () => {
      const r = await this.client.request({ type: "put", dir: path, name, replace, asString }, bytes);
      this.refresh(path);
      return r.name;
    }, { total: bytes.length, path });
  }

  /** Make directory `name` in `path`; resolves to its name. */
  mkdir(path, name) {
    return this.run(`Making ${name}…`, async () => {
      const r = await this.client.request({ type: "mkdir", dir: path, name });
      this.refresh(path);
      return r.name;
    }, { path });
  }

  /** Rename `from` in `path` to `to` (`nameClash` for a name in use); resolves to the new name. */
  rename(path, from, to) {
    return this.run(`Renaming ${from}…`, async () => {
      const r = await this.client.request({ type: "rename", dir: path, from, to });
      this.refresh(path);
      return r.to;
    }, { path });
  }

  /**
   * Purge `name` of `path`. A directory that holds variables fails with
   * `notEmpty` and their `count` until it is purged with that `count`
   * (after the question). Resolves to how many variables went with it.
   */
  purge(path, name, { count = null } = {}) {
    return this.run(`Purging ${name}…`, async () => {
      const r = await this.client.request({ type: "purge", dir: path, name, count });
      this.refresh(path);
      return r.count;
    }, { path });
  }

  /**
   * Everything in directory `path` (`["HOME"]`: all of it), one get per
   * variable: `{dir, items: [{path, fsPath, directory, data?}], skipped}`.
   * `skip` the kinds to leave out; `size` the expected bytes, for the progress.
   */
  getTree(path, { format = "binary", skip = [], size = null } = {}) {
    return this.run(`Getting ${path[path.length - 1]}…`, async () => {
      const r = await this.client.request({ type: "get-tree", dir: path, format, skip });
      return r;
    }, { total: size, kind: "read" });
  }

  /**
   * Put a tree (`items`: `[{path, data?}]`, file-system names) into
   * `path`: as a new directory `name`, or into `path` itself (`name`
   * null). Names in use fail with `namesInUse` until `replace`; a file
   * that is no object only with `asString`. Resolves to `{directories,
   * variables}`.
   */
  putTree(path, name, items, { replace = false, asString = false } = {}) {
    const total = items.reduce((n, i) => n + (i.data?.length ?? 0), 0);
    return this.run(`Putting ${name ?? path[path.length - 1]}…`, async () => {
      try {
        return await this.client.request({ type: "put-tree", dir: path, name, replace, asString }, items);
      } finally {
        // A tree put that stopped part of the way changed things too.
        this.refresh(null);
      }
    }, { total, verb: "Putting" });
  }

  /** The free memory and what a backup needs: `{free, home, needed, fits}`. */
  memory() {
    return this.run("Checking the free memory…", () => this.client.request({ type: "memory" }), { kind: "read" });
  }

  /** A backup of HOME: `{model, free, home, needed, data}`; `noRoom` (with `needed`, `free`) when it does not fit. */
  backup() {
    return this.run("Backing up…", () => this.client.request({ type: "backup" }), { kind: "read" });
  }

  /**
   * Restore `bytes` (a backup or a directory object): the calculator
   * restarts and leaves server mode, so the device is not connected
   * after it (`restarted`) until `connect` again.
   */
  restore(bytes) {
    return this.run("Restoring…", async () => {
      const r = await this.client.request({ type: "restore" }, bytes);
      this.restarted = true;
      this.listings.clear();
      return r;
    }, { total: bytes.length, kind: "restore" });
  }

  /**
   * A fast transfer (XModem; the 48GX and 49G): `bytes` put into variable
   * `name` of `path` (a binary file of the model's family), or `name`
   * got (no `bytes`): `{name, fileName, format, data}`. The calculator
   * leaves server mode first; `onPlan(plan)` gets what the user types
   * (`{direction, name, keys, model, switchedToRpn}`), and the device
   * waits `wait` seconds for it. Afterwards (done or not, once the server
   * ended) the device is not connected (`leftServer`) until `connect`.
   */
  xmodem(path, name, { bytes = null, size = null, wait = undefined, onPlan = () => {} } = {}) {
    const put = bytes !== null;
    const label = put ? `Waiting to send ${name}…` : `Waiting to get ${name}…`;
    return this.run(label, async () => {
      const plan = (e) => {
        const op = this.running;
        if (op) op.label = put ? `Sending ${name} (fast)…` : `Getting ${name} (fast)…`;
        this.busyEvent();
        onPlan(e.detail);
      };
      this.client.addEventListener("xmodem", plan);
      try {
        const r = await this.client.request({ type: put ? "xmodem-put" : "xmodem-get", dir: path, name, wait }, put ? bytes : undefined);
        return put ? { name: r.name } : { name: r.name, fileName: r.fileName, format: r.format, data: r.data };
      } finally {
        this.client.removeEventListener("xmodem", plan);
        if (!this.client.connection) {
          this.leftServer = true;
          this.listings.clear();
        }
      }
    }, { total: put ? bytes.length : size, tag: "xmodem", kind: "xmodem" });
  }

  /** Stop the fast transfer: the running one (it rejects as `aborted`), else the waiting ones (`cancelled`). Nothing else. */
  stopXmodem() {
    if (this.running?.tag === "xmodem") this.client.abort();
    else this.cancelQueued("xmodem");
  }

  /** The graphics screen (PICT) as a GROB file: `{width, height, data}`. */
  pict() {
    return this.run("Getting the graphics screen…", () => this.client.request({ type: "pict" }), { kind: "read" });
  }

  /** Put flag -35 back and end server mode (FINISH). */
  disconnect() {
    this.cancelQueued();
    return this.run("Ending server mode…", () => this.client.request({ type: "disconnect" }), { kind: "disconnect" });
  }

  /** Close the link and the Worker. */
  close() {
    this.cancelQueued();
    return this.client.close();
  }
}
