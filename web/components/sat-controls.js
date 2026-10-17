// <sat-controls>: the panel's controls. The calculator shown (its model
// and ROM, read only, Change ROM… for it; "Switch calculator", the home
// page where calculators are started, given a ROM and removed, is the
// panel's link under Files in index.html, iteration 53),
// Reset, Set the clock (web/clock.js), state save and load, speed,
// fullscreen, the links to the keyboard shortcuts and About, and the
// status line.
// Renders from the store, acts through the backend; fullscreen, the
// shortcuts dialog and the About panel are the page's, asked for by
// `sat-fullscreen`, `sat-shortcuts` and `sat-about` events, as are the
// screen images (`sat-copy-screen`, `sat-save-screen`, in the look of
// the store's `screenLook`). Light DOM (display: contents).

import { screenRefusal } from "../writes.js";
import { stepContrast } from "../contrast.js";
import { MODEL_TITLES, NO_CALCULATOR } from "../norom.js";
import { confirmFresh } from "../fresh.js";
import { radioStep, radioTabIndexes } from "../shared/radiogroup.js";
import { setOff } from "../shared/disable.js";
import { LOOKS, hasScreen } from "../screenshot.js";
import { THEMES, applyTheme } from "../shared/theme.js";
import { closeMenu, openMenu, openMenuKey } from "../shared/components/menu.js";
import { confirmAction } from "../shared/components/confirm.js";
import { MAX_STATE_FILE, STATE_EXTENSIONS, loadError, loadFileQuestion, loadQuestion, savedDate, savedMessage, stateFileName, stateRefusal } from "../savedstates.js";
import { icon } from "../shared/components/icons.js";
import { say } from "../shared/components/toast.js";
import { CLOCK_MODELS, clockMessage, clockRefusal, setClock } from "../clock.js";

const SPEEDS = ["1", "2", "4", "max"];

const title = (m) => MODEL_TITLES[m] ?? m;

const SPEED_HINTS = {
  "1": "Normal speed.",
  "2": "Twice as fast; waits for keys at normal speed.",
  "4": "Four times as fast; waits for keys at normal speed.",
  max: "As fast as this device can; waits for keys at normal speed.",
};

/** Where Save state keeps it, per host, in its tooltip with no save yet (iteration 34). */
const SAVE_WHERE = { file: "in this browser", dialog: "in the app" };
const STATE_FILE_HINT = "The calculator's state as a .sat file, for another browser, the desktop app or the command line. A 49G state contains the 49G's ROM.";

/** Why a control is off, in its tooltip (it gets its own back when on). */
const NOT_STARTED = "Start the calculator first";

