// A directory held in the page's memory with the shape of the File System
// Access API's handles (`FileSystemDirectoryHandle`, `FileSystemFileHandle`,
// the part web/tx/folder.js uses), so one `Folder` runs over a folder the
// user picked, the origin private file system and memory alike. The
// in-memory "Files" list where neither the picker nor OPFS exists, and
// the tests' fake of the other two (web/test/tx-folder.test.mjs adds
// permissions and a case-insensitive file system on top).
//
// The errors are the API's: a DOMException named `NotFoundError`,
// `TypeMismatchError` (a file where a directory is asked for, or back),
// `InvalidModificationError` (a non-empty directory removed without
// `recursive`), a TypeError for a name no handle takes (empty, `.`, `..`,
// one with `/` or `\`).

const fail = (name, message) => new DOMException(message, name);

function checkName(name) {
  if (typeof name !== "string" || !name || name === "." || name === ".." || /[/\\]/.test(name)) {
    throw new TypeError(`Name is not allowed: ${JSON.stringify(name)}`);
  }
}

class Entry {
  constructor(name, options) {
    this.name = name;
    this.options = options;
  }

  async isSameEntry(other) {
    return other === this;
  }
}

/** A file: its bytes and when they were written. */
export class MemoryFile extends Entry {
  constructor(name, options, data = new Uint8Array(0), modified = Date.now()) {
    super(name, options);
    this.kind = "file";
    this.data = data;
    this.modified = modified;
  }

  async getFile() {
    return new File([this.data], this.name, { lastModified: this.modified });
  }

  /** A writable that replaces the bytes when closed, as the API's does. */
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
        this.data = out;
        this.modified = this.options.now();
      },
      abort: async () => {
        open = false;
      },
    };
  }
}

/**
 * A directory. `caseInsensitive` makes `A.hp` and `a.hp` one entry, as on
 * macOS and Windows: the name first written stays. `now` gives the
 * modification times (tests pass a clock).
 */
export class MemoryDirectory extends Entry {
  constructor(name = "Files", { caseInsensitive = false, now = () => Date.now() } = {}) {
    super(name, { caseInsensitive, now });
    this.kind = "directory";
    this.children = new Map();
  }

  key(name) {
    // As folder.js's foldName: NFC, lower case, final sigma as sigma.
    return this.options.caseInsensitive ? name.normalize("NFC").toLowerCase().replaceAll("ς", "σ") : name;
  }

  async getFileHandle(name, { create = false } = {}) {
    checkName(name);
    const found = this.children.get(this.key(name));
    if (found) {
      if (found.kind !== "file") throw fail("TypeMismatchError", `${name} is a directory`);
      return found;
    }
    if (!create) throw fail("NotFoundError", `${name} not found`);
    const file = new MemoryFile(name, this.options, new Uint8Array(0), this.options.now());
    this.children.set(this.key(name), file);
    return file;
  }

  async getDirectoryHandle(name, { create = false } = {}) {
    checkName(name);
    const found = this.children.get(this.key(name));
    if (found) {
      if (found.kind !== "directory") throw fail("TypeMismatchError", `${name} is a file`);
      return found;
    }
    if (!create) throw fail("NotFoundError", `${name} not found`);
    const dir = new MemoryDirectory(name, this.options);
    this.children.set(this.key(name), dir);
    return dir;
  }

  async removeEntry(name, { recursive = false } = {}) {
    checkName(name);
    const found = this.children.get(this.key(name));
    if (!found) throw fail("NotFoundError", `${name} not found`);
    if (found.kind === "directory" && found.children.size && !recursive) {
      throw fail("InvalidModificationError", `${name} is not empty`);
    }
    this.children.delete(this.key(name));
  }

  async *entries() {
    for (const entry of [...this.children.values()]) yield [entry.name, entry];
  }

  async *keys() {
    for (const entry of [...this.children.values()]) yield entry.name;
  }

  async *values() {
    for (const entry of [...this.children.values()]) yield entry;
  }

  [Symbol.asyncIterator]() {
    return this.entries();
  }
}
