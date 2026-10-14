// One pane of the transfer page (kb/docs/saturnus-tx.md, "Layout"): a
// place, which is a calculator (on a cable, or emulated in the page) or,
// in the right pane only, a folder on the computer (folder.js), with its head (the place's name,
// its status, "Change", "⋯"), its directory tree, its list and its
// preview. What crosses panes (copies, the queue, the dialogs) is the
// page's (app.js), reached through `app`.
//
// Keys in the list: F5 copies the selection to the other pane, F2
// renames, Delete purges or deletes (actions.js); rows drag to the other
// pane, and files and folders dragged from the computer's file manager
// drop onto a pane (into the directory under the pointer, else the one
// shown).

import { el } from "../shared/dom.js";
import { FOLDER_COLUMNS, COLUMNS, homeOf, parentPath, samePath } from "../shared/device.js";
import { iconEl } from "../shared/components/icons.js";
import { openMenu } from "../shared/components/menu.js";
import { DRAG_TYPE } from "../shared/components/device-list.js";
import "../shared/components/device-tree.js";
import "../shared/components/device-list.js";
import { keyAction, paneActions, rowActions } from "./actions.js";
import { EmulatedCalculator } from "./emulator.js";
import { canPickFolder, clearRecentFolders, download as downloadFolder, fallbackFolder, keptFolder, nameProblem as fileNameProblem, openRecent, pickFolder, recentFolders } from "./folder.js";
import { EmulatorLink, hasWebSerial } from "./links.js";
import { clearRecentNative, computerFolder, isTauri, pickNative, recentNative, usedNative } from "./tauri.js";
import { KermitDevice } from "./kermit-device.js";
import { ProtocolClient } from "./protocol.js";
import { renderPreview, textObject, updateReading } from "./preview.js";
import { disconnectNote, downloadUrl, failureText, modelTitle, placeTitle, previewPlan, searchText, settingsText } from "./flows.js";
import { fileItem, folderItem, problemText, removeQuestion, restoreQuestion } from "./transfers.js";
import { XMODEM_WAIT_S, XmodemDialog, fastDone, fastWords, hasXmodem } from "./xmodem.js";

/** Whether a calculator on a cable can be reached: Web Serial, or the desktop app's ports. */
const hasSerial = () => hasWebSerial() || isTauri();

/** Failures that are no news: the user stopped it, or the link closed. */
const quiet = (err) => ["cancelled", "aborted", "closed", "declined"].includes(err?.kind);

/** The most blob URLs kept for drags out to the desktop, per pane. */
export const MAX_DRAG_URLS = 20;

/** The name of the files the page keeps where no folder can be picked (OPFS or memory), and what they are. */
export const BROWSER_FILES = "Files in this browser";
export const BROWSER_FILES_NOTE = "Not on your disk. Use ⋯ → Download to save a file.";

/** Below this width "Open in the emulator" moves into the pane's ⋯ menu, so the header's buttons stay on one row. */
export const NARROW_HEAD = "(max-width: 479px)";

/** The most of a folder's file read for its preview. */
const FILE_PREVIEW_BYTES = 256 * 1024;

let placeSerial = 0;

/** The fast transfer's dialog, one for the page, made on first use. */
let fastDialog = null;
const xmodemDialog = () => (fastDialog ??= new XmodemDialog());

export class Pane {
  /** `root` the pane's section (from `#pane-template`); `id` "left" or "right"; `app` the page. */
  constructor(root, { id, app }) {
    this.root = root;
    this.id = id;
    this.app = app;
    const q = (s) => root.querySelector(s);
    this.ui = {
      title: q(".pane-title"), status: q(".pane-status-text"), progress: q(".pane-progress"),
      change: q(".pane-change"), refresh: q(".pane-refresh"), more: q(".pane-more"), emulator: q(".pane-emulator"),
      empty: q(".pane-empty"), body: q(".pane-body"), tree: q("device-tree"), list: q("device-list"),
      crumbs: q(".crumbs"), preview: q(".preview"),
    };
    this.ui.title.id = `pane-title-${id}`;
    root.setAttribute("aria-labelledby", this.ui.title.id);
    root.dataset.pane = id;
    /** The place shown: null, or `{id, kind, title, heading, device | folder, model, emulated, emulator}`. */
    this.place = null;
    /** Starting an emulated calculator, a connect, a folder that waits for its permission: what the card says. */
    this.step = null;
    this.path = ["HOME"];
    this.selected = null;
    this.previews = new Map();
    this.note = "";
    /** A kept folder that waits for its permission (a click). */
    this.pending = null;
    /** Blob URLs of files read for a drag out to the desktop, by key. */
    this.dragUrls = new Map();
    /**
     * The calculator previews asked for and not done (iteration 45), by
     * key: `{key, path, name, from, done, total, baud, emulated, waiting,
     * started, at}`, drawn in the preview and on their rows at most once
     * per animation frame. One runs at a time; a newer one waits for it.
     */
    this.readings = new Map();
    this.readingFrame = false;
    this.wire();
  }

  // ------------------------------------------------------------ helpers

  get other() { return this.app.other(this); }

  /** Whether the place is a calculator with the fast transfer (XModem: the 48GX and 49G). */
  get xmodem() {
    const p = this.place;
    return p?.kind === "device" && hasXmodem(p.model);
  }
  /** Whether the pane may show a folder: the right one only; the left shows calculators. */
  get folders() { return this.id === "right"; }

  /** The pane's place kind when it can be copied to: "device" (connected) or "folder"; else null. */
  get ready() {
    const p = this.place;
    if (!p) return null;
    if (p.kind === "device") return p.device.connection ? "device" : null;
    return "folder";
  }

  say(text, opts) { this.app.say(text, opts); }

  setStatus(text) {
    if (this.ui.status.textContent !== text) this.ui.status.textContent = text;
    this.ui.progress.textContent = "";
  }

  setProgress(text) { this.ui.progress.textContent = text; }

  /** The status of an idle place. */
  idleStatus() {
    const p = this.place;
    if (!p) return "";
    if (p.kind === "device") {
      if (p.device.restarted) return "The calculator restarted.";
      if (p.device.leftServer) return "The calculator left server mode.";
      return p.device.connection ? `Connected at ${settingsText(p.device.connection.settings)}.` : "";
    }
    const f = p.folder;
    if (f.kind === "native") return "The files on this computer.";
    if (f.kind === "picked") return "A folder on this computer.";
    if (f.kind === "opfs") return BROWSER_FILES_NOTE;
    return "Lost when this page closes. Use ⋯ → Download to save a file.";
  }

  // ------------------------------------------------------------ the place

  /** Show `place` (null: none); the old one's events are let go. */
  show(place) {
    this.place = place;
    this.selected = null;
    this.previews.clear();
    this.dropDragUrls();
    const source = place?.device ?? place?.folder ?? null;
    this.path = [...homeOf(source)];
    this.ui.list.columns = place?.kind === "folder" ? FOLDER_COLUMNS : COLUMNS;
    this.ui.list.emptyDirText = place?.kind === "folder" ? "This folder is empty. Drop files here, or copy from the other pane." : "This directory is empty. Drop a file here to put it on the calculator.";
    this.ui.list.tableLabel = (path) => (place?.kind === "folder" ? `Files in ${path.join(" ")}` : `Variables in ${path.join(" ")}`);
    this.ui.tree.setAttribute("aria-label", place?.kind === "folder" ? "Folders" : "Directories");
    const usable = place && (place.kind === "folder" || place.device.connection);
    this.ui.tree.device = usable ? source : null;
    this.ui.list.device = usable ? source : null;
    // The desktop app's computer opens at the home folder, below its top.
    if (usable && source?.start) {
      this.path = [...source.start];
      this.ui.list.path = this.path;
    }
    this.ui.tree.current = this.path;
    this.render();
  }

  /** The pane's name, in its header and on the narrow switch: what it shows, or what it is waiting for. */
  get heading() {
    if (this.place) return this.place.heading;
    if (this.step) return this.step.title;
    return this.id === "left" ? "No calculator" : "No folder";
  }

  render() {
    const p = this.place;
    const usable = Boolean(p && (p.kind === "folder" || p.device.connection));
    this.ui.empty.hidden = usable;
    this.ui.body.hidden = !usable;
    this.ui.refresh.hidden = !usable;
    this.ui.more.hidden = !p;
    // The emulator's own calculator is shown there too (not one from a ROM file, nor in the desktop app).
    this.ui.emulator.hidden = !(p?.kind === "device" && p.sharedEmu && this.app.emulatorUrl?.(p.model));
    this.ui.title.textContent = this.heading;
    if (!usable) this.renderCard();
    if (!this.busyLabel) this.setStatus(this.step?.status ?? this.idleStatus());
    this.renderCrumbs();
    this.app.placesChanged();
  }

