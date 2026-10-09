// The page as an installed app: the service worker (offline, updates),
// the screen kept on through a long computation, and persistent storage
// for the kept ROMs, asked for in context. Only the browser's page registers the worker: never
// the desktop app (no network, and web/pwa/ is not in it), never over
// plain HTTP but on this computer. See web/pwa/sw.js and web/site.sh.

/** How long the calculator computes before the screen is kept on, in ms. */
const LONG_COMPUTATION_MS = 5000;

/** Whether this page may register the service worker. */
export function serviceWorkerAllowed(host, loc = location, nav = navigator) {
  if (host === "tauri" || !("serviceWorker" in nav)) return false;
  return loc.protocol === "https:" || (loc.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(loc.hostname));
}

/**
 * Register web/pwa/sw.js (shipped as sw.js) and offer its updates: a new
 * build installs in the background and waits. A page nobody has touched
 * yet (it just opened) takes it at once and reloads; one in use shows a
 * notice with Reload, as a reload restarts the calculator. Either is
 * refused while other saturnus pages are open (`others`: they keep their
 * build until all have closed). Resolves to
 * `{build}` once a worker controls the page, or null where none is
 * registered or the site has none (the page served from web/).
 */
export async function installServiceWorker(host, store) {
  if (!serviceWorkerAllowed(host)) return null;
  const sw = navigator.serviceWorker;
  let touched = false;
  const touch = () => { touched = true; };
  document.addEventListener("pointerdown", touch, { capture: true, once: true });
  document.addEventListener("keydown", touch, { capture: true, once: true });
  let reloading = false;
  sw.addEventListener("controllerchange", () => {
    // Only after asking for the new build: the first install's claim is no update.
    if (reloading) location.reload();
  });
  sw.addEventListener("message", (e) => {
    if (e.data?.type !== "others") return;
    reloading = false;
    showOthers();
  });
  let reg;
  try {
    reg = await sw.register("sw.js", { scope: "./", updateViaCache: "none" });
  } catch {
    return null; // no sw.js here: the page served from web/
  }
  const take = (worker) => {
    reloading = true;
    worker.postMessage({ type: "skip" });
  };
  const offer = (worker) => {
    if (!sw.controller) return; // the first install: this page is the build
    if (!touched) take(worker);
    else showUpdate(() => take(worker));
  };
  if (reg.waiting) offer(reg.waiting);
  reg.addEventListener("updatefound", () => {
    const w = reg.installing;
    w?.addEventListener("statechange", () => {
      if (w.state === "installed") offer(w);
    });
  });
  // An installed app comes back from the background without navigating:
  // look for a new build then, as a navigation would.
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) reg.update().catch(() => { /* offline */ });
  });
  await sw.ready;
  // On the first install the worker claims the page a moment after it is ready.
  if (!sw.controller) {
    await new Promise((resolve) => {
      sw.addEventListener("controllerchange", resolve, { once: true });
      setTimeout(resolve, 2000);
    });
  }
  const build = await askBuild();
  store.set({ build });
  return { build };
}

/** The build of the worker that controls this page, or null. */
function askBuild() {
  const c = navigator.serviceWorker.controller;
  if (!c) return Promise.resolve(null);
  return new Promise((resolve) => {
    const done = (e) => {
      if (e.data?.type !== "build") return;
      navigator.serviceWorker.removeEventListener("message", done);
      resolve(e.data.build);
    };
    navigator.serviceWorker.addEventListener("message", done);
    c.postMessage({ type: "build" });
    setTimeout(() => resolve(null), 2000);
  });
}

/** The notice of a waiting build: Reload takes it, Later keeps this one. */
function showUpdate(reload) {
  if (document.querySelector(".update-notice")) return;
  const box = document.createElement("div");
  box.className = "update-notice";
  box.setAttribute("role", "status");
  const text = document.createElement("p");
  text.textContent = "A new version of saturnus is ready. Reloading restarts the calculator (save its state first to keep it); otherwise the new version starts once every saturnus page has closed.";
  const later = document.createElement("button");
  later.type = "button";
  later.textContent = "Later";
  later.addEventListener("click", () => box.remove());
  const now = document.createElement("button");
  now.type = "button";
  now.className = "primary";
  now.textContent = "Reload";
  now.addEventListener("click", () => {
    now.disabled = true;
    reload();
  });
  const row = document.createElement("div");
  row.className = "update-actions";
  row.append(later, now);
  box.append(text, row);
  document.body.append(box);
}

