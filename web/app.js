// saturnus web UI: the composition root. Picks the backend (the wasm core
// in a Web Worker, or the Tauri app's native core), feeds its events into
// the store and hands both to the components; keeps the page chrome (side
// panel and memory view with their widths, drop-down sheet, fullscreen),
// the app's keyboard shortcuts (web/bindings.js), the preferences and the
// installed page's parts (web/pwa.js). No framework, no bundler. See
// web/README.md and web/protocol.md.

import { createBackend } from "./backend.js";
import { Bindings, action } from "./bindings.js";
import { stepContrast } from "./contrast.js";
import { editTarget } from "./editor.js";
import { WRITABLE_MODELS, orderModels } from "./norom.js";
import { dragResize } from "./resize.js";
import { Store, connect } from "./store.js";
import { watchMessages } from "./status.js";
import { MemoryView } from "./memory.js";
import { MemoryWrites } from "./writes.js";
import { ReferenceLoader } from "./palette.js";
import { lookOf } from "./screenshot.js";
import { installServiceWorker, keepScreenOnWhileComputing, showStorageOffer, StorageChoice } from "./pwa.js";
import { MODEL_TITLES } from "./components/sat-calculator.js";
import "./components/sat-controls.js";
import "./components/sat-about.js";
import "./components/sat-explorer.js";
import "./components/sat-palette.js";
import "./components/sat-shortcuts.js";
import { startFailure } from "./failure.js";

const PREFS = {
  model: "saturnus.model",
  speed: "saturnus.speed",
  panel: "saturnus.panel",
  layer: "saturnus.layer",
  layerTab: "saturnus.layerTab",
  panelWidth: "saturnus.panelWidth",
  layerWidth: "saturnus.layerWidth",
  treeWidth: "saturnus.treeWidth",
  keys: "saturnus.keys",
  storageAsk: "saturnus.storageAsk",
};

const prefs = {
  get(key) {
    try { return localStorage.getItem(PREFS[key]); } catch { return null; }
  },
  set(key, value) {
    try { localStorage.setItem(PREFS[key], value); } catch { /* storage blocked */ }
  },
  remove(key) {
    try { localStorage.removeItem(PREFS[key]); } catch { /* storage blocked */ }
  },
};

const isMac = /Mac|iPhone|iPad/.test(navigator.platform ?? "");

const $ = (id) => document.getElementById(id);
const ui = {
  stage: $("stage"),
  calc: document.querySelector("sat-calculator"),
  controls: document.querySelector("sat-controls"),
  about: document.querySelector("sat-about"),
  barFullscreen: $("bar-fullscreen"),
  leaveFullscreen: $("leave-fullscreen"),
  fsPalette: $("fs-palette"),
  panelHide: $("panel-hide"),
  panelShow: $("panel-show"),
  barMenu: $("bar-menu"),
  layer: document.querySelector("sat-explorer"),
  layerShow: $("layer-show"),
  barMemory: $("bar-memory"),
  palette: document.querySelector("sat-palette"),
  paletteShow: $("palette-show"),
  barPalette: $("bar-palette"),
  cmdlineEdit: $("cmdline-edit"),
  barEdit: $("bar-edit"),
  shortcuts: document.querySelector("sat-shortcuts"),
  panelResize: $("panel-resize"),
  layerResize: $("layer-resize"),
};

/**
 * The resizable edges: the side panel's right edge and the memory view's
 * left edge. Widths in CSS pixels, kept per browser (`prefs`), within
 * these limits: the minimums keep every control whole, the memory view
 * leaves the calculator room.
 */
const EDGES = {
  panel: { handle: "panelResize", pref: "panelWidth", prop: "--panel-w", min: 236, max: () => 480, grows: 1 },
  layer: { handle: "layerResize", pref: "layerWidth", prop: "--layer-w", min: 380, max: () => Math.max(380, Math.min(960, window.innerWidth - currentWidth("panel") - 320)), grows: -1 },
};

function currentWidth(edge) {
  const e = EDGES[edge];
  if (edge === "panel") return document.body.classList.contains("panel-hidden") ? 0 : document.getElementById("panel").getBoundingClientRect().width;
  return document.querySelector("sat-explorer").getBoundingClientRect().width || e.min;
}

