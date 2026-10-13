// "Serve a folder to a 38G, 39G or 40G" (kb/docs/saturnus-tx.md,
// "Aplets"): these calculators start every transfer themselves, so the
// page is the "disk drive" they send aplets to and get them from, in the
// spirit of HPGComm's small window: a folder, a cable, a waiting screen
// that says what to press, the transfers as they happen, Stop.
//
// - `ServeSession`: the folder served over one link. The page holds the
//   link and the folder; saturnus-tx's aplet server runs in its own Worker
//   (aplet-worker.js). Files the calculator asks for are read from the
//   folder when it asks; files it sends are written there (a file of the
//   same name, in any case, is replaced, as the calculator asked "Replace
//   existing aplet?" itself); its directory files (HP38DIR.*, HP39DIR.*)
//   are kept there too, as HPGComm kept them.
// - `ServeView`: the screen, over the panes while it is open; the
//   emulated 38G and 39G with their display and the keys a transfer needs.

import { el } from "../shared/dom.js";
import { iconEl } from "../shared/components/icons.js";
import { EmulatedCalculator } from "./emulator.js";
import { canPickFolder, fallbackFolder, pickFolder } from "./folder.js";
import { APLET_MODELS, keptRomBytes, keptRoms } from "./kept-roms.js";
import { EmulatorLink, WebSerialLink, hasWebSerial } from "./links.js";
import { TauriSerialLink, isTauri } from "./tauri.js";
import { SERVE_EMULATED, SERVE_NOTE, SERVE_STEPS, eventLine, progressLine, serveStatus } from "./serve-flows.js";

/** How much of a file the server needs (the header and the longest title). */
export const HEAD_BYTES = 264;
/** Files larger than this are no calculator's and are not read. */
export const MAX_FILE_BYTES = 1 << 20;
/** At most this many files of the folder are offered. */
export const MAX_FILES = 2000;

/** Whether `name` is a directory file the calculator reads whole. */
export const isDirFile = (name) => /^hp3[89]dir\./i.test(name);

/** The aplet Worker. */
export const apletWorker = () => new Worker(new URL("./aplet-worker.js", import.meta.url), { type: "module" });

/** The folder's files as the server wants them: `[{name, head}]` (all of a directory file). */
export async function folderFiles(folder, path = folder.home) {
  const listing = await folder.list(path);
  const out = [];
  for (const e of listing.entries) {
    if (out.length >= MAX_FILES) break;
    if (e.directory || (e.size ?? 0) > MAX_FILE_BYTES) continue;
    // Only a directory file is read whole; of the rest the header and the title.
    const head = await (isDirFile(e.name) ? folder.read(path, e.name).then((f) => f.data) : folder.head(path, e.name, HEAD_BYTES)).catch(() => null);
    if (head) out.push({ name: e.name, head });
  }
  return out;
}

export class ServeSession extends EventTarget {
  /**
   * Serve `folder` (its `path`, the folder itself by default) over `link`.
   * Events: `event` (`detail` the server's event, `received` after the
   * file is in the folder, with `storedAs`), `lost` (`detail` why).
   */
  constructor({ link, folder, path = folder.home, worker = apletWorker }) {
    super();
    this.link = link;
    this.folder = folder;
    this.path = path;
    this.makeWorker = worker;
    this.worker = null;
    this.io = Promise.resolve();
    this.closed = false;
    this.model = null;
    this.onData = (e) => this.worker?.postMessage({ cmd: "input", bytes: e.detail });
    this.onLost = (e) => this.lose(String(e.detail));
  }

  /** Read the folder, start the Worker, open the link at 9600 baud; on a failure everything started is stopped again. */
  async start() {
    try {
      return await this.starting();
    } catch (err) {
      await this.stop({ quiet: true });
      throw err;
    }
  }

  async starting() {
    const files = await folderFiles(this.folder, this.path);
    this.worker = this.makeWorker();
    const ready = new Promise((resolve, reject) => {
      this.worker.onmessage = (e) => {
        if (e.data?.type === "ready") resolve();
        else if (e.data?.type === "workerFailed") reject(new Error(e.data.message));
        else this.receive(e.data);
      };
      this.worker.onerror = (e) => reject(new Error(e.message || "the transfer part of the page failed to start"));
    });
    this.worker.postMessage({ cmd: "init", files });
    await ready;
    this.worker.onerror = (e) => this.lose(`the transfer part of the page stopped (${e.message})`);
    this.link.addEventListener("data", this.onData);
    this.link.addEventListener("lost", this.onLost);
    await this.link.open({ baud: 9600, parity: "none", dataBits: 8 });
    return files;
  }