/** The notice, if shown, after a refused update: other pages hold the build. */
function showOthers() {
  const box = document.querySelector(".update-notice");
  if (!box) return;
  box.querySelector("p").textContent = "A new version of saturnus is ready. Other saturnus pages are open and keep this version; it starts once every saturnus page has closed.";
  box.querySelector("button.primary")?.remove();
  const later = box.querySelector("button");
  if (later) later.textContent = "OK";
}

/**
 * Keep the screen on while the calculator computes for longer than
 * `LONG_COMPUTATION_MS` (the run loop's `frame`: not asleep waiting for a
 * key), where the Screen Wake Lock API exists. Released as soon as it
 * sleeps or stops; the browser releases it when the page is hidden, and
 * it is asked for again on return.
 */
export function keepScreenOnWhileComputing(store, { wakeLock = navigator.wakeLock, doc = document, delay = LONG_COMPUTATION_MS } = {}) {
  if (!wakeLock) return null;
  let lock = null;
  let timer = 0;
  let asking = false;
  const wanted = () => store.state.loop === "frame" && !doc.hidden;
  const release = () => {
    const l = lock;
    lock = null;
    l?.release().catch(() => { /* already released */ });
  };
  const acquire = async () => {
    timer = 0;
    if (!wanted() || lock || asking) return;
    asking = true;
    try {
      const l = await wakeLock.request("screen");
      l.addEventListener("release", () => { if (lock === l) lock = null; });
      lock = l;
      if (!wanted()) release();
    } catch { /* refused (battery saver, hidden) */ }
    asking = false;
  };
  const update = () => {
    if (wanted()) {
      if (!lock && !timer) timer = setTimeout(acquire, delay);
    } else {
      clearTimeout(timer);
      timer = 0;
      release();
    }
  };
  store.watch(["loop"], update);
  doc.addEventListener("visibilitychange", update);
  update();
  return { held: () => lock !== null };
}

/**
 * Persistent storage for the kept ROMs, asked for in context: never on
 * load (Firefox would prompt with no reason given), but in our words right
 * after the user keeps a ROM (`offer`), and from the ROMs panel. Without
 * it the browser may clear the ROMs under storage pressure (Safari after
 * seven days without a visit, unless the page is on the home screen).
 * Only in the browser's page where `navigator.storage.persist` exists.
 *
 * The store's `storage` says whether the browser keeps them for good:
 * "persistent", "best-effort", or null where this does not apply;
 * `storageOffer` drives the notice: "ask", "kept", "refused" or null.
 * "Not now" and a refusal are kept in `prefs` (`storageAsk`: "not-now",
 * "refused") and the notice not offered again; the panel's button still
 * asks.
 */
/** How long the screen stays still before the offer comes, in ms. */
export const QUIET_MS = 2000;

/**
 * Resolves once the calculator is idle: no calculator running, or no
 * new frame and nothing being typed for `ms` (a boot that is still
 * starting, or answering its first question, is not interrupted).
 */
export function whenQuiet(store, ms = QUIET_MS) {
  if (!store.state.booted) return Promise.resolve();
  return new Promise((resolve) => {
    let timer = null;
    let done = false;
    const arm = () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        if (store.state.busy) arm();
        else {
          done = true;
          resolve();
        }
      }, ms);
    };
    store.watch(["frame", "busy"], () => {
      if (!done) arm();
    });
    arm();
  });
}

