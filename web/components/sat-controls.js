// <sat-controls>: the panel's controls. Model and ROM, Run/Pause, Reset,
// state save and load, speed, view, the keyboard help and the status
// line. Renders from the store, acts through the backend; fullscreen and
// the About panel are the page's, asked for by `sat-fullscreen` and
// `sat-about` events. Light DOM (display: contents).

import { MODEL_TITLES } from "./sat-calculator.js";

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
    <label class="field rom-file">ROM file
      <input id="rom" type="file">
    </label>
    <div class="field rom-dialog" hidden>ROM file
      <button id="rom-pick" type="button">Choose ROM…</button>
    </div>
    <p class="hint rom-hint">Read in this page only, never uploaded or stored.</p>
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

  <p id="status" class="status" role="status">Pick a model and a ROM file to start.</p>`;

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
    };
    this.ui = ui;
    const dialog = backend.romSource === "dialog";
    $(".rom-file").hidden = dialog;
    $(".rom-dialog").hidden = !dialog;
    if (dialog) $(".rom-hint").textContent = "Read by this app from the file you choose; never stored.";

    const blurAfter = (fn) => (e) => {
      e.currentTarget.blur();
      fn();
    };
    ui.model.addEventListener("change", () => {
      prefs.set("model", ui.model.value);
      store.set({ model: ui.model.value });
      if (store.state.booted && ui.model.value !== store.state.booted) this.message("pick a ROM for the new model");
    });
    ui.rom.addEventListener("change", () => {
      const f = ui.rom.files?.[0];
      if (f) this.boot({ file: f });
      ui.rom.blur();
      this.dispatchEvent(new CustomEvent("sat-sheet", { bubbles: true, detail: false }));
    });
    ui.romPick.addEventListener("click", blurAfter(() => {
      this.dispatchEvent(new CustomEvent("sat-sheet", { bubbles: true, detail: false }));
      this.boot({});
    }));
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

  /** Boot the selected model from `file` (web) or a ROM dialog (Tauri). */
  async boot({ file }) {
    const store = this.store;
    try {
      const r = await this.backend.boot({ model: this.ui.model.value, file });
      if (!r) return;
      this.prefs.set("model", r.model);
      store.set({ message: "", messageError: false });
    } catch (err) {
      this.message(`Cannot start: ${err?.message ?? err}`, true);
    }
  }

  async refreshLoad() {
    const model = this.store.state.booted;
    const canLoad = model ? await this.backend.hasState(model) : false;
    this.store.set({ canLoad });
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
      text = s.message || "Pick a model and a ROM file to start.";
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