  lose(reason) {
    if (this.closed) return;
    this.dispatchEvent(new CustomEvent("lost", { detail: reason }));
  }

  emit(detail) { this.dispatchEvent(new CustomEvent("event", { detail })); }

  /** Run a link or folder operation after those before it. */
  later(fn) {
    this.io = this.io.then(fn).catch((err) => this.lose(String(err?.message ?? err)));
    return this.io;
  }

  receive(m) {
    switch (m?.type) {
      case "write":
        this.later(() => this.link.write(m.bytes));
        break;
      case "needFile":
        this.emit(m);
        this.later(async () => {
          const f = await this.folder.read(this.path, m.name).catch(() => null);
          this.worker?.postMessage({ cmd: "provide", name: m.name, data: f?.data });
        });
        break;
      case "received":
        this.later(async () => {
          try {
            const storedAs = await this.store(m.name, m.data);
            this.emit({ ...m, storedAs });
          } catch (err) {
            // The server must not list or offer what is not there.
            this.worker?.postMessage({ cmd: "forget", name: m.name });
            this.emit({ type: "storeFailed", name: m.name, message: String(err?.message ?? err) });
          }
        });
        break;
      case "model":
        this.model = m.model;
        this.emit(m);
        break;
      case "workerFailed":
        this.lose(`the transfer part of the page stopped (${m.message})`);
        break;
      default:
        this.emit(m);
    }
  }

  /** Write a received file; one of the same name in another case is replaced. Resolves to the name used. */
  async store(name, data) {
    const found = await this.folder.existing(this.path, name).catch(() => null);
    const target = found && !found.directory ? found.name : name;
    await this.folder.write(this.path, target, data, { replace: true });
    return target;
  }

  /** Send file `name` of the folder by XModem (39G, 40G: RECV, "HP39/40 (Wire)"). */
  async sendWire(name) {
    const f = await this.folder.read(this.path, name);
    this.worker?.postMessage({ cmd: "sendWire", data: f.data });
  }

  /** Stop the running transfer. */
  cancel() { this.worker?.postMessage({ cmd: "cancel" }); }

  /** Stop serving: the running transfer ends, the link closes (`quiet`: nothing was running yet). */
  async stop({ quiet = false } = {}) {
    if (this.closed) return;
    if (!quiet) {
      this.cancel();
      await new Promise((r) => setTimeout(r, 150));
    }
    this.closed = true;
    this.link.removeEventListener("data", this.onData);
    this.link.removeEventListener("lost", this.onLost);
    await this.io.catch(() => {});
    this.worker?.terminate?.();
    this.worker = null;
    await this.link.close().catch(() => {});
  }
}

// ------------------------------------------------------------ the view

/** The keys the emulated calculator offers, per family: `[name, label]`. */
const EMULATED_KEYS = {
  "38g": [["lib", "LIB"], ["home", "HOME"]],
  "39g": [["aplet", "APLET"], ["home", "HOME"]],
};
const ARROWS = [["up", "▲", "Up"], ["left", "◀", "Left"], ["down", "▼", "Down"], ["right", "▶", "Right"]];

/** `rgb(r, g, b)` as `[r, g, b]` (black when it is not one). */
export function rgbOf(text) {
  const m = /rgba?\((\d+)[ ,]+(\d+)[ ,]+(\d+)/.exec(text ?? "");
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : [0, 0, 0];
}

/**
 * A frame's pixels (base64, 8 per byte, rows padded) into a canvas: the
 * dark ones in the canvas's text colour, the rest left clear over its
 * background.
 */
export function drawFrame(canvas, frame, { ink = rgbOf(getComputedStyle(canvas).color) } = {}) {
  const { width: w, height: h } = frame;
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
  const bin = atob(frame.pixels);
  const row = Math.ceil(w / 8);
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const img = ctx.createImageData(w, h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const on = (bin.charCodeAt(y * row + (x >> 3)) >> (7 - (x & 7))) & 1;
      const i = (y * w + x) * 4;
      img.data[i] = ink[0];
      img.data[i + 1] = ink[1];
      img.data[i + 2] = ink[2];
      img.data[i + 3] = on ? 255 : 0;
    }
  }
  ctx.putImageData(img, 0, 0);
}

