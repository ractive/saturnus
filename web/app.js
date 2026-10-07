// saturnus web UI: the composition root. Picks the backend (the wasm core
// in a Web Worker, or the Tauri app's native core), feeds its events into
// the store and hands both to the components; keeps the page chrome (side
// panel, drop-down sheet, fullscreen) and the preferences. No framework,
// no bundler. See web/README.md and web/protocol.md.

import { createBackend } from "./backend.js";
import { Store, connect } from "./store.js";
import { MemoryView } from "./memory.js";
import { ReferenceLoader } from "./palette.js";
import { MODEL_TITLES } from "./components/sat-calculator.js";
import "./components/sat-controls.js";
import "./components/sat-about.js";
import "./components/sat-explorer.js";
import "./components/sat-palette.js";

const PREFS = {
  model: "saturnus.model",
  view: "saturnus.view",
  speed: "saturnus.speed",
  panel: "saturnus.panel",
  layer: "saturnus.layer",
  layerTab: "saturnus.layerTab",
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
  layer: document.querySelector("sat-explorer"),
  layerShow: $("layer-show"),
  barMemory: $("bar-memory"),
  palette: document.querySelector("sat-palette"),
  paletteShow: $("palette-show"),
  barPalette: $("bar-palette"),
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

/** Open or close the memory view beside (or, on a narrow window, over) the calculator. */
function setLayerOpen(memory, open) {
  document.body.classList.toggle("layer-open", open);
  ui.layer.hidden = !open;
  ui.layerShow.hidden = open;
  for (const b of [ui.layerShow, ui.barMemory]) b.setAttribute("aria-expanded", String(open));
  prefs.set("layer", open ? "open" : "closed");
  if (open) setSheetOpen(false);
  else if (ui.layer.contains(document.activeElement)) document.activeElement.blur();
  return memory.setOpen(open);
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

/**
 * The app's actions as the palette offers them: what the panel's buttons
 * do, by name. Built when the palette opens, so the titles follow the
 * state (Pause or Run, the speed that is on).
 */
function appActions(backend, store, memory) {
  const s = store.state;
  const layerTab = (tab) => async () => {
    await (s.layer || setLayerOpen(memory, true));
    ui.layer.setTab(tab);
  };
  const speed = (v, label) => ({
    id: `speed-${v}`,
    title: `Speed ${label}${s.speed === v ? " (on)" : ""}`,
    description: v === "max" ? "As fast as this device can." : v === "1" ? "Real time." : `${label} real time; the calculator's clock runs ${label} as fast.`,
    keywords: "speed fast slow real time",
    run: () => ui.controls.setSpeed(v),
  });
  const dialog = backend.romSource === "dialog";
  return [
    { id: "rom", title: `Choose the ${MODEL_TITLES[s.model] ?? s.model} ROM…`, description: dialog ? "Pick the ROM file in a dialog; the app remembers where it is." : "Pick the ROM file; it is kept in this browser.", keywords: "rom load open file boot start", run: () => ui.controls.chooseFor(s.model) },
    ...(s.booted ? [
      { id: "run", title: s.running ? "Pause the calculator" : "Run the calculator", description: "The Run/Pause switch: stops or resumes emulated time.", keywords: "pause run stop resume", run: () => backend.pause(s.running) },
      { id: "reset", title: "Reset the calculator", description: "Hardware reset; the memory is kept.", keywords: "reset restart", run: () => backend.reset() },
      { id: "save", title: "Save state", description: dialog ? "The whole machine, to a file." : "The whole machine, into this browser.", keywords: "save state snapshot", run: () => ui.controls.saveState() },
      ...(s.canLoad ? [{ id: "load", title: "Load state", description: "Restore the saved state of this model.", keywords: "load state restore snapshot", run: () => ui.controls.loadState() }] : []),
    ] : []),
    speed("1", "1×"), speed("2", "2×"), speed("4", "4×"), speed("max", "max"),
    { id: "vars", title: "Variables", description: "The memory view's Variables tab: the HOME tree, read live.", keywords: "memory explorer variables directory", run: layerTab("vars") },
    { id: "stack", title: "Stack", description: "The memory view's Stack tab.", keywords: "memory explorer stack levels", run: layerTab("stack") },
    { id: "flags", title: "Flags", description: "The memory view's Flags tab: system and user flags with their meanings.", keywords: "memory explorer flags toggle", run: layerTab("flags") },
    { id: "commands", title: "Browse commands by menu", description: "The Commands tab: the reference by the ROM's menus.", keywords: "commands reference menu browse help", run: layerTab("commands") },
    { id: "layer", title: s.layer ? "Hide the memory view" : "Show the memory view", description: "The layer beside the calculator (Alt+M).", keywords: "memory explorer toggle layer", run: () => setLayerOpen(memory, !s.layer) },
    { id: "fullscreen", title: document.fullscreenElement ? "Leave fullscreen" : "Fullscreen", description: "The calculator alone, on a dark background.", keywords: "fullscreen full screen", run: () => toggleFullscreen(store) },
    { id: "view", title: s.view === "skin" ? "Plain button grid" : "Drawn calculator", description: "Switch between the drawn calculator and the plain button grid.", keywords: "view skin grid drawn buttons", run: () => ui.controls.setView(s.view === "skin" ? "grid" : "skin") },
    { id: "panel", title: document.body.classList.contains("panel-hidden") ? "Show the controls panel" : "Hide the controls panel", description: "The panel with the model, ROM, speed and state controls.", keywords: "panel controls sidebar toggle", run: () => setPanelHidden(!document.body.classList.contains("panel-hidden")) },
    { id: "about", title: "About saturnus", description: "The project statement, its sources and the manuals.", keywords: "about sources manuals licence", run: () => ui.about.open() },
  ];
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

  const memory = new MemoryView(backend, store);
  const reference = new ReferenceLoader(new URL("./commands.json", import.meta.url));
  ui.controls.attach(backend, store, prefs);
  ui.calc.attach(backend, store);
  ui.layer.attach(memory, store, prefs, { reference, backend });
  ui.about.setReference(reference);
  ui.palette.attach(backend, store, {
    reference,
    actions: () => appActions(backend, store, memory),
    onMenu: async (menu) => {
      await (store.state.layer || setLayerOpen(memory, true));
      ui.layer.showMenu(menu.path);
    },
  });
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
  document.addEventListener("sat-layer", (e) => setLayerOpen(memory, Boolean(e.detail)));
  ui.layerShow.addEventListener("click", blurAfter(() => setLayerOpen(memory, true)));
  ui.barMemory.addEventListener("click", blurAfter(() => setLayerOpen(memory, !store.state.layer)));
  // Alt+M moves the keyboard between the calculator and the memory view
  // (opening it); every other key stays where the focus is.
  document.addEventListener("keydown", (e) => {
    if (!e.altKey || e.ctrlKey || e.metaKey || e.code !== "KeyM") return;
    e.preventDefault();
    if (ui.layer.hasFocus()) {
      document.activeElement.blur();
      return;
    }
    Promise.resolve(store.state.layer || setLayerOpen(memory, true)).then(() => ui.layer.focusIn());
  });
  // Cmd+K (Ctrl+K) opens and closes the command palette, wherever the
  // focus is; the palette's own keys are handled inside its dialog.
  const isMac = /Mac|iPhone|iPad/.test(navigator.platform ?? "");
  document.addEventListener("keydown", (e) => {
    if (!(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey || e.code !== "KeyK") return;
    e.preventDefault();
    ui.palette.toggle();
  });
  for (const b of [ui.paletteShow, ui.barPalette]) {
    b.addEventListener("click", blurAfter(() => {
      setSheetOpen(false);
      ui.palette.open();
    }));
  }
  ui.paletteShow.querySelector("kbd").textContent = isMac ? "⌘K" : "Ctrl K";
  ui.paletteShow.title = `Commands, variables and actions: the command palette (${isMac ? "⌘K" : "Ctrl+K"})`;
  ui.barPalette.title = `Command palette (${isMac ? "⌘K" : "Ctrl+K"})`;
  setLayerOpen(memory, prefs.get("layer") === "open");
  document.addEventListener("visibilitychange", () => backend.visibility(document.hidden));
  backend.visibility(document.hidden);
  setPanelHidden(prefs.get("panel") === "hidden");
  if (!document.fullscreenEnabled) {
    ui.controls.disableFullscreen();
    ui.barFullscreen.disabled = true;
  }

  // The remembered ROMs; the last model boots if its ROM is there.
  const started = ui.controls.startRoms();

  // Handle for debugging and automated checks.
  window.saturnus = {
    backend,
    store,
    /** Resolves once the ROM slots are read and the last model (if any) booted. */
    started,
    /** The display as text, `#` dark and `.` light. */
    screenText: () => ui.calc.screenText(),
    /** Boot the selected model (or the one the ROM fits) from a File, which is kept as its ROM. */
    startWithRom: (file) => ui.controls.chooseFiles(store.state.model, [file]),
    /** ROM Files for the selected model, as the picker gives them. */
    chooseRoms: (files) => ui.controls.chooseFiles(store.state.model, files),
    /** Whether the host is running passes, sleeping on a timer, or stopped. */
    get loop() { return store.state.loop; },
    get speed() { return store.state.speed; },
    setSpeed: (v) => ui.controls.setSpeed(v),
    /** Host counters: cycles, emulatedMs, workMs, ticks, wakes. */
    stats: () => backend.stats(),
    /** The memory view: its reads (`memory`) and its element (`explorer`). */
    memory,
    explorer: ui.layer,
    setLayer: (open) => setLayerOpen(memory, Boolean(open)),
    /** The drawn key group of `name`. */
    skinKey: (name) => ui.calc.skinKey(name),
    /** The command palette: its element and its model (rows, selection, command line). */
    palette: ui.palette,
    reference,
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
