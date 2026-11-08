// An HP 100LX or 200LX palmtop on a cable, its Filer in server mode
// (saturnus-tx's palmtop.rs through the protocol Worker, protocol.js), as
// a pane's place (kb/docs/saturnus-tx.md, "The HP 100LX and 200LX"). It
// is a file system, so it has a Folder's shape (folder.js): the pane and
// the copies (transfers.js) list, read, write, rename, delete and make
// directories on it as on a folder, and copy trees to and from it. And it
// is on a link, so it has a calculator device's life too (kermit-device.js):
// connect, the busy and progress events, Stop, not answering, disconnect.
//
//   home                             ["Palmtop"]; its listing is the drives (C:, A:, D:)
//   list(path)                       -> {path, entries: [{name, kind, directory, size,
//                                       checksum: null, modified, readOnly}]}
//   read, head, write, existing, mkdir, rename, count, purge, files, zip
//                                    as a Folder's
//   connect({baud}), disconnect(), close(), stop()
//
// Names are DOS names: 8.3, upper case, case-insensitive (a write of
// `notes.txt` replaces NOTES.TXT, after the question). A name the palmtop
// cannot make (longer than 8.3, a space) is refused before anything is
// sent, with the reason (saturnus-tx's `dos_name`). One operation at a
// time, as the link is half duplex; listings are kept until something
// changes that directory or the pane reads again.
//
// Events: `change` (`{path}`; no path: everything), `busy` (`{label,
// queued}`), `progress` (`{label, done, total}`), `connection`,
// `notAnswering`, `lost` (why the link went away).

import { pathKey, sortEntries } from "../shared/device.js";
import { FolderError } from "./folder.js";
import { zipParts } from "./zip.js";

/** The palmtop's top directory, whose listing is its drives. */
export const PALMTOP_HOME = "Palmtop";

/** The speed Files and the palmtop use unless the user fixed another: the stock UART's safe limit (wiki: hardware/hp100lx-200lx). */
export const PALMTOP_BAUD = 19200;

/** The speeds the Filer's Remote Settings offer, the default first. */
export const PALMTOP_SPEEDS = [19200, 38400, 57600, 115200, 9600, 4800, 2400, 1200];

/** The most the zip download of a palmtop's files reads (its disks hold far less). */
const MAX_ZIP_BYTES = 64 * 1024 * 1024;

/** A DOS time (`{year, month, day, hour, minute, second}`) as the palmtop's local time in ms, or null. */
export function dosTime(t) {
  if (!t) return null;
  return new Date(t.year, t.month - 1, t.day, t.hour, t.minute, t.second).getTime();
}

/** What the list's Type column says of an entry: "Drive", "Folder", "File", with "read-only", "hidden", "system". */
export function entryKind(e) {
  if (e.drive) return "Drive";
  const base = e.directory ? "Folder" : "File";
  const flags = [e.readOnly && "read-only", e.hidden && "hidden", e.system && "system"].filter(Boolean);
  return flags.length ? `${base}, ${flags.join(", ")}` : base;
}

/** `text` with a capital first letter and a full stop. */
const sentence = (text) => {
  const t = String(text ?? "");
  const s = t.charAt(0).toUpperCase() + t.slice(1);
  return /[.!?]$/.test(s) ? s : `${s}.`;
};

/** A failed palmtop request as a FolderError (its words are the palmtop's, not a calculator's). */
export function palmtopError(err) {
  if (err instanceof FolderError) return err;
  const kind = err?.kind ?? "failed";
  const message = String(err?.message ?? err ?? "failed");
  switch (kind) {
    case "stopped":
    case "cancelled":
    case "aborted":
      return new FolderError("cancelled", "Stopped.", { cause: err });
    case "closed":
    case "notConnected":
      return new FolderError("closed", "Not connected to the palmtop.", { cause: err });
    case "notAnswering":
      return new FolderError("notAnswering", "The palmtop stopped answering. Check that Filer is open in front with server mode on, then reconnect.", { cause: err });
    case "noAnswer":
      return new FolderError("noAnswer", noAnswerText(err?.baud), { cause: err });
    case "exists":
      return new FolderError("exists", sentence(message), { name: err.varName, cause: err });
    default:
      return new FolderError(kind, sentence(message), { name: err?.varName ?? null, cause: err });
  }
}

