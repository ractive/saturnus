// The memory view's writes (web/protocol.md, "The user memory, written"):
// store a file, fetch a variable into a file, purge, rename, create a
// directory, change directory, set or clear a flag. Each is one hidden Kermit transaction
// on the host; the page runs one at a time, shows what runs (`writing`
// in the store: the busy overlay) and what came of it (`writeMessage`).
// The memory view follows by itself: the host tells it the memory
// changed.

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
 * Why `name` cannot be a new directory in `dir`, whose variables are
 * `vars`, or null. The page checks what it can see; the host checks the
 * name itself (plain names only), as for a rename.
 */
export function newDirectoryRefusal(name, vars, dir) {
  if (!name) return "Give the new directory a name.";
  if (vars.some((v) => v.name === name)) return `${name} already exists in ${pathText(dir)}.`;
  return null;
}

/** Seconds for a message: `0.12 s`. */
const seconds = (ms) => `${(ms / 1000).toFixed(2)} s`;

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
   * page has it (the browser: a download; the app writes it itself).
   */
  constructor(backend, store, { save = download, now = () => performance.now() } = {}) {
    this.backend = backend;
    this.store = store;
    this.save = save;
    this.now = now;
  }

  /** Whether a write is running. */
  busy() {
    return Boolean(this.store.state.writing);
  }

  /**
   * Run `fn` as the write called `label`; its message (`fn`'s result
   * turned into words by `done`, or the error). `null` from `fn` is a
   * cancelled dialog: no message. Refused while another write runs.
   */
  async run(label, fn, done) {
    if (this.busy()) {
      this.store.set({ writeMessage: { text: "Another write is still running.", error: true } });
      return null;
    }
    this.store.set({ writing: label, writeMessage: null });
    const start = this.now();
    try {
      const r = await fn();
      if (r !== null && r !== undefined) {
        this.store.set({ writeMessage: { text: `${done(r)}, in ${seconds(this.now() - start)}.`, error: false } });
      }
      return r;
    } catch (err) {
      this.store.set({ writeMessage: { text: `${label.replace(/…$/, "")} failed: ${message(err)}`, error: true } });
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
        (r) => `${file.name} stored as ${r.name} in ${pathText(dir)}`);
      if (r === null) break;
    }
    if (big.length) {
      const before = this.store.state.writeMessage?.text;
      const text = `Larger than ${MAX_FILE_BYTES / 1024} KiB, not stored: ${big.map((f) => f.name).join(", ")}.`;
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
    return this.run(`Fetching ${name}…`, async () => {
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

  changeDir(dir) {
    return this.run(`Changing to ${pathText(dir)}…`, () => this.backend.changeDir(dir), () => `The calculator is in ${pathText(dir)}`);
  }

  setFlag(flag, on) {
    const what = `flag ${flag}`;
    return this.run(`${on ? "Setting" : "Clearing"} ${what}…`, () => this.backend.setFlag(flag, on),
      (r) => `${what[0].toUpperCase()}${what.slice(1)} ${on ? "set" : "cleared"}${r.keys ? " (by keys: the 49G is in algebraic mode)" : ""}`);
  }
}