  /** The card where there is no list: the ways in, a permission to give, a restart to reconnect after. */
  renderCard() {
    const p = this.place;
    const box = el("div", { class: "start" });
    const actions = el("div", { class: "start-actions" });
    const button = (text, icon, run, { primary = false, id = null } = {}) => {
      const b = el("button", { type: "button", class: primary ? "primary" : null, "data-action": id }, iconEl(icon), el("span", { text }));
      b.addEventListener("click", run);
      b.disabled = Boolean(this.step?.busy);
      actions.append(b);
      return b;
    };
    if (p?.kind === "device" && p.device.restarted) {
      box.append(
        el("h2", { text: "The calculator restarted" }),
        el("p", { text: p.emulated ? "The restore is done and the emulated calculator left server mode. Reconnect starts its server again." : "The restore is done and the calculator left server mode. Start its server on the calculator again, then reconnect." }),
      );
      button("Reconnect", "plug", () => this.reconnect(), { primary: true, id: "reconnect" });
      button(p.emulated && !p.sharedEmu ? "Turn it off" : "Disconnect", "disconnect", () => this.close(), { id: "close" });
    } else if (p?.kind === "device" && p.device.leftServer) {
      box.append(
        el("h2", { text: "The calculator left server mode" }),
        el("p", { text: p.emulated ? "The fast transfer ended the emulated calculator's server. Reconnect starts it again." : "The fast transfer ended the calculator's server. Start its server on the calculator again, then reconnect." }),
      );
      button("Reconnect", "plug", () => this.reconnect(), { primary: true, id: "reconnect" });
      button(p.emulated && !p.sharedEmu ? "Turn it off" : "Disconnect", "disconnect", () => this.close(), { id: "close" });
    } else if (this.pending) {
      box.append(
        el("h2", { text: `Use the folder ${this.pending.title} again?` }),
        el("p", { text: "The browser asks whether this page may read and write it. Nothing in it changes without a question." }),
      );
      button("Allow", "folder-open", () => this.allowPending(), { primary: true, id: "allow" });
      button("Choose another folder", "folder", () => this.openFolder(), { id: "pick" });
    } else if (this.step) {
      box.append(el("h2", { text: this.step.title }), el("p", { class: "muted", text: this.step.status ?? "" }));
    } else {
      const left = !this.folders;
      box.append(
        el("h2", { text: left ? "A calculator" : "A folder, or a calculator" }),
        el("p", { text: left ? "This side shows a calculator: one on a cable, or one emulated in this page. Copy to the other side by dragging, with F5, or from a row's menu." : "This side shows a folder on this computer, or a second calculator. Copy between the sides by dragging, with F5, or from a row's menu." }),
      );
      const serial = hasSerial();
      const pick = canPickFolder();
      const app = isTauri();
      if (!left) {
        if (pick) button("Open a folder", "folder-open", () => this.openFolder(), { primary: true, id: "open-folder" });
        if (app) button("This computer's files", "folder", () => this.useComputer(), { id: "use-computer" });
        else button(`Use ${BROWSER_FILES.toLowerCase()}`, "folder", () => this.useFiles(), { primary: !pick, id: "use-files" });
      }
      if (serial) button("Add a calculator", "plug", () => this.app.openConnect(this), { primary: left, id: "add-serial" });
      button("Add an emulated calculator", "calculator", () => this.app.openEmulated(this), { primary: left && !serial, id: "add-emulated" });
      box.append(actions);
      const serveLink = el("button", { type: "button", class: "link", id: "serve-aplets", text: "Serve a folder to a 38G, 39G or 40G" });
      serveLink.addEventListener("click", () => this.app.openServe());
      if (left) box.append(el("p", { class: "serve-link" }, serveLink));
      if (!serial && left) box.append(el("p", { class: "start-note", text: "This browser cannot reach a serial port. For a calculator on a cable, open this page in Chrome or Edge on a computer. The emulated calculator works here too." }));
      if (pick && !left && !app) box.append(el("p", { class: "start-note", text: "The browser does not open your home folder, Desktop or Downloads themselves: choose or make a folder inside one of them." }));
      if (!pick && !left) box.append(el("p", { class: "start-note", text: "This browser cannot open a folder on the computer: the page keeps files in the browser instead, not on your disk. Add files to them, and download what you copy there (⋯ → Download)." }));
      if (this.note) box.append(el("p", { class: "start-note last-note", role: "status", text: this.note }));
      this.ui.empty.replaceChildren(box);
      return;
    }
    box.append(actions);
    if (this.note) box.append(el("p", { class: "start-note last-note", role: "status", text: this.note }));
    this.ui.empty.replaceChildren(box);
  }

  renderCrumbs() {
    const parts = [];
    this.path.forEach((name, i) => {
      if (i) parts.push(el("span", { class: "crumb-sep", "aria-hidden": "true", text: "›" }));
      const here = i === this.path.length - 1;
      const b = el("button", { type: "button", class: "link crumb", "aria-current": here ? "location" : null, text: name });
      if (!here) b.addEventListener("click", () => this.navigate(this.path.slice(0, i + 1)));
      parts.push(b);
    });
    this.ui.crumbs.replaceChildren(...parts);
  }

  navigate(path) {
    // The desktop app keeps the folders shown for "Recent folders".
    if (this.place?.folder?.kind === "native" && path.length > 1) usedNative(path.slice(1)).catch(() => {});
    this.path = [...path];
    this.ui.tree.current = path;
    this.ui.list.path = path;
    this.select(null);
    this.renderCrumbs();
  }

  /** Whether the place has jobs running or waiting. */
  get inUse() { return Boolean(this.place && this.app.queue.uses(this.place.id)); }

  /** Let go of the place: a calculator leaves server mode; nothing while its jobs run. */
  async close({ note = null } = {}) {
    const p = this.place;
    if (!p) {
      this.pending = null;
      this.render();
      return true;
    }
    if (this.inUse) {
      this.say(`${p.title} still has copies running or waiting: let them finish, or cancel the waiting ones.`, { error: true });
      return false;
    }
    if (p.kind === "device") {
      // Ending the server, and letting go of the emulator's calculator (which waits for a send there to end), may take a while: said.
      this.step = { title: p.heading, status: "Disconnecting…", busy: true };
      this.setStatus(this.step.status);
      try {
        if (p.device.connection) {
          try {
            await p.device.disconnect();
          } catch (err) {
            this.say(`The calculator did not answer the end of server mode: ${failureText(err)}`, { error: true });
          }
        }
        if (this.place === p) this.render();
        await p.device.close().catch(() => {});
        await p.emulator?.close();
      } finally {
        this.step = null;
      }
      this.note = note ?? (p.sharedEmu ? "Disconnected. The emulator has the calculator as you left it." : p.emulated ? "The emulated calculator was turned off." : disconnectNote(p.model));
    } else {
      this.note = note ?? "";
    }
    if (this.place === p) this.show(null);
    return true;
  }

  // ------------------------------------------------------------ calculators

  wireDevice(place) {
    const device = place.device;
    device.addEventListener("busy", (e) => {
      if (this.place !== place) return;
      const { label, queued } = e.detail;
      this.busyLabel = label;
      if (label) this.setStatus(queued ? `${label} (${queued} more waiting)` : label);
      else this.setStatus(this.idleStatus());
      this.readingBusy(place);
    });
    device.addEventListener("progress", (e) => {
      if (this.place !== place) return;
      const { label, done, total } = e.detail;
      if (this.ui.status.textContent !== label) this.ui.status.textContent = label;
      this.setProgress(this.app.progressText(done, total));
      const r = this.runningRead(device);
      if (r) {
        r.done = done;
        if (total) r.total = total;
        r.started ??= this.app.clock();
        r.at = this.app.clock();
        this.drawReading();
      }
    });
    device.addEventListener("search", (e) => {
      if (this.place === place) this.setStatus(searchText(e.detail));
    });
    device.addEventListener("lost", (e) => {
      if (this.place !== place) return;
      this.app.queue.cancelWaiting(place.id);
      device.close().catch(() => {});
      place.emulator?.close();
      this.note = "";
      this.show(null);
      this.say(`The calculator is gone: ${e.detail}`, { error: true });
    });
  }

