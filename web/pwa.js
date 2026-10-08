// The page as an installed app: the service worker (offline, updates),
// the screen kept on through a long computation, and persistent storage
// for the kept ROMs. Only the browser's page registers the worker: never
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
 * notice with Reload, as a reload restarts the calculator. Resolves to
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
  text.textContent = "A new version of saturnus is ready. Reloading restarts the calculator (save its state first to keep it); otherwise the new version starts the next time the page opens.";
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
 * Ask once for persistent storage when the first ROM is kept in this
 * browser (Safari otherwise clears a site's storage after seven days
 * without a visit, unless the page is on the home screen; others under
 * storage pressure). The answer goes to the store as `storage`:
 * "persistent", "best-effort", or null where the browser cannot tell.
 */
export function persistWhenKept(backend, store, storage = navigator.storage) {
  if (backend.romSource !== "file" || !storage?.persist) return;
  let asked = false;
  const check = async () => {
    const kept = store.state.roms?.slots.some((s) => s.fileName);
    if (!kept || asked) return;
    asked = true;
    let granted = false;
    try {
      granted = (await storage.persisted?.()) || (await storage.persist());
    } catch { /* not allowed here */ }
    store.set({ storage: granted ? "persistent" : "best-effort" });
  };
  store.watch(["roms"], check);
  check();
}
