// The home page (`#/`, iteration 50d), since iteration 53 the one place
// where calculators are chosen, given a ROM and removed: the calculators
// with a kept ROM, each with Calculator and Files and a "⋯" (Change ROM…,
// Remove…), those that run now marked; "New calculator", the models
// without a ROM, each with Choose… (and where to download it); the
// notice of what a choice found (another model's file, several at once,
// a file that fits more than one model); "Connect a calculator" (a
// calculator on a serial cable, in Files; where the browser has no Web
// Serial, the line that says why); and the ROMs' settings: start the
// last calculator when the app opens, whether the browser keeps them for
// good, Remove ROMs…. The ROM commands go through web/home-roms.js (the
// hub in the browser, the app's in the desktop app), so no view needs to
// be mounted; a chosen ROM opens its calculator (`#/calculator/<model>`).
// Rendered each time it is shown.

import { el } from "./shared/dom.js";
import { iconEl } from "./shared/components/icons.js";
import { closeMenu, openMenu, openMenuKey } from "./shared/components/menu.js";
import { confirmAction } from "./shared/components/confirm.js";
import { routeToasts, say } from "./shared/components/toast.js";
import { prefs } from "./shared/prefs.js";
import { Store } from "./store.js";
import { StorageChoice, showStorageOffer } from "./pwa.js";
import { confirmForget } from "./fresh.js";
import { DUMP_ONLY, NO_ROM_SOURCE, dropNotice } from "./norom.js";
import { keptMenu, newActions, removeQuestion, removeRefusal, removedMessage, sharedModels } from "./romrows.js";
import { keptRoms, keptStart } from "./tx/kept-roms.js";
import { hasWebSerial } from "./tx/links.js";
import { isTauri } from "./tx/tauri.js";
import "./shared/models.js";

/** How long the home page waits for the running calculators before it leaves the marks out. */
const ENGINES_MS = 2000;

const ORDER = globalThis.SATURNUS_MODEL_ORDER;
const TITLES = globalThis.SATURNUS_MODEL_TITLES;
const SERIAL = globalThis.SATURNUS_SERIAL;
const title = (m) => TITLES[m] ?? m;

/** Where a ROM is kept, per host. */
const KEPT_HINTS = {
  file: "Kept in this browser so you do not have to pick them again; never uploaded.",
  dialog: "The app reads each ROM from where it is; one you download is kept in the app's data folder. Nothing is uploaded.",
};
/** How a new calculator gets its ROM, per host (`touch`: a finger's screen, which cannot drop a file). */
const NEW_HINTS = {
  file: "Choose the model's ROM file, or drop it on this page. Several files at once each go to their model.",
  touch: "Choose the model's ROM file. Several files at once each go to their model.",
  dialog: "Choose the model's ROM file; the other ROMs in its folder are recognised and go to their models.",
};
/** Where the ROMs come from, per host (iteration 20b). */
const SOURCE_HINTS = {
  file: "Download opens the model's page on hpcalc.org: unzip the file from there and choose it here. The ROMs are HP's software; hpcalc.org hosts them with HP's permission.",
  dialog: "Download… fetches a model's ROM from hpcalc.org after asking, checks its checksum and keeps it in the app's data folder. The ROMs are HP's software, hosted by hpcalc.org with HP's permission for use with emulators; they are not part of saturnus.",
};
/** Whether the browser keeps them for good (`storage`, `StorageChoice` in web/pwa.js). */
const STORAGE_STATES = {
  persistent: "Your ROMs and saved states are stored permanently: the browser does not delete them to free space.",
  "best-effort": "The browser may delete your ROMs and saved states when this device runs low on space. Keep permanently asks it not to.",
};

