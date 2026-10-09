// The memory view's reads (web/protocol.md: `watchMemory`, `memoryTree`,
// `stack`, `flags`, `objectAt`, the `memoryChanged` event). While the
// layer is open the host is asked to watch the calculator's user memory;
// each `memoryChanged` is followed by one read of the tree, the stack and
// the flags into the store. The page never polls and never writes: no
// key, no memory.

const message = (err) => String(err?.message ?? err);

export class MemoryView {
  constructor(backend, store) {
    this.backend = backend;
    this.store = store;
    /** A read is in flight, and another change arrived meanwhile. */
    this.reading = false;
    this.again = false;
    /** Counts boots and closes: an answer for an older machine is dropped. */
    this.epoch = 0;
    backend.addEventListener("memoryChanged", () => this.refresh());
    store.watch(["booted", "romName"], () => this.watch());
    // A read that failed while the calculator computed is tried again
    // once it waits for a key.
    store.watch(["loop"], (s) => {
      if (s.memoryStale && s.loop !== "frame") this.refresh();
    });
  }

  /** Open or close the layer. */
  setOpen(open) {
    this.store.set({ layer: Boolean(open) });
    return this.watch();
  }

  clear(support = null) {
    this.store.set({
      memorySupport: support,
      memoryTree: null,
      memoryStack: null,
      memoryFlags: null,
      memoryErrors: { tree: null, stack: null, flags: null },
      memoryStale: false,
    });
  }

  /** Tell the host whether to watch, learn whether this model has a view, read. */
  async watch() {
    const epoch = ++this.epoch;
    const { layer, booted } = this.store.state;
    this.clear();
    let support;
    try {
      support = await this.backend.watchMemory(layer && Boolean(booted));
    } catch (err) {
      support = { supported: false, reason: message(err), error: true };
    }
    // Known with the layer closed too: the page offers no memory view
    // for a model without one (app.js).
    if (epoch !== this.epoch || !booted) return;
    this.store.set({ memorySupport: support });
    if (layer && support.supported) await this.refresh();
  }

  /** Whether there is a machine with a memory view and an open layer. */
  ready() {
    const s = this.store.state;
    return Boolean(s.layer && s.booted && s.memorySupport?.supported);
  }

  /** Read the tree, the stack and the flags into the store. */
  async refresh() {
    if (!this.ready()) return;
    if (this.reading) {
      this.again = true;
      return;
    }
    this.reading = true;
    try {
      do {
        this.again = false;
        // A machine booted while this read: its answer is another
        // machine's. `watch()` asks again once the new one is ready
        // (`again`); the loop then reads for the new epoch.
        if (!this.ready()) break;
        const epoch = this.epoch;
        const b = this.backend;
        const [tree, stack, flags] = await Promise.allSettled([b.memoryTree(), b.stack(), b.flags()]);
        if (epoch !== this.epoch) {
          this.again ||= this.ready();
          continue;
        }
        // While the calculator computes its structures are in motion: a
        // failed read keeps what was shown and is repeated when it idles.
        const busy = this.store.state.loop === "frame";
        const patch = {};
        const errors = { ...this.store.state.memoryErrors };
        let stale = false;
        for (const [key, name, r] of [["memoryTree", "tree", tree], ["memoryStack", "stack", stack], ["memoryFlags", "flags", flags]]) {
          if (r.status === "fulfilled") {
            patch[key] = r.value;
            errors[name] = null;
          } else if (busy && this.store.state[key]) {
            stale = true;
          } else {
            patch[key] = null;
            errors[name] = message(r.reason);
            stale ||= busy;
          }
        }
        this.store.set({ ...patch, memoryErrors: errors, memoryStale: stale });
      } while (this.again);
    } finally {
      this.reading = false;
    }
  }

  /** The typed object at `address`; rejects with a readable message. */
  async object(address) {
    try {
      return await this.backend.objectAt(address);
    } catch (err) {
      throw new Error(message(err));
    }
  }
}

/**
 * The object of the selected variable: one read per `key` (the
 * variable's address, checksum and size). A read that failed is not a
 * result to keep: the next `get` with `retry` reads again.
 */
export class ObjectLoader {
  /** `read(address)` resolves to the object; `done(state)` hears each read end. */
  constructor(read, done) {
    this.read = read;
    this.done = done;
    /** `{key}` while reading, then `{key, object}` or `{key, error}`. */
    this.current = null;
  }

  /** The state for `key`, starting a read when it is new or (with `retry`) had failed. */
  get(key, address, retry = true) {
    const c = this.current;
    if (c?.key === key && !(retry && c.error !== undefined)) return c;
    const mine = { key };
    this.current = mine;
    const settle = (state) => {
      if (this.current !== mine) return;
      this.current = state;
      this.done(state);
    };
    this.read(address).then(
      (object) => settle({ key, object }),
      (err) => settle({ key, error: message(err) }),
    );
    return mine;
  }

  clear() {
    this.current = null;
  }
}
