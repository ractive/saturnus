// The app's shell (iteration 50d): one document with three views, the
// home page (`#home-view`, home.js), Calculator (`#calculator-view`,
// app.js `mountCalculator`) and Files (`#files-view`, tx/app.js
// `mountFiles`), and the routes between them in the address's hash
// (router.js). A view's module is loaded and mounted on its first visit
// (its components define themselves on import); after that a view is
// hidden and shown, never unmounted, so the calculator runs on and a
// transfer goes on whichever view is shown. Shown: its root is un-hidden
// first, then told (`setShown(true)`); hidden: its toast and its open
// modal dialogs are closed first, so its Escape stays its own. The
// service worker is the document's (pwa.js). Both views reach the
// calculators through the one hub Worker (shared/calc-hub.js), each over
// a port of its own, and so does the home page for the ROMs
// (home-roms.js); in the desktop app through the one native calculator
// (native-hub.js). The home page is where calculators are chosen and
// given their ROM (iteration 53), in the desktop app too.

import { VIEWS, parseRoute, routeHash, startRoute } from "./router.js";
import { Home } from "./home.js";
import { appRoms, hubRoms } from "./home-roms.js";
import { nativeHub } from "./native-hub.js";
import { keptStart } from "./tx/kept-roms.js";
import { installServiceWorker } from "./pwa.js";
import { loadIcons } from "./shared/components/icons.js";
import { closeToastIn } from "./shared/components/toast.js";
import { applySiteLinks, inDesktopApp } from "./shared/site-links.js";

const desktop = inDesktopApp();
const roots = Object.fromEntries(VIEWS.map((v) => [v, document.getElementById(`${v}-view`)]));
/** The view shown now. */
let current = null;
/** Per view, the dialogs `hide` closed that come back with it (`data-reopen`). */
const reopen = { home: new Set(), calculator: new Set(), files: new Set() };
/** The mounted views' handles (a promise each, from the first visit on). */
const mounted = {};

loadIcons();
applySiteLinks();
// The links between the views (Files' "Calculator"), in the desktop app too (iteration 51).
for (const a of document.querySelectorAll("a[data-route]")) a.hidden = false;

// The document's service worker (offline, updates); its build goes to the calculator's About.
const pwa = installServiceWorker(desktop ? "tauri" : "worker", { set() {} });

/** The ROM commands of the home page: the app's, or the browser's hub's. */
const roms = desktop ? appRoms(nativeHub()) : hubRoms();

const home = new Home(roots.home, {
  roms,
  on: {
    connect: () => {
      go({ view: "files", model: null });
      mount("files").then((f) => f.openConnect());
    },
  },
});

/** Mount view `view` on its first visit; its handle. */
function mount(view) {
  if (view === "home") return Promise.resolve(home);
  mounted[view] ??= (async () => {
    const shown = () => current === view;
    if (view === "calculator") {
      const { mountCalculator } = await import("./app.js");
      return mountCalculator(roots.calculator, { shown, model: routeNow()?.model ?? null, pwa, onModel });
    }
    const { mountFiles } = await import("./tx/app.js");
    return mountFiles(roots.files, { shown });
  })();
  return mounted[view];
}

/** The route of the address now (null: none). */
function routeNow() {
  return parseRoute(location.hash);
}

/** Calculator runs `model` now: its route follows, so a reload opens it again. */
function onModel(model) {
  const r = routeNow();
  if (current !== "calculator" || (r?.model ?? null) === (model ?? null)) return;
  // Turned off: the route drops the model, so Back or a reload does not boot it again.
  history.replaceState(history.state, "", routeHash({ view: "calculator", model: model ?? null }));
}

/** Go to `route`: a new entry in the history, the view shown by `hashchange`. */
function go(route) {
  const hash = routeHash(route);
  if (location.hash === hash) show(route);
  else location.hash = hash;
}

/** Hide view `view`: its toast and its open modal dialogs closed (a modal left open would make the shown view inert), then its root hidden and the view told. A dialog marked `data-reopen` (Files' XModem dialog while a transfer runs: its Cancel and progress) is shown again with its view. */
function hide(view) {
  const root = roots[view];
  closeToastIn(root);
  for (const d of root.querySelectorAll("dialog[open]")) {
    if (!d.matches(":modal")) continue;
    if ("reopen" in d.dataset) reopen[view].add(d);
    d.close();
  }
  root.hidden = true;
  if (view !== "home") mounted[view]?.then((v) => v.setShown?.(false), () => {});
}

/** Show `route`'s view (mounted on its first visit) and hand it the route's model. */
function show(route) {
  const { view, model } = route;
  const first = !mounted[view] && view !== "home";
  if (current && current !== view) hide(current);
  current = view;
  document.body.dataset.shown = view;
  // Un-hidden before it is told, so it measures itself shown.
  roots[view].hidden = false;
  for (const d of reopen[view] ?? []) {
    if ("reopen" in d.dataset && !d.open) d.showModal();
  }
  reopen[view]?.clear();
  if (view === "home") {
    home.render();
    return;
  }
  mount(view).then((v) => {
    if (current !== view) return;
    v.setShown?.(true);
    if (view === "calculator" && !first) v.show(model);
    if (view === "files" && model) v.connectFromEmulator(model);
  }, (err) => console.error(`saturnus: the ${view} view did not start`, err));
}

addEventListener("hashchange", () => show(routeNow() ?? { view: "home", model: null }));

/** How long the desktop app's start waits for its slots before it opens the home page. */
const START_MS = 3000;

/**
 * The kept ROMs' start setting, `{lastModel, bootLast}` (`startRoute`):
 * the app's slots, or the browser's ROM store read directly (no hub
 * started for it). None readable in time: the home page.
 */
async function startSettings() {
  if (!desktop) return keptStart();
  try {
    const r = await Promise.race([roms.romSlots(), new Promise((_, reject) => setTimeout(() => reject(new Error("no answer")), START_MS))]);
    const last = r.slots.find((x) => x.model === r.lastModel);
    return { lastModel: last?.state === "ready" ? last.model : null, bootLast: r.bootLast };
  } catch {
    return { lastModel: null, bootLast: false };
  }
}

// The start: the address's route, else where the app starts (`startRoute`:
// the last calculator when the home page's setting asks for it), written
// into the address without a new history entry.
const start = routeNow() ?? startRoute(await startSettings());
if (!routeNow()) history.replaceState(history.state, "", routeHash(start));
show(start);

// For the tests: the view shown, the views' handles, the home page.
window.saturnusApp = {
  get view() { return current; },
  mounted: (view) => mount(view),
  home,
  pwa,
};