// A settings column (shared/components.css, "Section" and "Card"): one
// section per meaning, its title above a card. ROM: the calculator shown
// (its model and ROM, read only, Change ROM… beside them), its saved
// states and Reset, what belongs to the calculator's contents. Clock:
// Set the clock and its every-start switch (models with a clock). Speed,
// Display (the contrast's two steps, plain buttons), Screen images (the
// look, a real choice, and the four ways to take one), Theme; then the
// page's own rows, Fullscreen, the shortcuts and About, as one list.
// "Switch calculator" (the home page) is the panel's own link under
// Files (index.html, `.site-links`).
const TEMPLATE = `
  <section class="sec calc-now">
    <h2 class="sec-title">ROM</h2>
    <div class="card calc-card">
      <div class="card-row calc-line">
        <span class="row-main calc-name"><span id="calc-model" class="row-title calc-model"></span><span id="calc-rom" class="row-sub calc-rom"></span></span>
        <button id="calc-rom-pick" type="button" class="small" hidden>Change ROM…</button>
      </div>
      <input id="calc-rom-file" type="file" hidden>
      <div class="card-body">
        <div class="toolbar fill state-actions">
          <button id="save" type="button" disabled>${icon("save", "ic-sm")}Save state</button>
          <button id="load" type="button" disabled>${icon("load", "ic-sm")}Load state</button>
          <button id="state-more" type="button" class="icon more" disabled aria-haspopup="menu" aria-expanded="false" aria-label="More for saved states" title="More">${icon("more")}</button>
        </div>
      </div>
      <input id="state-file" type="file" accept="${STATE_EXTENSIONS.join(",")}" hidden>
      <div class="card-row">
        <span class="row-main"><span class="row-title">Restart the calculator</span><span class="row-sub">Its memory is kept.</span></span>
        <button id="reset" type="button" disabled>${icon("reset", "ic-sm")}Reset</button>
      </div>
    </div>
  </section>

  <section class="sec clock-sec" hidden>
    <h2 class="sec-title">Clock</h2>
    <div class="card">
      <div class="card-row">
        <span class="row-main"><span class="row-title">Date and time</span><span class="row-sub">This computer's, now.</span></span>
        <button id="set-clock" type="button" disabled hidden>${icon("clock", "ic-sm")}Set the clock</button>
      </div>
      <label class="card-row switch clock-start" hidden><input id="clock-start" type="checkbox"> <span>Set the clock on every start</span></label>
    </div>
  </section>

  <section class="sec">
    <h2 class="sec-title" id="speed-label">Speed</h2>
    <div class="card card-body">
      <div class="segmented fill" role="radiogroup" aria-labelledby="speed-label" id="speed">
        <button type="button" role="radio" data-speed="1">1×</button>
        <button type="button" role="radio" data-speed="2">2×</button>
        <button type="button" role="radio" data-speed="4">4×</button>
        <button type="button" role="radio" data-speed="max">Max</button>
      </div>
      <p class="card-hint hint" id="speed-hint">Normal speed.</p>
    </div>
  </section>

  <section class="sec">
    <h2 class="sec-title" id="contrast-label">Display</h2>
    <div class="card card-body">
      <div class="toolbar fill" role="group" aria-labelledby="contrast-label" id="contrast">
        <button type="button" data-darker="false" title="ON and −: a lighter display" disabled>${icon("lighter", "ic-sm")}Lighter</button>
        <button type="button" data-darker="true" title="ON and +: a darker display" disabled>${icon("darker", "ic-sm")}Darker</button>
      </div>
    </div>
  </section>

  <section class="sec">
    <h2 class="sec-title" id="screen-label">Screen images</h2>
    <div class="card card-body">
      <div class="segmented fill" role="radiogroup" aria-labelledby="screen-label" id="screen-look">
        <button type="button" role="radio" data-look="lcd" title="The display's own colours, at its contrast">LCD colours</button>
        <button type="button" role="radio" data-look="bw" title="Black pixels on white, whatever the contrast">Black on white</button>
      </div>
      <div class="toolbar fill screen-actions">
        <button id="copy-screen" type="button" disabled title="The display as a PNG image, to the clipboard">${icon("copy-screen", "ic-sm")}Copy screen</button>
        <button id="save-screen" type="button" disabled title="The display as a PNG file">${icon("save-screen", "ic-sm")}Save screen</button>
        <button id="screen-to-var" type="button" disabled title="The display as a graphic (GROB), stored in a variable you name">${icon("screen-var", "ic-sm")}To a variable…</button>
        <button id="screen-to-stack" type="button" disabled title="The display as a graphic (GROB), on stack level 1">${icon("screen-stack", "ic-sm")}To the stack</button>
      </div>
    </div>
  </section>

  <section class="sec">
    <h2 class="sec-title" id="theme-label">Theme</h2>
    <div class="card card-body">
      <div class="segmented fill" role="radiogroup" aria-labelledby="theme-label" id="theme">
        <button type="button" role="radio" data-theme-choice="system" title="Follow the device's light or dark setting">System</button>
        <button type="button" role="radio" data-theme-choice="light" title="Light colours, whatever the device's setting">Light</button>
        <button type="button" role="radio" data-theme-choice="dark" title="Dark colours, whatever the device's setting">Dark</button>
      </div>
    </div>
  </section>

  <section class="sec links">
    <div class="card">
      <button id="fullscreen" type="button" class="row-btn">${icon("expand", "ic-sm")}Fullscreen</button>
      <button id="shortcuts" type="button" class="row-btn">${icon("keyboard", "ic-sm")}<span>Keyboard shortcuts</span><kbd class="shortcuts-key kbd-hint" hidden></kbd></button>
      <button id="about" type="button" class="row-btn">${icon("info", "ic-sm")}<span>About saturnus and its sources</span></button>
    </div>
  </section>

  <p id="status" class="status" role="status">${NO_CALCULATOR}</p>
  <p id="tx-chip" class="tx-chip" role="status" hidden>Connected to Files · <a id="tx-show" class="link" href="#/files">Show</a></p>`;

