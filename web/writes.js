// The memory view's writes (web/protocol.md, "The user memory, written"):
// store a file, fetch a variable into a file, purge, rename, create a
// directory, change directory, set or clear a flag. Each is one hidden Kermit transaction
// on the host; the page runs one at a time, shows what runs (`writing`
// in the store: the busy overlay) and what came of it (`writeMessage`).
// The memory view follows by itself: the host tells it the memory
// changed.

import { WRITABLE_MODELS } from "./norom.js";

/** Largest file the page sends (the hosts' cap, `MAX_FILE_BYTES`). */
export const MAX_FILE_BYTES = 512 * 1024;

const message = (err) => String(err?.message ?? err);

/** The variable a file is stored as: its name without the folder and the last extension (`prog.hp` is `prog`). */
export function variableName(fileName) {
  const base = String(fileName).split(/[\\/]/).at(-1);
  const dot = base.lastIndexOf(".");
  return dot > 0 ? base.slice(0, dot) : base;
}

/** The file a fetched variable is saved as: its name, with the characters file systems refuse replaced, and `.hp`. */
export function fileNameFor(name) {
  const safe = String(name).replace(/[\\/:*?"<>|\u0000-\u001f]/g, "_");
  return `${safe || "variable"}.hp`;
}

/** `["HOME", "D"]` as the page writes a path. */
export const pathText = (dir) => dir.join(" › ");

/**
 * A store's error in plain words, or null to show it as it is: the
 * calculator's "Circular Reference" (the 49G's, when a text file holds
 * its own variable's name: `test.txt` containing `test`) says what it
 * means. `model` is the running model's name (`49g`).
 */
export function storeRefusal(why, fileName, name, model) {
  if (/circular reference/i.test(why)) {
    return `The ${(model ?? "").toUpperCase() || "calculator"} refused ${fileName}: it holds the name '${name}', which would refer to itself.`;
  }
  return null;
}

/**
 * Why files dropped on the memory view cannot be stored now (the store's
 * state), or null: no calculator runs, its model stores no files (no
 * Kermit server), or its memory is not read yet.
 */
export function dropRefusal({ booted, memoryTree }) {
  if (!booted) return "Start the calculator to store files on it.";
  if (!WRITABLE_MODELS.has(booted)) return `The ${booted.toUpperCase()} cannot store files: only the 48SX, 48GX and 49G can.`;
  if (!memoryTree) return "The calculator's memory is not read yet. Drop the files again in a moment.";
  return null;
}

/**
 * Why `name` cannot be a new directory in `dir`, whose variables are
 * `vars`, or null. The page checks what it can see; the host checks the
 * name itself (plain names only), as for a rename.
 */
export function newDirectoryRefusal(name, vars, dir) {
  if (!name) return "Give the new directory a name.";
  if (vars.some((v) => v.name === name)) return `${name} already exists in ${pathText(dir)}.`;
  return null;
}

/** Seconds for the console: `0.12 s`. */
const seconds = (ms) => `${(ms / 1000).toFixed(2)} s`;

/** Whether HOME (of a `memoryTree`) holds IOPAR, null when the tree is not known. */
const hasIopar = (tree) => (tree ? tree.variables.some((v) => v.name === "IOPAR") : null);

/**
 * Said once per page, after the first write that made IOPAR: the
 * calculator's Kermit server keeps its transfer settings there, creating
 * it on first use, as a real one does (wiki: protocols/iopar). Added to
 * that write's message once the memory read after it shows IOPAR in HOME.
 */
export const IOPAR_NOTE = "The calculator also made IOPAR in HOME, as a real one does for transfers.";

/** Save `bytes` as a download named `name` (the browser's own way). */
function download(bytes, name) {
  const url = URL.createObjectURL(new Blob([bytes], { type: "application/octet-stream" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.hidden = true;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

export class MemoryWrites {
  /**
   * `backend` serves the writes, `store` holds `writing` and
   * `writeMessage`; `save(bytes, name)` saves a fetched file where the
   * page has it (the browser: a download; the app writes it itself);
   * `log` takes how long each write took (the console, not the user).
   */
  constructor(backend, store, { save = download, now = () => performance.now(), log = (t) => console.debug(t) } = {}) {
    this.backend = backend;
    this.store = store;
    this.save = save;
    this.now = now;
    this.log = log;
    /** Whether IOPAR's note was given (`IOPAR_NOTE`). */
    this.ioparSaid = false;
    /** The message of a write after which HOME had no IOPAR, until a read shows it there. */
    this.ioparWatch = null;
    store.watch(["memoryTree"], (st) => this.noteIopar(st));
  }

  /**
   * IOPAR seen in HOME after the write that was waiting for it: its
   * message, still shown, gets the note. Seen without it: still waiting
   * (a read from before the write may arrive late); the next write ends
   * the wait.
   */
  noteIopar(st) {
    const w = this.ioparWatch;
    if (!w || hasIopar(st.memoryTree) !== true) return;
    this.ioparWatch = null;
    this.ioparSaid = true;
    if (st.writeMessage === w) this.store.set({ writeMessage: { ...w, text: `${w.text} ${IOPAR_NOTE}` } });
  }

  /** Whether a write is running. */
  busy() {
    return Boolean(this.store.state.writing);
  }

  /**
   * Run `fn` as the write called `label`; its message (`fn`'s result
   * turned into words by `done`, or the error, which `explain` may put
   * in plain words, null to keep it). `null` from `fn` is a cancelled
   * dialog: no message. Refused while another write runs.
   */
  async run(label, fn, done, explain = () => null) {
    if (this.busy()) {
      this.store.set({ writeMessage: { text: "Wait for the current change to finish.", error: true } });
      return null;
    }
    this.store.set({ writing: label, writeMessage: null });
    this.ioparWatch = null;
    const start = this.now();
    // A write through the calculator's server makes IOPAR when HOME has
    // none; one typed on the keys (the 49G's flags in algebraic mode) does not.
    const iopar = hasIopar(this.store.state.memoryTree);
    try {
      const r = await fn();
      if (r !== null && r !== undefined) {
        this.log(`${label.replace(/…$/, "")}: ${seconds(this.now() - start)}`);
        const writeMessage = { text: `${done(r)}.`, error: false };
        this.store.set({ writeMessage });
        if (iopar === false && !r.keys && !this.ioparSaid) {
          this.ioparWatch = writeMessage;
          this.noteIopar(this.store.state);
        }
      }
      return r;
    } catch (err) {
      const text = explain(message(err)) ?? `${label.replace(/…$/, "")} failed: ${message(err)}`;
      this.store.set({ writeMessage: { text, error: true } });
      return null;
    } finally {
      this.store.set({ writing: null });
    }
  }

  /**
   * Store each of `files` (File objects) in `dir`, one after the other,
   * until one fails; files over the cap are left out and named.
   */
  async storeFiles(dir, files) {
    const big = files.filter((f) => f.size > MAX_FILE_BYTES);
    for (const file of files.filter((f) => !big.includes(f))) {
      const name = variableName(file.name);
      const r = await this.run(`Storing ${file.name}…`, () => this.backend.storeFile(dir, name, file),
        (r) => `${file.name} stored as ${r.name} in ${pathText(dir)}`,
        (why) => storeRefusal(why, file.name, name, this.store.state.booted));
      if (r === null) break;
    }
    if (big.length) {
      const before = this.store.state.writeMessage?.text;
      const text = `Not stored, over ${MAX_FILE_BYTES / 1024} KB: ${big.map((f) => f.name).join(", ")}.`;
      this.store.set({ writeMessage: { text: before ? `${before} ${text}` : text, error: true } });
    }
  }

  /** Store a file the app asks for in a dialog (the desktop app). */
  storeAsked(dir) {
    return this.run("Storing a file…", () => this.backend.storeFile(dir, null, null),
      (r) => `Stored as ${r.name} in ${pathText(dir)}`);
  }

  /** Fetch variable `name` of `dir` into a file. */
  fetch(dir, name) {
    return this.run(`Saving ${name}…`, async () => {
      const r = await this.backend.fetchFile(dir, name);
      if (r?.data) {
        const file = fileNameFor(name);
        this.save(r.data, file);
        return { ...r, file };
      }
      return r;
    }, (r) => `${name} saved as ${r.file} (${r.size} bytes)`);
  }

  purge(dir, name) {
    return this.run(`Purging ${name}…`, () => this.backend.purge(dir, name), () => `${name} purged from ${pathText(dir)}`);
  }

  rename(dir, name, to) {
    return this.run(`Renaming ${name}…`, () => this.backend.rename(dir, name, to), () => `${name} renamed to ${to}`);
  }

  createDir(dir, name) {
    return this.run(`Creating ${name}…`, () => this.backend.createDir(dir, name), () => `Directory ${name} created in ${pathText(dir)}`);
  }

  /** Copy (or `move`) variable `name` of `dir` into directory `to`. */
  copy(dir, name, to, { move = false, replace = false } = {}) {
    return this.run(`${move ? "Moving" : "Copying"} ${name}…`, () => this.backend[move ? "move" : "copy"](dir, name, to, replace),
      () => `${move ? "Moved" : "Copied"} ${name} to ${pathText(to)}`);
  }

  changeDir(dir) {
    return this.run(`Making ${pathText(dir)} current…`, () => this.backend.changeDir(dir), () => `The calculator is in ${pathText(dir)}`);
  }

  setFlag(flag, on) {
    const what = `flag ${flag}`;
    return this.run(`${on ? "Setting" : "Clearing"} ${what}…`, () => this.backend.setFlag(flag, on),
      (r) => `${what[0].toUpperCase()}${what.slice(1)} ${on ? "set" : "cleared"}${r.keys ? " (typed on the keys, as the 49G is in algebraic mode)" : ""}`);
  }
}