/** Set an edge's width (clamped); `save` keeps it, null forgets it (the default again). */
function setWidth(edge, px, save = true) {
  const e = EDGES[edge];
  const h = ui[e.handle];
  if (px === null) {
    document.body.style.removeProperty(e.prop);
    if (save) prefs.remove(e.pref);
  } else {
    const w = Math.round(Math.min(e.max(), Math.max(e.min, px)));
    document.body.style.setProperty(e.prop, `${w}px`);
    if (save) prefs.set(e.pref, String(w));
  }
  h.setAttribute("aria-valuemin", String(e.min));
  h.setAttribute("aria-valuemax", String(Math.round(e.max())));
  h.setAttribute("aria-valuenow", String(Math.round(currentWidth(edge))));
}

/** Drag, arrow keys (16px a step) and double-click (the default) on an edge's handle. */
function resizable(edge) {
  const e = EDGES[edge];
  dragResize(ui[e.handle], { width: () => currentWidth(edge), set: (px, save) => setWidth(edge, px, save), grows: e.grows });
  const saved = Number(prefs.get(e.pref));
  if (saved > 0) setWidth(edge, saved, false);
}

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

/**
 * Fullscreen: the stage as the browser's fullscreen element or, where the
 * page cannot have one (the iPhone, whose Safari offers it to videos
 * only), the stage laid over the page (`fs-page`), which in the installed
 * app is the whole screen too.
 */
function isFullscreen() {
  return document.fullscreenElement === ui.stage || ui.stage.classList.contains("fs-page");
}

async function enterFullscreen(store, bindings) {
  if (isFullscreen()) return;
  if (!document.fullscreenEnabled) {
    ui.stage.classList.add("fs-page");
    onFullscreenChange(bindings);
    return;
  }
  try {
    await ui.stage.requestFullscreen({ navigationUI: "hide" });
  } catch (err) {
    // The browser's own words name its API; say it plainly.
    console.warn("saturnus: fullscreen refused:", err);
    store.set({ message: "Fullscreen is not allowed here.", messageError: false });
  }
}

async function exitFullscreen(store, bindings) {
  if (ui.stage.classList.contains("fs-page")) {
    ui.stage.classList.remove("fs-page");
    onFullscreenChange(bindings);
    return;
  }
  try {
    await document.exitFullscreen();
  } catch (err) {
    console.warn("saturnus: leaving fullscreen failed:", err);
    store.set({ message: "Could not leave fullscreen. Press Esc to leave it.", messageError: true });
  }
}

function toggleFullscreen(store, bindings) {
  setSheetOpen(false);
  return isFullscreen() ? exitFullscreen(store, bindings) : enterFullscreen(store, bindings);
}

/**
 * The app's actions as the palette offers them: what the panel's buttons
 * do, by name. Built when the palette opens, so the titles follow the
 * state (Pause or Run, the speed that is on).
 */
