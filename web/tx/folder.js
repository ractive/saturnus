// A folder on the computer as a pane's place (kb/docs/saturnus-tx.md,
// "The local folder"), with the shape of a Device (web/shared/device.js)
// where the two meet, so a pane lists, reads, writes, renames, purges
// and makes directories the same way on either:
//
//   home                             [title]: the folder itself, as a path
//   list(path)                       -> {path, entries: [{name, kind, directory,
//                                       size, checksum: null, modified}], collisions}
//   read(path, name)                 -> {name, fileName, data, size, modified}
//   write(path, name, bytes, {replace}) -> the name written
//   existing(path, name)             -> null or {name, directory, exact}
//   rename(path, from, to)           -> the new name
//   count(path, name)                -> the files and directories inside
//   purge(path, name, {recursive})   -> how many went with it
//   mkdir(path, name)                -> the name made
//   files(path, names), zip(path, names)  the zip download (1 GiB at most)
//   "change" event, detail {path}    a directory changed through this folder
//
// A path is `[title, "D1", ...]`, as a device's is `["HOME", "D1"]`.
// Names are file names (`A%2FB.hp`); turning a variable's name into one
// is the wasm's `file_name` (crates/saturnus-tx/src/names.rs).
//
// Nothing is replaced without `replace`, and never a directory. A name
// that differs from one already there only in case (`a.hp` beside
// `A.hp`) is a collision whatever the file system: on macOS and Windows
// they are one file, so writing one would replace the other, and a folder
// copied there would lose one. Names this page makes are portable: none
// of `/ \ : * ? " < > |`, no control character, no trailing dot or space,
// no Windows device name (the escaping of names.rs, so a file name made
// by `file_name` always passes).
//
// One class over four roots, all with the File System Access API's
// handle shape:
//
// - **Native** (the desktop app, web/tx/tauri.js): the computer's files
//   through the app's commands (`computerFolder`, kind "native"); a
//   folder chosen in the app's dialog for the aplet server (kind
//   "picked", `pickFolder`).
// - **Picked** (`pickFolder`, Chrome and Edge): `showDirectoryPicker` on
//   a click; the handles of the last `MAX_RECENT` folders picked are kept
//   in IndexedDB (`TRANSFER_DB`), the pane's "Recent folders"
//   (`recentFolders`, `openRecent`, `clearRecentFolders`); the newest is
//   offered again by `keptFolder` on the next visit, when the permission
//   is asked again (`permission()`, then `requestAccess()` on a click).
// - **The origin private file system** (`fallbackFolder`, where there is
//   no picker but OPFS can write): the page's "Files", in directory
//   `TRANSFER_OPFS_DIR`, kept across visits.
// - **Memory** (`fallbackFolder` without OPFS): the page's "Files" for
//   this visit, `MemoryDirectory` (web/tx/memory-handle.js).
//
// Files go out as downloads: one file as itself, a directory or a
// selection as a `.zip` (web/tx/zip.js). There is no change notification
// from outside the page (the API has none outside Chrome's trial of
// `FileSystemObserver`); a pane refreshes on demand.
// Tested by web/test/tx-folder.test.mjs over fakes.

import { sortEntries } from "../shared/device.js";
import { FOLDER_PICKER_ID, FOLDER_STORE, TRANSFER_DB, TRANSFER_OPFS_DIR } from "../shared/storage-names.js";
import { MemoryDirectory } from "./memory-handle.js";
import { isTauri, pickNativeFolder } from "./tauri.js";
import { MAX_ENTRIES, zipParts } from "./zip.js";
import { saveDownload } from "../shared/download.js";

/** The title of the in-page folder. */
export const FILES_TITLE = "Files";

/** A failure of a folder operation: `kind` says which, the message says it in plain words. */
export class FolderError extends Error {
  constructor(kind, message, detail = {}) {
    super(message);
    this.name = "FolderError";
    this.kind = kind;
    Object.assign(this, detail);
  }
}