/** The logo (as in the headers), drawn in a model's colours by its `data-logo` (style.css). */
const LOGO = '<svg class="logo" viewBox="0 0 32 32" width="36" height="36" aria-hidden="true"><path class="ring" d="M 1.6 16 A 14.4 4.6 0 0 1 30.4 16" transform="rotate(-24 16 16)"/><circle class="planet" cx="16" cy="16" r="8.6"/><path class="ring" d="M 30.4 16 A 14.4 4.6 0 0 1 1.6 16" transform="rotate(-24 16 16)"/></svg>';

function logo() {
  const t = document.createElement("template");
  t.innerHTML = LOGO;
  return t.content.firstElementChild;
}

/** Why "Connect a calculator" is not offered in this browser. */
export const NO_SERIAL_PORT = "A calculator on a cable needs Chrome or Edge on a computer: this browser cannot reach a serial port.";

/**
 * The rows of the home page's list (pure): one per model with a kept ROM
 * (`slots`, `romSlots`'s) in the page's model order, `{model, title,
 * romName, state, running, files, slot}` (`files`: Files can reach it, a
 * model with a serial port).
 */
export function homeRows(slots, running = []) {
  return ORDER.flatMap((model) => {
    const slot = slots.find((s) => s.model === model);
    if (!slot?.fileName) return [];
    return [{ model, title: title(model), romName: slot.fileName, state: slot.state, running: running.includes(model), files: SERIAL.includes(model), slot }];
  });
}

/** The models without a ROM, in the page's order (New calculator). */
export function newRows(slots) {
  return ORDER.flatMap((model) => {
    const slot = slots.find((s) => s.model === model);
    return slot && !slot.fileName ? [slot] : [];
  });
}

/**
 * The kept ROM file of each model: whether an action kept one. By name
 * only: the slots read from the browser's store before the hub runs
 * carry no revision.
 */
function keptFiles(slots) {
  return new Map(slots.filter((s) => s.fileName).map((s) => [s.model, s.fileName]));
}

/**
 * The home page in `root` (`#home-view`): `render()` reads the ROM slots
 * and the running calculators again and draws them. `roms` the ROM
 * commands (web/home-roms.js), `on.connect` the shell's (web/main.js).
 */