function appActions(backend, store, memory, bindings) {
  const s = store.state;
  const layerTab = (tab) => async () => {
    await (s.layer || setLayerOpen(memory, true));
    ui.layer.setTab(tab);
  };
  const speed = (v, label) => ({
    id: `speed-${v}`,
    title: `Speed ${label}${s.speed === v ? " (on)" : ""}`,
    description: v === "max" ? "As fast as this device can; waits for keys at normal speed." : v === "1" ? "Normal speed." : `${v === "2" ? "Twice" : "Four times"} as fast; waits for keys at normal speed.`,
    keywords: "speed fast slow real time",
    run: () => ui.controls.setSpeed(v),
  });
  const dialog = backend.romSource === "dialog";
  const name = MODEL_TITLES[s.model] ?? s.model;
  const romRunning = s.booted === s.model;
  const romAction = romRunning
    ? { id: "rom", title: `Change the ${name} ROM…`, description: dialog ? "Choose another ROM file. The app remembers where it is." : "Replace the kept ROM file.", keywords: "rom change choose replace load open file boot start", run: () => ui.controls.chooseFor(s.model) }
    : { id: "rom", title: `Choose the ${name} ROM…`, description: dialog ? "Pick the ROM file. The app remembers where it is." : "Pick the ROM file. It stays in this browser.", keywords: "rom load open file boot start", run: () => ui.controls.chooseFor(s.model) };
  return [
    ...(s.booted && s.cmdlineOpen ? [
      // After the palette has closed, which ends its search.
      { id: "edit-line", title: "Edit the command line here", description: "Opens the command line in the editor. Send back replaces what the calculator holds.", keywords: "edit command line editor pull replace", run: () => setTimeout(() => ui.palette.openEditor({ kind: "cmdline" }), 0) },
    ] : []),
    // The running model's ROM is changed, not chosen, and comes after the
    // calculator's own actions.
    ...(romRunning ? [] : [romAction]),
    ...(s.booted ? [
      { id: "run", title: s.running ? "Pause the calculator" : "Run the calculator", description: "Stops the calculator's clock, or starts it again.", keywords: "pause run stop resume", run: () => backend.pause(s.running) },
      { id: "reset", title: "Reset the calculator", description: "Restarts the calculator; its memory is kept.", keywords: "reset restart", run: () => backend.reset() },
      { id: "save", title: "Save state", description: dialog ? "Saves the calculator's whole state to a file." : "Saves the calculator's whole state in this browser.", keywords: "save state snapshot", run: () => ui.controls.saveState() },
      ...(romRunning ? [romAction] : []),
      { id: "darker", title: "Darker display", description: `One step darker, as ON and + on the calculator${keyHint(bindings, "darker")}.`, keywords: "contrast darker display lcd on plus", run: () => stepContrast(backend, store, true) },
      { id: "copy-screen", title: "Copy screen", description: `The display as a PNG image, to the clipboard (${s.screenLook === "bw" ? "black on white" : "LCD colours"})${keyHint(bindings, "copyScreen")}.`, keywords: "copy screen screenshot image picture png clipboard display lcd", run: () => ui.calc.copyScreen(s.screenLook) },
      { id: "copy-screen-other", title: `Copy screen (${s.screenLook === "bw" ? "LCD colours" : "black on white"})`, description: "The display as a PNG image in the other colours, to the clipboard.", keywords: "copy screen screenshot image picture png clipboard display lcd black white", run: () => ui.calc.copyScreen(s.screenLook === "bw" ? "lcd" : "bw") },
      { id: "save-screen", title: "Save screen", description: `The display as a PNG file (${s.screenLook === "bw" ? "black on white" : "LCD colours"})${keyHint(bindings, "saveScreen")}.`, keywords: "save screen screenshot image picture png file download display lcd", run: () => ui.calc.saveScreen(s.screenLook) },
      { id: "save-screen-other", title: `Save screen (${s.screenLook === "bw" ? "LCD colours" : "black on white"})`, description: "The display as a PNG file in the other colours.", keywords: "save screen screenshot image picture png file download display lcd black white", run: () => ui.calc.saveScreen(s.screenLook === "bw" ? "lcd" : "bw") },
      { id: "lighter", title: "Lighter display", description: `One step lighter, as ON and − on the calculator${keyHint(bindings, "lighter")}.`, keywords: "contrast lighter display lcd on minus", run: () => stepContrast(backend, store, false) },
      ...(s.canLoad ? [{ id: "load", title: "Load state", description: "Loads the state you saved for this model.", keywords: "load state restore snapshot", run: () => ui.controls.loadState() }] : []),
      // After the palette has closed, which gives the focus back to the page.
      { id: "fresh", title: "Start fresh", description: "Restart with empty memory, like a new calculator. Asks first; your saved state stays.", keywords: "start fresh new cold boot clear memory wipe empty", run: () => setTimeout(() => ui.controls.startFresh(), 0) },
    ] : []),
    speed("1", "1×"), speed("2", "2×"), speed("4", "4×"), speed("max", "max"),
    { id: "vars", title: "Variables", description: "The calculator's variables, read live.", keywords: "memory explorer variables directory", run: layerTab("vars") },
    { id: "stack", title: "Stack", description: "The memory view's Stack tab.", keywords: "memory explorer stack levels", run: layerTab("stack") },
    { id: "flags", title: "Flags", description: "The calculator's flags, with what each one means.", keywords: "memory explorer flags toggle", run: layerTab("flags") },
    { id: "commands", title: "Browse the reference by menu", description: "The command reference, by the calculator's menus.", keywords: "commands reference menu browse help", run: layerTab("commands") },
    { id: "layer", title: s.layer ? "Hide the memory view" : "Show the memory view", description: `The memory view beside the calculator${keyHint(bindings, "layer")}.`, keywords: "memory explorer toggle layer", run: () => setLayerOpen(memory, !s.layer) },
    { id: "fullscreen", title: isFullscreen() ? "Leave fullscreen" : "Fullscreen", description: `The calculator alone, edge to edge${keyHint(bindings, "fullscreen")}.`, keywords: "fullscreen full screen", run: () => toggleFullscreen(store, bindings) },
    // After the palette has closed, which gives the focus back to the page.
    { id: "shortcuts", title: "Keyboard shortcuts", description: `What each key does; change the keys for ON, α, the shifts and the ${backend.host === "tauri" ? "app" : "page"}'s actions${keyHint(bindings, "shortcuts")}.`, keywords: "keyboard shortcuts keys bindings rebind hotkeys layout", run: () => setTimeout(() => ui.shortcuts.open(), 0) },
    { id: "panel", title: document.body.classList.contains("panel-hidden") ? "Show the controls panel" : "Hide the controls panel", description: "The panel with model, ROM, speed and saved states.", keywords: "panel controls sidebar toggle", run: () => setPanelHidden(!document.body.classList.contains("panel-hidden")) },
    { id: "about", title: "About saturnus", description: "What saturnus is, what it was built from, and the manuals.", keywords: "about sources manuals licence", run: () => ui.about.open() },
  ];
}