export class SatControls extends HTMLElement {
  attach(backend, store, prefs) {
    this.backend = backend;
    this.store = store;
    this.prefs = prefs;
    this.innerHTML = TEMPLATE;
    const $ = (sel) => this.querySelector(sel);
    const ui = {
      calcModel: $("#calc-model"),
      calcRom: $("#calc-rom"),
      romPick: $("#calc-rom-pick"),
      romFile: $("#calc-rom-file"),
      reset: $("#reset"),
      setClock: $("#set-clock"),
      clockStart: $("#clock-start"),
      clockStartRow: $(".clock-start"),
      clockSec: $(".clock-sec"),
      save: $("#save"),
      load: $("#load"),
      stateMore: $("#state-more"),
      stateFile: $("#state-file"),
      speed: $("#speed"),
      speedHint: $("#speed-hint"),
      contrast: $("#contrast"),
      screenLook: $("#screen-look"),
      theme: $("#theme"),
      copyScreen: $("#copy-screen"),
      saveScreen: $("#save-screen"),
      screenToVar: $("#screen-to-var"),
      screenToStack: $("#screen-to-stack"),
      fullscreen: $("#fullscreen"),
      status: $("#status"),
      about: $("#about"),
    };
    this.ui = ui;
    // The handler's promise is not dropped: a failure it did not show is
    // shown here.
    const blurAfter = (fn) => (e) => {
      e.currentTarget.blur();
      Promise.resolve()
        .then(fn)
        .catch((err) => this.message(`${err?.message ?? err}`, true));
    };
    ui.reset.addEventListener("click", blurAfter(async () => {
      if (!store.state.booted) return;
      try {
        await backend.reset();
        this.message("");
      } catch (err) {
        this.message(`Could not reset: ${err?.message ?? err}`, true);
      }
    }));
    ui.reset.title = "Restart the calculator. Its memory is kept.";
    ui.setClock.title = "Set the calculator's date and time to this computer's.";
    ui.setClock.addEventListener("click", blurAfter(() => this.setClockNow()));
    ui.clockStart.checked = prefs.get("clockOnStart") === "on";
    ui.clockStart.addEventListener("change", () => {
      ui.clockStart.blur();
      prefs.set("clockOnStart", ui.clockStart.checked ? "on" : "off");
    });
    ui.clockStartRow.title = "Each time the calculator starts or a state is loaded, once it is idle.";
    ui.save.addEventListener("click", blurAfter(() => this.saveState()));
    ui.load.addEventListener("click", blurAfter(() => this.loadState()));
    ui.stateMore.addEventListener("click", (e) => {
      if (openMenuKey() === "states") closeMenu(e.detail === 0);
      else this.stateMenu();
    });
    ui.stateMore.addEventListener("keydown", (e) => {
      if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
      e.preventDefault();
      this.stateMenu(e.key === "ArrowUp");
    });
    ui.stateFile.addEventListener("change", () => {
      const file = ui.stateFile.files?.[0];
      ui.stateFile.value = "";
      if (file) this.loadStateFile(file).catch((err) => this.message(`${err?.message ?? err}`, true));
    });
    ui.speed.addEventListener("click", (e) => {
      const b = e.target.closest("button[data-speed]");
      if (!b) return;
      if (e.detail > 0) b.blur();
      this.setSpeed(b.dataset.speed);
    });
    // The radio group pattern: one tab stop, the arrows move the selection.
    ui.speed.addEventListener("keydown", (e) => {
      if (e.altKey || e.ctrlKey || e.metaKey) return;
      const v = radioStep(SPEEDS, store.state.speed, e.key);
      if (v === null) return;
      e.preventDefault();
      this.setSpeed(v);
      ui.speed.querySelector(`button[data-speed="${v}"]`)?.focus();
    });
    ui.contrast.addEventListener("click", (e) => {
      const b = e.target.closest("button[data-darker]");
      if (!b || !store.state.booted) return;
      b.blur();
      stepContrast(backend, store, b.dataset.darker === "true");
    });
    ui.screenLook.addEventListener("click", (e) => {
      const b = e.target.closest("button[data-look]");
      if (!b) return;
      if (e.detail > 0) b.blur();
      this.setScreenLook(b.dataset.look);
    });
    ui.screenLook.addEventListener("keydown", (e) => {
      if (e.altKey || e.ctrlKey || e.metaKey) return;
      const v = radioStep(LOOKS, store.state.screenLook, e.key);
      if (v === null) return;
      e.preventDefault();
      this.setScreenLook(v);
      ui.screenLook.querySelector(`button[data-look="${v}"]`)?.focus();
    });
    ui.theme.addEventListener("click", (e) => {
      const b = e.target.closest("button[data-theme-choice]");
      if (!b) return;
      if (e.detail > 0) b.blur();
      this.setTheme(b.dataset.themeChoice);
    });
    ui.theme.addEventListener("keydown", (e) => {
      if (e.altKey || e.ctrlKey || e.metaKey) return;
      const v = radioStep(THEMES, store.state.theme, e.key);
      if (v === null) return;
      e.preventDefault();
      this.setTheme(v);
      ui.theme.querySelector(`button[data-theme-choice="${v}"]`)?.focus();
    });
    // Inside the click: copying to the clipboard needs it.
    ui.copyScreen.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-copy-screen", { bubbles: true }));
    }));
    ui.saveScreen.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-save-screen", { bubbles: true }));
    }));
    ui.screenToVar.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-screen-to-var", { bubbles: true }));
    }));
    ui.screenToStack.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-screen-to-stack", { bubbles: true }));
    }));
    ui.fullscreen.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-fullscreen", { bubbles: true }));
    }));
    ui.about.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-about", { bubbles: true }));
    }));
    $("#shortcuts").addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-shortcuts", { bubbles: true }));
    }));

    store.watch(["booted"], (s) => {
      if (s.booted && s.model !== s.booted) store.set({ model: s.booted });
      this.refreshLoad();
    });
    store.watch(["booted", "romName", "wanted", "roms"], () => this.showCalc());
    // Change ROM… for the calculator shown, as the home page's row menu:
    // a file here (the app: its file dialog), and the calculator starts on it.
    ui.romPick.addEventListener("click", blurAfter(() => {
      const model = this.shownModel();
      if (!model) return;
      if (backend.romSource === "dialog") return this.changeRom(() => backend.chooseRom(model));
      ui.romFile.click();
    }));
    ui.romFile.addEventListener("change", () => {
      const model = this.shownModel();
      const files = [...ui.romFile.files];
      ui.romFile.value = "";
      if (model && files.length) this.changeRom(() => backend.chooseRom(model, files));
    });
    store.watch(["speed"], (s) => this.showSpeed(s.speed));
    store.watch(["screenLook"], (s) => this.showScreenLook(s.screenLook));
    store.watch(["theme"], (s) => this.showTheme(s.theme));
    // Off controls say why: no calculator, no saved state, no screen yet.
    const showOff = (s) => {
      // On, Save state and Load state name the save they replace or load.
      const at = s.canLoad && s.stateSaved ? savedDate(s.stateSaved) : null;
      ui.save.dataset.title = at
        ? `Save the calculator as it is now. Replaces the state saved ${at}.`
        : `Save the calculator as it is now, ${SAVE_WHERE[backend.romSource]}.`;
      ui.load.dataset.title = at ? `Go back to the state saved ${at}. What you did since is lost.` : "";
      for (const b of [ui.reset, ui.save, ui.stateMore, ...ui.contrast.querySelectorAll("button")]) setOff(b, !s.booted, NOT_STARTED);
      // Set the clock: only for a model with a clock; a busy calculator or
      // an open command line is said when it is pressed.
      const clock = CLOCK_MODELS.has(s.model);
      ui.setClock.hidden = !clock;
      ui.clockStartRow.hidden = !clock;
      ui.clockSec.hidden = !clock;
      setOff(ui.setClock, !s.booted || s.booted !== s.model, NOT_STARTED);
      setOff(ui.load, !s.canLoad, s.booted ? "No saved state for this model" : NOT_STARTED);
      // Copy and Save screen only with a screen to take (the model shown runs and has drawn).
      const why = s.booted === s.model ? "The calculator has not drawn its screen yet" : NOT_STARTED;
      for (const b of [ui.copyScreen, ui.saveScreen]) setOff(b, !hasScreen(s), why);
      // To a variable or the stack only where the memory can be written.
      const refusal = screenRefusal(s)?.replace(/\.$/, "");
      for (const b of [ui.screenToVar, ui.screenToStack]) setOff(b, Boolean(refusal), refusal);
    };
    store.watch(["booted", "model", "frame", "canLoad", "stateSaved", "writing", "busy", "cmdlineOpen"], showOff);
    showOff(store.state);
    store.watch(["booted", "romName", "running", "halted", "busy", "writing"], () => this.showStatus());
    this.showCalc();
    this.showSpeed(store.state.speed);
    this.showScreenLook(store.state.screenLook);
    this.showTheme(store.state.theme);
    this.showStatus();
  }

  /** What came of an action, shown as a toast (`routeToasts`); "" ends a step in progress. */
  message(text, isError = false) {
    if (text) say(this.store, text, isError);
    else this.store.set({ message: "", messageError: false });
  }

  // ---------------------------------------------------------- ROM slots

  /**
   * Run a ROM command (resolving to the slots) and show its result; an
   * error is shown and the slots read again. Resolves to the result, or
   * null.
   */
  async romCall(fn, failed = "Cannot start") {
    try {
      const r = await fn();
      if (!r) return null;
      const { booted, notice, bootError, start, ...roms } = r;
      this.store.set({ roms });
      if (booted) {
        this.prefs.set("model", booted.model);
        this.store.set({ message: "", messageError: false });
      } else if (bootError) {
        // The files are kept (the notice says what else was found); only
        // the boot failed.
        this.message(`Cannot start: ${bootError}`, true);
      }
      return r;
    } catch (err) {
      this.message(`${failed}: ${err?.message ?? err}`, true);
      await this.refreshRoms();
      return null;
    }
  }

  async refreshRoms() {
    try {
      const { booted, notice, bootError, start, ...roms } = await this.backend.romSlots();
      this.store.set({ roms });
    } catch (err) {
      this.message(`The kept ROMs cannot be read: ${err?.message ?? err}`, true);
    }
  }

  slot(model) {
    return this.store.state.roms?.slots.find((s) => s.model === model) ?? null;
  }

  /**
   * At start: read the slots and show the model the view was opened on
   * (its route, `#/calculator/<model>`: the home page, Files' "Show in
   * Calculator"); none asked for, nothing boots (web/router.js opens
   * the last calculator when the user asked for that).
   */
  async startRoms() {
    await this.refreshRoms();
    const asked = this.openModel;
    this.openModel = null;
    if (asked) await this.showModel(asked);
  }

  /**
   * Show `model` (its route, `#/calculator/<model>`): booted from its kept
   * ROM, or shown as it runs (the browser's hub attaches to it, or boots
   * it again on a ROM the home page changed); the app runs one
   * calculator, which a boot would start again, so a model it runs (by
   * the status heard, else the native hub's answer) is only resumed. Without a ROM the empty state says so, and the other
   * model's calculator pauses behind it; one whose file is gone or
   * changed is reported.
   */
  async showModel(model) {
    await this.refreshRoms();
    const s = this.store.state;
    this.store.set({ model, wanted: model });
    const slot = this.slot(model);
    if (slot?.state === "missing" || slot?.state === "changed") {
      const what = slot.state === "missing" ? "is no longer where it was" : "has changed since it was chosen";
      this.message(`${slot.fileName}, the ${title(model)} ROM, ${what}; choose it again on the home page.`, true);
      return;
    }
    if (slot?.state !== "ready") {
      if (s.booted && s.booted !== model && s.running) await this.backend.pause(true).catch(() => {});
      return;
    }
    if (s.booted === model && !s.running && !s.halted) await this.backend.pause(false).catch(() => {});
    if (this.backend.romSource === "dialog") {
      if (s.booted === model) return;
      // The home page's choice may have just booted it, its `status` not
      // here yet (Tauri does not order events before replies): asked.
      if ((await this.backend.hub?.knownRunning()) === model) return;
    }
    await this.romCall(() => this.backend.bootModel(model));
  }

  /** The model the view shows: the running one, else the route's (null: none). */
  shownModel() {
    return this.store.state.booted ?? this.store.state.wanted ?? null;
  }

  /** A ROM chosen for the calculator shown (`fn` the host's `chooseRom`): what else was found, said. */
  async changeRom(fn) {
    const r = await this.romCall(fn, "Cannot use that ROM");
    if (r?.notice) this.message(r.notice);
  }

  /** The calculator shown, read only: its model over its ROM (or what is missing); Change ROM… or Choose ROM… for it. */
  showCalc() {
    const s = this.store.state;
    const model = this.shownModel();
    this.ui.calcModel.textContent = model ? title(model) : "None";
    const slot = model ? this.slot(model) : null;
    this.ui.romPick.hidden = !model;
    this.ui.romPick.textContent = slot?.fileName ? "Change ROM…" : "Choose ROM…";
    const rom = s.booted ? s.romName : model ? (slot?.fileName ? `${slot.fileName}, not running` : "no ROM") : "";
    this.ui.calcRom.textContent = rom ?? "";
  }

  // The saved states (iteration 34), the same on both hosts: Save state
  // and Load state use the host's slot for the running model (IndexedDB,
  // or the app's data folder), the "⋯" menu's two commands a `.sat` file.

  /** Whether the running model has a saved state, and its time (`canLoad`, `stateSaved`). */
  async refreshLoad() {
    const model = this.store.state.booted;
    const saved = model ? (await this.backend.stateSlot(model).catch(() => null))?.saved ?? null : null;
    // A slower answer for a model booted before this one is stale.
    if (this.store.state.booted === model) this.store.set({ canLoad: Boolean(saved), stateSaved: saved });
  }

  /** Save state: into the slot at once; the message names the save it replaced. */
  async saveState() {
    const model = this.store.state.booted;
    if (!model) return;
    try {
      const before = (await this.backend.stateSlot(model).catch(() => null))?.saved ?? null;
      const { saved } = await this.backend.saveStateSlot(model);
      if (this.store.state.booted === model) this.store.set({ canLoad: true, stateSaved: saved ?? Date.now() });
      this.message(savedMessage(before));
    } catch (err) {
      this.message(`Could not save the state: ${err?.message ?? err}`, true);
    }
  }

  /** Load state: asks first, naming the save's date and time. */
  async loadState() {
    const model = this.store.state.booted;
    if (!model) return;
    await this.refreshLoad();
    const saved = this.store.state.stateSaved;
    if (!saved || this.store.state.booted !== model) {
      this.message("No saved state for this model.", true);
      return;
    }
    if (!(await confirmAction(loadQuestion(saved)))) return;
    try {
      await this.backend.loadStateSlot(model);
      this.started();
      this.message(`Loaded the state saved on ${savedDate(saved)}.`);
    } catch (err) {
      this.message(loadError(err, model, title), true);
    }
  }

  /** Save state to a file…: a download, or the app's save dialog, named after the model and the time. */
  async saveStateToFile() {
    const model = this.store.state.booted;
    if (!model) return;
    try {
      const where = await this.backend.saveStateFile(model, stateFileName(model, Date.now()));
      if (where) this.message(`State saved as ${where}.`);
    } catch (err) {
      this.message(`Could not save the state: ${err?.message ?? err}`, true);
    }
  }

  /** Load state from a file…: the page's file picker, or the app's open dialog. */
  async loadStateFromFile() {
    if (!this.store.state.booted) return;
    if (this.backend.romSource !== "dialog") {
      this.ui.stateFile.click();
      return;
    }
    let f;
    try {
      f = await this.backend.openStateFile();
    } catch (err) {
      // Too large, unreadable: the app's words, plainly.
      this.message(loadError(err, this.store.state.booted, title), true);
      return;
    }
    if (f) await this.loadStateBytes(f.name, f.bytes);
  }

  /** A state file the page's picker read (a File). */
  async loadStateFile(file) {
    // Not read when it cannot be a state.
    const bytes = file.size > MAX_STATE_FILE ? null : new Uint8Array(await file.arrayBuffer());
    await this.loadStateBytes(file.name, bytes);
  }

  /** Check the state `bytes` of the file `name`, ask, then load it. */
  async loadStateBytes(name, bytes) {
    const model = this.store.state.booted;
    if (!model) return;
    const refused = stateRefusal(bytes, model, title);
    if (refused) {
      this.message(refused, true);
      return;
    }
    if (!(await confirmAction(loadFileQuestion(name)))) return;
    try {
      await this.backend.loadStateBytes(bytes);
      this.started();
      this.message(`Loaded the state in ${name}.`);
    } catch (err) {
      this.message(loadError(err, model, title), true);
    }
  }

  /** The saved states' "⋯" menu: the two file commands. `last`: start on the last item. */
  stateMenu(last = false) {
    const b = this.ui.stateMore;
    if (b.disabled) return;
    b.setAttribute("aria-expanded", "true");
    openMenu({
      anchor: b,
      key: "states",
      label: "Saved states",
      last,
      returnFocus: () => b,
      onClose: () => b.setAttribute("aria-expanded", "false"),
      items: [
        { text: "Save state to a file…", icon: "save", title: STATE_FILE_HINT, run: () => this.saveStateToFile() },
        { text: "Load state from a file…", icon: "load", title: "Loads a .sat file saved here, in the desktop app or by the command line; asks first.", run: () => this.loadStateFromFile() },
      ],
    });
  }

  /**
   * Start the running model fresh, after the page's own question: a cold
   * boot with an empty memory, its auto-saved state forgotten (the
   * user's saved state stays).
   */
  async startFresh() {
    const model = this.store.state.booted;
    if (!model || !(await confirmFresh(title(model)))) return;
    const r = await this.romCall(() => this.backend.startFresh(model));
    if (r?.booted) {
      this.started();
      this.message("Started fresh, with an empty memory.");
    }
  }

  /** A start the page caused besides a boot (a state loaded, started fresh): `starts` (web/clock.js). */
  started() {
    this.store.set({ starts: this.store.state.starts + 1 });
  }

  /**
   * Set the clock: the page's refusal (no calculator, busy, a command
   * line open), else the host's (off, busy, its own words), else the
   * date and time it was set to.
   */
  async setClockNow() {
    const why = clockRefusal(this.store.state);
    if (why) {
      this.message(why, true);
      return;
    }
    const now = new Date();
    try {
      await setClock(this.backend, now);
      this.message(clockMessage(now, this.store.state.booted));
    } catch (err) {
      const text = String(err?.message ?? err);
      // The host's refusals are sentences; anything else is the calculator's.
      this.message(/^[A-Z].*\.$/.test(text) ? text : `Could not set the clock: ${text}`, true);
    }
  }

  setSpeed(value) {
    const speed = SPEEDS.includes(value) ? value : "1";
    this.prefs.set("speed", speed);
    this.store.set({ speed });
    this.backend.setSpeed(speed);
  }

  /** The look of the screen images ("lcd", "bw"), kept as a preference. */
  setScreenLook(value) {
    const look = LOOKS.includes(value) ? value : "lcd";
    this.prefs.set("screenLook", look);
    this.store.set({ screenLook: look });
  }

  /** The page's colour theme ("system", "light", "dark"), kept as a preference and applied at once. */
  setTheme(value) {
    const theme = THEMES.includes(value) ? value : "system";
    this.prefs.set("theme", theme);
    this.store.set({ theme });
  }

  showTheme(theme) {
    const buttons = [...this.ui.theme.querySelectorAll("button")];
    const tabs = radioTabIndexes(buttons.map((b) => b.dataset.themeChoice), theme);
    buttons.forEach((b, i) => {
      b.setAttribute("aria-checked", String(b.dataset.themeChoice === theme));
      b.tabIndex = tabs[i];
    });
    applyTheme(theme);
  }

  showScreenLook(look) {
    const buttons = [...this.ui.screenLook.querySelectorAll("button")];
    const tabs = radioTabIndexes(buttons.map((b) => b.dataset.look), look);
    buttons.forEach((b, i) => {
      b.setAttribute("aria-checked", String(b.dataset.look === look));
      b.tabIndex = tabs[i];
    });
  }

  showSpeed(speed) {
    const buttons = [...this.ui.speed.querySelectorAll("button")];
    const tabs = radioTabIndexes(buttons.map((b) => b.dataset.speed), speed);
    buttons.forEach((b, i) => {
      b.setAttribute("aria-checked", String(b.dataset.speed === speed));
      b.tabIndex = tabs[i];
    });
    this.ui.speedHint.textContent = SPEED_HINTS[speed];
  }

  /**
   * The status line: what runs ("HP 48SX · sxrom-j", paused, halted,
   * typing). What came of an action is a toast (web/shared/components/toast.js),
   * seen with the panel hidden too.
   */
  showStatus() {
    const s = this.store.state;
    let text;
    if (!s.booted) {
      text = NO_CALCULATOR;
    } else {
      const parts = [MODEL_TITLES[s.booted] ?? s.booted, s.romName];
      if (s.halted) parts.push(s.halted);
      else if (!s.running) parts.push("paused");
      if (s.busy) parts.push(s.writing ? "transferring…" : "typing…");
      text = parts.filter(Boolean).join(" · ");
    }
    const st = this.ui.status;
    if (st.textContent !== text) st.textContent = text;
    st.classList.toggle("error", Boolean(s.halted));
  }

  /** The shortcuts dialog's own key, shown beside its link ("" for none). */
  setShortcutsKey(label) {
    const k = this.querySelector(".shortcuts-key");
    k.textContent = label;
    k.hidden = !label;
  }

  /** The fullscreen button's label and icon. */
  setFullscreenLabel(on) {
    this.ui.fullscreen.innerHTML = on ? `${icon("collapse", "ic-sm")}Leave fullscreen` : `${icon("expand", "ic-sm")}Fullscreen`;
  }
}

customElements.define("sat-controls", SatControls);