/** `err` (a DOMException, a TypeError, anything) as a FolderError. */
export function folderError(err, name = "") {
  if (err instanceof FolderError) return err;
  const what = name ? `${name}: ` : "";
  const refusal = appRefusal(err, name);
  if (refusal) return new FolderError(refusal.kind, refusal.message, { cause: err });
  const kind = {
    NotFoundError: "missing",
    NotAllowedError: "permission",
    SecurityError: "permission",
    InvalidModificationError: "not-empty",
    TypeMismatchError: "type",
    QuotaExceededError: "full",
    NoModificationAllowedError: "busy",
    TypeError: "bad-name",
  }[err?.name] ?? "failed";
  const message = {
    missing: `${name || "That"} is not there any more.`,
    permission: "The page may not use this folder now. Allow it again.",
    "not-empty": `${name || "The folder"} is not empty.`,
    type: `${name || "That"} is a file where a folder was expected, or a folder where a file was.`,
    full: "There is no room left for the files in this browser.",
    busy: `${name || "The file"} is in use by another program.`,
    "bad-name": `${what}the file system does not take that name.`,
  }[kind] ?? `${what}${err?.message ?? String(err)}`;
  return new FolderError(kind, message, { cause: err });
}

/**
 * The desktop app's refusals (crates/saturnus-tx-tauri, files.rs): a change
 * outside the folders it may change ("outside"), a recursive remove of the
 * home folder or a folder directly in it, an aplet written into the home
 * folder ("refused"); as `{kind, message}` in plain words, or null.
 */
function appRefusal(err, name) {
  if (err?.name !== "NoModificationAllowedError") return null;
  const text = String(err.message ?? "");
  if (text.startsWith("outside the folders this app may change")) return { kind: "outside", message: `${name || "That"} is ${text}` };
  if (text.startsWith("this folder is not removed from here")) return { kind: "refused", message: `${name ? `${name}: ` : ""}${sentence(text)}.` };
  if (text.startsWith("the calculator's files are not written into the home folder")) return { kind: "refused", message: `${sentence(text)}.` };
  return null;
}

/** `text` with a capital first letter. */
const sentence = (text) => text.charAt(0).toUpperCase() + text.slice(1);

/**
 * A name as a case-insensitive file system compares it (macOS, Windows):
 * NFC, lower case, without context. `toLowerCase` makes a Σ at the end of
 * a word the final ς, so `XΣ 1` would not match `Xσ 1`; ς is folded to σ.
 */
export const foldName = (name) => name.normalize("NFC").toLowerCase().replaceAll("ς", "σ");

/**
 * The groups of `names` that differ only in case (or Unicode
 * normalization): `[["A.hp", "a.hp"]]`. Each group would be one file on
 * macOS or Windows.
 */
export function collisions(names) {
  const groups = new Map();
  for (const name of new Set(names)) {
    const key = foldName(name);
    groups.set(key, [...(groups.get(key) ?? []), name]);
  }
  return [...groups.values()].filter((g) => g.length > 1);
}