/**
 * Whether the calculator has a command line open (`cmdlineOpen`), read
 * from RAM a moment after the screen last changed: the "Edit line"
 * controls show while one is.
 */
function watchCommandLine(backend, store) {
  let timer = null;
  const read = async () => {
    timer = null;
    const s = store.state;
    if (!s.booted || s.busy) {
      store.set({ cmdlineOpen: false });
      return;
    }
    try {
      store.set({ cmdlineOpen: Boolean((await backend.commandLine()).active) });
    } catch {
      // No command line on this model, or the memory is not set up.
      store.set({ cmdlineOpen: false });
    }
  };
  store.watch(["frame", "booted", "busy"], () => {
    clearTimeout(timer);
    timer = setTimeout(read, 250);
  });
  store.watch(["cmdlineOpen"], (s) => {
    ui.cmdlineEdit.hidden = !s.cmdlineOpen;
    ui.barEdit.hidden = !s.cmdlineOpen;
  });
}

/** ` (Alt+K)`: an action's key for a description, or "" when it has none. */
function keyHint(bindings, id) {
  const k = bindings.labelOf(id);
  return k ? ` (${k})` : "";
}

const SPEEDS = ["1", "2", "4", "max"];

/** Run app action `id` of the bindings (web/bindings.js). */
function runBinding(id, { backend, store, memory, bindings }) {
  const s = store.state;
  switch (id) {
    case "palette":
      return ui.palette.toggle();
    case "edit":
      return editShortcut(backend, store);
    case "shortcuts":
      if (!ui.palette.isOpen()) return ui.shortcuts.toggle();
      // After the palette's close, which gives the focus back to the page.
      ui.palette.close();
      return setTimeout(() => ui.shortcuts.open(), 0);
    case "layerFocus":
      // Moves the keyboard between the calculator and the memory view (opening it).
      if (ui.layer.hasFocus()) {
        document.activeElement.blur();
        return undefined;
      }
      return Promise.resolve(s.layer || setLayerOpen(memory, true)).then(() => ui.layer.focusIn());
    case "layer":
      return setLayerOpen(memory, !s.layer);
    case "fullscreen":
      return toggleFullscreen(store, bindings);
    case "speed":
      return ui.controls.setSpeed(SPEEDS[(SPEEDS.indexOf(s.speed) + 1) % SPEEDS.length]);
    case "darker":
    case "lighter":
      return s.booted ? stepContrast(backend, store, id === "darker") : undefined;
    case "copyScreen":
      return ui.calc.copyScreen(s.screenLook);
    case "saveScreen":
      return ui.calc.saveScreen(s.screenLook);
    default:
      return undefined;
  }
}

/**
 * Where the edit shortcut acts when the focus is in neither: "view" after
 * a click or tap in the memory view (a row selected there keeps the
 * calculator's keys), "calculator" after a click or tap on the
 * calculator or a key typed to it.
 */
let editHere = "calculator";