export class Home {
  constructor(root, { roms, on = {}, serial = () => hasWebSerial() || isTauri(), idb = () => globalThis.indexedDB } = {}) {
    this.root = root;
    this.roms = roms;
    this.on = on;
    this.idb = idb;
    this.app = roms.romSource === "dialog";
    const $ = (sel) => root.querySelector(sel);
    this.ui = {
      list: $("#home-list"),
      empty: $("#home-empty"),
      newButton: $("#home-new"),
      newSection: $("#home-new-section"),
      newList: $("#home-models"),
      allKept: $("#home-all-kept"),
      file: $("#home-file"),
      notice: $("#home-notice"),
      settings: $("#home-settings"),
      bootLast: $("#home-boot-last"),
      storage: $("#home-storage"),
      keep: $("#home-keep"),
      keptHint: $("#home-kept-hint"),
      forget: $("#home-forget"),
    };
    /** The page's own state: the slots, the notice, the storage, its messages (toasts). */
    this.store = new Store();
    routeToasts(this.store);
    this.storageChoice = new StorageChoice(roms, this.store, prefs);
    this.storageChoice.read();
    showStorageOffer(this.store, this.storageChoice, document, $(".storage-offer"));
    this.store.watch(["storage"], () => this.showSettings());
    /** The model the file picker chooses for; the running calculators last drawn. */
    this.pickFor = null;
    this.running = [];
    this.drawn = 0;
    const ui = this.ui;
    const touch = !this.app && globalThis.matchMedia?.("(pointer: coarse)").matches;
    $("#home-new-hint").textContent = NEW_HINTS[touch ? "touch" : roms.romSource];
    $("#home-source-hint").textContent = SOURCE_HINTS[roms.romSource];
    if (this.app) $(".home-boot-last-label").textContent = "Start the last calculator when the app starts";

    // A handler's failure it did not show is shown here.
    const blurAfter = (fn) => (e) => {
      e.currentTarget.blur();
      Promise.resolve().then(fn).catch((err) => say(this.store, `${err?.message ?? err}`, true));
    };
    ui.newButton.addEventListener("click", blurAfter(() => this.toggleNew()));
    const connect = $("#home-connect");
    connect.addEventListener("click", () => this.on.connect?.());
    const can = serial();
    connect.hidden = !can;
    const why = $("#home-connect-why");
    why.hidden = can;
    why.textContent = can ? "" : NO_SERIAL_PORT;
    ui.file.addEventListener("change", () => {
      const files = [...(ui.file.files ?? [])];
      ui.file.value = "";
      if (files.length) this.chooseFiles(this.pickFor ?? ORDER[0], files);
    });
    // A new calculator's buttons, a kept one's ⋯, an offer's Use.
    root.addEventListener("click", (e) => {
      const b = e.target.closest("button[data-act], button[data-menu], button[data-offer]");
      if (!b) return;
      if (b.dataset.menu) {
        if (openMenuKey() === `home-${b.dataset.menu}`) closeMenu(e.detail === 0);
        else this.menu(b);
        return;
      }
      blurAfter(() => {
        if (b.dataset.act === "download") return this.download(b.dataset.model);
        if (b.dataset.act === "choose") return this.choose(b.dataset.model);
        return this.act(() => this.roms.takeOffer(b.dataset.model, Number(b.dataset.offer)), { keeps: true });
      })({ currentTarget: b });
    });
    root.addEventListener("keydown", (e) => {
      const b = e.target.closest?.("button[data-menu]");
      if (!b || (e.key !== "ArrowDown" && e.key !== "ArrowUp")) return;
      e.preventDefault();
      this.menu(b, e.key === "ArrowUp");
    });
    ui.bootLast.addEventListener("change", () => {
      ui.bootLast.blur();
      this.act(() => this.roms.romSettings(ui.bootLast.checked), { failed: "Cannot keep the setting" });
    });
    // Keep for good: `StorageChoice` calls persist() within this click.
    ui.keep.addEventListener("click", blurAfter(() => this.storageChoice.keep()));
    ui.forget.addEventListener("click", blurAfter(() => this.forgetAll()));
    this.acceptDrops();
  }

  /**
   * The slots as `romSlots` gives them; in the browser before the hub
   * runs (and `hub` asks for it), read from its store, with no download
   * links.
   */
  async slots(hub = false) {
    if (hub || this.roms.started()) {
      const { booted, notice, bootError, start, ...r } = await this.roms.romSlots();
      return r;
    }
    const idb = this.idb();
    const [kept, { lastModel, bootLast }] = await Promise.all([keptRoms(idb, ORDER, null), keptStart(idb)]);
    const slots = ORDER.map((model) => {
      const k = kept.find((x) => x.model === model);
      return { model, fileName: k?.name ?? null, revision: null, state: k ? "ready" : "empty", download: null };
    });
    return { slots, offers: [], lastModel, bootLast, remembered: null, note: null };
  }

  /** Read and draw everything again; `hub`: the slots from the hub, with their download links. */
  async render(hub = false) {
    const turn = ++this.drawn;
    let roms;
    try {
      roms = await this.slots(hub);
    } catch (err) {
      say(this.store, `The kept ROMs cannot be read: ${err?.message ?? err}`, true);
      roms = this.store.state.roms ?? { slots: [], offers: [], bootLast: true, note: null };
    }
    // A newer render started meanwhile: it draws.
    if (turn !== this.drawn) return;
    this.store.set({ roms });
    this.draw(this.running, turn);
    // The running marks when they come; a busy or stuck hub never holds the list.
    const running = await Promise.race([this.roms.running(), new Promise((r) => setTimeout(() => r(null), ENGINES_MS))]);
    if (running && turn === this.drawn) this.draw(running, turn);
  }