export class ServeView {
  /**
   * `root` the section it fills; `stage` the element whose `data-mode`
   * hides the panes; `app` the page (helpers, say, the panes' folders).
   */
  constructor(root, { stage, app, folders = () => [] }) {
    this.root = root;
    this.stage = stage;
    this.app = app;
    this.folders = folders;
    this.folder = null;
    this.calc = null;
    this.session = null;
    this.emulator = null;
    this.lines = [];
    this.files = [];
    this.lastMissing = null;
    this.status = "";
    this.busy = false;
  }

  get open() { return !this.root.hidden; }

  /** Show the setup screen. */
  async show() {
    this.root.hidden = false;
    this.stage.dataset.mode = "serve";
    if (!this.folder) {
      const shown = this.folders().find((f) => f.kind === "picked") ?? null;
      if (shown) this.folder = shown;
    }
    this.roms = await keptRoms(undefined, APLET_MODELS);
    this.render();
    this.root.querySelector("h2")?.focus();
  }

  /** Back to the panes (serving stops). */
  async close() {
    await this.stopServing();
    this.root.hidden = true;
    delete this.stage.dataset.mode;
    this.root.replaceChildren();
  }

  render() {
    if (this.session) this.renderServing();
    else this.renderSetup();
  }

  // ---------------------------------------------------------- setup

  renderSetup() {
    const head = el("div", { class: "serve-head" },
      el("h2", { id: "serve-title", tabindex: "-1", text: "Serve a folder to a 38G, 39G or 40G" }),
      el("p", { text: "These calculators start every transfer themselves: this page is the “disk drive” they send aplets to and get them from. Choose a folder and the calculator, start, then press SEND or RECV on the calculator." }),
    );
    const folderBox = el("section", { class: "serve-step", "aria-labelledby": "serve-folder-title" },
      el("h3", { id: "serve-folder-title", class: "caps", text: "Folder" }),
      el("p", { class: "serve-choice", text: this.folder ? this.folder.title : "None chosen yet." }),
    );
    const folderActions = el("div", { class: "start-actions" });
    if (canPickFolder()) folderActions.append(this.button("Open a folder…", "folder-open", () => this.chooseFolder(), { id: "serve-open-folder", primary: !this.folder }));
    for (const f of this.folders()) {
      if (f.kind === "picked" && f !== this.folder) folderActions.append(this.button(`Use ${f.title}`, "folder", () => this.setFolder(f)));
    }
    // The desktop app has the computer's folders; the page's own files are the browser's.
    if (!isTauri()) folderActions.append(this.button("Files in this browser", "folder", async () => this.setFolder(await fallbackFolder()), { id: "serve-page-files" }));
    folderBox.append(folderActions);

    const calcBox = el("section", { class: "serve-step", "aria-labelledby": "serve-calc-title" },
      el("h3", { id: "serve-calc-title", class: "caps", text: "Calculator" }),
    );
    const choices = el("div", { class: "serve-calcs", role: "radiogroup", "aria-labelledby": "serve-calc-title" });
    const choice = (key, label, sub, icon, pick) => {
      const on = this.calc?.key === key;
      const b = el("button", { type: "button", class: "rom-choice", role: "radio", "aria-checked": String(on), "data-calc": key },
        iconEl(icon), el("span", {}, el("strong", { text: label }), sub ? el("span", { class: "muted", text: sub }) : null));
      b.addEventListener("click", pick);
      choices.append(b);
    };
    if (hasWebSerial() || isTauri()) choice("serial", "A calculator on a cable", "Its own serial cable (the 38G's has 10 pins); you choose the port", "plug", () => this.setCalc({ key: "serial" }));
    for (const r of this.roms ?? []) {
      choice(`rom:${r.sha256}:${r.model}`, `Emulated ${SERVE_EMULATED[r.model]}`, r.name, "calculator", () => this.setCalc({ key: `rom:${r.sha256}:${r.model}`, rom: r }));
    }
    choice("file", "An emulated one from a ROM file…", "A 38G, 39G or 40G ROM; it is not kept", "calculator", () => this.chooseRomFile());
    calcBox.append(choices);
    if (!hasWebSerial() && !isTauri()) calcBox.append(el("p", { class: "start-note", text: "This browser cannot reach a serial port: for a real calculator, open this page in Chrome or Edge on a computer. The emulated ones work here too." }));
    if (!(this.roms ?? []).length) calcBox.append(el("p", { class: "muted", text: "The emulator on this site keeps the ROMs you give it; a 38G, 39G or 40G ROM kept there shows up here." }));

    const actions = el("div", { class: "tx-dialog-actions serve-actions" });
    actions.append(this.button("Back", null, () => this.close(), { id: "serve-back" }));
    const go = this.button("Start serving", "plug", () => this.start(), { id: "serve-start", primary: true });
    go.disabled = !this.folder || !this.calc || this.busy;
    actions.append(go);
    this.root.replaceChildren(el("div", { class: "serve-box" }, head, folderBox, calcBox, actions,
      this.status ? el("p", { class: "muted", role: "status", text: this.status }) : null));
  }

