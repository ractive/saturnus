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
      support = { supported: false, reason: message(err) };
    }
    if (epoch !== this.epoch || !layer || !booted) return;
    this.store.set({ memorySupport: support });
    if (support.supported) await this.refresh();
  }

  /** Read the tree, the stack and the flags into the store. */
  async refresh() {
    const s = this.store.state;
    if (!s.layer || !s.booted || !s.memorySupport?.supported) return;
    if (this.reading) {
      this.again = true;
      return;
    }
    this.reading = true;
    const epoch = this.epoch;
    try {
      do {
        this.again = false;
        const b = this.backend;
        const [tree, stack, flags] = await Promise.allSettled([b.memoryTree(), b.stack(), b.flags()]);
        if (epoch !== this.epoch) return;
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