/** What a connect that nothing answered says: what to check, and 19200 when another speed was tried. */
export function noAnswerText(baud = PALMTOP_BAUD) {
  const at = Number(baud) || PALMTOP_BAUD;
  const base = `No palmtop answered at ${at} baud. Is Filer open in front, with server mode on (MENU, Communications, Remote Settings), and the cable plugged in at both ends?`;
  return at === PALMTOP_BAUD ? base : `${base} The palmtop's speed must be ${at} too; 19200 is the safe one: set it on both sides.`;
}

export class PalmtopFolder extends EventTarget {
  /**
   * `client` a ProtocolClient on the palmtop's link (kind "palmtop");
   * `helpers()` the page's wasm (`palmtop_name_problem`); `title` the
   * top directory's name.
   */
  constructor(client, { helpers, title = PALMTOP_HOME } = {}) {
    super();
    this.client = client;
    this.helpers = helpers;
    this.title = title;
    this.kind = "palmtop";
    this.listings = new Map();
    this.queue = [];
    this.running = null;
    this.lost = null;
    /** The palmtop stopped answering: not connected until `connect` again. */
    this.notAnswering = false;
    /** Never true: the calculators' restore and fast transfer have no palmtop counterpart. */
    this.restarted = false;
    this.leftServer = false;
    client.addEventListener("connection", (e) => this.dispatchEvent(new CustomEvent("connection", { detail: e.detail })));
    client.addEventListener("lost", (e) => {
      this.lost = e.detail;
      this.dispatchEvent(new CustomEvent("lost", { detail: e.detail }));
    });
    client.addEventListener("progress", (e) => this.progress(e.detail));
  }

  /** The top directory as a path. */
  get home() { return [this.title]; }

  /** Its files stay on the palmtop. */
  get kept() { return true; }

  /** The connected palmtop (`{model: "palmtop", settings, revision}`) or null. */
  get connection() { return this.client.connection; }

  /** How many operations wait behind the running one. */
  get queued() { return this.queue.length; }

  /** Always granted: the palmtop asks nothing. */
  async permission() { return "granted"; }

  busyEvent() {
    this.dispatchEvent(new CustomEvent("busy", { detail: { label: this.running?.label ?? null, queued: this.queue.length } }));
  }

  progress(e) {
    const op = this.running;
    if (!op) return;
    if (e.total !== undefined) {
      op.done = e.sent;
      op.total = e.total;
    } else if (e.done !== undefined) {
      op.done = e.done;
    }
    this.dispatchEvent(new CustomEvent("progress", { detail: { label: op.label, done: op.done ?? 0, total: op.total ?? null } }));
  }