  button(text, icon, run, { primary = false, id = null } = {}) {
    const b = el("button", { type: "button", class: primary ? "primary" : null, id }, icon ? iconEl(icon) : null, el("span", { text }));
    b.addEventListener("click", run);
    return b;
  }

  setFolder(f) {
    this.folder = f;
    this.render();
  }

  setCalc(c) {
    this.calc = c;
    this.render();
    this.root.querySelector(`[data-calc="${CSS.escape(c.key)}"]`)?.focus();
  }

  async chooseFolder() {
    try {
      const f = await pickFolder();
      if (f) this.setFolder(f);
    } catch (err) {
      this.app.say(String(err?.message ?? err), { error: true });
    }
  }

  chooseRomFile() {
    this.app.chooseFile(async (file) => {
      const bytes = new Uint8Array(await file.arrayBuffer());
      this.setCalc({ key: "file", bytes, name: file.name });
    });
  }

  // ---------------------------------------------------------- serving

  /** Start serving with the chosen folder and calculator. */
  async start({ model = null, speed = "1" } = {}) {
    if (!this.folder || !this.calc || this.busy) return;
    this.busy = true;
    this.status = "Starting…";
    this.render();
    let link;
    let emulator = null;
    let session = null;
    try {
      if (this.calc.key === "serial" && isTauri()) {
        // The app's chooser; cancelled is as the browser's chooser cancelled.
        const port = await this.app.choosePort();
        if (!port) throw new DOMException("no port chosen", "NotFoundError");
        link = new TauriSerialLink(port);
      } else if (this.calc.key === "serial") {
        const port = await navigator.serial.requestPort();
        link = new WebSerialLink(port);
      } else {
        emulator = new EmulatedCalculator();
        const bytes = this.calc.bytes ?? (await keptRomBytes(this.calc.rom.sha256));
        if (!bytes) throw new Error("That ROM could not be read from this browser's storage.");
        this.status = "Starting the emulated calculator…";
        this.render();
        // A 512 KB file here is a 38G's (the 48GX's is the same size, but
        // it is no calculator a folder is served to); 2 MB the 39G's.
        const guess = bytes.length === 512 * 1024 ? "38g" : "39g";
        const booted = await emulator.boot(bytes, this.calc.name ?? this.calc.rom.name, model ?? this.calc.rom?.model ?? guess, { aplets: true });
        if (speed !== "1") await emulator.setSpeed(speed);
        // The cold start's "Memory Clear": OK.
        await emulator.tap("f");
        await new Promise((r) => setTimeout(r, 1500));
        emulator.booted = booted;
        link = new EmulatorLink(emulator);
      }
      link = this.app.tapLink?.(link, "serve") ?? link;
      session = new ServeSession({ link, folder: this.folder, worker: this.app.apletWorker ?? apletWorker });
      session.addEventListener("event", (e) => this.onEvent(e.detail));
      session.addEventListener("lost", (e) => this.onLost(e.detail));
      this.files = await session.start();
      this.session = session;
      this.emulator = emulator;
      this.linkLabel = emulator ? `the emulated ${SERVE_EMULATED[emulator.booted]}` : `the calculator on the ${link.label}`;
      this.model = emulator ? (emulator.booted === "38g" ? "38g" : "39g") : null;
      if (emulator) emulator.addEventListener("frame", (e) => this.onFrame(e.detail));
    } catch (err) {
      emulator?.close();
      this.busy = false;
      this.status = "";
      this.render();
      if (err?.name !== "NotFoundError") this.app.say(String(err?.message ?? err), { error: true });
      return;
    }
    this.busy = false;
    this.status = "";
    this.lines = [];
    this.current = null;
    this.wire = false;
    await this.refreshWire();
    this.render();
    this.root.querySelector("h2")?.focus();
  }

  async stopServing() {
    const s = this.session;
    if (!s) return;
    this.session = null;
    await s.stop();
    this.emulator?.close();
    this.emulator = null;
  }