const REFUSED = /[/\\:*?"<>|\u0000-\u001f\u007f]/;
const DEVICE = /^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/i;

/** Why the page will not make a file or folder named `name`, in plain words, or null. */
export function nameProblem(name) {
  if (typeof name !== "string" || !name.length) return "The name is empty.";
  if (name === "." || name === "..") return `${name} is not a name.`;
  const bad = name.match(REFUSED);
  if (bad) return bad[0] < " " || bad[0] === "\u007f" ? "The name holds a control character." : `The name cannot hold ${bad[0]}.`;
  if (/[. ]$/.test(name)) return "The name cannot end with a dot or a space.";
  if (DEVICE.test(name.split(".")[0])) return `${name.split(".")[0]} is a name Windows keeps for a device.`;
  if (new TextEncoder().encode(name).length > 255) return "The name is too long.";
  return null;
}

/** Leftovers of Chrome's writes (`NAME.crswap`), never shown. */
const hidden = (name) => name.endsWith(".crswap");

export class Folder extends EventTarget {
  /**
   * `root` a directory handle (picked, OPFS or `MemoryDirectory`); `kind`
   * `"picked"`, `"opfs"` or `"memory"`; `title` the place's name, the
   * first element of every path (the picked folder's own name by default).
   */
  constructor(root, { kind = "memory", title = root.name } = {}) {
    super();
    this.root = root;
    this.kind = kind;
    this.title = title;
    /** The changes in turn: each waits for the one before (`exclusive`). */
    this.queue = Promise.resolve();
  }

  /**
   * Run `fn` when the changes before it are done, so the check and the act
   * of write, mkdir, rename and purge never interleave with another's.
   */
  exclusive(fn) {
    const run = this.queue.then(fn, fn);
    this.queue = run.catch(() => {});
    return run;
  }

  /** The folder itself as a path. */
  get home() {
    return [this.title];
  }

  /** Whether the files stay after the page closes. */
  get kept() {
    return this.kind !== "memory";
  }

  /**
   * The read and write permission now: `"granted"`, `"prompt"` (ask with
   * `requestAccess` on a click) or `"denied"`. Always granted for OPFS
   * and memory.
   */
  async permission() {
    if (typeof this.root.queryPermission !== "function") return "granted";
    try {
      return await this.root.queryPermission({ mode: "readwrite" });
    } catch (err) {
      throw folderError(err);
    }
  }

  /** Ask for read and write permission; call it from a click (the browser shows its question). */
  async requestAccess() {
    if (typeof this.root.requestPermission !== "function") return "granted";
    try {
      return await this.root.requestPermission({ mode: "readwrite" });
    } catch (err) {
      throw folderError(err);
    }
  }

  /** Run `fn` with the permission checked first and the errors as FolderErrors. */
  async guard(name, fn) {
    if ((await this.permission()) !== "granted") {
      throw new FolderError("permission", `The page may not use ${this.title} now. Allow it again.`);
    }
    try {
      return await fn();
    } catch (err) {
      throw folderError(err, name);
    }
  }

  /** The directory handle of `path`. */
  async dir(path) {
    if (!Array.isArray(path) || path[0] !== this.title) {
      throw new FolderError("missing", `${Array.isArray(path) ? path.join("/") : path} is not in ${this.title}.`);
    }
    let dir = this.root;
    for (const part of path.slice(1)) {
      try {
        dir = await dir.getDirectoryHandle(part);
      } catch (err) {
        throw folderError(err, part);
      }
    }
    return dir;
  }

  changed(path) {
    this.dispatchEvent(new CustomEvent("change", { detail: { path: [...path] } }));
  }

  /** What is in `path`: its entries in the list's order, and the groups of names that collide. */
  list(path) {
    return this.guard(path[path.length - 1], async () => {
      const dir = await this.dir(path);
      const entries = [];
      for await (const [name, handle] of dir.entries()) {
        if (hidden(name)) continue;
        if (handle.kind === "directory") {
          entries.push({ name, kind: "Folder", directory: true, size: null, checksum: null, modified: null });
        } else {
          const file = await handle.getFile();
          entries.push({ name, kind: "File", directory: false, size: file.size, checksum: null, modified: file.lastModified });
        }
      }
      return { path: [...path], entries: sortEntries(entries), collisions: collisions(entries.map((e) => e.name)) };
    });
  }

  /** The entry of `dir` named `name`, or the one that differs only in case: `{name, directory, exact}` or null. */
  async find(dir, name) {
    const key = foldName(name);
    let folded = null;
    for await (const [n, handle] of dir.entries()) {
      if (hidden(n)) continue;
      if (n === name) return { name: n, directory: handle.kind === "directory", exact: true };
      if (!folded && foldName(n) === key) folded = { name: n, directory: handle.kind === "directory", exact: false };
    }
    return folded;
  }

  /** What in `path` already has `name`, or differs from it only in case (`exact` false): for the "Replace X?" question. */
  existing(path, name) {
    return this.guard(name, async () => this.find(await this.dir(path), name));
  }

  /** Throw unless `name` may be made in `dir`: a portable name, no collision, an existing file only with `replace`. */
  async checkNew(dir, name, { replace = false, file = true } = {}) {
    const problem = nameProblem(name);
    if (problem) throw new FolderError("bad-name", problem, { name });
    const found = await this.find(dir, name);
    if (!found) return null;
    if (!found.exact) {
      throw new FolderError("collision", `${name} and ${found.name} differ only in case; on macOS and Windows they are the same file.`, { name, other: found.name });
    }
    if (found.directory) throw new FolderError("exists", `There is already a folder named ${name}.`, { name, directory: true });
    if (!replace || !file) throw new FolderError("exists", `${name} is already there.`, { name, directory: false });
    return found;
  }

  /** File `name` of `path`: its bytes. */
  read(path, name) {
    return this.guard(name, async () => {
      const dir = await this.dir(path);
      const file = await (await dir.getFileHandle(name)).getFile();
      const data = new Uint8Array(await file.arrayBuffer());
      return { name, fileName: name, data, size: data.length, modified: file.lastModified };
    });
  }

  /** The first `n` bytes of file `name` of `path` (all of a shorter file). */
  head(path, name, n) {
    return this.guard(name, async () => {
      const dir = await this.dir(path);
      const file = await (await dir.getFileHandle(name)).getFile();
      return new Uint8Array(await file.slice(0, n).arrayBuffer());
    });
  }

  /** Write `bytes` as file `name` of `path`; an existing file only with `replace`. Resolves to the name. */
  write(path, name, bytes, { replace = false } = {}) {
    return this.exclusive(() => this.guard(name, async () => {
      const dir = await this.dir(path);
      const old = await this.checkNew(dir, name, { replace });
      await writeFile(dir, name, bytes, { fresh: !old });
      this.changed(path);
      return name;
    }));
  }

  /** Make directory `name` in `path`. */
  mkdir(path, name) {
    return this.exclusive(() => this.guard(name, async () => {
      const dir = await this.dir(path);
      await this.checkNew(dir, name, { file: false });
      await dir.getDirectoryHandle(name, { create: true });
      this.changed(path);
      return name;
    }));
  }

  /**
   * Rename `from` in `path` to `to`; never over another entry. A change of
   * case only (`a.hp` to `A.hp`) goes through a temporary name, so a
   * case-insensitive file system does not take the two for one.
   */
  rename(path, from, to) {
    return this.exclusive(() => this.guard(from, async () => {
      const dir = await this.dir(path);
      const source = await this.find(dir, from);
      if (!source?.exact) throw new FolderError("missing", `${from} is not there any more.`, { name: from });
      if (from === to) return to;
      const problem = nameProblem(to);
      if (problem) throw new FolderError("bad-name", problem, { name: to });
      const found = await this.find(dir, to);
      if (found && found.name !== from) {
        if (found.exact) throw new FolderError("exists", `${to} is already there.`, { name: to, directory: found.directory });
        throw new FolderError("collision", `${to} and ${found.name} differ only in case; on macOS and Windows they are the same file.`, { name: to, other: found.name });
      }
      try {
        if (found) {
          const temp = await freeName(this, dir, from);
          await move(dir, from, temp, source.directory);
          try {
            await move(dir, temp, to, source.directory);
          } catch (err) {
            // Back to the old name; where that fails too, say where the data is.
            try {
              await move(dir, temp, from, source.directory);
            } catch {
              throw new FolderError("incomplete", `Renaming ${from} to ${to} failed half-way: it is now named ${temp}.`, { name: temp, original: from, cause: err });
            }
            throw err;
          }
        } else {
          await move(dir, from, to, source.directory);
        }
      } finally {
        // Whatever happened, the listing may differ now.
        this.changed(path);
      }
      return to;
    }));
  }

  /** How many files and directories `name` of `path` holds, all the way down (0 for a file). */
  count(path, name) {
    return this.guard(name, async () => {
      const dir = await this.dir(path);
      const found = await this.find(dir, name);
      if (!found?.exact) throw new FolderError("missing", `${name} is not there any more.`, { name });
      return found.directory ? countIn(await dir.getDirectoryHandle(name)) : 0;
    });
  }

  /**
   * Remove `name` from `path`. A directory that is not empty only with
   * `recursive` (ask first, with `count`); otherwise a "not-empty"
   * FolderError carrying the `count`. Resolves to how many entries went
   * with it.
   */
  purge(path, name, { recursive = false } = {}) {
    return this.exclusive(() => this.guard(name, async () => {
      const dir = await this.dir(path);
      const found = await this.find(dir, name);
      if (!found?.exact) throw new FolderError("missing", `${name} is not there any more.`, { name });
      const inside = found.directory ? await countIn(await dir.getDirectoryHandle(name)) : 0;
      if (inside && !recursive) {
        throw new FolderError("not-empty", `${name} holds ${inside} ${inside === 1 ? "item" : "items"}.`, { name, count: inside });
      }
      // The caller's `recursive`: a directory filled since the count (by
      // another program) is refused as not empty, not removed unasked.
      await dir.removeEntry(name, found.directory ? { recursive } : {});
      this.changed(path);
      return inside;
    }));
  }

  /**
   * The files under `names` of `path` (all of `path` when null) as zip
   * entries, paths relative to `path`: `{name: "D1/A.hp", data,
   * modified}`, and `{name: "D1", directory: true}` for each directory.
   * The sizes are added up before any file is read: more than `maxBytes`
   * (1 GiB) or `maxEntries` (what a zip without Zip64 holds) is a
   * "too-big" FolderError, early.
   */
  files(path, names = null, { maxBytes = MAX_ZIP_BYTES, maxEntries = MAX_ENTRIES } = {}) {
    return this.guard(path[path.length - 1], async () => {
      const dir = await this.dir(path);
      const found = [];
      let total = 0;
      const walk = async (handle, prefix) => {
        for await (const [n, h] of handle.entries()) {
          if (hidden(n)) continue;
          await add(h, `${prefix}${n}`);
        }
      };
      const add = async (handle, full) => {
        if (handle.kind === "directory") {
          found.push({ name: full, directory: true });
          await walk(handle, `${full}/`);
        } else {
          // getFile() reads nothing yet: the size comes first.
          const file = await handle.getFile();
          total += file.size;
          found.push({ name: full, file });
        }
        if (found.length > maxEntries) {
          throw new FolderError("too-big", `That is more than ${maxEntries.toLocaleString("en-US")} files and folders, more than one .zip holds. Download fewer at once.`);
        }
        if (total > maxBytes) {
          throw new FolderError("too-big", `That is more than ${sizeText(maxBytes)}, too much for one download. Download fewer at once.`);
        }
      };
      if (names === null) {
        await walk(dir, "");
      } else {
        for (const name of names) {
          const entry = await this.find(dir, name);
          if (!entry?.exact) throw new FolderError("missing", `${name} is not there any more.`, { name });
          await add(entry.directory ? await dir.getDirectoryHandle(name) : await dir.getFileHandle(name), name);
        }
      }
      const out = [];
      for (const e of found) {
        out.push(e.directory ? e : { name: e.name, data: new Uint8Array(await e.file.arrayBuffer()), modified: e.file.lastModified });
      }
      return out;
    });
  }

  /**
   * A `.zip` of `names` of `path` (all of it when null): `{name, blob,
   * size, collisions}`, the Blob made from the zip's parts (the files'
   * bytes are held once). Named after the one directory zipped, else
   * after `path`'s last directory. `collisions`: the paths in it that
   * differ only in case, which unzipping on macOS or Windows would merge.
   * `limits` as for `files`.
   */
  async zip(path, names = null, limits = {}) {
    const entries = await this.files(path, names, limits);
    const single = names?.length === 1 && entries[0]?.directory;
    const name = `${single ? names[0] : path[path.length - 1]}.zip`;
    let parts;
    try {
      parts = zipParts(entries);
    } catch (err) {
      if (err?.code === "too-big") throw new FolderError("too-big", "That is too much for one .zip. Download fewer at once.", { cause: err });
      if (err?.code === "bad-name") throw new FolderError("bad-name", `A name cannot go into a .zip (${err.message.replace(/^zip: /, "")}).`, { cause: err });
      throw folderError(err);
    }
    const blob = new Blob(parts, { type: "application/zip" });
    return { name, blob, size: blob.size, collisions: collisions(entries.map((e) => e.name)) };
  }
}

/** The most a `.zip` download holds: 1 GiB, all of it in the page's memory. */
export const MAX_ZIP_BYTES = 2 ** 30;

/** `n` bytes in words: `1 GiB`, `12 MB`. */
function sizeText(n) {
  if (n >= 2 ** 30) return `${+(n / 2 ** 30).toFixed(1)} GiB`;
  if (n >= 2 ** 20) return `${+(n / 2 ** 20).toFixed(1)} MB`;
  return `${n.toLocaleString("en-US")} bytes`;
}

/**
 * Write `bytes` into file `name` of handle `dir`, made if missing; the old
 * content goes only when the write is complete. A `fresh` file (there was
 * none of that name) that fails is removed again, so no empty file stays.
 */
async function writeFile(dir, name, bytes, { fresh = false } = {}) {
  const handle = await dir.getFileHandle(name, { create: true });
  try {
    const writable = await handle.createWritable();
    try {
      await writable.write(bytes);
      await writable.close();
    } catch (err) {
      await writable.abort?.().catch(() => {});
      throw err;
    }
  } catch (err) {
    if (fresh) await dir.removeEntry(name).catch(() => {});
    throw err;
  }
}

/** Copy entry `from` of `src` to `to` of `dst`, a directory with its contents. */
async function copy(src, from, dst, to, directory) {
  if (!directory) {
    const file = await (await src.getFileHandle(from)).getFile();
    await writeFile(dst, to, new Uint8Array(await file.arrayBuffer()));
    return;
  }
  const source = await src.getDirectoryHandle(from);
  const target = await dst.getDirectoryHandle(to, { create: true });
  for await (const [n, h] of source.entries()) {
    if (!hidden(n)) await copy(source, n, target, n, h.kind === "directory");
  }
}

/**
 * Move `from` to `to` within `dir`: a copy, then the original removed (the
 * API has no rename for a directory, and `move` on files is not in every
 * browser). A copy that fails is removed again and the original stays.
 */
async function move(dir, from, to, directory) {
  // A real file system renames in place (the desktop app's folders).
  if (typeof dir.renameEntry === "function") return dir.renameEntry(from, to);
  try {
    await copy(dir, from, dir, to, directory);
  } catch (err) {
    try {
      await dir.removeEntry(to, { recursive: true });
    } catch (cleanup) {
      // Nothing made yet is fine; a partial copy left behind is said.
      if (cleanup?.name !== "NotFoundError") {
        throw new FolderError("incomplete", `${to} is an incomplete copy of ${from}; ${from} is unchanged.`, { name: to, original: from, cause: err });
      }
    }
    throw err;
  }
  await dir.removeEntry(from, directory ? { recursive: true } : {});
}

/**
 * A name beside `name` in `dir` that nothing has, case ignored: `name~1`,
 * or `rename~1.tmp` where that would not be a portable name (too long).
 */
async function freeName(folder, dir, name) {
  for (let i = 1; ; i++) {
    const beside = `${name}~${i}`;
    const candidate = nameProblem(beside) ? `rename~${i}.tmp` : beside;
    if (!(await folder.find(dir, candidate))) return candidate;
  }
}

/** The files and directories inside handle `dir`, all the way down. */
async function countIn(dir) {
  let n = 0;
  for await (const [name, handle] of dir.entries()) {
    if (hidden(name)) continue;
    n += 1;
    if (handle.kind === "directory") n += await countIn(handle);
  }
  return n;
}

// ---- Choosing the folder ----

/** Whether this browser can pick a folder (the File System Access API: Chrome and Edge on the desktop; the desktop app's dialog). */
export const canPickFolder = (win = globalThis) => typeof win.showDirectoryPicker === "function" || isTauri(win);

/** The most folders "Recent folders" keeps. */
export const MAX_RECENT = 8;

/**
 * The picked folders' handles in IndexedDB (`TRANSFER_DB`, store
 * `FOLDER_STORE`; key an id, value `{handle, name, used}`): `list()` the
 * entries `[{id, handle, name, used}]`, `put(id, value)`, `remove(id)`,
 * `clear()`. A value of another shape is left out of the list.
 */
export function handleStore(idb = globalThis.indexedDB) {
  const open = () => new Promise((resolve, reject) => {
    const req = idb.open(TRANSFER_DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(FOLDER_STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
    req.onblocked = () => reject(new Error(`${TRANSFER_DB} is blocked by another page`));
  });
  /** `fn` gets the store and returns a request, or a list of them: their results. */
  const run = async (mode, fn) => {
    const db = await open();
    try {
      return await new Promise((resolve, reject) => {
        const tx = db.transaction(FOLDER_STORE, mode);
        const req = fn(tx.objectStore(FOLDER_STORE));
        tx.oncomplete = () => resolve(Array.isArray(req) ? req.map((r) => r.result) : req.result);
        tx.onerror = () => reject(tx.error);
        tx.onabort = () => reject(tx.error);
      });
    } finally {
      db.close();
    }
  };
  return {
    list: async () => {
      const [keys, values] = await run("readonly", (s) => [s.getAllKeys(), s.getAll()]);
      return keys.map((id, i) => ({ id, ...values[i] })).filter((e) => e.handle?.kind === "directory" && typeof e.used === "number");
    },
    put: (id, value) => run("readwrite", (s) => s.put(value, id)),
    remove: (id) => run("readwrite", (s) => s.delete(id)),
    clear: () => run("readwrite", (s) => s.clear()),
  };
}

/** A new entry id. */
const newId = () => globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;

/**
 * Keep `handle` at the top of the recent folders: the entry of the same
 * folder (`isSameEntry`) moved there, or a new one; past `MAX_RECENT` the
 * oldest go. Resolves to its id.
 */
async function remember(store, handle, { id = null, now = Date.now() } = {}) {
  const list = (await store.list()).sort((a, b) => b.used - a.used);
  let same = id ? list.find((e) => e.id === id) : null;
  if (!same) {
    for (const e of list) {
      if (await e.handle.isSameEntry?.(handle).catch(() => false)) {
        same = e;
        break;
      }
    }
  }
  const key = same?.id ?? newId();
  await store.put(key, { handle, name: handle.name, used: now });
  const rest = list.filter((e) => e.id !== key);
  for (const e of rest.slice(MAX_RECENT - 1)) await store.remove(e.id);
  return key;
}

/** A recent entry as a picked Folder. */
function recentFolder(entry) {
  const folder = new Folder(entry.handle, { kind: "picked" });
  folder.remembered = true;
  folder.recentId = entry.id;
  return folder;
}

/**
 * Let the user pick a folder (call it from a click), read and write, and
 * keep its handle at the top of the recent folders. Resolves to the
 * Folder, or null when the user closed the picker. `remembered` on the
 * result is false when the handle could not be kept (the folder works for
 * this visit).
 */
export async function pickFolder({ win = globalThis, store = handleStore(), now = Date.now } = {}) {
  if (isTauri(win)) {
    try {
      return await pickNativeFolder();
    } catch (err) {
      throw folderError(err);
    }
  }
  if (!canPickFolder(win)) throw new FolderError("unsupported", "This browser cannot open a folder on the computer.");
  let handle;
  try {
    handle = await win.showDirectoryPicker({ id: FOLDER_PICKER_ID, mode: "readwrite" });
  } catch (err) {
    if (err?.name === "AbortError") return null;
    throw folderError(err);
  }
  const folder = new Folder(handle, { kind: "picked" });
  try {
    folder.recentId = await remember(store, handle, { now: now() });
    folder.remembered = true;
  } catch {
    folder.remembered = false;
  }
  return folder;
}

/**
 * The recent folders, newest first: `[{id, name, used, handle}]` (at most
 * `MAX_RECENT`); none when the store cannot be read.
 */
export async function recentFolders({ store = handleStore() } = {}) {
  try {
    return (await store.list()).sort((a, b) => b.used - a.used).slice(0, MAX_RECENT);
  } catch {
    return [];
  }
}

/**
 * Open `entry` of `recentFolders()` (from a click): the browser's
 * permission asked first, in this click, when it is not granted; then the
 * folder moved to the top. Resolves to the Folder; a "permission"
 * FolderError when the user says no.
 */
export async function openRecent(entry, { store = handleStore(), now = Date.now } = {}) {
  const folder = recentFolder(entry);
  if ((await folder.requestAccess()) !== "granted") {
    throw new FolderError("permission", `The page may not use ${folder.title}. Choose it again, or another folder.`);
  }
  await remember(store, entry.handle, { id: entry.id, now: now() }).catch(() => {});
  return folder;
}

/**
 * The newest recent folder (the one used last), or null. Its permission
 * is usually `"prompt"` again on a new visit: ask `requestAccess()` on a
 * click before using it.
 */
export async function keptFolder({ store = handleStore() } = {}) {
  let list;
  try {
    list = await store.list();
  } catch {
    return null;
  }
  const newest = list.sort((a, b) => b.used - a.used)[0];
  return newest ? recentFolder(newest) : null;
}

/** "Clear recent folders": every kept handle forgotten (the files stay where they are). */
export async function clearRecentFolders({ store = handleStore() } = {}) {
  await store.clear();
}

/** Whether OPFS files can be written from the page (`createWritable`; Safari before 18 only writes in Workers). */
export const canWriteOpfs = (win = globalThis) => typeof win.FileSystemFileHandle?.prototype?.createWritable === "function";

/**
 * The page's own "Files" where no folder can be picked: in the origin
 * private file system when it can be written (kept across visits), in
 * memory otherwise (gone with the page).
 */
export async function fallbackFolder({ storage = globalThis.navigator?.storage, writable = canWriteOpfs() } = {}) {
  if (writable && typeof storage?.getDirectory === "function") {
    try {
      const root = await storage.getDirectory();
      const dir = await root.getDirectoryHandle(TRANSFER_OPFS_DIR, { create: true });
      return new Folder(dir, { kind: "opfs", title: FILES_TITLE });
    } catch {
      // A private window, or storage switched off: memory instead.
    }
  }
  return new Folder(new MemoryDirectory(TRANSFER_OPFS_DIR), { kind: "memory", title: FILES_TITLE });
}

// ---- Downloads ----

/** Save `bytes` (or a Blob) as download `name` (an `<a download>` clicked; `save(name, bytes)` instead where given: the desktop app's save dialog). */
export function downloadBytes(name, bytes, { doc = globalThis.document, urls = globalThis.URL, later = setTimeout, save = null } = {}) {
  if (save) return save(name, bytes);
  saveDownload(name, bytes, { type: name.endsWith(".zip") ? "application/zip" : "application/octet-stream", doc, urls, later });
  return undefined;
}

/**
 * Download `names` of `folder`'s `path`: one file as itself, a directory
 * or more than one entry as a `.zip`. Resolves to `{name, size,
 * collisions}` (what was saved; the zip's colliding paths).
 */
export async function download(folder, path, names, deps = {}, limits = {}) {
  if (names.length === 1) {
    const found = await folder.existing(path, names[0]);
    if (found?.exact && !found.directory) {
      const file = await folder.read(path, names[0]);
      await downloadBytes(file.name, file.data, deps);
      return { name: file.name, size: file.size, collisions: [] };
    }
  }
  const z = await folder.zip(path, names, limits);
  await downloadBytes(z.name, z.blob, deps);
  return { name: z.name, size: z.size, collisions: z.collisions };
}