/**
 * The edit shortcut, where the keys are (or were last used, `editHere`):
 * in the memory view its selected
 * object; on the calculator its open command line, else stack level 1
 * (read now when the view is closed); quietly nothing when there is none
 * to edit (`editTarget`). From inside the memory view the focus comes
 * back there when the editor closes.
 */
async function editShortcut(backend, store) {
  const s = store.state;
  const writable = WRITABLE_MODELS.has(s.booted) && !s.writing && !s.busy;
  if (!writable) return;
  const inView = ui.layer.hasFocus() || editHere === "view";
  const picked = inView ? ui.layer.editSelection() : null;
  const cmdline = !picked && Boolean((await backend.commandLine().catch(() => null))?.active);
  const stack = picked || cmdline ? null : (s.memoryStack ?? (await backend.stack().catch(() => null)));
  const target = editTarget({ writable, inView, picked, cmdline, stack });
  if (!target) return;
  const from = document.activeElement;
  await ui.palette.openEditor(target, { returnFocus: ui.layer.contains(from) ? () => from : null });
}

/** Fullscreen began or ended: the calculator edge to edge, the labels, Escape. */
async function onFullscreenChange(bindings) {
  const on = isFullscreen();
  ui.stage.classList.toggle("fs", on);
  ui.calc.setEdge(on);
  ui.controls.setFullscreenLabel(on);
  ui.barFullscreen.querySelector("use")?.setAttribute("href", on ? "#ic-collapse" : "#ic-expand");
  // Keep Escape for the ON key where the browser allows it (Chromium's
  // keyboard lock; a held Escape still leaves fullscreen).
  try {
    if (on && document.fullscreenElement && navigator.keyboard?.lock && bindings.keys("on").includes("Escape")) await navigator.keyboard.lock(["Escape"]);
    else if (!on) navigator.keyboard?.unlock?.();
  } catch { /* not granted */ }
}