  async stop() {
    await this.stopServing();
    this.status = "Stopped serving.";
    this.render();
  }

  onLost(reason) {
    if (!this.session) return;
    this.stopServing().then(() => {
      this.status = "";
      this.render();
      this.app.say(`The calculator is gone: ${reason}`, { error: true });
    });
  }

  onFrame(frame) {
    this.lastFrame = frame;
    const c = this.root.querySelector("canvas.serve-lcd");
    if (c) drawFrame(c, frame);
  }

  /** The folder's binary aplets the 39G and 40G take by wire: `[{name, title}]`. */
  async refreshWire() {
    const api = await this.app.helpers();
    this.wireAplets = this.files
      .map((f) => ({ name: f.name, info: api.aplet_info(f.head) }))
      .filter((f) => f.info?.wire)
      .map((f) => ({ name: f.name, title: f.info.title }));
  }

  onEvent(e) {
    const progress = progressLine(e);
    if (progress) {
      this.current = progress;
      this.updateStatus();
      return;
    }
    if (e.type === "model") this.model = e.model;
    if (e.type === "notFound") this.lastMissing = e.name;
    if (e.type === "wireStarted") this.wire = true;
    if (["wireDone", "wireFailed"].includes(e.type)) this.wire = false;
    if (e.type === "received" && !e.directory) {
      const head = e.data?.slice?.(0, HEAD_BYTES);
      this.files = [...this.files.filter((f) => f.name.toLowerCase() !== (e.storedAs ?? e.name).toLowerCase()), { name: e.storedAs ?? e.name, head }];
      this.refreshWire().then(() => this.renderWire());
    }
    let line = null;
    if (e.type === "storeFailed") line = { kind: "error", text: `${e.name} could not be saved in the folder: ${e.message}` };
    else if (e.type === "refused") line = { kind: "error", text: e.message };
    else line = eventLine(e, { last: e.type === "calculatorSays" ? this.lastMissing : null });
    if (e.type === "calculatorSays") this.lastMissing = null;
    if (["sent", "received", "notFound", "calculatorSays", "failed", "discarded", "wireDone", "wireFailed"].includes(e.type)) this.current = null;
    if (line) {
      this.lines.unshift({ ...line, at: new Date() });
      this.renderLog();
    }
    this.updateStatus();
    if (e.type === "model") this.renderSteps();
  }

  updateStatus() {
    const s = this.root.querySelector(".serve-status");
    if (s) s.textContent = serveStatus({ model: this.model, current: this.current, wire: this.wire && !this.current });
  }

  renderServing() {
    const head = el("div", { class: "serve-head" },
      el("h2", { id: "serve-title", tabindex: "-1", text: `Serving ${this.folder.title}` }),
      el("p", { class: "serve-to muted", text: `To ${this.linkLabel}, at 9600 baud.` }),
      el("p", { class: "serve-status", role: "status", "aria-live": "polite", text: "" }),
    );
    const actions = el("div", { class: "serve-actions" },
      this.button("Stop serving", "close", () => this.stop(), { id: "serve-stop", primary: true }),
    );
    const steps = el("section", { class: "serve-steps", "aria-labelledby": "serve-steps-title" });
    const wire = el("section", { class: "serve-wire", "aria-labelledby": "serve-wire-title" });
    const screen = this.emulator ? this.renderEmulated() : null;
    const log = el("section", { class: "serve-log-box", "aria-labelledby": "serve-log-title" },
      el("h3", { id: "serve-log-title", class: "caps", text: "Transfers" }),
      el("ol", { class: "serve-log" }));
    this.root.replaceChildren(el("div", { class: "serve-box serving" }, head, actions,
      el("div", { class: "serve-grid" }, el("div", { class: "serve-left" }, steps, wire), screen ? el("div", { class: "serve-right" }, screen) : null),
      log));
    this.renderSteps();
    this.renderWire();
    this.renderLog();
    this.updateStatus();
    if (this.lastFrame) this.onFrame(this.lastFrame);
  }