  draw(running, turn = this.drawn) {
    const r = this.store.state.roms;
    const rows = homeRows(r.slots, running);
    this.running = running;
    this.ui.list.replaceChildren(...rows.map((x) => this.row(x)));
    this.ui.list.hidden = rows.length === 0;
    this.ui.empty.hidden = rows.length > 0;
    this.drawNew();
    this.drawNotice();
    this.showSettings();
    this.root.dataset.rendered = String(turn);
  }

  row(r) {
    const name = el("h3", { class: "home-title" }, el("span", { text: r.title }));
    if (r.running) name.append(el("span", { class: "home-running", text: "Running" }));
    const bad = r.state === "missing" || r.state === "changed";
    const rom = el("p", { class: `home-rom muted${bad ? " error" : ""}`, text: bad ? `${r.romName} (${r.state})` : r.romName, title: r.slot.revision ? `${r.romName} (${r.slot.revision})` : r.romName });
    const actions = el("div", { class: "home-actions" },
      el("a", { class: "button primary", href: `#/calculator/${r.model}`, "data-model": r.model, "data-go": "calculator" }, iconEl("calculator", "ic-sm"), el("span", { text: "Calculator" })),
      el("button", { type: "button", class: "icon more", "data-menu": r.model, "aria-haspopup": "menu", "aria-expanded": "false", "aria-label": `More for ${r.title}`, title: "More" }, iconEl("more")),
    );
    if (r.files) {
      // Before Calculator, so Calculator lines up on the right in every row.
      actions.prepend(el("a", { class: "button", href: `#/files/${r.model}`, "data-model": r.model, "data-go": "files", title: `Files on the ${r.title}: move files between it and this computer` }, iconEl("copy-to", "ic-sm"), el("span", { text: "Files" })));
    }
    return el("li", { class: `home-calc${r.running ? " running" : ""}`, "data-model": r.model, "data-logo": r.model },
      logo(),
      el("div", { class: "home-place" }, name, rom),
      actions,
    );
  }

  // ---------------------------------------------------------- new calculator

  /** "New calculator": the models without a ROM, read with their download links. */
  async toggleNew() {
    const open = this.ui.newSection.hidden;
    this.ui.newSection.hidden = !open;
    this.ui.newButton.setAttribute("aria-expanded", String(open));
    if (!open) return;
    // The browser's store has no download links; the hub's slots have.
    if (!this.roms.started()) await this.render(true);
    this.ui.newSection.scrollIntoView?.({ block: "nearest" });
  }

  drawNew() {
    const slots = newRows(this.store.state.roms?.slots ?? []);
    const items = slots.map((slot) => {
      const t = title(slot.model);
      const d = slot.download;
      let where = "";
      if (d && !this.app) {
        // The page cannot fetch from hpcalc.org: its page; the file name to expect in the tooltip.
        where = el("a", { href: d.page, target: "_blank", rel: "noopener noreferrer", title: `Download from hpcalc.org, unzip, then choose ${d.file} (${d.revision})`, "aria-label": `Download ${d.file}, the ${t} ROM, from hpcalc.org`, text: "Download from hpcalc.org" });
      } else if (d) {
        where = `${d.file}, from hpcalc.org`;
      } else if (DUMP_ONLY.includes(slot.model)) {
        where = "HP never published it: read it out of your own calculator.";
      } else if (NO_ROM_SOURCE.includes(slot.model)) {
        where = "No download: choose the file if you have it.";
      }
      const buttons = newActions(slot, this.app).map((a, i, all) => el("button", {
        type: "button",
        class: i === all.length - 1 ? "primary" : null,
        "data-act": a.id,
        "data-model": slot.model,
        "aria-label": `${a.id === "download" ? "Download" : "Choose"} the ${t} ROM`,
        text: a.label,
      }));
      return el("li", { class: "home-calc", "data-model": slot.model, "data-logo": slot.model },
        logo(),
        el("div", { class: "home-place" }, el("h3", { class: "home-title" }, el("span", { text: t })), el("p", { class: "home-rom muted" }, where)),
        el("div", { class: "home-actions" }, ...buttons),
      );
    });
    this.ui.newList.replaceChildren(...items);
    this.ui.newList.hidden = items.length === 0;
    this.ui.allKept.hidden = items.length > 0;
  }

