// <sat-controls>: the panel's controls. Model and ROM, Reset, state save
// and load, speed, fullscreen, the links to the keyboard shortcuts and
// About, and the status line. Renders from the store, acts through the
// backend; fullscreen, the shortcuts dialog and the About panel are the
// page's, asked for by `sat-fullscreen`, `sat-shortcuts` and `sat-about`
// events, as are the screen images (`sat-copy-screen`, `sat-save-screen`,
// in the look of the store's `screenLook`). Light DOM (display: contents).

import { MODEL_TITLES } from "./sat-calculator.js";
import { stepContrast } from "../contrast.js";
import { WRITABLE_MODELS, dropNotice, orderModels, switchModel } from "../norom.js";
import { confirmForget, confirmFresh } from "../fresh.js";
import { radioStep, radioTabIndexes } from "../radiogroup.js";
import { setOff } from "../disable.js";
import { LOOKS, hasScreen } from "../screenshot.js";
import { THEMES, applyTheme } from "../theme.js";
import { removeQuestion, removedMessage, rowAction, sharedModels } from "../romrows.js";
import { closeMenu, openMenu } from "./menu.js";
import { confirmAction } from "./confirm.js";
import { icon, iconEl } from "./icons.js";

const SPEEDS = ["1", "2", "4", "max"];

/** The ROM hints per host and whether this browser keeps ROMs. */
const ROM_HINTS = {
  kept: "Kept in this browser so you do not have to pick it again; never uploaded.",
  app: "The app remembers where each ROM file is and reads it from there; it never copies or uploads it.",
};
/** Whether the browser keeps them for good (`storage`, `StorageChoice` in web/pwa.js). */
const STORAGE_STATES = {
  persistent: "Stored permanently",
  "best-effort": "May be cleared when space runs low",
};
/** Where the ROMs come from, per host (iteration 20b). */
const SOURCE_HINTS = {
  file: "Each Download link goes to the model's page on hpcalc.org: download the zip there, unzip it and drop the file on this page, or choose it. The ROMs are HP's software, hosted by hpcalc.org with HP's permission for use with emulators; they are not part of saturnus.",
  dialog: "Download… fetches a model's ROM from hpcalc.org after asking, checks its checksum and keeps it in the app's data folder. The ROMs are HP's software, hosted by hpcalc.org with HP's permission for use with emulators; they are not part of saturnus.",
};
const FORGET_HINTS = {
  file: "Choose several files at once, or drop them on the page: each goes to its model. Remove ROMs removes the ROMs from this browser, and the saved 49G state (it contains the 49G's ROM); other saved states stay. A row's Change menu removes one.",
  dialog: "The other ROMs in the folder of the one you choose are recognised and go to their models. Remove ROMs takes them all off the list (a row's Change menu takes one); the files and saved states stay.",
};

const title = (m) => MODEL_TITLES[m] ?? m;

const SPEED_HINTS = {
  "1": "Normal speed.",
  "2": "Twice as fast; waits for keys at normal speed.",
  "4": "Four times as fast; waits for keys at normal speed.",
  max: "As fast as this device can; waits for keys at normal speed.",
};

/** Why a control is off, in its tooltip (it gets its own back when on). */
const NOT_STARTED = "Start the calculator first";