async function main() {
  const backend = createBackend();
  const store = new Store();
  connect(backend, store);
  watchMessages(store);
  const hello = await backend.hello();
  const saved = prefs.get("model");
  // The plain button grid is gone; drop its old setting.
  try { localStorage.removeItem("saturnus.view"); } catch { /* storage blocked */ }
  store.set({
    host: hello.host,
    models: orderModels(hello.models),
    version: hello.version ?? null,
    model: saved && hello.models.includes(saved) ? saved : hello.models[0],
    speed: ["1", "2", "4", "max"].includes(prefs.get("speed")) ? prefs.get("speed") : "1",
    screenLook: lookOf(prefs.get("screenLook")),
  });
  if (backend.host === "tauri") {
    const tagline = document.querySelector(".tagline");
    if (tagline) tagline.textContent = "The HP 48SX, 48GX, 49G, 38G, 39G, 40G and 42S, emulated.";
  }

  const memory = new MemoryView(backend, store);
  const reference = new ReferenceLoader(new URL("./commands.json", import.meta.url));
  // The keyboard shortcuts, kept per browser (the app's webview keeps its
  // localStorage too).
  let savedKeys = null;
  try { savedKeys = JSON.parse(prefs.get("keys") ?? "null"); } catch { /* unreadable: the defaults */ }
  const bindings = new Bindings({ isMac, host: backend.host, saved: savedKeys });
  ui.controls.attach(backend, store, prefs);
  ui.calc.attach(backend, store, bindings);
  const writes = new MemoryWrites(backend, store);
  ui.layer.attach(memory, store, prefs, { reference, backend, bindings, writes, edit: (target, opts) => ui.palette.openEditor(target, opts) });
  ui.about.setReference(reference);
  ui.about.setStore(store);
  ui.shortcuts.attach(bindings, { where: backend.host === "tauri" ? "Shortcuts are kept by the app." : "Shortcuts are kept in this browser." });
  ui.palette.attach(backend, store, {
    reference,
    bindings,
    actions: () => appActions(backend, store, memory, bindings),
    onMenu: async (menu) => {
      await (store.state.layer || setLayerOpen(memory, true));
      ui.layer.showMenu(menu.path);
    },
  });
  backend.setSpeed(store.state.speed);

  document.addEventListener("sat-fullscreen", () => toggleFullscreen(store, bindings));
  document.addEventListener("sat-choose-rom", (e) => ui.controls.chooseFor(e.detail));
  document.addEventListener("sat-download-rom", (e) => ui.controls.downloadFor(e.detail));
  document.addEventListener("sat-sheet", (e) => setSheetOpen(Boolean(e.detail)));
  // A message beside the ROMs that must be seen (a dropped file that is
  // not a ROM): the controls come into view, as the sheet on a phone.
  document.addEventListener("sat-show-controls", () => {
    if (matchMedia("(max-width: 759px)").matches) setSheetOpen(true);
    else if (document.body.classList.contains("panel-hidden")) setPanelHidden(false);
  });
  document.addEventListener("sat-about", () => {
    setSheetOpen(false);
    ui.about.open();
  });
  // The panel's screen image buttons, in the look chosen there.
  document.addEventListener("sat-copy-screen", () => ui.calc.copyScreen(store.state.screenLook));
  document.addEventListener("sat-save-screen", () => ui.calc.saveScreen(store.state.screenLook));
  document.addEventListener("sat-shortcuts", () => {
    setSheetOpen(false);
    ui.shortcuts.open();
  });
  ui.barFullscreen.addEventListener("click", blurAfter(() => toggleFullscreen(store, bindings)));
  ui.leaveFullscreen.addEventListener("click", blurAfter(() => exitFullscreen(store, bindings)));
  document.addEventListener("fullscreenchange", () => onFullscreenChange(bindings));
  ui.panelHide.addEventListener("click", blurAfter(() => setPanelHidden(true)));
  ui.panelShow.addEventListener("click", blurAfter(() => setPanelHidden(false)));
  ui.barMenu.addEventListener("click", blurAfter(() => setSheetOpen(!document.body.classList.contains("sheet-open"))));
  ui.stage.addEventListener("pointerdown", () => setSheetOpen(false));
  // A click or tap anywhere on the calculator (its keys, its display)
  // gives it the keys back from the memory view; a key still presses.
  ui.calc.addEventListener("pointerdown", () => {
    editHere = "calculator";
    if (ui.layer.hasFocus()) document.activeElement.blur();
  }, true);
  // Input sent to the calculator by any path (a key, a letter, a paste,
  // Firefox's Ctrl+click, which has no pointerdown): its keys.
  ui.calc.addEventListener("sat-key", () => {
    editHere = "calculator";
    if (ui.layer.hasFocus()) document.activeElement.blur();
  });
  ui.layer.addEventListener("pointerdown", () => { editHere = "view"; }, true);
  ui.layer.addEventListener("focusin", () => { editHere = "view"; });
  // The keys given back (Escape, the indicator, Alt+M): the calculator's.
  ui.layer.addEventListener("focusout", () => setTimeout(() => {
    if (!ui.layer.hasFocus()) editHere = "calculator";
  }, 0));
  document.addEventListener("sat-layer", (e) => setLayerOpen(memory, Boolean(e.detail)));
  ui.layerShow.addEventListener("click", blurAfter(() => setLayerOpen(memory, true)));
  ui.barMemory.addEventListener("click", blurAfter(() => setLayerOpen(memory, !store.state.layer)));
  // The app's shortcuts (web/bindings.js). An open dialog keeps its keys,
  // but for the palette's and the shortcuts dialog's own; a text field
  // keeps the keys it types (a binding with Ctrl or Cmd still works, and
  // one with Alt except on a Mac, where Option types letters: ś, µ).
  document.addEventListener("keydown", (e) => {
    if (e.defaultPrevented) return;
    const id = bindings.match(e);
    if (!id || action(id).group !== "app") return;
    const inDialog = e.composedPath().some((n) => n instanceof Element && n.matches("dialog[open]"));
    if (inDialog && id !== "palette" && id !== "shortcuts") return;
    const t = e.target;
    const typing = t instanceof HTMLTextAreaElement || (t instanceof HTMLInputElement && !["checkbox", "radio", "button", "file"].includes(t.type));
    if (typing && !e.ctrlKey && !e.metaKey && (!e.altKey || isMac)) return;
    e.preventDefault();
    if (e.repeat && id !== "darker" && id !== "lighter") return;
    Promise.resolve(runBinding(id, { backend, store, memory, bindings }))
      .catch((err) => store.set({ message: String(err?.message ?? err), messageError: true }));
  });
  // The palette by touch: the buttons, and in fullscreen the search icon
  // and a swipe down on the display (sat-calculator's `sat-palette`).
  const openPalette = () => {
    setSheetOpen(false);
    ui.palette.open();
  };
  for (const b of [ui.paletteShow, ui.barPalette, ui.fsPalette]) b.addEventListener("click", blurAfter(openPalette));
  // The command line, edited here: the controls beside the calculator.
  watchCommandLine(backend, store);
  for (const b of [ui.cmdlineEdit, ui.barEdit]) {
    b.addEventListener("click", blurAfter(() => {
      setSheetOpen(false);
      ui.palette.openEditor({ kind: "cmdline" });
    }));
  }
  document.addEventListener("sat-palette", () => {
    if (!ui.palette.isOpen()) openPalette();
  });
  // Every label of a key follows the bindings; a change is kept.
  const showBindings = () => {
    const palette = bindings.labelOf("palette");
    ui.paletteShow.querySelector("kbd").textContent = palette;
    ui.paletteShow.querySelector("kbd").hidden = !palette;
    ui.paletteShow.title = `Command palette: commands, variables and actions${palette ? ` (${palette})` : ""}`;
    ui.barPalette.title = `Command palette${palette ? ` (${palette})` : ""}`;
    ui.layerShow.title = `The calculator's variables, stack and flags, and the command reference${keyHint(bindings, "layer")}`;
    ui.controls.setShortcutsKey(bindings.labelOf("shortcuts"));
    ui.layer.showKeys();
  };
  bindings.onChange(() => {
    prefs.set("keys", JSON.stringify(bindings));
    showBindings();
  });
  showBindings();
  // The layout's own labels for the physical keys, where the browser tells them (Chromium).
  navigator.keyboard?.getLayoutMap?.()
    .then((map) => bindings.setLayout(map))
    .catch(() => { /* not allowed here: US labels */ });
  resizable("panel");
  resizable("layer");
  window.addEventListener("resize", () => {
    for (const edge of Object.keys(EDGES)) {
      if (document.body.style.getPropertyValue(EDGES[edge].prop)) setWidth(edge, Number(prefs.get(EDGES[edge].pref)) || currentWidth(edge), false);
    }
  });
  setLayerOpen(memory, prefs.get("layer") === "open");
  // Hidden: the calculator's state is saved now (iteration 27); on a phone
  // this may be the last moment before the system ends the page.
  document.addEventListener("visibilitychange", () => backend.visibility(document.hidden));
  window.addEventListener("pagehide", () => backend.visibility(true));
  window.addEventListener("pageshow", () => backend.visibility(document.hidden));
  backend.visibility(document.hidden);
  setPanelHidden(prefs.get("panel") === "hidden");

  // The remembered ROMs; the last model boots if its ROM is there.
  const started = ui.controls.startRoms();

  // The installed page: offline and updates, the screen on through a long
  // computation, the kept ROMs kept for good where the user and the
  // browser agree: asked after a ROM is kept, never on load.
  const pwa = installServiceWorker(backend.host, store).catch(() => null);
  const screenOn = keepScreenOnWhileComputing(store);
  const storageChoice = new StorageChoice(backend, store, prefs);
  storageChoice.read();
  showStorageOffer(store, storageChoice, document, ui.controls.querySelector(".storage-offer"));
  ui.controls.addEventListener("sat-rom-kept", () => storageChoice.offer());
  ui.controls.addEventListener("sat-keep-storage", () => storageChoice.keep());

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
    /** The keyboard shortcuts (web/bindings.js) and their dialog. */
    bindings,
    shortcuts: ui.shortcuts,
    reference,
    /** Resolves to `{build}` once a service worker controls the page, or null without one (web/pwa.js). */
    pwa,
    /** Whether the screen is kept on for a long computation (null without the Wake Lock API). */
    screenOn: () => screenOn?.held() ?? null,
    /** Persistent storage for the kept ROMs, asked for after one is kept (web/pwa.js). */
    storageChoice,
    /** Whether the calculator is shown fullscreen, edge to edge. */
    get fullscreen() { return isFullscreen(); },
  };
}

main().catch((err) => {
  let status = document.getElementById("status");
  if (!status) {
    status = document.createElement("p");
    status.className = "status";
    document.querySelector(".panel")?.append(status);
  }
  status.textContent = startFailure(err, Boolean(window.__TAURI__));
  status.classList.add("error");
});