  /** What the last choice found, and the offers to take (`Use x for the HP y`). */
  drawNotice() {
    const s = this.store.state;
    const offers = (s.roms?.offers ?? []).flatMap((o) => o.models.map((m) => el("button", { type: "button", "data-offer": String(o.id), "data-model": m, text: `Use ${o.fileName} for the ${title(m)}` })));
    const n = this.ui.notice;
    n.querySelector(".home-notice-text").textContent = s.romNotice;
    n.querySelector(".home-offers").replaceChildren(...offers);
    n.hidden = !s.romNotice && !offers.length;
  }

  // ---------------------------------------------------------- the actions

  /** Choose `model`'s ROM: the app's dialog, or this page's file picker. */
  async choose(model) {
    if (this.app) {
      await this.act(() => this.roms.chooseRom(model), { keeps: true });
      return;
    }
    this.pickFor = model;
    this.ui.file.click();
  }

  /** ROM files from the picker or a drop, for `model`. */
  chooseFiles(model, files) {
    return this.act(() => this.roms.chooseRom(model, files), { keeps: true });
  }

  /** Download `model`'s ROM from hpcalc.org (the app: it asks first). A failure names the page to download it from by hand. */
  async download(model) {
    say(this.store, `Downloading the ${title(model)} ROM from hpcalc.org…`);
    const r = await this.act(() => this.roms.downloadRom(model), { keeps: true, failed: `Cannot download the ${title(model)} ROM` });
    if (r === null && !this.store.state.messageError) this.store.set({ message: "", messageError: false });
  }

  /**
   * Run a ROM command and show its result: the slots, the notice and the
   * offers drawn. A command that `keeps` ROMs (a choice, an offer taken, a
   * download) also offers to keep a newly kept ROM for good, and opens the
   * calculator it started (the app) or would start (the browser), unless
   * there is a notice or an offer to read first: then the page stays, and
   * the calculator opens from its row. Resolves to the result; null when
   * cancelled or failed (the failure said, the slots read again).
   */
  async act(fn, { keeps = false, failed = "Cannot keep the ROM" } = {}) {
    const before = keptFiles(this.store.state.roms?.slots ?? []);
    let r;
    try {
      r = await fn();
    } catch (err) {
      say(this.store, `${failed}: ${err?.message ?? err}`, true);
      await this.render();
      return null;
    }
    if (!r) return null;
    const { booted, notice, bootError, start, ...roms } = r;
    this.store.set({ roms, romNotice: notice ?? "" });
    this.draw(this.running);
    if (bootError) say(this.store, `Cannot start: ${bootError}`, true);
    if (!keeps) return r;
    const after = keptFiles(roms.slots);
    if ([...after].some(([m, f]) => before.get(m) !== f)) this.storageChoice.offer();
    const go = start ?? booted?.model ?? null;
    if (go && !bootError && !notice && !roms.offers?.length) location.hash = `#/calculator/${go}`;
    return r;
  }

  /** A kept calculator's "⋯" menu; `last`: start on its last item (ArrowUp). */
  menu(button, last = false) {
    const model = button.dataset.menu;
    const slot = this.store.state.roms?.slots.find((s) => s.model === model);
    if (!slot) return;
    const run = {
      choose: () => this.choose(model),
      download: () => this.download(model),
      remove: () => this.askRemove(model),
    };
    button.setAttribute("aria-expanded", "true");
    openMenu({
      anchor: button,
      key: `home-${model}`,
      label: title(model),
      last,
      returnFocus: () => button,
      onClose: () => button.setAttribute("aria-expanded", "false"),
      items: keptMenu(slot, this.app).map((it) => (it === "-" ? it : { text: it.text, danger: it.danger, icon: it.id === "remove" ? "trash" : undefined, run: run[it.id] })),
    });
  }

