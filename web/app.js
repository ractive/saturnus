// saturnus web UI: the calculator view's composition root,
// `mountCalculator(root, {shown})` (iteration 50c; the document's shell,
// web/main.js, mounts it on the route #/calculator). Picks the backend (the wasm core in a Web Worker, or
// the Tauri app's native core), feeds its events into the store and hands
// both to the components; keeps the view's chrome (side panel and memory
// view with their widths, drop-down sheet, fullscreen), the app's keyboard
// shortcuts (web/bindings.js), the preferences and the installed page's
// parts (web/pwa.js). Nothing runs at import. No framework, no bundler.
// See web/README.md and web/protocol.md.

import { WorkerBackend } from "./backend.js";
import { nativeHub } from "./native-hub.js";
import { Bindings, action } from "./bindings.js";
import { stepContrast } from "./contrast.js";
import { editButtonState, editTarget } from "./editor.js";
import { ariaKeys } from "./bindings.js";
import { WRITABLE_MODELS, notCalculatorFiles, orderModels } from "./norom.js";
import { dragResize } from "./shared/resize.js";
import { Store, connect } from "./store.js";
import { watchMessages, watchWriteMessages } from "./status.js";
import { MemoryView } from "./memory.js";
import { lockZoom } from "./nozoom.js";
import { MemoryWrites, calculatorFiles, dropRefusal, screenRefusal } from "./writes.js";
import { ReferenceLoader } from "./palette.js";
import { lookOf } from "./screenshot.js";
import { installServiceWorker, keepScreenOnWhileComputing } from "./pwa.js";
import { loadIcons } from "./shared/components/icons.js";
import { showNote } from "./shared/components/note.js";
import { TOAST_MS, routeToasts, say } from "./shared/components/toast.js";
import "./components/sat-calculator.js";
import "./components/sat-controls.js";
import "./components/sat-about.js";
import { NO_MEMORY_VIEW } from "./components/sat-explorer.js";
import "./components/sat-palette.js";
import "./components/sat-shortcuts.js";
import "./components/sat-tour.js";
import { chaptersFor } from "./tour.js";
import { startFailure } from "./failure.js";
import { themeOf } from "./shared/theme.js";
import { KEYS as PREFS } from "./shared/storage-names.js";
import { prefs } from "./shared/prefs.js";
import { watchLogo } from "./logo.js";
import { CLOCK_MODELS, watchClockOnStart } from "./clock.js";
import { setView, shown } from "./view.js";
import { applySiteLinks, connectModel, inDesktopApp, siteHref } from "./shared/site-links.js";

const isMac = /Mac|iPhone|iPad/.test(navigator.platform ?? "");

/** Where the memory view takes the calculator's place, not beside it (style.css). */
const LAYER_COVERS = "(max-width: 999px)";

/** The view's root (`#calculator-view`), which carries the layout's classes (style.css). */
let root = null;
const $ = (id) => root.querySelector(`#${id}`);
/** The view's elements, found once it is mounted (`findParts`). */
let ui = null;

function findParts() {
  const one = (sel) => root.querySelector(sel);
  return {
    panel: $("panel"),
    stage: $("stage"),
    calc: one("sat-calculator"),
    controls: one("sat-controls"),
    about: one("sat-about"),
    barFullscreen: $("bar-fullscreen"),
    leaveFullscreen: $("leave-fullscreen"),
    fsPalette: $("fs-palette"),
    panelHide: $("panel-hide"),
    panelShow: $("panel-show"),
    barMenu: $("bar-menu"),
    layer: one("sat-explorer"),
    layerShow: $("layer-show"),
    barMemory: $("bar-memory"),
    palette: one("sat-palette"),
    paletteShow: $("palette-show"),
    barPalette: $("bar-palette"),
    cmdlineEdit: $("cmdline-edit"),
    barEdit: $("bar-edit"),
    shortcuts: one("sat-shortcuts"),
    tour: one("sat-tour"),
    panelResize: $("panel-resize"),
    layerResize: $("layer-resize"),
  };
}

/**
 * The resizable edges: the side panel's right edge and the memory view's
 * left edge. Widths in CSS pixels, kept per browser (`prefs`), within
 * these limits: the minimums keep every control whole, the memory view
 * leaves the calculator its 300px, and beside an open memory view the
 * panel leaves the view its minimum (style.css `--layer-col`, `--panel-col`).
 */
const EDGES = {
  panel: { handle: "panelResize", pref: "panelWidth", prop: "--panel-w", min: 236, max: () => (layerBeside() ? Math.max(236, Math.min(480, window.innerWidth - 300 - 380)) : 480), grows: 1 },
  layer: { handle: "layerResize", pref: "layerWidth", prop: "--layer-w", min: 380, max: () => Math.max(380, Math.min(960, window.innerWidth - currentWidth("panel") - 300)), grows: -1 },
};

/** The memory view open as a column beside the calculator (not over it). */
function layerBeside() {
  return root.classList.contains("layer-open") && !matchMedia(LAYER_COVERS).matches;
}

function currentWidth(edge) {
  const e = EDGES[edge];
  if (edge === "panel") return root.classList.contains("panel-hidden") ? 0 : ui.panel.getBoundingClientRect().width;
  return ui.layer.getBoundingClientRect().width || e.min;
}