export class StorageChoice {
  constructor(backend, store, prefs, storage = globalThis.navigator?.storage, { quiet = whenQuiet } = {}) {
    this.quiet = quiet;
    this.store = store;
    this.prefs = prefs;
    this.storage = storage;
    this.available = backend.romSource === "file" && typeof storage?.persist === "function";
    // No ROM kept any more (Forget ROMs): nothing to keep, the notice goes.
    store.watch(["roms"], (s) => {
      if (!s.roms?.slots.some((x) => x.fileName)) store.set({ storageOffer: null });
    });
  }

  /** At load: what the browser already granted. Never asks. */
  async read() {
    if (!this.available) return;
    this.store.set({ storage: (await this.persisted()) ? "persistent" : "best-effort" });
  }

  /**
   * A ROM was just kept by the user: offer to keep it for good, once the
   * calculator it started is idle (`whenQuiet`).
   */
  async offer() {
    if (!this.available) return;
    const persisted = await this.persisted();
    this.store.set({ storage: persisted ? "persistent" : "best-effort" });
    if (persisted || this.prefs.get("storageAsk")) return;
    await this.quiet(this.store);
    // Answered meanwhile (the panel's button), or the ROMs forgotten.
    if (this.prefs.get("storageAsk") || this.store.state.storage === "persistent") return;
    if (!this.store.state.roms?.slots?.some((x) => x.fileName) && this.store.state.roms) return;
    this.store.set({ storageOffer: "ask" });
  }

  /**
   * "Keep it" or "Keep permanently": calls `persist()` at once, inside
   * the click (a user gesture), where Firefox shows its own prompt.
   */
  async keep() {
    if (!this.available) return;
    let granted = false;
    try {
      granted = await this.storage.persist();
    } catch { /* not allowed here */ }
    // A refusal is remembered like "Not now": Chrome refuses silently by
    // heuristic, and would refuse again on every ROM.
    if (!granted) this.prefs.set("storageAsk", "refused");
    this.store.set({ storage: granted ? "persistent" : "best-effort", storageOffer: granted ? "kept" : "refused" });
  }

  /** "Not now": remembered, and the notice is not offered again. */
  notNow() {
    this.prefs.set("storageAsk", "not-now");
    this.store.set({ storageOffer: null });
  }

  dismiss() {
    this.store.set({ storageOffer: null });
  }

  async persisted() {
    try {
      return Boolean(await this.storage.persisted?.());
    } catch {
      return false;
    }
  }
}

/** The outcome of "Keep it", one line. */
const STORAGE_OUTCOMES = {
  kept: "Kept on this device.",
  refused: "The browser said no: it may still clear it when space runs low.",
};

/**
 * The storage notice (`StorageChoice`): asking after a ROM is kept, then
 * the outcome with OK; in the panel (the phone's sheet) under the ROM
 * row (`host`), where Keep permanently is, not over the keys.
 */
export function showStorageOffer(store, choice, doc = document, host = null) {
  const render = () => {
    const offer = store.state.storageOffer;
    let box = doc.querySelector(".storage-notice");
    if (!offer) {
      box?.remove();
      return;
    }
    if (!box) {
      box = doc.createElement("div");
      box.className = host ? "storage-notice in-panel" : "storage-notice";
      box.setAttribute("role", "status");
      (host ?? doc.body).append(box);
    }
    const text = doc.createElement("p");
    const row = doc.createElement("div");
    row.className = "notice-actions";
    const button = (label, fn, primary = false) => {
      const b = doc.createElement("button");
      b.type = "button";
      b.textContent = label;
      if (primary) b.className = "primary";
      b.addEventListener("click", fn);
      return b;
    };
    if (offer === "ask") {
      text.textContent = "Keep this ROM on this device? Without this, the browser may delete it when space runs low, and you'd have to pick it again.";
      const keep = button("Keep it", () => {
        keep.disabled = true;
        choice.keep();
      }, true);
      row.append(button("Not now", () => choice.notNow()), keep);
    } else {
      text.textContent = STORAGE_OUTCOMES[offer];
      row.append(button("OK", () => choice.dismiss()));
    }
    box.classList.toggle("outcome", offer !== "ask");
    box.replaceChildren(text, row);
  };
  store.watch(["storageOffer"], render);
  render();
}