  /**
   * Remove `model`'s ROM (and the 39G's or 40G's that shares its file),
   * after the question: the calculator that runs from it stops (refused
   * while a send types, and then nothing is removed).
   */
  async askRemove(model) {
    const slots = this.store.state.roms?.slots ?? [];
    const models = sharedModels(slots, model);
    const running = (await this.roms.running()).find((m) => models.includes(m)) ?? null;
    if (!(await confirmAction(removeQuestion(models, title, this.app, running)))) return;
    closeMenu();
    for (const m of models) {
      if (!(await this.remove(() => this.roms.forgetRom(m), running ?? m, "its ROM"))) return;
    }
    say(this.store, removedMessage(models, title, this.app));
  }

  /** Remove ROMs…: every ROM, after the question. */
  async forgetAll() {
    if (!(await confirmForget(this.app))) return;
    const running = (await this.roms.running())[0] ?? null;
    if (await this.remove(() => this.roms.forgetRom(null), running, "the ROMs")) {
      say(this.store, this.app ? "ROMs removed from the list. The files stay where they are." : "ROMs and the saved 49G state removed. Other saved states stay.");
    }
  }

  /** A removal: the slots drawn, or the refusal said in plain words (`model` the calculator that would stop). */
  async remove(fn, model, what) {
    try {
      const { booted, notice, bootError, start, ...roms } = await fn();
      this.store.set({ roms, romNotice: "" });
      await this.render();
      return true;
    } catch (err) {
      say(this.store, removeRefusal(err, title(model ?? ""), what), true);
      await this.render();
      return false;
    }
  }

  /** The ROMs' settings: start the last calculator, the browser's storage, Remove ROMs…. */
  showSettings() {
    const s = this.store.state;
    const r = s.roms;
    const ui = this.ui;
    ui.settings.hidden = !r?.slots.some((x) => x.fileName);
    if (!r) return;
    ui.bootLast.checked = r.bootLast ?? true;
    const storage = STORAGE_STATES[s.storage];
    ui.storage.querySelector(".storage-state").textContent = storage ?? "";
    ui.storage.hidden = !storage;
    ui.keep.hidden = s.storage !== "best-effort";
    ui.keptHint.textContent = r.note ?? KEPT_HINTS[this.roms.romSource];
    ui.keptHint.classList.toggle("error", Boolean(r.note));
  }

  /**
   * Files dropped on the page (shown) are ROMs, for the model of the row
   * they were dropped on, else the first model; what is no ROM is said
   * in the notice. The app takes no ROM by drop.
   */
  acceptDrops() {
    const hasFiles = (e) => [...(e.dataTransfer?.types ?? [])].includes("Files");
    document.addEventListener("dragover", (e) => {
      if (!hasFiles(e) || this.root.hidden) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      if (!this.app) this.root.classList.add("rom-drop");
    });
    document.addEventListener("dragleave", (e) => {
      if (!e.relatedTarget) this.root.classList.remove("rom-drop");
    });
    document.addEventListener("drop", async (e) => {
      this.root.classList.remove("rom-drop");
      if (!hasFiles(e) || this.root.hidden) return;
      e.preventDefault();
      const files = [...e.dataTransfer.files];
      if (!files.length) return;
      const r = this.app ? null : await this.chooseFiles(e.target?.closest?.("[data-model]")?.dataset.model ?? ORDER[0], files);
      const text = dropNotice(files.map((f) => f.name), r, this.roms.romSource);
      if (!text) return;
      this.store.set({ romNotice: text });
      this.drawNotice();
    });
  }
}