/** Set an edge's width (clamped); `save` keeps it, null forgets it (the default again). */
function setWidth(edge, px, save = true) {
  const e = EDGES[edge];
  const h = ui[e.handle];
  if (px === null) {
    root.style.removeProperty(e.prop);
    if (save) prefs.remove(e.pref);
  } else {
    const w = Math.round(Math.min(e.max(), Math.max(e.min, px)));
    root.style.setProperty(e.prop, `${w}px`);
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
  root.classList.toggle("panel-hidden", hidden);
  ui.panelShow.hidden = !hidden;
  prefs.set("panel", hidden ? "hidden" : "shown");
}

function setSheetOpen(open) {
  root.classList.toggle("sheet-open", open);
  ui.barMenu.setAttribute("aria-expanded", String(open));
}

/**
 * Open or close the memory view beside (or, on a narrow window, over) the
 * calculator; `remember: false` leaves the remembered choice as it was
 * (the page closing it by itself).
 */
function setLayerOpen(memory, open, { remember = true } = {}) {
  root.classList.toggle("layer-open", open);
  ui.layer.hidden = !open;
  ui.layerShow.hidden = open;
  for (const b of [ui.layerShow, ui.barMemory]) b.setAttribute("aria-expanded", String(open));
  if (remember) prefs.set("layer", open ? "open" : "closed");
  if (open) setSheetOpen(false);
  else if (ui.layer.contains(document.activeElement)) document.activeElement.blur();
  // The panel's kept width, clamped beside the view, back when it closes.
  refit?.();
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
    say(store, "Fullscreen is not allowed here.", true);
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
    say(store, "Could not leave fullscreen. Press Esc to leave it.", true);
  }
}

function toggleFullscreen(store, bindings) {
  setSheetOpen(false);
  return isFullscreen() ? exitFullscreen(store, bindings) : enterFullscreen(store, bindings);
}

/**
 * Screen to a variable…: the screen as the page shows it now, kept while
 * the memory view's name field asks for the name (in the calculator's
 * current directory). Refused with the reason in the status line.
 */
async function screenToVariable(store, memory) {
  const s = store.state;
  const why = screenRefusal(s);
  if (why) return store.set({ message: why, messageError: true });
  const frame = s.frame;
  await (s.layer || setLayerOpen(memory, true));
  ui.layer.setTab("vars");
  return ui.layer.openScreenName(frame);
}

/** Screen to stack: the screen as the page shows it now, on stack level 1 as a graphic. */
function screenToStack(store) {
  const s = store.state;
  const why = screenRefusal(s);
  if (why) return store.set({ message: why, messageError: true });
  return ui.layer.writes?.screenToStack(s.frame);
}

/**
 * The app's actions as the palette offers them: what the panel's buttons
 * do, by name. Built when the palette opens, so the titles follow the
 * state (Pause or Run, the speed that is on).
 */