  /**
   * Connect over `link`, show the calculator (`emulated`, `model` the
   * expected one, `emulator` the EmulatedCalculator behind the link).
   * Resolves to the place, or null when it failed (said in a toast).
   */
  async connectLink(link, { emulated = false, model = null, emulator = null, settings = { find: true } } = {}) {
    if (this.place && !(await this.close())) return null;
    // A connect still running in this pane lets go of its link first: the newer one wins.
    const stale = this.mine;
    this.mine = null;
    if (stale?.device) {
      await stale.device.close().catch(() => {});
      if (stale.emulator !== emulator) stale.emulator?.close();
    }
    link = this.app.wrapLink?.(link) ?? link;
    // The cable console sees every byte (cable.js); the running job's size names "412 of 1,106 bytes".
    let device = null;
    link = this.app.tapLink?.(link, this.id, () => device?.running?.total ?? null) ?? link;
    const client = new ProtocolClient(link, this.app.deviceOptions);
    // An emulated calculator's server is started again when it ended behind the page's back (kermit-device.js), gently: never into a calculator in use.
    device = new KermitDevice(client, { emulated, recover: emulator ? () => emulator.startServer({ gentle: true }) : null });
    emulator?.addEventListener("keysElsewhere", () => device.suspectServer());
    const place = { id: `${this.id}:${++placeSerial}`, kind: "device", device, model, emulated, emulator, linkLabel: link.label, title: "the calculator", heading: placeTitle({ model, emulated, linkLabel: link.label }) };
    this.wireDevice(place);
    this.note = "";
    this.step = { title: place.heading, status: "Looking for the calculator…", busy: true };
    this.place = null;
    this.mine = place;
    this.render();
    try {
      const conn = await device.connect(emulated ? { find: true } : settings);
      if (this.mine !== place) return null;
      place.model = conn.model;
      place.heading = placeTitle({ model: conn.model, emulated, linkLabel: link.label });
      place.title = place.heading;
    } catch (err) {
      if (this.mine === place) {
        this.step = null;
        this.mine = null;
        await device.close().catch(() => {});
        emulator?.close();
        this.render();
        this.say(failureText(err), { error: true });
      }
      return null;
    }
    this.step = null;
    this.show(place);
    return place;
  }

  /**
   * The question before a command line open on the emulator's calculator
   * `title` is cancelled to start its server (emulator.js `startServer`):
   * what was typed there is never lost unasked.
   */
  cancelLineQuestion(title) {
    return (text) => {
      const typed = text.trim();
      const shown = typed.length > 40 ? `${typed.slice(0, 40)}…` : typed;
      return this.app.ask({
        title: "Cancel the command line?",
        body: `The emulator's ${title} has a command line open${shown ? ` (${shown})` : ""}. Connecting cancels it: what is typed there is lost.`,
        action: "Cancel it and connect",
      });
    };
  }

  /**
   * Boot `rom` in an emulated calculator, start its server and connect.
   * `model` the expected one (null: a ROM file, its model not known until
   * it boots); `speed` the emulated calculator's: "max" (the ROM's waits
   * for the line keep real time; typing SERVER and serving run as fast as
   * the computer can), or "1" as a real one.
   */
  async addEmulated(rom, name, model = null, { speed = "max", shared = false } = {}) {
    if (this.place && !(await this.close())) return null;
    const emu = new EmulatedCalculator({ shared });
    const mine = {};
    this.mine = mine;
    this.note = "";
    this.step = { title: model ? `Emulated ${modelTitle(model)}` : "Emulated calculator", status: shared ? "Joining the emulator's calculator…" : "Starting the emulated calculator…", busy: true };
    this.render();
    let booted;
    try {
      booted = await emu.boot(rom, name, model ?? "48gx");
      if (this.mine !== mine) throw new Error("replaced");
      this.step = { title: `Emulated ${modelTitle(booted)}`, status: "Starting its server…", busy: true };
      this.render();
      if (speed !== "1") await emu.setSpeed(speed);
      await emu.startServer({ cancelLine: shared ? this.cancelLineQuestion(modelTitle(booted)) : null });
    } catch (err) {
      emu.close();
      if (this.mine !== mine) return null;
      this.step = null;
      this.mine = null;
      this.render();
      this.say(String(err?.message ?? err), { error: true });
      return null;
    }
    if (this.mine !== mine) {
      emu.close();
      return null;
    }
    const place = await this.connectLink(new EmulatorLink(emu), { emulated: true, model: booted, emulator: emu });
    if (place && shared) {
      place.sharedEmu = true;
      this.render();
    }
    // Its ROM names the commands in a file from the same model (the preview).
    if (place) place.rom = rom;
    return place;
  }

  /** After a restore or a fast transfer: start the emulated server again, then connect. */
  async reconnect() {
    const p = this.place;
    if (p?.kind !== "device") return;
    this.step = { title: p.heading, status: "Looking for the calculator…", busy: true };
    this.renderCard();
    try {
      if (p.emulator) await p.emulator.startServer({ cancelLine: p.sharedEmu ? this.cancelLineQuestion(modelTitle(p.model)) : null });
      const conn = await this.app.queue.add({ label: `Reconnect to ${p.title}`, places: [p.id], run: () => p.device.connect(p.emulated ? { find: true } : this.app.settings) });
      p.model = conn.model;
    } catch (err) {
      this.step = null;
      this.render();
      if (!quiet(err)) this.say(failureText(err), { error: true });
      return;
    }
    this.step = null;
    this.show(p);
    this.say("Connected again.");
  }

  // ------------------------------------------------------------ folders

  /** Pick a folder (from a click): its permission is asked now, not at the first write. */
  async openFolder() {
    if (!this.folders) return null;
    if (isTauri()) return this.openNative();
    let folder;
    try {
      folder = await pickFolder();
    } catch (err) {
      this.say(problemText(err), { error: true });
      return null;
    }
    if (!folder) return null;
    return this.setFolder(folder);
  }

  /**
   * The desktop app's "Open a folder…": the folder chosen in its dialog,
   * shown in the computer's files (opened first where another place is
   * shown). Resolves to the place, or null.
   */
  async openNative() {
    let names;
    try {
      names = await pickNative();
    } catch (err) {
      this.say(problemText(err), { error: true });
      return null;
    }
    if (!names) return null;
    const place = this.place?.folder?.kind === "native" ? this.place : await this.useComputer();
    if (!place) return null;
    this.navigate(place.folder.pathOf(names));
    return place;
  }

  /** The desktop app's files: the whole computer, shown at the home folder. */
  async useComputer() {
    if (!this.folders) return null;
    let folder;
    try {
      folder = await computerFolder();
    } catch (err) {
      this.say(problemText(err), { error: true });
      return null;
    }
    return this.setFolder(folder);
  }

  /** Recent folder `names` of the desktop app (from the place switcher): shown in the computer's files. */
  async openRecentNative(names) {
    const place = this.place?.folder?.kind === "native" ? this.place : await this.useComputer();
    if (!place) return null;
    this.navigate(place.folder.pathOf(names));
    return place;
  }

  /** Recent folder `entry` of the browser (from a click: its permission is asked in it). */
  async openRecentFolder(entry) {
    let folder;
    try {
      folder = await openRecent(entry);
    } catch (err) {
      this.say(problemText(err), { error: true });
      return null;
    }
    return this.setFolder(folder);
  }

  /** "Recent folders" for the place switcher: menu items, none where folders are not picked. */
  async recentItems() {
    if (!this.folders) return [];
    let items = [];
    let clear;
    try {
      if (isTauri()) {
        const pathText = (names) => (/^[A-Z]:$/i.test(names[0] ?? "") ? names.join("\\") : `/${names.join("/")}`);
        items = (await recentNative()).map((r) => ({ text: r.names.at(-1) ?? "/", title: pathText(r.names), icon: "folder", run: () => this.openRecentNative(r.names) }));
        clear = clearRecentNative;
      } else if (canPickFolder()) {
        // The browser's click must reach requestPermission: the entry's handle is at hand.
        items = (await recentFolders()).map((r) => ({ text: r.name, icon: "folder", run: () => this.openRecentFolder(r) }));
        clear = () => clearRecentFolders();
      }
    } catch {
      return [];
    }
    if (!items.length) return [];
    return [{ note: "Recent folders" }, ...items, { text: "Clear recent folders", icon: "close", run: () => clear().catch((err) => this.say(problemText(err), { error: true })) }, "-"];
  }