  /** Run `fn` when the operations before it are done; `label` says what it is. */
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
      if ((this.client.notAnswering || err?.kind === "notAnswering") && !this.notAnswering) {
        this.notAnswering = true;
        this.listings.clear();
        this.cancelQueued();
        this.dispatchEvent(new CustomEvent("notAnswering"));
      }
      op.reject(palmtopError(err));
    } finally {
      this.running = null;
      this.busyEvent();
      this.pump();
    }
  }

  /** Drop the operations not started yet (each rejects as cancelled); with `tag`, only those of that tag. */
  cancelQueued(tag = null) {
    const dropped = this.queue.filter((op) => tag === null || op.tag === tag);
    this.queue = this.queue.filter((op) => !dropped.includes(op));
    for (const op of dropped) op.reject(new FolderError("cancelled", "Stopped."));
    this.busyEvent();
    return dropped.length;
  }

  /** Stop the running operation at the palmtop's next reply: a file being read is closed, one being written deleted. */
  stop() {
    if (this.running) this.client.stop();
  }

  /** The directory names below the top (`["C:", "_DAT"]`). */
  dirOf(path) {
    if (!Array.isArray(path) || path[0] !== this.title) throw new FolderError("missing", `${Array.isArray(path) ? path.join("/") : path} is not on the palmtop.`);
    return path.slice(1);
  }

  /** Connect at `baud` (19200 by default). */
  connect({ baud = PALMTOP_BAUD } = {}) {
    return this.run("Connecting to the palmtop…", async () => {
      const r = await this.client.request({ type: "connect", baud });
      this.notAnswering = false;
      this.listings.clear();
      return r.connection;
    });
  }

  /** The listing of `path`: kept, or read (once, however many ask). */
  list(path) {
    const key = pathKey(path);
    if (!this.listings.has(key)) {
      let dir;
      try {
        dir = this.dirOf(path);
      } catch (err) {
        return Promise.reject(err);
      }
      const p = this.run(`Reading ${path[path.length - 1]}…`, async () => {
        const r = await this.client.request({ type: "list", dir });
        const entries = r.entries.map((e) => ({
          name: e.name,
          kind: entryKind(e),
          directory: e.directory,
          size: e.directory ? null : e.size,
          checksum: null,
          modified: dosTime(e.modified),
          readOnly: e.readOnly,
        }));
        return { path: [...path], entries: sortEntries(entries), collisions: [] };
      });
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

  /** A changed directory: its listing goes, and the tree and the list read it again. */
  changed(path) { this.refresh(path); }

  /** What in `path` has `name`, case ignored as DOS ignores it: `{name, directory, exact: true}` or null. */
  async existing(path, name) {
    const key = String(name).toUpperCase();
    const found = (await this.list(path)).entries.find((e) => e.name.toUpperCase() === key);
    return found ? { name: found.name, directory: found.directory, exact: true } : null;
  }

  /** Why the palmtop does not take `name` for a new file or directory, or null. */
  async nameProblem(name) {
    const h = await this.helpers();
    return h.palmtop_name_problem(name) ?? null;
  }

  async checkName(name) {
    const problem = await this.nameProblem(name);
    if (problem) throw new FolderError("bad-name", `${name}: ${problem.charAt(0).toLowerCase()}${problem.slice(1)}`, { name });
  }

  /** File `name` of `path`: `{name, fileName, data, size, modified}`. `size` (bytes) for the progress. */
  read(path, name, { size = null, tag = null } = {}) {
    const dir = this.dirOf(path);
    return this.run(`Getting ${name}…`, async () => {
      const r = await this.client.request({ type: "get", dir, name });
      return { name: r.name, fileName: r.name, data: r.data, size: r.data.length, modified: dosTime(r.modified) };
    }, { total: size, tag });
  }

  /** The first `n` bytes of file `name` (the whole file is read: the Filer reads in 2 KB blocks anyway). */
  async head(path, name, n) {
    return (await this.read(path, name)).data.slice(0, n);
  }

  /** Write `bytes` as file `name` of `path`; an existing file only with `replace`. Resolves to the name made (upper case). */
  async write(path, name, bytes, { replace = false } = {}) {
    const dir = this.dirOf(path);
    await this.checkName(name);
    try {
      return await this.run(`Putting ${name.toUpperCase()}…`, async () => {
        const r = await this.client.request({ type: "put", dir, name, replace }, bytes);
        return r.name;
      }, { total: bytes.length });
    } finally {
      this.changed(path);
    }
  }

  /** Make directory `name` in `path`. */
  async mkdir(path, name) {
    const dir = this.dirOf(path);
    if (dir.length === 0) throw new FolderError("refused", "Directories go on a drive: open C: or A: first.");
    await this.checkName(name);
    try {
      return await this.run(`Making ${name.toUpperCase()}…`, async () => (await this.client.request({ type: "mkdir", dir, name })).name);
    } finally {
      this.changed(path);
    }
  }

  /** Rename `from` in `path` to `to`; never over another name. Resolves to the new name. */
  async rename(path, from, to) {
    const dir = this.dirOf(path);
    if (dir.length === 0) throw new FolderError("refused", "A drive cannot be renamed.");
    await this.checkName(to);
    try {
      return await this.run(`Renaming ${from}…`, async () => (await this.client.request({ type: "rename", dir, from, to })).to);
    } finally {
      this.changed(path);
    }
  }

  /** How many files and directories `name` of `path` holds, all the way down (0 for a file). */
  async count(path, name) {
    const found = await this.existing(path, name);
    if (!found) throw new FolderError("missing", `${name} is not there any more.`, { name });
    if (!found.directory) return 0;
    const walk = async (p) => {
      let n = 0;
      for (const e of (await this.list(p)).entries) {
        n += 1;
        if (e.directory) n += await walk([...p, e.name]);
      }
      return n;
    };
    return walk([...path, found.name]);
  }

  /**
   * Delete `name` from `path`: a directory with everything in it only
   * with `recursive` (ask first, with `count`). Resolves to how many
   * entries went with it.
   */
  async purge(path, name, { recursive = false } = {}) {
    const dir = this.dirOf(path);
    if (dir.length === 0) throw new FolderError("refused", "A drive cannot be deleted.");
    const found = await this.existing(path, name);
    if (!found) throw new FolderError("missing", `${name} is not there any more.`, { name });
    if (found.directory && !recursive) {
      const inside = await this.count(path, name);
      if (inside) throw new FolderError("not-empty", `${name} holds ${inside} ${inside === 1 ? "item" : "items"}.`, { name, count: inside });
    }
    try {
      return await this.run(`Deleting ${found.name}…`, async () => (await this.client.request({ type: "remove", dir, name: found.name, directory: found.directory })).count, { tag: "remove" });
    } finally {
      // Everything below it is gone too.
      this.listings.clear();
      this.changed(path);
    }
  }

  /**
   * The files under `names` of `path` (all of `path` when null) as zip
   * entries: `{name: "D1/A.TXT", data, modified}`, `{name: "D1",
   * directory: true}`.
   */
  async files(path, names = null) {
    const out = [];
    let total = 0;
    const add = async (p, e, prefix) => {
      const full = `${prefix}${e.name}`;
      if (e.directory) {
        out.push({ name: full, directory: true });
        for (const c of (await this.list([...p, e.name])).entries) await add([...p, e.name], c, `${full}/`);
        return;
      }
      total += e.size ?? 0;
      if (total > MAX_ZIP_BYTES) throw new FolderError("too-big", "That is more than 64 MB, too much for one download from the palmtop. Download fewer at once.");
      const file = await this.read(p, e.name, { size: e.size });
      out.push({ name: full, data: file.data, modified: file.modified ?? e.modified ?? Date.now() });
    };
    const listing = await this.list(path);
    const chosen = names === null ? listing.entries : names.map((n) => {
      const e = listing.entries.find((x) => x.name === n);
      if (!e) throw new FolderError("missing", `${n} is not there any more.`, { name: n });
      return e;
    });
    for (const e of chosen) await add(path, e, "");
    return out;
  }

  /** A `.zip` of `names` of `path` (all of it when null): `{name, blob, size, collisions}`. */
  async zip(path, names = null) {
    const entries = await this.files(path, names);
    const single = names?.length === 1 && entries[0]?.directory;
    // A drive's colon is no file name's.
    const base = (single ? names[0] : path[path.length - 1]).replace(/:$/, "");
    let parts;
    try {
      parts = zipParts(entries.map((e) => (e.directory ? e : { ...e, name: e.name.replace(/:/g, "") })));
    } catch (err) {
      throw new FolderError(err?.code === "too-big" ? "too-big" : "bad-name", `That cannot go into one .zip (${String(err?.message ?? err).replace(/^zip: /, "")}).`, { cause: err });
    }
    const blob = new Blob(parts, { type: "application/zip" });
    return { name: `${base}.zip`, blob, size: blob.size, collisions: [] };
  }

  /** End the session (one DISCONNECT, not awaited: the palmtop stays in server mode). */
  disconnect() {
    this.cancelQueued();
    return this.run("Disconnecting…", () => this.client.request({ type: "disconnect" }));
  }

  /** Close the link and the Worker. */
  close() {
    this.cancelQueued();
    return this.client.close();
  }
}