function appActions(backend, store, memory, bindings, ai = null) {
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
  // The home page: where calculators are started, given a ROM and removed (iteration 53).
  const home = { id: "home", title: "Switch calculator", description: "The home page: start, add or remove a calculator.", keywords: "switch calculator model rom change choose new remove delete home start", run: () => { location.hash = "#/"; } };
  // The Edit button's own words: what it edits, or why it cannot.
  const edit = editState(store, bindings);
  // The screen images in the look chosen in the panel; the twins in the
  // other look are found by a search only.
  const look = s.screenLook === "bw" ? ", black on white" : "";
  const otherLook = s.screenLook === "bw" ? ", in the LCD's colours" : ", black on white";
  return [
    // After the palette has closed, which ends its search.
    { id: "edit", title: edit.off ? "Edit" : edit.title.replace(/ \(.*\)$/, ""), description: `Opens it in the editor${keyHint(bindings, "edit")}.`, off: edit.off ? edit.title : null, keywords: "edit editor command line stack level variable change", run: () => setTimeout(() => editShortcut(backend, store), 0) },
    // With no calculator running, the way to one comes first.
    ...(s.booted ? [] : [home]),
    ...(s.booted ? [
      { id: "run", title: s.running ? "Pause the calculator" : "Run the calculator", description: "Stops the calculator, or lets it run again.", keywords: "pause run stop resume", run: () => backend.pause(s.running) },
      { id: "reset", title: "Reset the calculator", description: "Restarts the calculator; its memory is kept.", keywords: "reset restart", run: () => backend.reset() },
      { id: "save", title: "Save state", description: `Saves the calculator's whole state in ${dialog ? "the app" : "this browser"}, one per model. It replaces the last one.`, keywords: "save state snapshot", run: () => ui.controls.saveState() },
      home,
      ...(CLOCK_MODELS.has(s.booted) ? [{ id: "set-clock", title: "Set the clock", description: "Sets the calculator's date and time to this computer's.", keywords: "clock time date set now", run: () => ui.controls.setClockNow() }] : []),
      { id: "darker", title: "Darker display", description: `One step darker, as ON and + on the calculator${keyHint(bindings, "darker")}.`, keywords: "contrast darker display lcd on plus", run: () => stepContrast(backend, store, true) },
      { id: "copy-screen", title: "Copy screen", description: `Copy the display as a PNG image${look}${keyHint(bindings, "copyScreen")}.`, keywords: "copy screen screenshot image picture png clipboard display lcd", run: () => ui.calc.copyScreen(s.screenLook) },
      { id: "copy-screen-other", title: `Copy screen (${s.screenLook === "bw" ? "LCD colours" : "black on white"})`, description: `Copy the display as a PNG image${otherLook}.`, keywords: "copy screen screenshot image picture png clipboard display lcd black white", searchOnly: true, run: () => ui.calc.copyScreen(s.screenLook === "bw" ? "lcd" : "bw") },
      { id: "save-screen", title: "Save screen", description: `Save the display as a PNG file${look}${keyHint(bindings, "saveScreen")}.`, keywords: "save screen screenshot image picture png file download display lcd", run: () => ui.calc.saveScreen(s.screenLook) },
      { id: "save-screen-other", title: `Save screen (${s.screenLook === "bw" ? "LCD colours" : "black on white"})`, description: `Save the display as a PNG file${otherLook}.`, keywords: "save screen screenshot image picture png file download display lcd black white", searchOnly: true, run: () => ui.calc.saveScreen(s.screenLook === "bw" ? "lcd" : "bw") },
      // The screen as a graphic (GROB): off, saying why, where the memory cannot be written.
      { id: "screen-to-var", title: "Screen to a variable…", description: `Store the display as a graphic (GROB) in a variable you name, in the current directory${keyHint(bindings, "screenToVar")}.`, keywords: "screen screenshot grob graphic picture lcd variable store sto", off: screenRefusal(s)?.replace(/\.$/, "") ?? null, run: () => setTimeout(() => screenToVariable(store, memory), 0) },
      { id: "screen-to-stack", title: "Screen to stack", description: `Put the display as a graphic (GROB) on stack level 1${keyHint(bindings, "screenToStack")}.`, keywords: "screen screenshot grob graphic picture lcd stack level", off: screenRefusal(s)?.replace(/\.$/, "") ?? null, run: () => screenToStack(store) },
      { id: "lighter", title: "Lighter display", description: `One step lighter, as ON and − on the calculator${keyHint(bindings, "lighter")}.`, keywords: "contrast lighter display lcd on minus", run: () => stepContrast(backend, store, false) },
      // After the palette has closed, which gives the focus back to the page.
      { id: "load", title: "Load state", description: "Loads the state you saved for this model; asks first.", off: s.canLoad ? null : "No saved state for this model", keywords: "load state restore snapshot", run: () => setTimeout(() => ui.controls.loadState(), 0) },
      { id: "load-file", title: "Load state from a file…", description: "Loads a .sat file; asks first.", keywords: "load state restore snapshot file open import sat", run: () => setTimeout(() => ui.controls.loadStateFromFile(), 0) },
      { id: "save-file", title: "Save state to a file…", description: "The calculator's whole state as a .sat file. A 49G state contains the 49G's ROM.", keywords: "save state snapshot file download export sat", run: () => ui.controls.saveStateToFile() },
      // After the palette has closed, which gives the focus back to the page.
      { id: "fresh", title: "Start fresh", description: "Restart with empty memory, like a new calculator. Asks first; your saved state stays.", keywords: "start fresh new cold boot clear memory wipe empty", run: () => setTimeout(() => ui.controls.startFresh(), 0) },
    ] : []),
    speed("1", "1×"), speed("2", "2×"), speed("4", "4×"), speed("max", "Max"),
    { id: "vars", title: "Variables", description: "The calculator's variables, read live.", keywords: "memory explorer variables directory", run: layerTab("vars") },
    { id: "stack", title: "Stack", description: "The calculator's stack, level by level.", keywords: "memory explorer stack levels", run: layerTab("stack") },
    { id: "flags", title: "Flags", description: "The calculator's flags, with what each one means.", keywords: "memory explorer flags toggle", run: layerTab("flags") },
    { id: "commands", title: "Browse the reference by menu", description: "The command reference, by the calculator's menus.", keywords: "commands reference menu browse help", run: layerTab("commands") },
    // The docs' command reference (iteration 57), on this model, in a new tab; not in the desktop app, which has no docs.
    ...(inDesktopApp() ? [] : [{ id: "docs-commands", title: "Command reference", description: "Every command of the 12 models with examples, in the docs. Opens a new tab.", keywords: "commands reference docs documentation manual help web page", run: () => window.open(`${siteHref("commands", location.pathname)}${s.booted ?? s.model ? `#${s.booted ?? s.model}` : ""}`, "_blank", "noopener") }]),
    { id: "layer", title: s.layer ? "Hide the memory view" : "Show the memory view", description: `The memory view beside the calculator${keyHint(bindings, "layer")}.`, keywords: "memory explorer toggle layer", run: () => setLayerOpen(memory, !s.layer) },
    ...["system", "light", "dark"].map((t) => ({
      id: `theme-${t}`,
      title: `Theme: ${t[0].toUpperCase()}${t.slice(1)}${s.theme === t ? " (on)" : ""}`,
      description: t === "system" ? "The app's colours follow the device's light or dark setting." : `${t[0].toUpperCase()}${t.slice(1)} colours, whatever the device's setting.`,
      keywords: "theme colours colors dark light mode system appearance",
      run: () => ui.controls.setTheme(t),
    })),
    { id: "fullscreen", title: isFullscreen() ? "Leave fullscreen" : "Fullscreen", description: `The calculator alone, edge to edge${keyHint(bindings, "fullscreen")}.`, keywords: "fullscreen full screen", run: () => toggleFullscreen(store, bindings) },
    // After the palette has closed, which gives the focus back to the page.
    { id: "shortcuts", title: "Keyboard shortcuts", description: `What each key does; change the keys for ON, α, the shifts and the app's actions${keyHint(bindings, "shortcuts")}.`, keywords: "keyboard shortcuts keys bindings rebind hotkeys layout", run: () => setTimeout(() => ui.shortcuts.open(), 0) },
    { id: "panel", title: root.classList.contains("panel-hidden") ? "Show the controls panel" : "Hide the controls panel", description: "The panel with the calculator, Reset, saved states and the speed.", keywords: "panel controls sidebar toggle", run: () => setPanelHidden(!root.classList.contains("panel-hidden")) },
    { id: "about", title: "About saturnus", description: "What saturnus is, what it was built from, and the manuals.", keywords: "about sources manuals licence", run: () => ui.about.open() },
    // Files, on this calculator when it can reach it (its route, web/router.js).
    // AI access (web/ai.js), where there is a relay; after the palette has closed.
    ...(ai ? [
      { id: "ai", title: "Connect your AI app…", description: window.__TAURI__ ? "Let AI apps on this computer (Claude Code, Codex, VS Code, Cursor, …) use the calculators." : "Let your AI app (Claude, ChatGPT, Claude Code, …) use the calculators in this tab.", keywords: "ai assistant mcp claude chatgpt agent connect llm tool", run: () => setTimeout(() => openAi(ai), 0) },
      { id: "ai-calls", title: aiLogView?.isOpen ? "Hide the AI's calls" : "Show the AI's calls", description: "Every call your AI app makes here, with its arguments and result, and the connection's events.", keywords: "ai calls log mcp tool history claude chatgpt debug", run: () => setTimeout(() => toggleAiLog(ai), 0) },
    ] : []),
    { id: "open-tx", title: "Open Files", description: "Move files between calculators, emulated or on a cable, and this computer.", keywords: "files tx transfer send receive kermit serial cable backup computer", run: () => { location.hash = filesHref(store); } },
    // The tour, and each chapter alone (found by a search).
    { id: "tour", title: "Show me around", description: "A short tour of the app. Esc ends it.", keywords: "tour help guide introduction tutorial show around new start", run: () => startTour("start") },
    ...chaptersFor({ booted: s.booted }).map((c) => ({ id: `tour-${c.id}`, title: `Tour: ${c.title}`, description: `The tour's chapter on ${c.title[0].toLowerCase()}${c.title.slice(1)}.`, keywords: "tour help guide introduction tutorial", searchOnly: true, run: () => startTour(c.id) })),
  ];
}

/**
 * Files' route from here (web/router.js): its left pane on the running
 * calculator when it can reach it (`connectModel`: a kept ROM's, of a
 * model with a serial port), else Files as it is.
 */
function filesHref(store) {
  const booted = store.state.booted;
  const model = connectModel({ booted, kept: Boolean(booted) && ui.controls.slot(booted)?.state === "ready" });
  return model ? `#/files/${model}` : "#/files";
}

/**
 * The panel's note when another tab keeps a model's state so this one
 * does not (the hub's `autoSaveOff`, also said in the status line).
 */
function watchUnshared(backend, store) {
  const note = $("unshared-note");
  if (!note || backend.host !== "worker") return;
  /** The models whose state another tab keeps, with what is said; only the running one's is shown. */
  const off = new Map();
  const show = () => {
    const line = off.get(store.state.booted) ?? "";
    note.textContent = line;
    note.hidden = !line;
  };
  backend.addEventListener("autoSaveOff", (e) => {
    off.set(e.detail.model, e.detail.message);
    show();
    store.set({ message: e.detail.message, messageError: false });
  });
  // The other tab let go: this one keeps the state again.
  backend.addEventListener("autoSaveOn", (e) => {
    off.delete(e.detail.model);
    show();
  });
  store.watch(["booted"], show);
}

/**
 * The panel's link to Files: its route on the running calculator when
 * Files can reach it (`filesHref`); none on a model without a serial
 * port (the 27S, the 42S, …: Files has nothing for them).
 */
function watchTxLink(store) {
  const a = $("to-transfer");
  const set = () => {
    const booted = store.state.booted;
    a.hidden = Boolean(booted) && !globalThis.SATURNUS_SERIAL.includes(booted);
    a.setAttribute("href", filesHref(store));
  };
  store.watch(["booted", "roms"], set);
  set();
}

/** Open "Connect your AI app" (web/ai-dialog.js) for AI access `ai` (a promise of web/ai.js's `AiAccess`). */
function openAi(ai) {
  Promise.all([ai, import("./ai-dialog.js")]).then(([a, d]) => d.openAiDialog(a), (err) => console.error("saturnus: AI access did not load", err));
}

/**
 * Show or hide "The AI's calls" (web/ai-log.js) for AI access `ai` (a
 * promise); `from` gets the focus back when it closes. `restore`: open
 * it as this browser left it.
 */
function toggleAiLog(ai, from = null, { restore = false } = {}) {
  Promise.all([ai, import("./ai-log.js")]).then(([a, { AiLogView }]) => {
    aiLogView ??= new AiLogView($("ai-log"), { log: a.log, say: (text, { error = false } = {}) => say(aiStore, text, error), prefs });
    if (restore) aiLogView.open({ remember: false });
    else aiLogView.toggle({ from });
  }, (err) => console.error("saturnus: AI access did not load", err));
}

/**
 * The sidebar's AI access entry (iteration 56): it says the state, opens
 * "Connect your AI app" on a click, and the line below it offers what to
 * do (Show; Calls and Disconnect; Calls and Turn off), read out by a live
 * region. While an AI app is connected the calculator keeps running in a
 * hidden tab (`keepRunning`, web/ai.js `keepsRunning`). The client is
 * loaded as soon as the entry shows, so the click's activation is not
 * spent on loading it.
 */
function watchAi(ai, store) {
  const button = $("ai-connect");
  const label = button.querySelector(".ai-label");
  const dot = button.querySelector(".ai-dot");
  const chip = $("ai-chip");
  const live = $("ai-live");
  aiStore = store;
  button.hidden = false;
  button.addEventListener("click", () => openAi(ai));
  const link = (text, run) => {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "link";
    b.textContent = text;
    b.addEventListener("click", run);
    return b;
  };
  Promise.all([ai, import("./ai-log.js")]).then(([a, { aiView }]) => {
    a.load().catch(() => {});
    // Open as it was left in this browser.
    if (prefs.get("aiLog") === "open") toggleAiLog(ai, null, { restore: true });
    const render = () => {
      if (keepRunning !== a.keepsRunning) {
        keepRunning = a.keepsRunning;
        visibility?.(document.hidden);
      }
      const v = aiView(a);
      label.textContent = v.entry;
      dot.hidden = !v.dot;
      dot.dataset.tone = v.dot ?? "";
      const calls = () => link("Calls", (e) => toggleAiLog(ai, e.currentTarget));
      const actions = {
        show: () => [link("Show", () => openAi(ai))],
        // The desktop app: one switch for every AI app, so its word is Turn off.
        connected: () => [calls(), " · ", link(a.local ? "Turn off" : "Disconnect", () => a.disconnect())],
        lost: () => [calls(), " · ", link("Turn off", () => a.disconnect())],
      }[v.actions];
      const line = actions ? actions() : v.note ? [v.note] : [];
      chip.replaceChildren(...line);
      chip.hidden = line.length === 0;
      if (live.textContent !== v.said) live.textContent = v.said;
    };
    a.addEventListener("change", render);
    render();
  }, () => { button.hidden = true; });
}

/**
 * Files dropped on the view (the memory view takes its own): those with
 * an HP transfer header, and `.txt` files that read as text, are
 * calculator files (`calculatorFiles`), stored in the running
 * calculator's current directory (`MemoryWrites.storeFiles`, whose
 * message is a toast); the rest are said not to be (a ROM among them:
 * ROMs are chosen on the home page, iteration 53). A hidden view
 * (view.js) takes no drops.
 */
function acceptDrops(backend, store, writes) {
  const hasFiles = (e) => [...(e.dataTransfer?.types ?? [])].includes("Files");
  const storeHere = async (files) => {
    // A calculator just booted sets up its memory first: asked again for a few seconds.
    let tree = null;
    for (let i = 0; i < 20; i++) {
      const s = store.state;
      if (!s.booted || !WRITABLE_MODELS.has(s.booted)) break;
      tree = await backend.memoryTree().catch(() => null);
      if (tree) break;
      await new Promise((r) => setTimeout(r, 500));
    }
    const why = dropRefusal({ booted: store.state.booted, memoryTree: tree });
    if (why) {
      say(store, why, true);
      return;
    }
    await writes.storeFiles(tree.path, files);
  };
  document.addEventListener("dragover", (e) => {
    if (!hasFiles(e) || !shown()) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    root.classList.add("file-drop");
  });
  document.addEventListener("dragleave", (e) => {
    if (!e.relatedTarget) root.classList.remove("file-drop");
  });
  document.addEventListener("drop", async (e) => {
    root.classList.remove("file-drop");
    if (!hasFiles(e) || !shown() || e.defaultPrevented) return;
    e.preventDefault();
    const { calc, rest } = await calculatorFiles([...e.dataTransfer.files]);
    if (rest.length) say(store, notCalculatorFiles(rest.map((f) => f.name)), true);
    if (calc.length) await storeHere(calc);
  });
}

/**
 * "Connected to Files · Show" under the status line while Files works
 * the calculator this view shows (the hub's `peers`). Show is Files'
 * route on that calculator (`#/files/<model>`: the view is shown, nothing
 * reloaded). No Disconnect: Files disconnects in Files (owner,
 * 2026-10-14). In the desktop app `peers` comes from web/native-hub.js.
 */
function watchTxPeer(backend, store) {
  const chip = $("tx-chip");
  const show = $("tx-show");
  if (!chip || !show) return;
  const set = () => show.setAttribute("href", store.state.booted ? `#/files/${store.state.booted}` : "#/files");
  store.watch(["booted"], set);
  set();
  backend.addEventListener("peers", (e) => {
    chip.hidden = !e.detail?.tx;
  });
}

/**
 * Whether the calculator has a command line open (`cmdlineOpen`), and
 * else its stack level 1 (`stackTop`: null for an empty stack, undefined
 * not read yet), read from RAM a moment after the screen changed:
 * what the Edit buttons would edit (`showEdit`). Only once the
 * calculator waits for a key: the frames of a running program change
 * nothing the buttons need until it stops, and a read then would cost
 * its time. Level 1 alone (`stackTop`, the host decodes no other level);
 * with the memory view open, its own stack read serves. A new machine
 * starts unknown at once.
 */
function watchCommandLine(backend, store) {
  let timer = null;
  let seq = 0;
  const read = async () => {
    timer = null;
    const s = store.state;
    if (!s.booted || !WRITABLE_MODELS.has(s.booted)) {
      store.set({ cmdlineOpen: false, stackTop: null });
      return;
    }
    // Computing or typing: keep the last answer; the end is read.
    if (s.busy || s.loop === "frame") return;
    const mine = ++seq;
    const booted = s.booted;
    try {
      const open = Boolean((await backend.commandLine()).active);
      const top = open || store.state.memoryStack ? {} : { stackTop: (await backend.stackTop()).level1 ?? null };
      if (mine === seq && store.state.booted === booted) store.set({ cmdlineOpen: open, ...top });
    } catch {
      // The memory not set up (the ROM still starting): nothing to edit.
      if (mine === seq && store.state.booted === booted) store.set({ cmdlineOpen: false, stackTop: null });
    }
  };
  // A read at most 250 ms after the first change, not after the last: a
  // blinking cursor changes the screen every ~120 ms, which would put a
  // debounced read off for as long as the command line stays open.
  const later = () => {
    timer ??= setTimeout(read, 250);
  };
  store.watch(["booted"], () => {
    seq++;
    store.set({ cmdlineOpen: false, stackTop: undefined });
    later();
  });
  // The memory view closing hands level 1 back to this read.
  store.watch(["frame", "busy", "loop", "layer"], later);
}

/**
 * The Edit buttons (the top bar's, the one over the calculator): gone on
 * a model without the editor, else off with the reason as their tooltip when nothing can be
 * edited, else naming what Cmd/Ctrl+E would edit (`editButtonState`).
 * `aria-disabled`, not `disabled`, so the tooltip shows and the focus can
 * reach them.
 */
function showEdit(store, bindings) {
  const { off, title } = editState(store, bindings);
  // A model without the editor (all but the 48SX, 48GX and 49G) shows no Edit at all.
  const none = Boolean(store.state.booted) && !WRITABLE_MODELS.has(store.state.booted);
  for (const b of [ui.cmdlineEdit, ui.barEdit]) {
    b.hidden = none;
    b.setAttribute("aria-disabled", String(off));
    b.title = title;
    b.setAttribute("aria-label", off ? `Edit: ${title}` : title.replace(/ \(.*\)$/, ""));
  }
}

/** What Edit would edit now, or why it cannot (`editButtonState`). */
function editState(store, bindings) {
  const s = store.state;
  const inView = ui.layer.hasFocus() || editHere === "view";
  return editButtonState({
    booted: s.booted,
    supported: WRITABLE_MODELS.has(s.booted),
    busy: Boolean(s.writing || s.busy),
    inView,
    picked: inView ? ui.layer.editSelection() : null,
    cmdline: s.cmdlineOpen,
    level1: s.memoryStack ? (s.memoryStack[0] ?? null) : s.stackTop,
    key: bindings.labelOf("edit"),
  });
}

/**
 * What the tour (components/sat-tour.js) may open, and how it puts the
 * page back: the controls (the side panel, the phone's sheet), the
 * calculator in sight, the memory view on a tab. Views only; the
 * calculator is never touched.
 */
function tourHooks(memory, store, bindings) {
  const phone = () => matchMedia("(max-width: 759px)").matches;
  // Below 1000 px the memory view lies over the calculator.
  const over = () => matchMedia("(max-width: 999px)").matches;
  const body = root.classList;
  const scrollers = () => [ui.panel, ui.stage];
  return {
    // The stage alone fills the screen in fullscreen: the tour runs on the page.
    prepare: async () => {
      if (!isFullscreen()) return;
      await exitFullscreen(store, bindings);
      for (let i = 0; i < 20 && document.fullscreenElement; i++) await new Promise((r) => setTimeout(r, 25));
    },
    snapshot: () => ({
      panelHidden: body.contains("panel-hidden"),
      sheet: body.contains("sheet-open"),
      layer: Boolean(store.state.layer),
      tab: ui.layer.tab,
      // Where the panel and the stage were scrolled to: a step scrolls its element into view.
      scroll: scrollers().map((e) => [e.scrollTop, e.scrollLeft]),
    }),
    async restore(snap) {
      if (Boolean(store.state.layer) !== snap.layer) await setLayerOpen(memory, snap.layer);
      if (ui.layer.tab !== snap.tab) ui.layer.setTab(snap.tab);
      if (body.contains("panel-hidden") !== snap.panelHidden) setPanelHidden(snap.panelHidden);
      setSheetOpen(snap.sheet);
      scrollers().forEach((e, i) => e.scrollTo(snap.scroll[i][1], snap.scroll[i][0]));
    },
    async setup(kind) {
      if (kind === "panel") {
        if (phone()) {
          if (store.state.layer) await setLayerOpen(memory, false);
          setSheetOpen(true);
        } else if (body.contains("panel-hidden")) setPanelHidden(false);
      } else if (kind === "calculator") {
        setSheetOpen(false);
        if (store.state.layer && over()) await setLayerOpen(memory, false);
      } else if (kind?.startsWith("layer:")) {
        if (!store.state.layer) await setLayerOpen(memory, true);
        const tab = kind.slice("layer:".length);
        if (ui.layer.tab !== tab) ui.layer.setTab(tab);
      }
    },
  };
}

/** Start the tour's chapter `id` once a dialog it came from (Search, About) has closed. */
function startTour(id) {
  setSheetOpen(false);
  setTimeout(() => ui.tour.start(id), 0);
}

/** ` (Alt+K)`: an action's key for a description, or "" when it has none or on touch, which has no keys. */
function keyHint(bindings, id) {
  if (matchMedia("(pointer: coarse)").matches) return "";
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
    case "screenToVar":
      return screenToVariable(store, memory);
    case "screenToStack":
      return screenToStack(store);
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
  // No pinch or double-tap zoom while fullscreen, and a zoom made before
  // taken back to scale 1 (web/nozoom.js); leaving frees it again.
  lockZoom(on);
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

async function main({ model = null, pwa: installed = null, onModel = null, ai = null } = {}) {
  applySiteLinks(root);
  // The icon sprite (shared/icons.svg), fetched once.
  loadIcons();
  // The desktop app: the one native calculator, shared with Files (web/native-hub.js).
  const backend = window.__TAURI__ ? nativeHub().backend : new WorkerBackend();
  const store = new Store();
  connect(backend, store);
  watchUnshared(backend, store);
  // The store's message clears as its toast goes (an error on the next action).
  watchMessages(store, { ms: TOAST_MS });
  watchWriteMessages(store);
  routeToasts(store);
  const hello = await backend.hello();
  const saved = prefs.get("model");
  // The plain button grid is gone; drop its old setting.
  try { localStorage.removeItem(PREFS.view); } catch { /* storage blocked */ }
  const models = orderModels(hello.models);
  store.set({
    host: hello.host,
    models,
    version: hello.version ?? null,
    // The remembered model, else the first in the page's order (theme-boot.js
    // picks the same for the logo before the first paint).
    model: saved && models.includes(saved) ? saved : models[0],
    speed: ["1", "2", "4", "max"].includes(prefs.get("speed")) ? prefs.get("speed") : "1",
    screenLook: lookOf(prefs.get("screenLook")),
    theme: themeOf(prefs.get("theme")),
  });
  watchLogo(store);
  if (backend.host === "tauri") {
    const tagline = root.querySelector(".tagline");
    if (tagline) tagline.textContent = "The HP 48SX, 48GX, 49G, 38G, 39G, 40G, 42S, 27S, 17BII, 17B, 28S and 19BII, emulated.";
  }

  const memory = new MemoryView(backend, store);
  const reference = new ReferenceLoader(new URL("./commands.json", import.meta.url));
  // The keyboard shortcuts, kept per browser (the app's webview keeps its
  // localStorage too).
  let savedKeys = null;
  try { savedKeys = JSON.parse(prefs.get("keys") ?? "null"); } catch { /* unreadable: the defaults */ }
  const bindings = new Bindings({ isMac, host: backend.host, saved: savedKeys });
  ui.controls.attach(backend, store, prefs);
  watchClockOnStart(backend, store, prefs);
  ui.calc.attach(backend, store, bindings);
  const writes = new MemoryWrites(backend, store);
  acceptDrops(backend, store, writes);
  ui.layer.attach(memory, store, prefs, { reference, backend, bindings, writes, edit: (target, opts) => ui.palette.openEditor(target, opts) });
  ui.about.setReference(reference);
  ui.about.setStore(store);
  ui.shortcuts.attach(bindings, { where: backend.host === "tauri" ? "Shortcuts are kept by the app." : "Shortcuts are kept in this browser." });
  ui.palette.attach(backend, store, {
    reference,
    bindings,
    actions: () => appActions(backend, store, memory, bindings, ai),
    onMenu: async (menu) => {
      await (store.state.layer || setLayerOpen(memory, true));
      ui.layer.showMenu(menu.path);
    },
  });
  backend.setSpeed(store.state.speed);

  document.addEventListener("sat-fullscreen", () => toggleFullscreen(store, bindings));
  ui.tour.attach({ hooks: tourHooks(memory, store, bindings), store, bindings, prefs, host: backend.romSource === "dialog" ? "app" : "browser", mac: isMac });
  // About's "Show me around".
  document.addEventListener("sat-tour", (e) => startTour(e.detail ?? "start"));
  document.addEventListener("sat-about", () => {
    setSheetOpen(false);
    ui.about.open();
  });
  // The panel's screen image buttons, in the look chosen there.
  document.addEventListener("sat-copy-screen", () => ui.calc.copyScreen(store.state.screenLook));
  document.addEventListener("sat-save-screen", () => ui.calc.saveScreen(store.state.screenLook));
  document.addEventListener("sat-screen-to-var", () => screenToVariable(store, memory));
  document.addEventListener("sat-screen-to-stack", () => screenToStack(store));
  document.addEventListener("sat-shortcuts", () => {
    setSheetOpen(false);
    ui.shortcuts.open();
  });
  ui.barFullscreen.addEventListener("click", blurAfter(() => toggleFullscreen(store, bindings)));
  ui.leaveFullscreen.addEventListener("click", blurAfter(() => exitFullscreen(store, bindings)));
  document.addEventListener("fullscreenchange", () => onFullscreenChange(bindings));
  ui.panelHide.addEventListener("click", blurAfter(() => setPanelHidden(true)));
  ui.panelShow.addEventListener("click", blurAfter(() => setPanelHidden(false)));
  ui.barMenu.addEventListener("click", blurAfter(() => setSheetOpen(!root.classList.contains("sheet-open"))));
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
  // A model without a memory view (the 42S, the aplet models): no button
  // for it, and the view closes, saying why when it was open.
  store.watch(["memorySupport"], (s) => {
    const none = s.memorySupport?.supported === false && !s.memorySupport.error;
    root.classList.toggle("no-memory-view", none);
    if (!none || !s.layer) return;
    setLayerOpen(memory, false, { remember: false });
    store.set({ message: NO_MEMORY_VIEW, messageError: false });
  });
  ui.barMemory.addEventListener("click", blurAfter(() => setLayerOpen(memory, !store.state.layer)));
  // The app's shortcuts (web/bindings.js). An open dialog keeps its keys,
  // but for the palette's and the shortcuts dialog's own; a text field
  // keeps the keys it types (a binding with Ctrl or Cmd still works, and
  // one with Alt except on a Mac, where Option types letters: ś, µ). A
  // hidden view (view.js) takes none.
  document.addEventListener("keydown", (e) => {
    if (e.defaultPrevented || !shown()) return;
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
  // Edit: what Cmd/Ctrl+E edits, where the keys are. A press does not
  // take the focus (it would take the keys from the memory view first).
  for (const b of [ui.cmdlineEdit, ui.barEdit]) {
    b.addEventListener("pointerdown", (e) => e.preventDefault());
    b.addEventListener("click", () => {
      // Off: say why under it (touch shows no tooltip).
      if (b.getAttribute("aria-disabled") === "true") {
        showNote(b, b.title);
        return;
      }
      setSheetOpen(false);
      editShortcut(backend, store).catch((err) => store.set({ message: String(err?.message ?? err), messageError: true }));
    });
  }
  const edit = () => showEdit(store, bindings);
  store.watch(["booted", "busy", "writing", "cmdlineOpen", "stackTop", "memoryStack", "layer"], edit);
  for (const type of ["focusin", "focusout", "pointerdown", "sat-selection"]) ui.layer.addEventListener(type, () => setTimeout(edit, 0));
  for (const type of ["pointerdown", "sat-key"]) ui.calc.addEventListener(type, () => setTimeout(edit, 0));
  bindings.onChange(edit);
  edit();
  document.addEventListener("sat-palette", () => {
    if (!ui.palette.isOpen()) openPalette();
  });
  // Every label of a key follows the bindings; a change is kept.
  const showBindings = () => {
    const palette = bindings.labelOf("palette");
    ui.paletteShow.querySelector("kbd").textContent = palette;
    ui.paletteShow.querySelector("kbd").hidden = !palette;
    ui.paletteShow.title = `Search${palette ? ` (${palette})` : ""}: commands, variables and actions`;
    for (const [b, id] of [[ui.paletteShow, "palette"], [ui.cmdlineEdit, "edit"], [ui.barEdit, "edit"]]) {
      const keys = bindings.keys(id).map(ariaKeys).join(" ");
      if (keys) b.setAttribute("aria-keyshortcuts", keys);
      else b.removeAttribute("aria-keyshortcuts");
    }
    ui.barPalette.title = `Search${palette ? ` (${palette})` : ""}: commands, variables and actions`;
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
  // The kept widths, clamped to the window again; a hidden view's when it
  // is shown again (`setShown`), as it measures nothing while hidden.
  refit = () => {
    for (const edge of Object.keys(EDGES)) {
      if (root.style.getPropertyValue(EDGES[edge].prop)) setWidth(edge, Number(prefs.get(EDGES[edge].pref)) || currentWidth(edge), false);
    }
  };
  window.addEventListener("resize", () => {
    if (shown()) refit();
  });
  // Where the view takes the calculator's place (a phone, a narrow
  // window: style.css), the page opens on the calculator.
  setLayerOpen(memory, prefs.get("layer") === "open" && !matchMedia(LAYER_COVERS).matches, { remember: false });
  // Hidden: the calculator's state is saved now (iteration 27); on a phone
  // this may be the last moment before the system ends the page. About
  // the document, not the view: a hidden view keeps its calculator running
  // unless its host says otherwise (`setShown(false, {visibility: true})`).
  // While an AI app's session is connected (web/ai.js), a hidden page
  // keeps its calculator running (owner, option 1, iteration 55).
  visibility = (hidden) => backend.visibility((hidden && !keepRunning) || viewHidden);
  document.addEventListener("visibilitychange", () => visibility(document.hidden));
  window.addEventListener("pagehide", () => backend.visibility(true));
  window.addEventListener("pageshow", () => visibility(document.hidden));
  visibility(document.hidden);
  setPanelHidden(prefs.get("panel") === "hidden");

  // The kept ROMs, and the model of the view's route (`#/calculator/<model>`,
  // web/router.js) shown: booted, or as it runs.
  ui.controls.openModel = model;
  const started = ui.controls.startRoms();
  showModel = (m) => started.catch(() => {}).then(() => ui.controls.showModel(m));
  refreshRoms = () => started.catch(() => {}).then(() => ui.controls.refreshRoms());
  // The host keeps its route on the model that runs (web/main.js).
  if (onModel) store.watch(["booted"], (s) => onModel(s.booted));
  watchTxLink(store);
  watchTxPeer(backend, store);
  if (ai) watchAi(ai, store);

  // The tour offered once, after the first start, until taken or dismissed;
  // put away for this visit at the first key pressed on the calculator.
  started.then(() => ui.tour.offer(), () => ui.tour.offer());
  ui.calc.addEventListener("sat-key", () => ui.tour.putAway(), { once: true });

  // The installed page: offline and updates, the screen on through a long
  // computation. The document's (web/main.js), or this view's own where
  // its host registers none.
  const pwa = (installed ?? installServiceWorker(backend.host, store)).catch(() => null);
  pwa.then((r) => r?.build && store.set({ build: r.build }));
  const screenOn = keepScreenOnWhileComputing(store);

  aiParts = { backend, store, reference };

  // Handle for debugging and automated checks.
  window.saturnus = {
    backend,
    store,
    /** Resolves once the ROM slots are read and the last model (if any) booted. */
    started,
    /** The display as text, `#` dark and `.` light. */
    screenText: () => ui.calc.screenText(),
    /** Boot the model shown (or the one the ROM fits) from a File, which is kept as its ROM (the tests' way in; people choose ROMs on the home page). */
    startWithRom: (file) => ui.controls.romCall(() => backend.chooseRom(store.state.model, [file])),
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
    /** The tour (components/sat-tour.js): `start(id)`, `step(dir)`, `end()`, `probe(id)`. */
    tour: ui.tour,
    reference,
    /** Resolves to `{build}` once a service worker controls the page, or null without one (web/pwa.js). */
    pwa,
    /** Whether the screen is kept on for a long computation (null without the Wake Lock API). */
    screenOn: () => screenOn?.held() ?? null,
    /** Whether the calculator is shown fullscreen, edge to edge. */
    get fullscreen() { return isFullscreen(); },
  };
}

/** Whether the host has said the view's calculator counts as hidden (`setShown`). */
let viewHidden = false;
/** Whether an AI app's session keeps the calculator running in a hidden page (`watchAi`). */
let keepRunning = false;
/** "The AI's calls" drawer (web/ai-log.js), once opened; the store its toasts go to. */
let aiLogView = null;
let aiStore = null;
/** What an AI app's tools use of the view: `{backend, store, reference}` (set in `main`). */
let aiParts = null;
/** `backend.visibility` with the view's part (set in `main`). */
let visibility = null;
/** The kept widths clamped to the window (set in `main`). */
let refit = null;
/** Show a model as the route asks (`#/calculator/<model>`, set in `main`). */
let showModel = null;
/** Read the kept ROMs again, which the home page changes (set in `main`). */
let refreshRoms = null;

/**
 * Mount the calculator view on `root` (`#calculator-view`, with the
 * view's markup inside: web/index.html), once. `shown()` says whether the
 * view is shown: its document- and window-level handlers (keys, paste,
 * drops, the palette's and the tour's keys, resize) ask it first
 * (web/view.js). A hidden view is not unmounted; its calculator keeps
 * running. `model`: the model to show first (the route's,
 * `#/calculator/<model>`); `pwa`: the document's service worker
 * (web/pwa.js `installServiceWorker`'s promise), else the view registers
 * it; `onModel(model)` hears the model that runs (null: none).
 *
 * Returns `ready` (resolves once the view is set up, or has said why it
 * could not; `window.saturnus.started` is the ROMs'), `show(model)` (the
 * route again on a mounted view) and `setShown(on, {visibility})`, which the host calls when it shows or
 * hides the view, after un-hiding its root: the widths are fitted again,
 * the kept ROMs read again and a running tour placed again on show, and with `visibility: true` a
 * hidden view counts as a hidden page for the calculator (web/protocol.md
 * `visibility`: a computing machine in the browser stops).
 */
export function mountCalculator(el, { shown: isShown = () => true, model = null, pwa = null, onModel = null, ai = null } = {}) {
  if (root) throw new Error("the calculator view is already mounted");
  root = el;
  setView(el, isShown);
  ui = findParts();
  const ready = main({ model, pwa, onModel, ai }).catch((err) => {
    let status = $("status");
    if (!status) {
      status = document.createElement("p");
      status.className = "status";
      ui.panel?.append(status);
    }
    status.textContent = startFailure(err, Boolean(window.__TAURI__));
    status.classList.add("error");
  });
  return {
    root: el,
    ready,
    show(m) {
      if (m) ready.then(() => showModel?.(m));
    },
    /** The view's calculator for an AI app's tools (web/ai.js): `{backend, store, reference}`, once set up. */
    ai: () => ready.then(() => aiParts),
    setShown(on, { visibility: counts = false } = {}) {
      const hidden = counts && !on;
      if (hidden !== viewHidden) {
        viewHidden = hidden;
        visibility?.(document.hidden);
      }
      if (on) {
        refit?.();
        refreshRoms?.();
        // A tour that ran on while the view was hidden: its ring and bubble measured again.
        if (ui.tour?.run) ui.tour.place();
      }
    },
  };
}