  /** The files in this browser (OPFS, or memory). */
  async useFiles() {
    if (!this.folders) return null;
    return this.setFolder(await fallbackFolder());
  }

  /** The kept folder from the last visit: shown as a card that asks for its permission. */
  async offerKept() {
    if (!this.folders) return false;
    const kept = await keptFolder().catch(() => null);
    if (!kept || this.place) return false;
    let perm = "prompt";
    try {
      perm = await kept.permission();
    } catch {
      return false;
    }
    if (perm === "granted") {
      await this.setFolder(kept);
      return true;
    }
    this.pending = kept;
    this.render();
    return true;
  }

  async allowPending() {
    const f = this.pending;
    if (!f) return;
    let perm;
    try {
      perm = await f.requestAccess();
    } catch (err) {
      this.say(problemText(err), { error: true });
      return;
    }
    if (perm !== "granted") {
      this.say(`The page may not use ${f.title}. Choose it again, or another folder.`, { error: true });
      return;
    }
    this.pending = null;
    await this.setFolder(f);
  }

  /** Show `folder` (a Folder); never in the left pane, which shows calculators only (null). */
  async setFolder(folder) {
    if (!this.folders) return null;
    if (this.place && !(await this.close())) return null;
    this.pending = null;
    this.note = "";
    const heading = folder.kind === "native" ? folder.title : folder.kind === "picked" ? `Folder ${folder.title}` : BROWSER_FILES;
    const browser = folder.kind === "opfs" || folder.kind === "memory";
    const place = { id: `${this.id}:${++placeSerial}`, kind: "folder", folder, title: browser ? BROWSER_FILES : folder.title, heading, browser, picked: folder.kind === "picked" || folder.kind === "native" };
    this.show(place);
    return place;
  }

  // ------------------------------------------------------------ preview

  previewKey(path, entry) { return `${path.join("/")}/${entry.name}#${entry.checksum ?? entry.modified ?? ""}#${entry.size ?? ""}`; }

  previewState(entry) {
    if (!entry) return null;
    const kept = this.previews.get(this.previewKey(this.path, entry));
    if (kept) return kept;
    if (entry.directory) return { status: "none" };
    if (this.place?.kind === "folder") return Number(entry.size) <= FILE_PREVIEW_BYTES ? { status: "loading" } : { status: "ask", note: "A large file: it is read only when asked." };
    const plan = previewPlan(entry);
    if (!plan.fetch) return { status: "none" };
    return { status: plan.auto ? "loading" : "ask" };
  }

  /**
   * Keep `bytes` as a blob URL for a drag out to the desktop under `key`:
   * at most `MAX_DRAG_URLS`, the oldest let go first, one replaced let go.
   */
  keepDragUrl(key, bytes) {
    const old = this.dragUrls.get(key);
    if (old) {
      URL.revokeObjectURL(old);
      this.dragUrls.delete(key);
    }
    while (this.dragUrls.size >= MAX_DRAG_URLS) {
      const [k, url] = this.dragUrls.entries().next().value;
      URL.revokeObjectURL(url);
      this.dragUrls.delete(k);
    }
    this.dragUrls.set(key, URL.createObjectURL(new Blob([bytes], { type: "application/octet-stream" })));
  }

  /** Let go of every kept drag URL. */
  dropDragUrls() {
    for (const url of this.dragUrls.values()) URL.revokeObjectURL(url);
    this.dragUrls.clear();
  }

  async fetchPreview(entry) {
    const place = this.place;
    const path = this.path;
    const key = this.previewKey(path, entry);
    this.previews.set(key, { status: "loading" });
    if (place.kind === "device") this.startReading(place, path, entry, key);
    this.showPreview();
    let result;
    try {
      const h = await this.app.helpers();
      if (place.kind === "folder") {
        const file = await place.folder.read(path, entry.name);
        const kind = h.file_kind(file.data);
        if (this.place === place) this.keepDragUrl(key, file.data);
        if (kind === "binary") result = await this.describeFile(h, file.data);
        else if (kind === "text") result = { status: "shown", object: textObject("", h.text_of(file.data)) };
        else result = { status: "none", note: "Not a calculator file: put on a calculator, it becomes a string." };
      } else {
        const plan = previewPlan(entry);
        place.device.cancelQueued("preview");
        const file = await place.device.read(path, entry.name, { text: plan.fetch === "text", size: entry.size, tag: "preview" }).finally(() => this.endReading(key));
        if (plan.fetch === "text") result = { status: "shown", object: textObject(entry.kind, h.text_of(file.data)) };
        else {
          result = { ...(await this.describeFile(h, file.data)), binary: file };
          if (this.place === place) this.keepDragUrl(key, file.data);
        }
      }
    } catch (err) {
      this.endReading(key);
      if (quiet(err) || this.place !== place) {
        this.previews.delete(key);
        if (this.selected?.name === entry.name && samePath(this.path, path)) this.showPreview();
        return;
      }
      result = { status: "error", error: problemText(err) };
    }
    // A place shown since is not this preview's.
    if (this.place !== place) return;
    this.previews.set(key, result);
    if (this.selected?.name === entry.name && samePath(this.path, path)) this.showPreview();
  }

  /**
   * The preview of binary file `data`: decoded with the command names of
   * the ROM of the file's model where the page has one (an emulated
   * calculator of that model, or a ROM the emulator keeps), so programs
   * and algebraics show their text; without, why they do not.
   */
  async describeFile(h, data) {
    const model = h.file_model(data) ?? null;
    let names = model ? await this.app.fileNames?.(model) : null;
    let object = null;
    if (names) {
      try {
        object = names.describe_file(data);
      } catch {
        // Too large or too deep with the ROM's objects: as without names.
        names = null;
      }
    }
    object ??= h.describe_file(data);
    const result = { status: "shown", object };
    if (!names && (object?.type === "program" || object?.type === "algebraic") && object.source == null) {
      const what = object.type === "program" ? "this program's" : "this expression's";
      const m = model?.toUpperCase();
      result.model = model;
      result.reason = model === "48sx" || model === "48gx"
        ? `To show ${what} text, the page needs an HP ${m} ROM: start an emulated ${m} here, or add a ${m} ROM in the emulator on this site. Then select the file again.`
        : `To show ${what} text, copy it to a calculator: its preview there shows the text.`;
    }
    return result;
  }

  // ------------------------------------------------------------ the read in progress (iteration 45)

  /** A calculator preview starts: the preview and the row say what is read, from where, how far. */
  startReading(place, path, entry, key) {
    const conn = place.device.connection;
    const r = {
      key, path: [...path], name: entry.name, from: place.model ? `${place.emulated ? "emulated " : ""}${place.model.toUpperCase()}` : "calculator",
      done: 0, total: Number(entry.size) || null, baud: conn?.settings?.baud ?? null, emulated: Boolean(place.emulated),
      waiting: null, started: null, at: null,
    };
    this.readings.set(key, r);
    r.waiting = this.waitingFor(place.device, r);
    this.ui.list.setBusy(path, entry.name, null);
  }

  /** The read ended (shown, failed or cancelled): its marks go. */
  endReading(key) {
    const r = this.readings.get(key);
    if (!r) return;
    this.readings.delete(key);
    this.ui.list.setBusy(r.path, r.name, undefined);
  }

  /** The preview read the device runs now, or null (KermitDevice's `read` labels it "Getting NAME…"). */
  runningRead(device) {
    const op = device.running;
    if (op?.tag !== "preview") return null;
    for (const r of this.readings.values()) {
      if (op.label === `Getting ${r.name}…` || op.label === `Getting ${r.name} as text…`) return r;
    }
    return null;
  }

  /** What read `r` waits for: the label of the job the device runs, or null when it runs (or nothing does). */
  waitingFor(device, r) {
    const op = device.running;
    return op && this.runningRead(device) !== r ? op.label : null;
  }

  /** The reading state of `entry` in the directory shown. */
  readingFor(entry) {
    for (const r of this.readings.values()) if (r.name === entry.name && samePath(r.path, this.path)) return r;
    return null;
  }

  /** The device's queue moved: a preview waits behind another job, or its turn came. */
  readingBusy(place) {
    let changed = false;
    for (const r of this.readings.values()) {
      const waiting = this.waitingFor(place.device, r);
      if (waiting !== r.waiting) {
        r.waiting = waiting;
        changed = true;
      }
    }
    if (changed) this.drawReading();
  }

