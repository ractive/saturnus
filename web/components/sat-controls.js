// <sat-controls>: the panel's controls. Model and ROM, Run/Pause, Reset,
// state save and load, speed, view, the keyboard help and the status
// line. Renders from the store, acts through the backend; fullscreen and
// the About panel are the page's, asked for by `sat-fullscreen` and
// `sat-about` events. Light DOM (display: contents).

import { MODEL_TITLES } from "./sat-calculator.js";

/** The ROM hints per host and whether this browser keeps ROMs. */
const ROM_HINTS = {
  kept: "Kept in this browser so you do not have to pick it again; never uploaded.",
  app: "The app remembers where each ROM file is and reads it from there; it never copies or uploads it.",
};
const FORGET_HINTS = {
  file: "Choose several files at once, or drop them on the page: each goes to its model. Forget ROMs removes the ROMs from this browser; saved states stay.",
  dialog: "The other ROMs in the folder of the one you choose are recognised and go to their models. Forget ROMs makes the app forget where the ROMs are; the files and saved states stay.",
};

const title = (m) => MODEL_TITLES[m] ?? m;

const SPEED_HINTS = {
  "1": "Real time.",
  "2": "Twice real time; the calculator's clock runs twice as fast.",
  "4": "Four times real time; the calculator's clock runs four times as fast.",
  max: "As fast as this device can; the calculator's clock runs fast.",
};

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
    <div id="rom-notice" class="rom-notice" role="status" hidden>
      <p class="rom-notice-text"></p>
      <div class="rom-offers"></div>
    </div>
    <details class="help roms" id="roms">
      <summary>ROMs of every model <span class="rom-count"></span></summary>
      <table class="rom-slots"><tbody></tbody></table>
      <label class="check"><input id="boot-last" type="checkbox" checked> <span class="boot-last-label">Start the last model when the page opens</span></label>
      <p class="rom-forget"><button id="rom-forget" type="button">Forget ROMs</button></p>
      <p class="hint forget-hint"></p>
    </details>
  </section>

  <section class="group row">
    <button id="run" type="button" disabled>Pause</button>
    <button id="reset" type="button" disabled>Reset</button>
    <button id="save" type="button" disabled>Save state</button>
    <button id="load" type="button" disabled>Load state</button>
  </section>

  <section class="group">
    <div class="field-label" id="speed-label">Speed</div>
    <div class="segmented" role="radiogroup" aria-labelledby="speed-label" id="speed">
      <button type="button" role="radio" data-speed="1">1×</button>
      <button type="button" role="radio" data-speed="2">2×</button>
      <button type="button" role="radio" data-speed="4">4×</button>
      <button type="button" role="radio" data-speed="max">Max</button>
    </div>
    <p class="hint" id="speed-hint">Real time.</p>
  </section>

  <section class="group row">
    <button id="fullscreen" type="button">Fullscreen</button>
    <label class="check"><input id="view-skin" type="checkbox" checked> Drawn calculator</label>
  </section>

  <details class="help">
    <summary>Keyboard</summary>
    <dl>
      <dt><kbd>a</kbd>–<kbd>z</kbd> <kbd>A</kbd>–<kbd>Z</kbd></dt><dd>letters, through the calculator's alpha mode (and its shift for lowercase)</dd>
      <dt><kbd>Tab</kbd></dt><dd>α (twice for alpha lock on the 48 and 49G)</dd>
      <dt><kbd>[</kbd> <kbd>]</kbd></dt><dd>left and right shift (<kbd>[</kbd> is the only shift on the 38G, 39G and 40G)</dd>
      <dt><kbd>Esc</kbd> <kbd>\`</kbd></dt><dd>ON. In fullscreen, <kbd>Esc</kbd> leaves fullscreen unless the browser lets the page keep it (Chrome: hold <kbd>Esc</kbd> to leave); <kbd>\`</kbd> always works.</dd>
      <dt><kbd>F1</kbd>–<kbd>F6</kbd></dt><dd>the menu keys</dd>
      <dt><kbd>0</kbd>–<kbd>9</kbd> <kbd>.</kbd> <kbd>+</kbd> <kbd>-</kbd> <kbd>*</kbd> <kbd>/</kbd> <kbd>^</kbd> <kbd>'</kbd></dt><dd>as printed</dd>
      <dt><kbd>Enter</kbd> <kbd>Space</kbd> <kbd>⌫</kbd> <kbd>Del</kbd> arrows</dt><dd>ENTER, SPC, ⬅, DEL, the cursor keys</dd>
    </dl>
    <p>Click or tap the drawn keys for everything else. The drawings are our own, measured from photographs of the calculators and the keyboard figures in their user's guides.</p>
  </details>

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
      run: $("#run"),
      reset: $("#reset"),
      save: $("#save"),
      load: $("#load"),
      speed: $("#speed"),
      speedHint: $("#speed-hint"),
      fullscreen: $("#fullscreen"),
      viewSkin: $("#view-skin"),
      status: $("#status"),
      about: $("#about"),
      romName: $("#rom-name"),
      romHint: $(".rom-hint"),
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
    // A row's Choose/Change and an offer's Use.
    this.addEventListener("click", (e) => {
      const b = e.target.closest("button[data-choose], button[data-offer]");
      if (!b) return;
      b.blur();
      if (b.dataset.choose) this.chooseFor(b.dataset.choose);
      else this.takeOffer(b.dataset.model, Number(b.dataset.offer));
    });
    ui.bootLast.addEventListener("change", () => {
      ui.bootLast.blur();
      this.romCall(() => backend.romSettings(ui.bootLast.checked));
    });
    ui.romForget.addEventListener("click", blurAfter(async () => {
      if (await this.romCall(() => backend.forgetRom())) {
        this.message(dialog ? "ROMs forgotten; the files stay where they are." : "ROMs forgotten; saved states stay.");
      }
    }));
    if (!dialog) this.acceptDrops();
    ui.run.addEventListener("click", blurAfter(() => this.backend.pause(store.state.running)));
    ui.reset.addEventListener("click", blurAfter(async () => {
      if (!store.state.booted) return;
      try {
        await backend.reset();
        this.message("");
      } catch (err) {
        this.message(String(err?.message ?? err), true);
      }
    }));
    ui.save.addEventListener("click", blurAfter(() => this.saveState()));
    ui.load.addEventListener("click", blurAfter(() => this.loadState()));
    ui.speed.addEventListener("click", (e) => {
      const b = e.target.closest("button[data-speed]");
      if (!b) return;
      b.blur();
      this.setSpeed(b.dataset.speed);
    });
    ui.fullscreen.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-fullscreen", { bubbles: true }));
    }));
    ui.viewSkin.addEventListener("change", () => {
      ui.viewSkin.blur();
      const view = ui.viewSkin.checked ? "skin" : "grid";
      prefs.set("view", view);
      store.set({ view });
    });
    ui.about.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-about", { bubbles: true }));
    }));

    store.watch(["models", "model"], (s) => this.fillModels(s));
    store.watch(["booted"], (s) => {
      for (const b of [ui.run, ui.reset, ui.save]) b.disabled = !s.booted;
      if (s.booted && ui.model.value !== s.booted) {
        ui.model.value = s.booted;
        store.set({ model: s.booted });
      }
      this.refreshLoad();
    });
    store.watch(["running"], (s) => {
      ui.run.textContent = s.running ? "Pause" : "Run";
    });
    store.watch(["speed"], (s) => this.showSpeed(s.speed));
    store.watch(["view"], (s) => {
      ui.viewSkin.checked = s.view === "skin";
    });
    store.watch(["canLoad"], (s) => {
      ui.load.disabled = !s.canLoad;
    });
    store.watch(["booted", "romName", "running", "halted", "message", "messageError"], () => this.showStatus());
    store.watch(["roms", "romNotice", "model"], () => this.showRoms());
    this.fillModels(store.state);
    this.showSpeed(store.state.speed);
    ui.viewSkin.checked = store.state.view === "skin";
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
  async romCall(fn) {
    try {
      const r = await fn();
      if (!r) return null;
      const { booted, notice, ...roms } = r;
      this.store.set({ roms, romNotice: notice ?? this.store.state.romNotice });
      if (booted) {
        this.prefs.set("model", booted.model);
        this.store.set({ message: "", messageError: false });
      }
      return r;
    } catch (err) {
      this.message(`Cannot start: ${err?.message ?? err}`, true);
      await this.refreshRoms();
      return null;
    }
  }

  async refreshRoms() {
    try {
      const { booted, notice, ...roms } = await this.backend.romSlots();
      this.store.set({ roms });
    } catch (err) {
      this.message(`The ROM slots cannot be read: ${err?.message ?? err}`, true);
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
   * The selected model changed: boot it from its remembered ROM. One that
   * is missing or changed is reported and, in the app, asked for again
   * (`ask`); nothing else boots in its place.
   */
  async bootSelected(model, ask) {
    const slot = this.slot(model);
    if (!slot || slot.state === "empty") {
      this.message(`Choose the ${title(model)} ROM to start it.`);
      return;
    }
    const r = await this.romCall(() => this.backend.bootModel(model));
    if (!r && ask && this.backend.romSource === "dialog") await this.chooseFor(model);
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

  /** ROM files from the picker or a drop, for `model`. */
  chooseFiles(model, files) {
    this.dispatchEvent(new CustomEvent("sat-sheet", { bubbles: true, detail: false }));
    return this.romCall(() => this.backend.chooseRom(model, files));
  }

  takeOffer(model, offer) {
    return this.romCall(() => this.backend.takeOffer(model, offer));
  }

  /** Files dropped anywhere on the page are ROMs for the selected model. */
  acceptDrops() {
    const hasFiles = (e) => [...(e.dataTransfer?.types ?? [])].includes("Files");
    document.addEventListener("dragover", (e) => {
      if (!hasFiles(e)) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      document.body.classList.add("rom-drop");
    });
    document.addEventListener("dragleave", (e) => {
      if (!e.relatedTarget) document.body.classList.remove("rom-drop");
    });
    document.addEventListener("drop", (e) => {
      document.body.classList.remove("rom-drop");
      if (!hasFiles(e)) return;
      e.preventDefault();
      const files = [...e.dataTransfer.files];
      if (files.length) this.chooseFiles(this.store.state.model, files);
    });
  }

  showRoms() {
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
    if (!r) return;
    ui.bootLast.checked = r.bootLast;

    const rows = r.slots.map((slot) => {
      const tr = document.createElement("tr");
      const th = document.createElement("th");
      th.scope = "row";
      th.textContent = title(slot.model);
      const name = document.createElement("td");
      name.className = "rom-file-name";
      name.textContent = slot.fileName
        ? `${slot.fileName}${slot.state !== "ready" ? ` (${slot.state})` : ""}`
        : "—";
      if (slot.revision) name.title = slot.revision;
      name.classList.toggle("error", slot.state === "missing" || slot.state === "changed");
      const act = document.createElement("td");
      const b = document.createElement("button");
      b.type = "button";
      b.dataset.choose = slot.model;
      b.textContent = slot.fileName ? "Change…" : "Choose…";
      b.setAttribute("aria-label", `${slot.fileName ? "Change" : "Choose"} the ${title(slot.model)} ROM`);
      act.append(b);
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
      this.message(`save failed: ${err?.message ?? err}`, true);
    }
  }

  async loadState() {
    const model = this.store.state.booted;
    if (!model) return;
    try {
      const text = await this.backend.loadState(model);
      if (text) this.message(text);
    } catch (err) {
      this.message(`load failed: ${err?.message ?? err}`, true);
    }
  }

  setSpeed(value) {
    const speed = ["1", "2", "4", "max"].includes(value) ? value : "1";
    this.prefs.set("speed", speed);
    this.store.set({ speed });
    this.backend.setSpeed(speed);
  }

  showSpeed(speed) {
    for (const b of this.ui.speed.querySelectorAll("button")) {
      b.setAttribute("aria-checked", String(b.dataset.speed === speed));
    }
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
      if (s.message) parts.push(s.message);
      text = parts.filter(Boolean).join(" · ");
    }
    const st = this.ui.status;
    if (st.textContent !== text) st.textContent = text;
    st.classList.toggle("error", Boolean(s.halted) || s.messageError);
  }

  /** Fullscreen is unavailable: disable its button. */
  disableFullscreen() {
    this.ui.fullscreen.disabled = true;
  }

  /** The fullscreen button's label. */
  setFullscreenLabel(on) {
    this.ui.fullscreen.textContent = on ? "Leave fullscreen" : "Fullscreen";
  }
}

customElements.define("sat-controls", SatControls);