  renderSteps() {
    const box = this.root.querySelector(".serve-steps");
    if (!box) return;
    const families = this.model ? [this.model === "38g" ? "38g" : "39g"] : ["38g", "39g"];
    const parts = [el("h3", { id: "serve-steps-title", class: "caps", text: "On the calculator" })];
    for (const f of families) {
      const s = SERVE_STEPS[f];
      const card = el("div", { class: "connect-steps serve-family", "data-family": f });
      if (families.length > 1) card.append(el("h4", { text: s.title }));
      card.append(
        el("p", { class: "serve-what", text: "To put an aplet in the folder:" }),
        el("ol", {}, ...s.send.map((t) => el("li", { text: t }))),
        el("p", { class: "keys", "aria-hidden": "true" }, ...s.sendKeys.map((k) => el("kbd", { class: "calc-key", text: k }))),
        el("p", { class: "serve-what", text: "To get one from the folder:" }),
        el("ol", {}, ...s.receive.map((t) => el("li", { text: t }))),
        el("p", { class: "keys", "aria-hidden": "true" }, ...s.receiveKeys.map((k) => el("kbd", { class: "calc-key", text: k }))),
      );
      parts.push(card);
    }
    parts.push(el("p", { class: "muted serve-note", text: SERVE_NOTE }));
    box.replaceChildren(...parts);
    this.renderWire();
  }

  renderWire() {
    const box = this.root.querySelector(".serve-wire");
    if (!box) return;
    if (this.model === "38g") {
      box.hidden = true;
      return;
    }
    box.hidden = false;
    const parts = [
      el("h3", { id: "serve-wire-title", class: "caps", text: "By wire (39G and 40G)" }),
      el("p", { text: "Binary aplets, most games among them, can also go by the calculator's wire transfer. Untested on a real 39G or 40G so far: tell us how it went." }),
    ];
    const aplets = this.wireAplets ?? [];
    if (!aplets.length) {
      parts.push(el("p", { class: "muted", text: "The folder has no binary 39G or 40G aplet." }));
    } else {
      const sel = el("select", { id: "serve-wire-file", "aria-label": "The aplet to send by wire" },
        ...aplets.map((a) => el("option", { value: a.name, text: a.title ? `${a.title} (${a.name})` : a.name })));
      const send = this.button("Send by wire", null, () => this.sendWire(sel.value), { id: "serve-wire-send" });
      send.disabled = this.wire;
      const stop = this.button("Stop", null, () => this.session?.cancel(), { id: "serve-wire-stop" });
      stop.hidden = !this.wire;
      parts.push(el("div", { class: "serve-wire-row" }, sel, send, stop));
      parts.push(el("ol", { class: "serve-wire-steps" }, ...SERVE_STEPS["39g"].wire.map((t) => el("li", { text: t }))));
    }
    box.replaceChildren(...parts);
  }

  async sendWire(name) {
    if (!this.session || !name) return;
    this.wire = true;
    this.renderWire();
    this.updateStatus();
    try {
      await this.session.sendWire(name);
    } catch (err) {
      this.wire = false;
      this.renderWire();
      this.app.say(String(err?.message ?? err), { error: true });
    }
  }

  renderLog() {
    const ol = this.root.querySelector(".serve-log");
    if (!ol) return;
    if (!this.lines.length) {
      ol.replaceChildren(el("li", { class: "serve-line muted", text: "Nothing yet: press SEND or RECV on the calculator." }));
      return;
    }
    ol.replaceChildren(...this.lines.slice(0, 200).map((l) => el("li", { class: `serve-line ${l.kind}` },
      el("time", { class: "muted", datetime: l.at.toISOString(), text: l.at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }) }),
      el("span", { text: l.text }))));
  }

  renderEmulated() {
    const family = this.model === "38g" ? "38g" : "39g";
    const canvas = el("canvas", { class: "serve-lcd", width: "131", height: "64", role: "img", "aria-label": "The emulated calculator's display" });
    const key = (name, label, aria = label) => {
      const b = el("button", { type: "button", class: "serve-key", "data-key": name, "aria-label": aria, text: label });
      b.addEventListener("click", () => this.emulator?.tap(name).catch(() => {}));
      return b;
    };
    const menu = el("div", { class: "serve-menu-keys" }, ...["a", "b", "c", "d", "e", "f"].map((k, i) => key(k, "", `Menu key ${i + 1}`)));
    const arrows = el("div", { class: "serve-arrows" }, ...ARROWS.map(([k, l, a]) => key(k, l, a)));
    const others = el("div", { class: "serve-other-keys" }, ...EMULATED_KEYS[family].map(([k, l]) => key(k, l)), key("on", "ON", "ON (cancel)"));
    return el("div", { class: "serve-emulated" },
      el("h3", { class: "caps", text: `The emulated ${SERVE_EMULATED[this.emulator.booted] ?? "calculator"}` }),
      el("div", { class: "serve-screen" }, canvas), menu, el("div", { class: "serve-keys" }, others, arrows));
  }
}