  /** Draw the reads' progress at the next animation frame (once, however many packets come before it). */
  drawReading() {
    if (this.readingFrame) return;
    this.readingFrame = true;
    this.app.frame(() => {
      this.readingFrame = false;
      for (const r of this.readings.values()) this.ui.list.setBusy(r.path, r.name, r.total && !r.waiting ? Math.min(r.done / r.total, 1) : null);
      const shown = this.selected ? this.readingFor(this.selected) : null;
      if (shown) updateReading(this.ui.preview, shown);
    });
  }

  showPreview(extra = null) {
    const entry = extra?.entry ?? this.selected;
    if (!entry) {
      renderPreview(this.ui.preview, null, null);
      return;
    }
    let st = extra?.state ?? this.previewState(entry);
    if (st?.status === "loading" && !extra) st = { ...st, reading: this.readingFor(entry) ?? { name: entry.name } };
    const close = el("button", { type: "button", class: "icon preview-close", title: "Close the preview", "aria-label": "Close the preview" }, iconEl("close"));
    close.addEventListener("click", () => {
      if (extra) this.showPreview();
      else this.ui.list.select(null);
    });
    const actions = [...(extra?.actions ?? []), close];
    if (!extra && !entry.directory) {
      const copy = el("button", { type: "button", class: "icon", title: `Copy ${entry.name} to the other pane (F5)`, "aria-label": `Copy ${entry.name} to the other pane` }, iconEl("copy-to"));
      copy.addEventListener("click", () => this.copySelection([entry]));
      actions.unshift(copy);
    }
    renderPreview(this.ui.preview, entry, st, { onShow: () => this.fetchPreview(entry), actions });
  }

  select(entry) {
    this.selected = entry;
    this.showPreview();
    if (!entry) return;
    // One shown without its ROM's names is read again once the ROM is here.
    const kept = this.previews.get(this.previewKey(this.path, entry));
    if (kept?.reason && kept.model) {
      const path = this.path;
      this.app.fileNames?.(kept.model).then((names) => {
        if (!names || this.selected !== entry || !samePath(this.path, path) || this.previews.get(this.previewKey(path, entry)) !== kept) return;
        this.fetchPreview(entry);
      });
    }
    const st = this.previewState(entry);
    const key = this.previewKey(this.path, entry);
    if (st.status === "loading" && !this.previews.has(key)) this.fetchPreview(entry);
  }

  // ------------------------------------------------------------ actions

  /** The selection, or the row's entry when it is outside it. */
  selection() {
    return this.ui.list.selectedEntries;
  }

  /** Run action `id` on `entries` (a row's menu, a key). */
  async act(id, entries = this.selection()) {
    const p = this.place;
    if (!p || !entries.length) return;
    const one = entries[0];
    switch (id) {
      case "open":
        this.navigate([...this.path, one.name]);
        break;
      case "copy":
        await this.copySelection(entries);
        break;
      case "getOne":
        await this.getOne(one);
        break;
      case "download":
        await this.download(entries);
        break;
      case "downloadText":
        await this.downloadText(one);
        break;
      case "rename":
        await this.rename(one);
        break;
      case "remove":
        await this.remove(entries);
        break;
      case "xmodemGet":
        await this.xmodemGet(one);
        break;
      case "xmodemPut": {
        const other = this.other;
        if (!other.xmodem || !other.ready) return;
        let file;
        try {
          file = await p.folder.read([...this.path], one.name);
        } catch (err) {
          this.say(`${one.name} could not be read: ${problemText(err)}`, { error: true });
          return;
        }
        await other.xmodemPut(file.data, one.name);
        break;
      }
      case "restoreFrom": {
        const other = this.other;
        if (other.ready !== "device") return;
        let file;
        try {
          file = await p.folder.read([...this.path], one.name);
        } catch (err) {
          this.say(`${one.name} could not be read: ${problemText(err)}`, { error: true });
          return;
        }
        await other.restore(file.data, one.name);
        break;
      }
      default:
    }
  }

  /** Copy `entries` (the selection) to the shown directory of the other pane. */
  async copySelection(entries = this.selection(), { single = false } = {}) {
    const other = this.other;
    const p = this.place;
    if (!p || !entries.length) return null;
    if (!other.ready) {
      this.say(other.folders ? "Show a calculator or a folder in the other pane first." : "Show a calculator in the other pane first.", { error: true });
      return null;
    }
    const items = p.kind === "folder" ? entries.map((e) => folderItem(p.folder, this.path, e)) : entries;
    return this.app.transfers.copy({ from: p, fromPath: [...this.path], items, to: other.place, toPath: [...other.path], single });
  }

  /** A calculator directory as one file: to the other pane's folder, else a download. */
  async getOne(entry) {
    if (this.other.ready === "folder") return this.copySelection([entry], { single: true });
    return this.download([entry]);
  }

  /** Download `entries`: a variable as its `.hp`, a folder's file as itself, several or a folder as a `.zip`. */
  async download(entries) {
    const p = this.place;
    // The directory as it is now: the pane may move on while the job waits.
    const path = [...this.path];
    try {
      if (p.kind === "folder") {
        const r = await this.app.queue.add({ label: `Download from ${p.title}`, places: [p.id], run: () => downloadFolder(p.folder, path, entries.map((e) => e.name), this.app.downloadDeps) });
        this.say(r.collisions.length ? `${r.name} saved to your downloads. Some names in it differ only in case: unzipped on macOS or Windows, one of each pair is lost.` : `${r.name} saved to your downloads.`);
        return;
      }
      for (const e of entries) {
        const file = await this.app.queue.add({ label: `Download ${e.name}`, places: [p.id], total: e.size, run: ({ report }) => this.app.transfers.watch([p], report, () => p.device.read(path, e.name, { size: e.size })) });
        this.app.download(file.fileName, file.data);
        this.say(`${file.fileName} saved to your downloads.`);
      }
    } catch (err) {
      if (!quiet(err)) this.say(`Not downloaded: ${problemText(err)}`, { error: true });
    }
  }

  /** A variable as the calculator's text: to the other pane's folder, else a download. */
  async downloadText(entry) {
    const p = this.place;
    const path = [...this.path];
    try {
      const file = await this.app.queue.add({ label: `Download ${entry.name} as text`, places: [p.id], total: entry.size, run: ({ report }) => this.app.transfers.watch([p], report, () => p.device.read(path, entry.name, { text: true, size: entry.size })) });
      this.app.download(file.fileName, file.data);
      this.say(`${file.fileName} saved to your downloads.`);
    } catch (err) {
      if (!quiet(err)) this.say(`Could not download ${entry.name} as text: ${problemText(err)}`, { error: true });
    }
  }

  /** A name check for this place: why `name` is refused, or null. */
  async nameCheck(name) {
    if (this.place?.kind === "folder") return fileNameProblem(name);
    const h = await this.app.helpers();
    return h.name_problem(name) ? "The calculator does not take this as a name." : null;
  }

  async rename(entry) {
    const p = this.place;
    const to = await this.app.askName({ title: `Rename ${entry.name}`, why: p.kind === "folder" ? "The new name:" : "The new name on the calculator:", suggested: entry.name, action: "Rename", check: (n) => this.nameCheck(n) });
    if (!to || to === entry.name) return;
    const path = [...this.path];
    try {
      await this.app.queue.add({ label: `Rename ${entry.name}`, places: [p.id], run: () => (p.kind === "folder" ? p.folder.rename(path, entry.name, to) : p.device.rename(path, entry.name, to)) });
      this.say(`${entry.name} is now ${to}.`);
    } catch (err) {
      if (quiet(err)) return;
      if (err?.kind === "nameClash" || err?.kind === "exists") this.say(`${to} is already there: rename or remove that first.`, { error: true });
      else this.say(`${entry.name} was not renamed: ${problemText(err)}`, { error: true });
    }
  }

  async mkdir() {
    const p = this.place;
    const name = await this.app.askName({ title: p.kind === "folder" ? "New folder" : "New directory", why: p.kind === "folder" ? `A new folder in ${this.path.at(-1)}:` : `A new directory in ${this.path.at(-1)}:`, suggested: p.kind === "folder" ? "New folder" : "NEWDIR", action: "Make", check: (n) => this.nameCheck(n) });
    if (!name) return;
    const path = [...this.path];
    try {
      await this.app.queue.add({ label: `Make ${name}`, places: [p.id], run: () => (p.kind === "folder" ? p.folder.mkdir(path, name) : p.device.mkdir(path, name)) });
    } catch (err) {
      if (!quiet(err)) this.say(`${name} was not made: ${problemText(err)}`, { error: true });
    }
  }