const TEMPLATE = `
  <section class="group">
    <label class="field">Model
      <select id="model"></select>
    </label>
    <div class="field">ROM
      <div class="rom-current">
        <span id="rom-name" class="rom-name">none chosen</span>
        <button id="rom-pick" type="button">Choose…</button>
      </div>
    </div>
    <input id="rom" type="file" multiple hidden>
    <p class="hint rom-hint"></p>
    <div class="storage-offer"></div>
    <div id="rom-notice" class="rom-notice" role="status" hidden>
      <p class="rom-notice-text"></p>
      <div class="rom-offers"></div>
    </div>
    <details class="help roms" id="roms">
      <summary>${icon("chevron-right")}<span>ROMs of every model <span class="rom-count"></span></span></summary>
      <table class="rom-slots"><tbody></tbody></table>
      <p class="hint source-hint"></p>
      <label class="check"><input id="boot-last" type="checkbox" checked> <span class="boot-last-label">Start the last model when the page opens</span></label>
      <p class="rom-storage" hidden><span class="storage-state"></span> <button id="rom-keep" type="button" hidden>Keep permanently</button></p>
      <p class="rom-forget"><button id="rom-forget" type="button">Remove ROMs…</button></p>
      <p class="hint forget-hint"></p>
    </details>
  </section>

  <section class="group actions">
    <button id="reset" type="button" disabled>${icon("reset", "ic-sm")}Reset</button>
    <button id="save" type="button" disabled>${icon("save", "ic-sm")}Save state</button>
    <button id="load" type="button" disabled>${icon("load", "ic-sm")}Load state</button>
  </section>

  <section class="group">
    <div class="field-label" id="speed-label">Speed</div>
    <div class="segmented" role="radiogroup" aria-labelledby="speed-label" id="speed">
      <button type="button" role="radio" data-speed="1">1×</button>
      <button type="button" role="radio" data-speed="2">2×</button>
      <button type="button" role="radio" data-speed="4">4×</button>
      <button type="button" role="radio" data-speed="max">Max</button>
    </div>
    <p class="hint" id="speed-hint">Normal speed.</p>
    <div class="contrast-row">
      <span class="field-label" id="contrast-label">Display</span>
      <div class="segmented" role="group" aria-labelledby="contrast-label" id="contrast">
        <button type="button" data-darker="false" title="ON and −: a lighter display" disabled>${icon("lighter", "ic-sm")}Lighter</button>
        <button type="button" data-darker="true" title="ON and +: a darker display" disabled>${icon("darker", "ic-sm")}Darker</button>
      </div>
    </div>
    <div class="contrast-row screen-row">
      <span class="field-label" id="screen-label">Screen images</span>
      <div class="segmented" role="radiogroup" aria-labelledby="screen-label" id="screen-look">
        <button type="button" role="radio" data-look="lcd" title="The display's own colours, at its contrast">LCD colours</button>
        <button type="button" role="radio" data-look="bw" title="Black pixels on white, whatever the contrast">Black on white</button>
      </div>
      <div class="screen-actions">
        <button id="copy-screen" type="button" disabled title="The display as a PNG image, to the clipboard">${icon("copy-screen", "ic-sm")}Copy screen</button>
        <button id="save-screen" type="button" disabled title="The display as a PNG file">${icon("save-screen", "ic-sm")}Save screen</button>
      </div>
    </div>
    <div class="contrast-row theme-row">
      <span class="field-label" id="theme-label">Theme</span>
      <div class="segmented" role="radiogroup" aria-labelledby="theme-label" id="theme">
        <button type="button" role="radio" data-theme-choice="system" title="Follow the device's light or dark setting">System</button>
        <button type="button" role="radio" data-theme-choice="light" title="Light colours, whatever the device's setting">Light</button>
        <button type="button" role="radio" data-theme-choice="dark" title="Dark colours, whatever the device's setting">Dark</button>
      </div>
    </div>
  </section>

  <section class="group actions">
    <button id="fullscreen" type="button">${icon("expand", "ic-sm")}Fullscreen</button>
  </section>

  <p class="panel-link"><button id="shortcuts" type="button" class="link">Keyboard shortcuts</button> <kbd class="shortcuts-key kbd-hint" hidden></kbd></p>
  <p class="about-link"><button id="about" type="button" class="link">About saturnus and its sources</button></p>

  <p id="status" class="status" role="status">Pick a model and its ROM to start.</p>`;

