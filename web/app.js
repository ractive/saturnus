// saturnus web UI: the composition root. Picks the backend (the wasm core
// in a Web Worker, or the Tauri app's native core), feeds its events into
// the store and hands both to the components; keeps the page chrome (side
// panel, drop-down sheet, fullscreen) and the preferences. No framework,
// no bundler. See web/README.md and web/protocol.md.

import { createBackend } from "./backend.js";
import { Store, connect } from "./store.js";
import "./components/sat-calculator.js";
import "./components/sat-controls.js";
import "./components/sat-about.js";

const PREFS = {
  model: "saturnus.model",
  view: "saturnus.view",
  speed: "saturnus.speed",
  panel: "saturnus.panel",
};

const prefs = {
  get(key) {
    try { return localStorage.getItem(PREFS[key]); } catch { return null; }
  },
  set(key, value) {
    try { localStorage.setItem(PREFS[key], value); } catch { /* storage blocked */ }
  },
};

const $ = (id) => document.getElementById(id);
const ui = {
  stage: $("stage"),
  calc: document.querySelector("sat-calculator"),
  controls: document.querySelector("sat-controls"),
  about: document.querySelector("sat-about"),
  barFullscreen: $("bar-fullscreen"),
  leaveFullscreen: $("leave-fullscreen"),
  panelHide: $("panel-hide"),
  panelShow: $("panel-show"),
  barMenu: $("bar-menu"),
};

function blurAfter(fn) {
  return (e) => {
    e.currentTarget.blur();
    fn();
  };
}

function setPanelHidden(hidden) {
  document.body.classList.toggle("panel-hidden", hidden);
  ui.panelShow.hidden = !hidden;
  prefs.set("panel", hidden ? "hidden" : "shown");
}

function setSheetOpen(open) {
  document.body.classList.toggle("sheet-open", open);
  ui.barMenu.setAttribute("aria-expanded", String(open));
}

async function enterFullscreen(store) {
  if (document.fullscreenElement) return;
  try {
    await ui.stage.requestFullscreen({ navigationUI: "hide" });
  } catch (err) {
    store.set({ message: `fullscreen refused: ${err.message ?? err}`, messageError: false });
  }
}

function toggleFullscreen(store) {
  setSheetOpen(false);
  if (document.fullscreenElement) document.exitFullscreen();
  else enterFullscreen(store);
}

async function onFullscreenChange() {
  const on = document.fullscreenElement === ui.stage;
  ui.controls.setFullscreenLabel(on);
  // Keep Escape for the ON key where the browser allows it (Chromium's
  // keyboard lock; a held Escape still leaves fullscreen).
  try {
    if (on && navigator.keyboard?.lock) await navigator.keyboard.lock(["Escape"]);
    else if (!on) navigator.keyboard?.unlock?.();
  } catch { /* not granted */ }
}

async function main() {
  const backend = createBackend();
  const store = new Store();
  connect(backend, store);
  const hello = await backend.hello();
  const saved = prefs.get("model");
  store.set({
    host: hello.host,
    models: hello.models,
    model: saved && hello.models.includes(saved) ? saved : hello.models[0],
    view: prefs.get("view") === "grid" ? "grid" : "skin",
    speed: ["1", "2", "4", "max"].includes(prefs.get("speed")) ? prefs.get("speed") : "1",
  });
  if (backend.host === "tauri") {
    const tagline = document.querySelector(".tagline");
    if (tagline) tagline.textContent = "A clean-room emulator of the Saturn calculators.";
  }

  ui.controls.attach(backend, store, prefs);
  ui.calc.attach(backend, store);
  backend.setSpeed(store.state.speed);

  document.addEventListener("sat-fullscreen", () => toggleFullscreen(store));
  document.addEventListener("sat-sheet", (e) => setSheetOpen(Boolean(e.detail)));
  document.addEventListener("sat-about", () => {
    setSheetOpen(false);
    ui.about.open();
  });
  ui.barFullscreen.addEventListener("click", blurAfter(() => toggleFullscreen(store)));
  ui.leaveFullscreen.addEventListener("click", blurAfter(() => document.exitFullscreen()));
  document.addEventListener("fullscreenchange", onFullscreenChange);
  ui.panelHide.addEventListener("click", blurAfter(() => setPanelHidden(true)));
  ui.panelShow.addEventListener("click", blurAfter(() => setPanelHidden(false)));
  ui.barMenu.addEventListener("click", blurAfter(() => setSheetOpen(!document.body.classList.contains("sheet-open"))));
  ui.stage.addEventListener("pointerdown", () => setSheetOpen(false));
  document.addEventListener("visibilitychange", () => backend.visibility(document.hidden));
  backend.visibility(document.hidden);
  setPanelHidden(prefs.get("panel") === "hidden");
  if (!document.fullscreenEnabled) {
    ui.controls.disableFullscreen();
    ui.barFullscreen.disabled = true;
  }

  // Handle for debugging and automated checks.
  window.saturnus = {
    backend,
    store,
    /** The display as text, `#` dark and `.` light. */
    screenText: () => ui.calc.screenText(),
    /** Boot the selected model (or the one the ROM fits) from a File. */
    startWithRom: (file) => ui.controls.boot({ file }),
    /** Whether the host is running passes, sleeping on a timer, or stopped. */
    get loop() { return store.state.loop; },
    get speed() { return store.state.speed; },
    setSpeed: (v) => ui.controls.setSpeed(v),
    /** Host counters: cycles, emulatedMs, workMs, ticks, wakes. */
    stats: () => backend.stats(),
    /** The drawn key group of `name`. */
    skinKey: (name) => ui.calc.skinKey(name),
  };
}

main().catch((err) => {
  let status = document.getElementById("status");
  if (!status) {
    status = document.createElement("p");
    status.className = "status";
    document.querySelector(".panel")?.append(status);
  }
  status.textContent = `Failed to start: ${err?.message ?? err}`;
  status.classList.add("error");
});