  /**
   * Purge (a calculator) or delete (a folder) `entries`, after one
   * question that counts what directories hold.
   */
  async remove(entries) {
    const p = this.place;
    const path = [...this.path];
    const device = p.kind === "device";
    // What the directories hold, every level, from their listings (a
    // calculator) or the folder: the question names it, and the purge sends
    // the same count, which the calculator side checks again.
    const counts = new Map();
    try {
      for (const e of entries.filter((x) => x.directory)) {
        counts.set(e.name, device ? await countTree(p.device, [...path, e.name]) : await p.folder.count(path, e.name));
      }
    } catch (err) {
      if (!quiet(err)) this.say(`What it holds could not be read: ${problemText(err)}`, { error: true });
      return;
    }
    const total = [...counts.values()].reduce((n, c) => n + c, 0);
    if (!(await this.app.ask(removeQuestion(entries.map((e) => e.name), { device, count: total, where: device ? "the calculator" : p.title })))) return;
    const gone = [];
    for (const e of entries) {
      try {
        await this.app.queue.add({ label: `${device ? "Purge" : "Delete"} ${e.name}`, places: [p.id], run: () => (device ? p.device.purge(path, e.name, { count: e.directory ? counts.get(e.name) ?? 0 : null }) : p.folder.purge(path, e.name, { recursive: e.directory })) });
        gone.push(e.name);
      } catch (err) {
        if (quiet(err)) continue;
        if (err?.kind === "notEmpty" || err?.kind === "not-empty") this.say(`${e.name} changed since the question: it holds ${err.count} now. Nothing was removed.`, { error: true });
        else this.say(`${e.name} was not removed: ${problemText(err)}`, { error: true });
      }
    }
    if (gone.length) this.say(gone.length === 1 ? `${gone[0]} ${device ? "purged" : "deleted"}.` : `${gone.length} ${device ? "purged" : "deleted"}.`);
  }

  // ------------------------------------------------------------ backup, restore, PICT

  /** The memory check, then the backup dialog (app.js). */
  async backup() {
    const p = this.place;
    let memory;
    try {
      memory = await this.app.queue.add({ label: `Check the memory of ${p.title}`, places: [p.id], run: () => p.device.memory() });
    } catch (err) {
      if (!quiet(err)) this.say(`The free memory could not be read: ${problemText(err)}`, { error: true });
      return;
    }
    const choice = await this.app.backupDialog({ memory, model: p.model, folder: this.other.ready === "folder" ? this.other.place.title : null, otherFolders: this.other.folders });
    if (choice === "backup") await this.runBackup();
    else if (choice === "copyAll") await this.copyAll();
  }

  async runBackup() {
    const p = this.place;
    // Where it goes is decided now, when asked: the other pane may change while the backup runs.
    const dest = this.other.ready === "folder" ? { place: this.other.place, path: [...this.other.path] } : null;
    const h = await this.app.helpers();
    let got;
    try {
      got = await this.app.queue.add({ label: `Back up ${p.title}`, places: [p.id], run: ({ report }) => this.app.transfers.watch([p], report, () => p.device.backup()) });
    } catch (err) {
      if (quiet(err)) return;
      if (err?.kind === "noRoom") this.say(`There is not room for a backup: it needs ${this.app.bytesText(err.needed)} free, the calculator has ${this.app.bytesText(err.free)}. "Copy everything to a folder" needs none.`, { error: true });
      else this.say(`No backup: ${problemText(err)}`, { error: true });
      return;
    }
    const d = this.app.now();
    const name = h.backup_file_name(got.model, d.getFullYear(), d.getMonth() + 1, d.getDate(), d.getHours(), d.getMinutes());
    if (dest) {
      try {
        await this.app.queue.add({ label: `Save ${name}`, places: [dest.place.id], run: async () => {
          const there = await dest.place.folder.existing(dest.path, name);
          let replace = false;
          if (there) {
            if (!(await this.app.ask({ title: `Replace ${name}?`, body: `${dest.place.title} already has a file named ${name}. The backup takes its place.`, action: "Replace", danger: true }))) throw Object.assign(new Error("not replaced"), { kind: "declined" });
            replace = true;
          }
          await dest.place.folder.write(dest.path, name, got.data, { replace });
        } });
        this.say(`The backup is ${name} in ${dest.place.title}.`);
      } catch (err) {
        if (!quiet(err)) this.say(`The backup was not saved: ${problemText(err)}`, { error: true });
      }
    } else {
      this.app.download(name, got.data);
      this.say(`${name} saved to your downloads.`);
    }
  }

  /** HOME as folders and `.hp` files in the other pane's folder, in a new folder named after the model and the time. */
  async copyAll() {
    const p = this.place;
    const other = this.other;
    if (other.ready !== "folder") {
      this.say(other.folders ? "Show a folder in the other pane first." : "A folder opens only in the right pane: show this calculator in the left one.", { error: true });
      return null;
    }
    // The folder and its directory as they are now, before anything waits: getting HOME can take minutes.
    const to = other.place;
    const toPath = [...other.path];
    const d = this.app.now();
    const h = await this.app.helpers();
    const root = h.backup_file_name(p.model, d.getFullYear(), d.getMonth() + 1, d.getDate(), d.getHours(), d.getMinutes()).replace(/-backup-/, "-HOME-").replace(/\.hp$/, "");
    try {
      await this.app.queue.add({ label: `Copy everything to ${to.title}`, places: [p.id, to.id], run: ({ report }) => this.app.transfers.watch([p], report, async () => {
        const tree = await p.device.getTree(["HOME"]);
        const files = tree.items.map((i) => (i.directory ? { path: i.fsPath } : { path: i.fsPath, data: i.data }));
        await this.app.transfers.writeTree(to, toPath, root, files);
      }) });
      this.say(`Everything is in the folder ${root} of ${to.title}.`);
      return root;
    } catch (err) {
      if (!quiet(err)) this.say(`Not copied: ${problemText(err)}`, { error: true });
      return null;
    }
  }

  /** Restore `bytes` (`fileName` for the question): asks first, then the calculator restarts. */
  async restore(bytes, fileName) {
    const p = this.place;
    // Only a backup or a directory object restores: a binary file that is
    // told to be something else (a program, say), or no binary file at all,
    // is said and not sent. One the preview cannot decode is left to the
    // calculator.
    const h = await this.app.helpers();
    const binary = h.file_kind(bytes) === "binary";
    let kind = null;
    if (binary) {
      try {
        kind = h.describe_file(bytes)?.kind ?? null;
      } catch {
        kind = null;
      }
    }
    if (!binary || (kind && kind !== "Backup" && kind !== "Directory")) {
      this.say(`${fileName} is not a backup${kind ? ` (it holds a ${kind.toLowerCase()})` : ""}: nothing was restored.`, { error: true });
      return false;
    }
    if (!(await this.app.ask(restoreQuestion(fileName, p.title)))) return false;
    try {
      await this.app.queue.add({ label: `Restore ${p.title}`, places: [p.id], total: bytes.length, run: ({ report }) => this.app.transfers.watch([p], report, () => p.device.restore(bytes)) });
    } catch (err) {
      if (!quiet(err)) this.say(`Not restored: ${problemText(err)}`, { error: true });
      return false;
    }
    this.show(p);
    this.say("The calculator restarted and left server mode.");
    return true;
  }

  // ------------------------------------------------------------ the fast transfer (XModem)