export class SatControls extends HTMLElement {
  attach(backend, store, prefs) {
    this.backend = backend;
    this.store = store;
    this.prefs = prefs;
    this.innerHTML = TEMPLATE;
    const $ = (sel) => this.querySelector(sel);
    const ui = {
      model: $("#model"),
      rom: $("#rom"),
      romPick: $("#rom-pick"),
      reset: $("#reset"),
      save: $("#save"),
      load: $("#load"),
      speed: $("#speed"),
      speedHint: $("#speed-hint"),
      contrast: $("#contrast"),
      screenLook: $("#screen-look"),
      theme: $("#theme"),
      copyScreen: $("#copy-screen"),
      saveScreen: $("#save-screen"),
      fullscreen: $("#fullscreen"),
      status: $("#status"),
      about: $("#about"),
      romName: $("#rom-name"),
      romHint: $(".rom-hint"),
      romStorage: $(".rom-storage"),
      romKeep: $("#rom-keep"),
      romNotice: $("#rom-notice"),
      romSlots: $(".rom-slots tbody"),
      romCount: $(".rom-count"),
      bootLast: $("#boot-last"),
      romForget: $("#rom-forget"),
    };
    this.ui = ui;
    const dialog = backend.romSource === "dialog";
    /** The model the file picker is choosing for. */
    this.pickFor = null;
    ui.romHint.textContent = dialog ? ROM_HINTS.app : ROM_HINTS.kept;
    $(".forget-hint").textContent = FORGET_HINTS[backend.romSource];
    $(".source-hint").textContent = SOURCE_HINTS[backend.romSource];
    if (dialog) $(".boot-last-label").textContent = "Start the last model when the app starts";

    const blurAfter = (fn) => (e) => {
      e.currentTarget.blur();
      fn();
    };
    ui.model.addEventListener("change", () => {
      prefs.set("model", ui.model.value);
      store.set({ model: ui.model.value });
      this.bootSelected(ui.model.value, true);
    });
    ui.rom.addEventListener("change", () => {
      const files = [...(ui.rom.files ?? [])];
      ui.rom.value = "";
      if (files.length) this.chooseFiles(this.pickFor ?? store.state.model, files);
    });
    ui.romPick.addEventListener("click", blurAfter(() => this.chooseFor(store.state.model)));
    // A row's Choose… or its menu, and an offer's Use.
    this.addEventListener("click", (e) => {
      const b = e.target.closest("button[data-choose], button[data-rom-menu], button[data-offer]");
      if (!b) return;
      if (b.dataset.romMenu) {
        this.romMenu(b);
        return;
      }
      b.blur();
      if (b.dataset.choose) this.chooseFor(b.dataset.choose);
      else this.takeOffer(b.dataset.model, Number(b.dataset.offer));
    });
    ui.bootLast.addEventListener("change", () => {
      ui.bootLast.blur();
      this.romCall(() => backend.romSettings(ui.bootLast.checked));
    });
    // Asked first: in the browser the ROMs and the saved 49G state are deleted.
    ui.romForget.addEventListener("click", blurAfter(async () => {
      if (!(await confirmForget(dialog))) return;
      const running = this.store.state.booted;
      if (await this.romCall(() => backend.forgetRom())) {
        if (running) await this.backend.unload();
        this.message(dialog ? "ROMs removed from the list. The files stay where they are." : "ROMs and the saved 49G state removed. Other saved states stay.");
      }
    }));
    // Keep for good: the page's `StorageChoice` calls persist() within this click.
    ui.romKeep.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-keep-storage", { bubbles: true }));
    }));
    this.acceptDrops(dialog);
    ui.reset.addEventListener("click", blurAfter(async () => {
      if (!store.state.booted) return;
      try {
        await backend.reset();
        this.message("");
      } catch (err) {
        this.message(`Could not reset: ${err?.message ?? err}`, true);
      }
    }));
    ui.save.addEventListener("click", blurAfter(() => this.saveState()));
    ui.load.addEventListener("click", blurAfter(() => this.loadState()));
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
    ui.fullscreen.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-fullscreen", { bubbles: true }));
    }));
    ui.about.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-about", { bubbles: true }));
    }));
    $("#shortcuts").addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-shortcuts", { bubbles: true }));
    }));

    store.watch(["models", "model"], (s) => this.fillModels(s));
    store.watch(["booted"], (s) => {
      if (s.booted && ui.model.value !== s.booted) {
        ui.model.value = s.booted;
        store.set({ model: s.booted });
      }
      this.refreshLoad();
    });
    store.watch(["speed"], (s) => this.showSpeed(s.speed));
    store.watch(["screenLook"], (s) => this.showScreenLook(s.screenLook));
    store.watch(["theme"], (s) => this.showTheme(s.theme));
    // Off controls say why: no calculator, no saved state, no screen yet.
    const showOff = (s) => {
      for (const b of [ui.reset, ui.save, ...ui.contrast.querySelectorAll("button")]) setOff(b, !s.booted, NOT_STARTED);
      setOff(ui.load, !s.canLoad, s.booted ? "No saved state for this model" : NOT_STARTED);
      // Copy and Save screen only with a screen to take (the model shown runs and has drawn).
      const why = s.booted === s.model ? "The calculator has not drawn its screen yet" : NOT_STARTED;
      for (const b of [ui.copyScreen, ui.saveScreen]) setOff(b, !hasScreen(s), why);
    };
    store.watch(["booted", "model", "frame", "canLoad"], showOff);
    showOff(store.state);
    store.watch(["booted", "romName", "running", "halted", "message", "messageError", "busy", "writing"], () => this.showStatus());
    store.watch(["roms", "romNotice", "model", "storage"], () => this.showRoms());
    this.fillModels(store.state);
    this.showSpeed(store.state.speed);
    this.showScreenLook(store.state.screenLook);
    this.showTheme(store.state.theme);
    this.showStatus();
  }

  message(text, isError = false) {
    this.store.set({ message: text, messageError: isError });
  }

  fillModels(s) {
    const sel = this.ui.model;
    if (sel.options.length !== s.models.length) {
      sel.replaceChildren();
      for (const name of s.models) {
        const o = document.createElement("option");
        o.value = name;
        o.textContent = MODEL_TITLES[name] ?? name;
        sel.append(o);
      }
    }
    if (s.model && sel.value !== s.model) sel.value = s.model;
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
      const { booted, notice, bootError, ...roms } = r;
      this.store.set({ roms, romNotice: notice ?? this.store.state.romNotice });
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
      const { booted, notice, bootError, ...roms } = await this.backend.romSlots();
      this.store.set({ roms });
    } catch (err) {
      this.message(`The kept ROMs cannot be read: ${err?.message ?? err}`, true);
    }
  }

  slot(model) {
    return this.store.state.roms?.slots.find((s) => s.model === model) ?? null;
  }

  /**
   * At start: read the slots and, unless a machine already runs (the app
   * keeps its machine across a reload), boot the last model if its ROM is
   * there and the setting asks for it.
   */
  async startRoms() {
    await this.refreshRoms();
    const r = this.store.state.roms;
    if (!r || this.store.state.booted || !r.bootLast || !r.lastModel) return;
    const last = this.slot(r.lastModel);
    if (last?.state === "missing" || last?.state === "changed") {
      // Reported, not asked for: no dialog opens by itself at start.
      this.store.set({ model: r.lastModel });
      const what = last.state === "missing" ? "is no longer where it was" : "has changed since it was chosen";
      this.message(`${last.fileName}, the ${title(r.lastModel)} ROM, ${what}; choose it again.`, true);
      return;
    }
    if (last?.state !== "ready") return;
    this.store.set({ model: r.lastModel });
    await this.romCall(() => this.backend.bootModel(r.lastModel));
  }

  /**
   * The selected model changed: resume it if it is the machine that runs,
   * else boot it from its remembered ROM, else pause the other model's
   * machine and ask for the ROM (`switchModel`). One that is missing or
   * changed is reported and, in the app, asked for again (`ask`); nothing
   * else boots in its place.
   */
  async bootSelected(model, ask) {
    let done;
    try {
      done = await switchModel(this.backend, this.store.state, model, this.slot(model));
    } catch (err) {
      this.message(String(err?.message ?? err), true);
      return;
    }
    if (done === "resumed") {
      this.message("");
      return;
    }
    if (done === "no-rom") {
      this.message(`Choose the ${title(model)} ROM to start it.`);
      return;
    }
    const r = await this.romCall(() => this.backend.bootModel(model));
    // Asked again only for a file that is gone or changed (the slots are
    // read again after the failure), not after any failed boot.
    const state = this.slot(model)?.state;
    if (!r?.booted && ask && this.backend.romSource === "dialog" && (state === "missing" || state === "changed")) {
      await this.chooseFor(model);
    }
  }

  /** Choose `model`'s ROM: the app's dialog, or this page's file picker. */
  async chooseFor(model) {
    this.dispatchEvent(new CustomEvent("sat-sheet", { bubbles: true, detail: false }));
    if (this.backend.romSource === "dialog") {
      await this.romCall(() => this.backend.chooseRom(model));
      return;
    }
    this.pickFor = model;
    this.ui.rom.click();
  }

  /**
   * Download `model`'s ROM from hpcalc.org (the app only: it asks first,
   * keeps the file and boots it). A failure names the page to download
   * it from by hand.
   */
  async downloadFor(model) {
    if (!this.backend.downloadRom) return;
    this.dispatchEvent(new CustomEvent("sat-sheet", { bubbles: true, detail: false }));
    this.message(`Downloading the ${title(model)} ROM from hpcalc.org…`);
    const before = this.keptFiles();
    const r = await this.romCall(() => this.backend.downloadRom(model), `Cannot download the ${title(model)} ROM`);
    if (r === null && !this.store.state.messageError) this.message("");
    this.kept(before, r);
  }

  /** ROM files from the picker or a drop, for `model`. */
  async chooseFiles(model, files) {
    this.dispatchEvent(new CustomEvent("sat-sheet", { bubbles: true, detail: false }));
    const before = this.keptFiles();
    return this.kept(before, await this.romCall(() => this.backend.chooseRom(model, files)));
  }

  async takeOffer(model, offer) {
    const before = this.keptFiles();
    return this.kept(before, await this.romCall(() => this.backend.takeOffer(model, offer)));
  }

  /** The kept ROM of each model, `fileName|revision` (`slots` or the store's). */
  keptFiles(slots = this.store.state.roms?.slots ?? []) {
    return new Map(slots.filter((s) => s.fileName).map((s) => [s.model, `${s.fileName}|${s.revision ?? ""}`]));
  }

  /**
   * After the user picked, dropped or downloaded a ROM: `sat-rom-kept`
   * only if this action kept one (a slot's file differs from `before`,
   * `keptFiles` taken first), for the page to offer keeping it for good;
   * a rejected file keeps nothing and offers nothing. Passes `r` (a
   * `romCall` result) through.
   */
  kept(before, r) {
    const after = this.keptFiles(r?.slots ?? []);
    if ([...after].some(([model, file]) => before.get(model) !== file)) {
      this.dispatchEvent(new CustomEvent("sat-rom-kept", { bubbles: true }));
    }
    return r;
  }

  /**
   * Files dropped anywhere on the page (the memory view takes its own)
   * are ROMs for the selected model; the app takes no ROM by drop
   * (`dialog`). Files that went nowhere are said so (`dropped`).
   */
  acceptDrops(dialog) {
    const hasFiles = (e) => [...(e.dataTransfer?.types ?? [])].includes("Files");
    document.addEventListener("dragover", (e) => {
      if (!hasFiles(e)) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      if (!dialog) document.body.classList.add("rom-drop");
    });
    document.addEventListener("dragleave", (e) => {
      if (!e.relatedTarget) document.body.classList.remove("rom-drop");
    });
    document.addEventListener("drop", (e) => {
      document.body.classList.remove("rom-drop");
      if (!hasFiles(e)) return;
      e.preventDefault();
      const files = [...e.dataTransfer.files];
      if (!files.length) return;
      if (dialog) this.dropped(files, null);
      else this.chooseFiles(this.store.state.model, files).then((r) => this.dropped(files, r));
    });
  }

  /**
   * After a drop (`r` the `chooseRom` result, null in the app): files
   * that are not ROMs are said so beside the ROMs, pointing to the memory
   * view when the running calculator takes files, and the controls come
   * into view (`sat-show-controls`) so the message is seen.
   */
  dropped(files, r) {
    const s = this.store.state;
    const text = dropNotice(files.map((f) => f.name), r, this.backend.romSource, WRITABLE_MODELS.has(s.booted));
    if (!text) return;
    this.store.set({ romNotice: text });
    this.dispatchEvent(new CustomEvent("sat-show-controls", { bubbles: true }));
  }

  showRoms() {
    const dialog = this.backend.romSource === "dialog";
    const s = this.store.state;
    const r = s.roms;
    const ui = this.ui;
    const mine = this.slot(s.model);
    const missing = mine && mine.state !== "ready" && mine.state !== "empty";
    ui.romName.textContent = mine?.fileName
      ? `${mine.fileName}${missing ? ` (${mine.state})` : ""}`
      : "none chosen";
    ui.romName.title = mine?.revision ?? "";
    ui.romName.classList.toggle("error", Boolean(missing));
    ui.romPick.textContent = mine?.fileName ? "Change…" : "Choose…";
    ui.romPick.setAttribute("aria-label", `${mine?.fileName ? "Change" : "Choose"} the ${title(s.model)} ROM`);
    if (r?.note) ui.romHint.textContent = r.note;
    else ui.romHint.textContent = this.backend.romSource === "dialog" ? ROM_HINTS.app : ROM_HINTS.kept;
    ui.romHint.classList.toggle("error", Boolean(r?.note));
    const storage = r?.slots.some((x) => x.fileName) ? STORAGE_STATES[s.storage] : null;
    ui.romStorage.querySelector(".storage-state").textContent = storage ? `${storage}.` : "";
    ui.romStorage.hidden = !storage;
    ui.romKeep.hidden = s.storage !== "best-effort";
    if (!r) return;
    ui.bootLast.checked = r.bootLast;

    const rows = orderModels(r.slots, (x) => x.model).map((slot) => {
      const tr = document.createElement("tr");
      const th = document.createElement("th");
      th.scope = "row";
      th.textContent = title(slot.model);
      const name = document.createElement("td");
      name.className = "rom-file-name";
      const d = slot.download;
      if (slot.fileName) {
        name.textContent = `${slot.fileName}${slot.state !== "ready" ? ` (${slot.state})` : ""}`;
      } else if (d && !dialog) {
        // The page cannot fetch from hpcalc.org: its page, and the file
        // name to expect.
        const a = document.createElement("a");
        a.href = d.page;
        a.target = "_blank";
        a.rel = "noopener noreferrer";
        // The cell is narrow: the file name to expect is in the tooltip
        // and in the display's own message.
        a.textContent = "Download";
        a.title = `Download from hpcalc.org, unzip, then drop ${d.file} on this page (${d.revision})`;
        a.setAttribute("aria-label", `Download ${d.file}, the ${title(slot.model)} ROM, from hpcalc.org`);
        name.append(a);
      } else {
        name.textContent = "none";
        name.classList.add("muted");
      }
      if (slot.revision) name.title = slot.revision;
      name.classList.toggle("error", slot.state === "missing" || slot.state === "changed");
      const act = document.createElement("td");
      act.className = "rom-act";
      act.append(this.rowButton(slot, dialog));
      tr.append(th, name, act);
      return tr;
    });
    ui.romSlots.replaceChildren(...rows);
    const kept = r.slots.filter((x) => x.fileName).length;
    ui.romCount.textContent = `(${kept} of ${r.slots.length})`;
    ui.romForget.disabled = kept === 0;

    const offers = r.offers.flatMap((o) => o.models.map((m) => {
      const b = document.createElement("button");
      b.type = "button";
      b.dataset.offer = String(o.id);
      b.dataset.model = m;
      b.textContent = `Use ${o.fileName} for the ${title(m)}`;
      return b;
    }));
    ui.romNotice.querySelector(".rom-offers").replaceChildren(...offers);
    ui.romNotice.querySelector(".rom-notice-text").textContent = s.romNotice;
    ui.romNotice.hidden = !s.romNotice && !offers.length;
  }

  /** A row's one button: Choose…, or Add/Change with its menu (web/romrows.js). */
  rowButton(slot, app) {
    const a = rowAction(slot, app);
    const b = document.createElement("button");
    b.type = "button";
    b.className = "rom-row-button";
    if (a.kind === "choose") {
      b.dataset.choose = slot.model;
      b.textContent = a.label;
      b.setAttribute("aria-label", `Choose the ${title(slot.model)} ROM`);
      return b;
    }
    b.dataset.romMenu = slot.model;
    b.setAttribute("aria-haspopup", "menu");
    b.setAttribute("aria-expanded", "false");
    b.setAttribute("aria-label", `${a.label} the ${title(slot.model)} ROM`);
    b.append(a.label, iconEl("chevron-down"));
    return b;
  }

  /** The menu of a row's Add or Change button. */
  romMenu(button) {
    const model = button.dataset.romMenu;
    const slot = this.slot(model);
    const a = rowAction(slot, this.backend.romSource === "dialog");
    const run = {
      choose: () => this.chooseFor(model),
      download: () => this.downloadFor(model),
      remove: () => this.askRemove(model),
    };
    button.setAttribute("aria-expanded", "true");
    openMenu({
      anchor: button,
      key: `rom-${model}`,
      label: `${title(model)} ROM`,
      returnFocus: () => button,
      onClose: () => button.setAttribute("aria-expanded", "false"),
      items: a.items.map((it) => (it === "-" ? it : { text: it.text, danger: it.danger, icon: it.id === "remove" ? "trash" : undefined, run: run[it.id] })),
    });
  }

  /** Ask (the shared modal), then remove `model`'s ROM. */
  async askRemove(model) {
    const models = sharedModels(this.store.state.roms?.slots ?? [], model);
    const q = removeQuestion(models, title, this.backend.romSource === "dialog");
    if (await confirmAction(q)) await this.removeRom(model);
  }

  /**
   * Remove `model`'s ROM (and the 39G's or 40G's that shares its file):
   * the host forgets the slot (the browser deletes the ROM and a 49G's
   * saved state); a model that runs from it stops, and its display says
   * there is no ROM.
   */
  async removeRom(model) {
    const slots = this.store.state.roms?.slots ?? [];
    const models = sharedModels(slots, model);
    const app = this.backend.romSource === "dialog";
    closeMenu();
    let ok = true;
    for (const m of models) ok = Boolean(await this.romCall(() => this.backend.forgetRom(m), "Could not remove the ROM")) && ok;
    if (!ok) return;
    if (models.includes(this.store.state.booted)) await this.backend.unload();
    this.showRoms();
    this.message(removedMessage(models, title, app));
  }

  async refreshLoad() {
    const model = this.store.state.booted;
    const canLoad = model ? await this.backend.hasState(model) : false;
    // A slower answer for a model booted before this one is stale.
    if (this.store.state.booted === model) this.store.set({ canLoad });
  }

  async saveState() {
    const model = this.store.state.booted;
    if (!model) return;
    try {
      const text = await this.backend.saveState(model);
      if (text) {
        this.message(text);
        this.store.set({ canLoad: true });
      }
    } catch (err) {
      this.message(`Could not save the state: ${err?.message ?? err}`, true);
    }
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
    if (r?.booted) this.message("Started fresh, with an empty memory.");
  }

  async loadState() {
    const model = this.store.state.booted;
    if (!model) return;
    try {
      const text = await this.backend.loadState(model);
      if (text) this.message(text);
    } catch (err) {
      this.message(`Could not load the state: ${err?.message ?? err}`, true);
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

  showStatus() {
    const s = this.store.state;
    let text;
    if (!s.booted) {
      text = s.message || "Pick a model and its ROM to start.";
    } else {
      const parts = [MODEL_TITLES[s.booted] ?? s.booted, s.romName];
      if (s.halted) parts.push(s.halted);
      else if (!s.running) parts.push("paused");
      if (s.busy) parts.push(s.writing ? "transferring…" : "typing…");
      if (s.message) parts.push(s.message);
      text = parts.filter(Boolean).join(" · ");
    }
    const st = this.ui.status;
    if (st.textContent !== text) st.textContent = text;
    st.classList.toggle("error", Boolean(s.halted) || s.messageError);
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
