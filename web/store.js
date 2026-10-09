// The page's shared state. Components render from it and send commands
// only through the backend; the backend's events and the controls update
// it. One-way: event -> store -> components.

export class Store extends EventTarget {
  constructor() {
    super();
    this.state = {
      /** "worker" or "tauri". */
      host: null,
      /** Model names the host runs. */
      models: [],
      /** The model chosen in the selector. */
      model: null,
      /** The running machine's model and ROM name, from `status`. */
      booted: null,
      romName: "",
      running: false,
      halted: null,
      loop: "stopped",
      /** A long send is typing: the screen is frozen (`status.busy`). */
      busy: false,
      speed: "1",
      /** A status line message and whether it is an error. */
      message: "",
      messageError: false,
      /** The last `frame` event. */
      frame: null,
      /** Keys down in the machine, from `keys`. */
      keysDown: [],
      /**
       * The ROM slots (`romSlots`): `{slots: [{model, fileName, revision,
       * state}], offers, lastModel, bootLast, remembered, note}`, and the
       * notice of the last choice (what else was found).
       */
      roms: null,
      romNotice: "",
      /** Whether the browser keeps the kept ROMs for good (`StorageChoice`, pwa.js): "persistent", "best-effort" or null, */
      storage: null,
      /** and its notice: "ask" (after a ROM is kept), "kept", "refused" or null. */
      storageOffer: null,
      /** The service worker's build hash (pwa.js), or null without one. */
      build: null,
      /** Whether a saved state can be loaded. */
      canLoad: false,
      /** The memory view (`memory.js`): whether the layer is open, */
      layer: false,
      /** `{supported, reason}` of the running model, or null before it is known, */
      memorySupport: null,
      /** the last reads (`{path, variables}`, the levels, `{system, user, set}`) */
      memoryTree: null,
      memoryStack: null,
      memoryFlags: null,
      /** and why a read failed, per part: `{tree, stack, flags}` messages or null, */
      memoryErrors: { tree: null, stack: null, flags: null },
      /** and whether what is shown is older than the calculator's memory. */
      memoryStale: false,
      /** The calculator has a command line open (read from RAM after the screen changed; app.js). */
      cmdlineOpen: false,
      /** Else stack level 1's object, null for an empty stack, undefined not read yet (read with it; the Edit buttons). */
      stackTop: undefined,
      /** The write running (`writes.js`: its label, the busy overlay) or null, */
      writing: null,
      /** and what the last one came to: `{text, error}` or null. */
      writeMessage: null,
    };
  }

  /** Merge `patch`; components hear one `change` with the changed keys. */
  set(patch) {
    const changed = new Set();
    for (const [k, v] of Object.entries(patch)) {
      if (this.state[k] !== v) {
        this.state[k] = v;
        changed.add(k);
      }
    }
    if (changed.size) this.dispatchEvent(new CustomEvent("change", { detail: changed }));
  }

  /**
   * Call `fn(state, changed)` on every change touching one of `keys`;
   * returns the function that stops it.
   */
  watch(keys, fn) {
    const listener = (e) => {
      if (keys.some((k) => e.detail.has(k))) fn(this.state, e.detail);
    };
    this.addEventListener("change", listener);
    return () => this.removeEventListener("change", listener);
  }
}

/** Feed the backend's events into the store. */
export function connect(backend, store) {
  backend.addEventListener("frame", (e) => store.set({ frame: e.detail }));
  backend.addEventListener("keys", (e) => store.set({ keysDown: e.detail.down }));
  backend.addEventListener("status", (e) => {
    const s = e.detail;
    store.set({
      booted: s.model ? s.model : null,
      romName: s.romName ?? "",
      running: s.running,
      halted: s.halted,
      loop: s.loop,
      busy: Boolean(s.busy),
    });
  });
  backend.addEventListener("error", (e) => store.set({ message: e.detail.message, messageError: true }));
}