  /**
   * Run a fast transfer on this pane's calculator: the dialog says what
   * happens and what to type, the job waits in the queue like any other.
   * Resolves to `{result, error, left, words}` (`left`: the server ended).
   */
  async fast({ direction, name, path, bytes = null, size = null }) {
    const p = this.place;
    const d = xmodemDialog();
    const base = { direction, name, model: p.model, emulated: p.emulated, waitS: XMODEM_WAIT_S };
    let planned = null;
    const put = direction === "put";
    const onProgress = (e) => {
      if (!planned) return;
      const { done, total } = e.detail;
      d.progress(`${put ? "Sending" : "Getting"} ${name}… ${this.app.progressText(done, total)}`.trim(), done, total);
    };
    /** The queue's job: while it waits there, Cancel drops it; once it runs, the device stops it. */
    let job = null;
    d.start(fastWords(base), {
      cancel: () => {
        if (job && this.app.queue.cancel(job.id)) return;
        p.device.stopXmodem();
      },
      type: () => planned && p.emulator?.typeLine(planned.keys).catch((err) => this.say(`The command was not typed: ${String(err?.message ?? err)}`, { error: true })),
    }, { emulated: p.emulated });
    p.device.addEventListener("progress", onProgress);
    let result = null;
    let error = null;
    try {
      job = this.app.queue.add({
        label: `Fast transfer of ${name} ${put ? "to" : "from"} ${p.title}`,
        places: [p.id],
        total: bytes?.length ?? size,
        run: ({ report }) => {
          const go = () => p.device.xmodem(path, name, {
            bytes, size, wait: XMODEM_WAIT_S,
            onPlan: (plan) => {
              planned = plan;
              d.plan(fastWords({ ...base, plan }));
            },
          });
          return this.app.transfers.watch([p], report, () => (p.emulator ? p.emulator.atSpeed("1", go) : go()));
        },
      });
      result = await job;
    } catch (err) {
      error = err;
    } finally {
      p.device.removeEventListener("progress", onProgress);
    }
    const left = p.device.leftServer;
    if (left && this.place === p) this.show(p);
    return { result, error, left, base };
  }

  /** The dialog's last words, and the reconnect it offers. */
  async fastFinish({ base, error, left }, where = null) {
    const p = this.place;
    const answer = await xmodemDialog().finish(fastDone({ ...base, error, left, where }));
    if (answer === "reconnect" && this.place === p && p?.device.leftServer) await this.reconnect();
  }

  /** Get `entry` by the fast transfer: into the other pane's folder, else a download. */
  async xmodemGet(entry) {
    const p = this.place;
    if (!this.xmodem) return;
    const path = [...this.path];
    const dest = this.other.ready === "folder" ? { place: this.other.place, path: [...this.other.path] } : null;
    const run = await this.fast({ direction: "get", name: entry.name, path, size: entry.size });
    let where = null;
    if (run.result) {
      const file = run.result;
      try {
        if (dest) {
          where = await this.saveTo(dest, file.fileName, file.data);
        } else {
          this.app.download(file.fileName, file.data);
          where = `${file.fileName} in your downloads`;
        }
      } catch (err) {
        if (!quiet(err)) run.error = { kind: "calculator", message: `it came over, but was not saved: ${problemText(err)}` };
        else run.error = err;
      }
    }
    if (p === this.place) await this.fastFinish(run, where);
  }

  /** Save `bytes` as `name` in `dest` (`{place, path}`), asking before a replace; resolves to where it went. */
  async saveTo(dest, name, bytes) {
    await this.app.queue.add({ label: `Save ${name}`, places: [dest.place.id], run: async () => {
      const there = await dest.place.folder.existing(dest.path, name);
      let replace = false;
      if (there) {
        if (!(await this.app.ask({ title: `Replace ${name}?`, body: `${dest.place.title} already has a file named ${name}. The one from the calculator takes its place.`, action: "Replace", danger: true }))) throw Object.assign(new Error("not replaced"), { kind: "declined" });
        replace = true;
      }
      await dest.place.folder.write(dest.path, name, bytes, { replace });
    } });
    return `${name} in ${dest.place.title}`;
  }

  /** Put `bytes` (a file named `fileName`) into the shown directory by the fast transfer, after asking for the name. */
  async xmodemPut(bytes, fileName) {
    const p = this.place;
    if (!this.xmodem || !p.device.connection) return;
    const h = await this.app.helpers();
    let suggested = "";
    try {
      suggested = h.var_name(fileName);
    } catch {
      suggested = "";
    }
    const name = await this.app.askName({ title: "Fast transfer to the calculator", why: `The name of the variable in ${this.path.at(-1)}:`, suggested, action: "Continue", check: (n) => this.nameCheck(n) });
    if (!name) return;
    const run = await this.fast({ direction: "put", name, path: [...this.path], bytes });
    if (p !== this.place) return;
    if (run.error?.kind === "nameClash") run.error = { kind: "calculator", message: `${name} is already there: a fast transfer does not replace it. Purge it first, or use the normal copy, which can.` };
    await this.fastFinish(run);
  }

  /**
   * Graphics screen file `data` to the other pane's shown directory,
   * through the copy of dropped files (its questions, the queue, the
   * other pane refreshed): PICT.hp in a folder, a variable of a name
   * asked on a calculator; a download when the other pane shows nothing.
   */
  async savePict(data) {
    const other = this.other;
    if (!other.ready) {
      this.app.download("PICT.hp", data);
      this.say("PICT.hp saved to your downloads.");
      return null;
    }
    let fileName = "PICT.hp";
    if (other.place.kind === "device") {
      // PICT is the calculator's own name for the graphics screen: sent
      // under it, the 48SX stores the variable PICT.1. Another name, asked.
      const h = await this.app.helpers();
      const name = await this.app.askName({
        title: "A name for the graphics screen",
        why: `PICT is the calculator's own name for its graphics screen. The variable in ${other.path.at(-1)} on the other calculator:`,
        suggested: "PIC",
        action: "Copy",
        check: (n) => (n === "PICT" ? "PICT is the graphics screen itself: choose another name." : h.name_problem(n) ? "The calculator does not take this as a name." : null),
      });
      if (!name) return null;
      fileName = h.file_name(name, "binary");
    }
    const item = { name: fileName, directory: false, size: data.length, read: async () => data, children: async () => [] };
    return this.app.transfers.copy({ from: null, items: [item], to: other.place, toPath: [...other.path] });
  }

  /** The graphics screen as a GROB, in the preview. */
  async pict() {
    const p = this.place;
    const h = await this.app.helpers();
    let got;
    try {
      got = await this.app.queue.add({ label: `Graphics screen of ${p.title}`, places: [p.id], run: () => p.device.pict() });
    } catch (err) {
      if (!quiet(err)) this.say(`Could not get the graphics screen: ${problemText(err)}`, { error: true });
      return;
    }
    let state;
    try {
      state = { status: "shown", object: h.describe_file(got.data) };
    } catch (err) {
      state = { status: "error", error: String(err?.message ?? err) };
    }
    this.showPict(got, state);
  }

  /**
   * The graphics screen `got` in the preview, with the variable preview's
   * button: a copy to the other pane when it shows a folder (PICT.hp) or a
   * calculator (a variable, its name asked), else a download. A click
   * after the other pane changed draws the button anew and does nothing
   * else, so it never does what it does not say.
   */
  showPict(got, state) {
    const entry = { name: "Graphics screen", kind: `Graphic, ${got.width} × ${got.height}`, size: got.data.length, checksum: null };
    const to = this.other.ready ? this.other.place : null;
    const save = to
      ? el("button", { type: "button", class: "icon", title: `Copy the graphics screen to the other pane (${to.kind === "folder" ? "PICT.hp" : "as a variable"})`, "aria-label": "Copy the graphics screen to the other pane" }, iconEl("copy-to"))
      : el("button", { type: "button", class: "icon", title: "Download the graphics screen as PICT.hp", "aria-label": "Download the graphics screen as PICT.hp" }, iconEl("save"));
    save.addEventListener("click", () => {
      if ((this.other.ready ? this.other.place : null) !== to) {
        this.showPict(got, state);
        this.say("The other pane changed: the button now says where the graphics screen goes.");
        return;
      }
      this.savePict(got.data);
    });
    this.showPreview({ entry, state, actions: [save] });
  }

  async runPaneAction(id) {
    const p = this.place;
    if (!p) return;
    switch (id) {
      case "mkdir": return this.mkdir();
      case "backup": return this.backup();
      case "restore": return this.app.chooseFile((file) => file.arrayBuffer().then((b) => this.restore(new Uint8Array(b), file.name)));
      case "pict": return this.pict();
      case "xmodemPut": return this.app.chooseFile((file) => file.arrayBuffer().then((b) => this.xmodemPut(new Uint8Array(b), file.name)));
      case "copyAll": return this.copyAll();
      case "settings": return this.app.openSettings();
      case "refresh": return this.refresh();
      case "close": return this.close();
      case "emulator": return p.kind === "device" && p.sharedEmu ? this.app.openEmulator(p.model) : undefined;
      case "upload": return this.app.chooseFiles((files) => this.drop(files.map(fileItem)));
      case "zipAll": return this.download((this.ui.list.listing?.entries ?? []));
      default:
    }
  }

  refresh() {
    const p = this.place;
    if (!p) return;
    this.previews.clear();
    this.dropDragUrls();
    if (p.kind === "device") p.device.refresh(null);
    else this.ui.list.reload();
    this.ui.tree.device = this.ui.tree.device;
    this.ui.tree.current = this.path;
  }

  /** Copy dropped files and folders (FsItems) into `path` (the shown directory by default). */
  async drop(items, path = this.path) {
    if (!this.ready || !items.length) return null;
    return this.app.transfers.copy({ from: null, items, to: this.place, toPath: [...path] });
  }

  // ------------------------------------------------------------ wiring

  wire() {
    const { list, tree } = this.ui;
    list.emptyText = "";
    list.rowsDraggable = true;
    // The page's token with the pane: a drag from another tab of this page is not ours.
    list.dragSource = { page: this.app.pageToken, pane: this.id };
    list.dragOut = (entries) => {
      if (entries.length !== 1 || entries[0].directory) return null;
      const e = entries[0];
      const url = this.dragUrls.get(this.previewKey(this.path, e));
      if (!url) return null;
      return downloadUrl(this.place?.kind === "folder" ? e.name : `${e.name}.hp`, url);
    };
    list.actions = (entry, entries) => rowActions({ kind: this.place?.kind, entries: entries.length ? entries : [entry], other: this.other.ready, otherFolders: this.other.folders, xmodem: this.place?.kind === "device" ? this.xmodem : this.other.xmodem }).map((it) => (it === "-" ? it : { ...it, hint: it.key, run: () => this.act(it.id, entries.length ? entries : [entry]) }));
    // One row selected: its preview; none or several: none.
    list.addEventListener("select", (e) => this.select(e.detail.entries.length === 1 ? e.detail.entry : null));
    list.addEventListener("open", (e) => {
      const entry = e.detail.entry;
      this.navigate(entry.up ? parentPath(this.path) : [...this.path, entry.name]);
    });
    tree.addEventListener("navigate", (e) => this.navigate(e.detail.path));
    this.ui.refresh.addEventListener("click", () => this.refresh());
    this.ui.change.addEventListener("click", () => this.openSwitcher());
    this.ui.emulator.addEventListener("click", async () => {
      const p = this.place;
      if (!(p?.kind === "device" && p.sharedEmu)) return;
      // Both pages use it at once: nothing here stops.
      this.app.openEmulator(p.model);
    });
    this.ui.more.addEventListener("click", () => {
      const p = this.place;
      if (!p) return;
      const items = paneActions({ kind: p.kind, connected: Boolean(p.device?.connection), emulated: p.emulated, picked: p.picked, other: this.other.ready, otherFolders: this.other.folders, xmodem: this.xmodem });
      // A phone's header has no room for "Open in the emulator": it is here instead.
      if (!this.ui.emulator.hidden && globalThis.matchMedia?.(NARROW_HEAD).matches) items.unshift({ id: "emulator", text: "Open in the emulator", icon: "calculator" }, "-");
      openMenu({ anchor: this.ui.more, label: p.kind === "device" ? "Calculator" : "Folder", returnFocus: () => this.ui.more, items: items.map((it) => (it === "-" ? it : { ...it, run: () => this.runPaneAction(it.id) })) });
    });
    this.root.addEventListener("keydown", (e) => {
      if (e.target.closest("input, textarea, select, dialog, .menu")) return;
      const id = keyAction(e);
      if (!id) return;
      // F5 anywhere in a pane copies, never reloads the page (which would end every connection and job).
      if (id === "copy") e.preventDefault();
      if (!this.ui.list.contains(e.target)) {
        if (id === "copy") this.say("Select what to copy in the list first.", { error: true });
        return;
      }
      const entries = this.selection();
      if (!entries.length) {
        if (id === "copy") this.say("Select what to copy in the list first.", { error: true });
        return;
      }
      if (id === "rename" && entries.length !== 1) return;
      e.preventDefault();
      this.act(id, entries);
    });
    // Drops: rows from the other pane, files and folders from the file manager.
    this.root.addEventListener("dragover", (e) => {
      const types = [...(e.dataTransfer?.types ?? [])];
      if (!(types.includes(DRAG_TYPE) || types.includes("Files"))) return;
      // Taken either way, so the browser never opens a dropped file in the tab; refused where nothing can go.
      e.preventDefault();
      e.dataTransfer.dropEffect = this.ready ? "copy" : "none";
      this.root.classList.toggle("drop", Boolean(this.ready));
    });
    this.root.addEventListener("dragleave", (e) => {
      if (!this.root.contains(e.relatedTarget)) this.root.classList.remove("drop");
    });
    this.root.addEventListener("drop", (e) => {
      this.root.classList.remove("drop");
      if (!e.dataTransfer) return;
      e.preventDefault();
      if (!this.ready) return;
      // Onto a directory's row: into that directory.
      const row = e.target.closest?.("device-list tr.dir[data-name]");
      const path = row ? [...this.path, row.dataset.name] : [...this.path];
      const own = e.dataTransfer.getData(DRAG_TYPE);
      if (own) {
        let data;
        try {
          data = JSON.parse(own);
        } catch {
          return;
        }
        // Rows of another tab of this page name panes of that tab: not copied from here.
        if (data.source?.page !== this.app.pageToken) {
          this.say("Rows dragged from another tab cannot be copied here; drag them within one tab.", { error: true });
          return;
        }
        const from = this.app.paneById(data.source.pane);
        if (!from || from === this) return;
        this.app.dropFrom(from, data, this, path);
        return;
      }
      const items = droppedItems(e.dataTransfer);
      if (!items.length) return;
      this.drop(items, path);
    });
  }

  /** The place switcher: a calculator, an emulated one; in the right pane also a folder or the files in this browser. */
  async openSwitcher() {
    const items = [];
    if (this.folders) {
      if (canPickFolder()) items.push({ text: "A folder on this computer…", icon: "folder-open", run: () => this.openFolder() });
      if (isTauri()) items.push({ text: "This computer's files", icon: "folder", run: () => this.useComputer() }, "-");
      else items.push({ text: BROWSER_FILES, icon: "folder", run: () => this.useFiles() }, "-");
      items.push(...(await this.recentItems()));
    }
    if (hasSerial()) items.push({ text: "A calculator on a cable…", icon: "plug", run: () => this.app.openConnect(this) });
    items.push({ text: "An emulated calculator…", icon: "calculator", run: () => this.app.openEmulated(this) });
    items.push("-", { text: "Serve a folder to a 38G, 39G or 40G…", icon: "calculator", run: () => this.app.openServe() });
    if (this.place) items.push("-", { text: this.place.kind === "device" ? (this.place.emulated && !this.place.sharedEmu ? "Nothing (turn the emulated calculator off)" : "Nothing (disconnect)") : "Nothing (close the folder)", icon: "close", run: () => this.close() });
    openMenu({ anchor: this.ui.change, align: "end", label: "Show in this pane", returnFocus: () => this.ui.change, items });
  }
}

/** How many variables and directories `path` of `device` holds, every level (as a purge counts them). */
export async function countTree(device, path) {
  const listing = await device.list(path);
  let n = listing.entries.length;
  for (const e of listing.entries.filter((x) => x.directory)) n += await countTree(device, [...path, e.name]);
  return n;
}

/**
 * What a drop from the file manager holds, as FsItems: folders where the
 * browser gives them (`webkitGetAsEntry`, read now: the items are gone
 * after the event), files otherwise.
 */
export function droppedItems(dt) {
  const out = [];
  const entries = [...(dt.items ?? [])].filter((i) => i.kind === "file").map((i) => i.webkitGetAsEntry?.() ?? null);
  if (entries.length && entries.every(Boolean)) {
    for (const entry of entries) out.push(entryItem(entry));
    return out;
  }
  return [...(dt.files ?? [])].map(fileItem);
}

/** A FileSystemEntry (a dropped file or folder) as an FsItem. */
export function entryItem(entry) {
  if (entry.isDirectory) {
    return {
      name: entry.name,
      directory: true,
      size: null,
      read: async () => new Uint8Array(),
      children: async () => {
        const reader = entry.createReader();
        const all = [];
        for (;;) {
          const batch = await new Promise((resolve, reject) => reader.readEntries(resolve, reject));
          if (!batch.length) break;
          all.push(...batch);
        }
        return all.map(entryItem);
      },
    };
  }
  return {
    name: entry.name,
    directory: false,
    size: null,
    read: async () => {
      const file = await new Promise((resolve, reject) => entry.file(resolve, reject));
      return new Uint8Array(await file.arrayBuffer());
    },
    children: async () => [],
  };
}
