// saturnus tx, the transfer page (/saturnus/transfer/): the composition
// root (kb/docs/saturnus-tx.md, "Layout"). Two panes (pane.js), each
// showing a place: a calculator on a serial cable (Web Serial) or one
// emulated in the page; the right pane also a folder on the computer
// (folder.js; the page's own files where no folder can be picked), the
// left never. The calculator left and a folder right by default; below 760 px one pane at a time, with a
// switch above them. Copies between the panes and from the file manager
// (transfers.js) go through one queue (queue.js), shown under the panes:
// one job per place at a time, the waiting ones cancellable. The
// protocol runs in a dedicated Worker per calculator (protocol-worker.js);
// this page holds the links and the dialogs. Every link is tapped for the
// cable console (cable.js, cable-view.js: "Show the packets"). `window.tx`
// is for the tests.

import init, * as wasmApi from "./pkg/saturnus_tx_web.js";
import { el } from "../shared/dom.js";
import { saveDownload } from "../shared/download.js";
import { MAX_ROM_FILE } from "../shared/limits.js";
import { radioStep, radioTabIndexes } from "../shared/radiogroup.js";
import { KEYS, TRANSFER_KEYS } from "../shared/storage-names.js";
import { THEMES, THEME_KEY, applyTheme, themeOf } from "../shared/theme.js";
import { confirmAction } from "../shared/components/confirm.js";
import { iconEl, loadIcons } from "../shared/components/icons.js";
import { placeToasts, toast } from "../shared/components/toast.js";
import { HUB_RESTORE } from "./actions.js";
import { SERIAL_MODELS, keptRomBytes, keptRoms } from "./kept-roms.js";
import { KermitDevice } from "./kermit-device.js";
import { FakeLink, RecordingLink, WebSerialLink, hasWebSerial } from "./links.js";
import { Folder, canPickFolder, fallbackFolder } from "./folder.js";
import { TauriSerialLink, isTauri, saveFile, serialPorts } from "./tauri.js";
import { MemoryDirectory } from "./memory-handle.js";
import { Pane } from "./pane.js";
import { ServeView } from "./serve.js";
import { JobQueue } from "./queue.js";
import { Transfers, fileItem, flatten, folderItem } from "./transfers.js";
import { CableLog } from "./cable.js";
import { CableView } from "./cable-view.js";
import { inDesktopApp, linkTab, nameTab, onSiteMessage, openSiteTab, refusalText, siteHref, takeParam } from "../shared/site-links.js";
import { EMULATED_NAMES, MODELS, MODEL_NAMES, SERVER_SHOWN, START_SERVER, bytesText, linkSettings, progressText, settingsText } from "./flows.js";

const $ = (id) => document.getElementById(id);
const ui = {
  stage: $("stage"), panes: $("panes"), switch: $("pane-switch"), template: $("pane-template"),
  queue: $("queue"), queueList: $("queue-list"), cancelWaiting: $("cancel-waiting"),
  fileInput: $("file-input"), restoreInput: $("restore-input"), romInput: $("rom-input"),
  connectDialog: $("connect-dialog"), connectModels: $("connect-models"), connectSteps: $("connect-steps"),
  connectGo: $("connect-go"), connectOther: $("connect-other"), connectKnown: $("connect-known"),
  portDialog: $("port-dialog"), portSelect: $("port-select"), portNone: $("port-none"), portAgain: $("port-again"), portGo: $("port-go"),
  emulatedDialog: $("emulated-dialog"), emulatedRoms: $("emulated-roms"), emulatedNone: $("emulated-none"), emulatedFile: $("emulated-file"),
  settingsDialog: $("settings-dialog"), setFind: $("set-find"), setBaud: $("set-baud"), setParity: $("set-parity"), setTheme: $("set-theme"),
  nameDialog: $("name-dialog"), nameTitle: $("name-title"), nameWhy: $("name-why"), nameInput: $("name-input"), nameProblem: $("name-problem"), nameOk: $("name-ok"),
  backupDialog: $("backup-dialog"), backupText: $("backup-text"), backupMemory: $("backup-memory"), backupAlt: $("backup-alt"),
  backupGo: $("backup-go"), backupCopy: $("backup-copy"), backupCancel: $("backup-cancel"),
};

// ------------------------------------------------------------ preferences

const store = {
  get(key) {
    try { return localStorage.getItem(key); } catch { return null; }
  },
  set(key, value) {
    try { localStorage.setItem(key, value); } catch { /* storage blocked: kept for this visit */ }
  },
};

// ------------------------------------------------------------ the wasm helpers

let wasm = null;
/** The bindings in the page (files, names, the copy plan, the preview); the protocol's own copy runs in its Worker. */
const helpers = () => (wasm ??= init().then(() => wasmApi));

/**
 * The command names of a `model` ROM ("48sx", "48gx") for the preview of
 * a file from that model (`FileNames`), or null: from an emulated
 * calculator of that model in a pane, else from the ROMs the emulator
 * keeps on this site. Built once per ROM (by its SHA-256, a ROM that
 * fails is not tried again); none found is asked again next time (a ROM
 * may have been added since).
 */
const fileNamesBySha = new Map();
const romShas = new WeakMap();
async function romSha(bytes) {
  if (!romShas.has(bytes)) {
    const d = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
    romShas.set(bytes, [...d].map((b) => b.toString(16).padStart(2, "0")).join(""));
  }
  return romShas.get(bytes);
}
async function fileNames(model) {
  if (model !== "48sx" && model !== "48gx") return null;
  const h = await helpers();
  const build = (sha, bytes) => {
    if (!fileNamesBySha.has(sha)) {
      let names = null;
      try {
        names = new h.FileNames(model, bytes);
      } catch {
        names = null;
      }
      fileNamesBySha.set(sha, names);
    }
    return fileNamesBySha.get(sha);
  };
  for (const p of panes) {
    const place = p.place;
    if (place?.kind === "device" && place.model === model && place.rom) {
      const names = build(await romSha(place.rom), place.rom);
      if (names) return names;
    }
  }
  for (const r of await keptRoms()) {
    if (r.model !== model) continue;
    if (fileNamesBySha.has(r.sha256)) {
      if (fileNamesBySha.get(r.sha256)) return fileNamesBySha.get(r.sha256);
      continue;
    }
    const bytes = await keptRomBytes(r.sha256);
    const names = bytes ? build(r.sha256, bytes) : null;
    if (names) return names;
  }
  return null;
}

// ------------------------------------------------------------ the page

const say = (text, { error = false } = {}) => toast(text, { error });

const queue = new JobQueue();

/**
 * The page's questions, one after the other: jobs on different places run
 * at once, and a question one of them asks waits for the one open, rather
 * than cancelling it (confirm.js answers an open question with Cancel when
 * asked again).
 */
let asking = Promise.resolve();
function ask(question) {
  const answer = asking.then(() => confirmAction(question));
  asking = answer.catch(() => false);
  return answer;
}

/** Every packet on every link, for "Show the packets" (a ring buffer; drawn only while the panel is open). */
const cable = new CableLog();

/** What the panes reach of the page. */
const app = {
  /** This tab's token: rows dragged carry it, so a drop from another tab is told apart. */
  pageToken: Math.random().toString(36).slice(2) + Date.now().toString(36),
  settings: linkSettings(store.get(TRANSFER_KEYS.link)),
  /** Options for the next calculator's protocol client (the tests: a fixed seed, a fake Worker). */
  deviceOptions: {},
  /** Wraps the next link (the tests: a RecordingLink). */
  wrapLink: null,
  cable,
  /** `link` with its traffic in the cable console as `source` ("left", "right", "serve"); `expected()` the running job's bytes. */
  tapLink: (link, source, expected) => cable.tap(link, { source, expected }),
  /** The next animation frame (progress is drawn at most once per frame) and the clock in ms. */
  frame: (fn) => requestAnimationFrame(fn),
  clock: () => performance.now(),
  /** The emulator page on `model`'s calculator (`?model=`), or null in the desktop app. */
  emulatorUrl: (model) => {
    if (inDesktopApp()) return null;
    const href = siteHref("emulator", location.pathname);
    if (!href) return null;
    const url = new URL(href, location.href);
    url.searchParams.set("model", model);
    return url.href;
  },
  /** The emulator in its tab on `model` (shared/site-links.js): brought forward and told to show it when open, else opened with `?model=`. */
  openEmulator: (model) => {
    const url = app.emulatorUrl(model);
    if (url && openSiteTab("emulator", url, { show: model }, { here: false }) === "blocked") say("The browser did not open the emulator's tab: allow pop-ups for this site.", { error: true });
  },
  /** The last download's `{name, size}`; with `keepDownloads` (the tests, tx-chrome.mjs) also its `bytes`. */
  lastDownload: null,
  keepDownloads: false,
  downloadDeps: {},
  queue,
  helpers,
  fileNames,
  say,
  ask: (q) => ask(q),
  askName: (q) => askName(q),
  now: () => new Date(),
  bytesText,
  progressText: (done, total) => progressText({ label: "", done, total }).trim(),
  other: (pane) => (pane === panes[0] ? panes[1] : panes[0]),
  paneById: (id) => panes.find((p) => p.id === id) ?? null,
  openConnect: (pane) => openConnectDialog(pane),
  openEmulated: (pane) => openEmulatedDialog(pane),
  openSettings: () => openSettings(),
  download: (name, bytes) => download(name, bytes),
  chooseFile: (fn) => choose(ui.restoreInput, (files) => files[0] && fn(files[0])),
  chooseFiles: (fn) => choose(ui.fileInput, fn),
  backupDialog: (opts) => backupDialog(opts),
  placesChanged: () => { renderSwitch(); renderPacketsButton(); },
  dropFrom: (from, data, to, path) => dropFrom(from, data, to, path),
};
/** "Serve a folder to a 38G, 39G or 40G" (serve.js), over the panes while open. */
const serve = new ServeView($("serve"), {
  stage: ui.stage,
  app,
  folders: () => panes.map((p) => (p.place?.kind === "folder" ? p.place.folder : null)).filter(Boolean),
});
app.openServe = () => serve.show();
$("connect-serve").addEventListener("click", () => {
  ui.connectDialog.close();
  serve.show();
});
// The desktop app saves downloads through its save dialog.
if (isTauri()) app.downloadDeps = { save: saveFile };
app.transfers = new Transfers({
  queue, helpers, ask: app.ask, askName: app.askName, say,
  frame: (fn) => app.frame(fn),
  mark: (place, path, name, value) => {
    for (const p of panes) if (p.place === place) p.ui.list.setBusy(path, name, value);
  },
});
app.cableView = new CableView($("cable"), { log: cable, toggle: $("show-cable"), say });

/** The two panes: left and right. */
const panes = [];
for (const id of ["left", "right"]) {
  const root = ui.template.content.firstElementChild.cloneNode(true);
  ui.panes.append(root);
  panes.push(new Pane(root, { id, app }));
}

// ------------------------------------------------------------ narrow: the switch

let active = "left";

function setActive(id) {
  active = id;
  ui.panes.dataset.active = id;
  renderSwitch();
}

/**
 * "Show the packets" is gone while there is nothing to show: the panes'
 * calculators (shown or still connecting) are all the emulator's, whose
 * link has no packets, nothing is served, and no packet came yet. An open
 * panel keeps it (and is never closed from here).
 */
function renderPacketsButton() {
  const devices = panes.map((p) => p.place ?? p.mine).filter((d) => d?.kind === "device");
  const serving = !$("serve").hidden;
  $("show-cable").hidden = devices.length > 0 && devices.every((d) => d.hub) && !serving && !app.cableView?.isOpen && cable.size === 0;
}

function renderSwitch() {
  if (panes.length < 2) return;
  const tabs = radioTabIndexes(["left", "right"], active);
  ui.switch.replaceChildren(...panes.map((p, i) => el("button", {
    type: "button", role: "radio", "aria-checked": String(p.id === active), tabindex: String(tabs[i]), "data-pane": p.id,
  }, el("span", { class: "switch-name", text: p.heading }))));
}

ui.switch.addEventListener("click", (e) => {
  const b = e.target.closest("[data-pane]");
  if (b) setActive(b.dataset.pane);
});
ui.switch.addEventListener("keydown", (e) => {
  const next = radioStep(["left", "right"], active, e.key);
  if (next) {
    e.preventDefault();
    setActive(next);
    ui.switch.querySelector('[aria-checked="true"]')?.focus();
  }
});

// ------------------------------------------------------------ the queue

/** The jobs and their states the list shows: when only progress changed, the rows are updated in place. */
let queueShape = "";

/** A running job's amount: `1.2 KB of 3.4 KB`, or "Running…". */
const jobAmount = (j) => (j.done ? progressText({ label: "", done: j.done, total: j.total }).trim() : "");

function renderQueue() {
  const jobs = queue.jobs;
  ui.queue.hidden = jobs.length === 0;
  ui.cancelWaiting.hidden = queue.waiting.length < 2;
  const shape = jobs.map((j) => `${j.id}:${j.state}:${j.total ? 1 : 0}`).join(",");
  if (shape === queueShape) {
    // Progress only: the rows stay, and with them a focused Cancel button.
    for (const j of queue.running) {
      const li = ui.queueList.querySelector(`[data-job="${j.id}"]`);
      if (!li) continue;
      const amount = jobAmount(j);
      li.querySelector(".queue-state").textContent = amount || "Running…";
      const bar = li.querySelector("progress");
      if (bar) {
        bar.max = j.total;
        bar.value = Math.min(j.done, j.total);
        bar.setAttribute("aria-label", `${j.label}: ${amount || "started"}`);
      }
    }
    return;
  }
  queueShape = shape;
  const focused = document.activeElement?.closest?.("#queue-list [data-job]")?.dataset.job ?? null;
  ui.queueList.replaceChildren(...jobs.map((j) => {
    const running = j.state === "running";
    const li = el("li", { class: `queue-job ${j.state}`, "data-job": String(j.id) });
    const amount = running ? jobAmount(j) : "";
    li.append(
      el("span", { class: "queue-label", text: j.label }),
      el("span", { class: "queue-state muted", text: running ? amount || "Running…" : "Waiting" }),
    );
    if (running && j.total) li.append(el("progress", { class: "queue-bar", max: String(j.total), value: String(Math.min(j.done, j.total)), "aria-label": `${j.label}: ${amount || "started"}` }));
    if (!running) {
      const b = el("button", { type: "button", class: "icon queue-cancel", title: `Cancel: ${j.label}`, "aria-label": `Cancel ${j.label}` }, iconEl("close"));
      b.addEventListener("click", () => queue.cancel(j.id));
      li.append(b);
    }
    return li;
  }));
  // A Cancel that had the focus keeps it while its job still waits; else the next one, or the list's head.
  if (focused !== null) {
    const again = ui.queueList.querySelector(`[data-job="${focused}"] .queue-cancel`) ?? ui.queueList.querySelector(".queue-cancel");
    (again ?? (ui.cancelWaiting.hidden ? null : ui.cancelWaiting))?.focus();
  }
}

queue.addEventListener("change", renderQueue);
ui.cancelWaiting.addEventListener("click", () => queue.cancelWaiting());
/** Whether leaving the page now would cut something short: a transfer running or waiting, a calculator working, or one on a cable (its port and server mode are not got back by a reload). */
function busyToLeave() {
  if (queue.running.length + queue.waiting.length > 0) return true;
  return panes.some((p) => p.place?.kind === "device" && (p.place.device.running || (!p.place.emulated && p.place.device.connection)));
}

// Leaving mid-transfer or with a calculator on a cable (closing, reloading,
// a link, the emulator's link loading this tab) asks first: the browser's "Leave site?".
window.addEventListener("beforeunload", (e) => {
  if (!busyToLeave()) return;
  e.preventDefault();
  e.returnValue = "";
});

// ------------------------------------------------------------ files in and out

/** Save `bytes` as a download named `name` (in the desktop app: where its save dialog says). */
function download(name, bytes) {
  // Not the bytes: a page open for hours would hold every file it saved.
  app.lastDownload = { name, size: bytes.length, ...(app.keepDownloads ? { bytes } : {}) };
  if (isTauri()) {
    saveFile(name, bytes).catch((err) => say(`${name} was not saved: ${err?.message ?? err}`, { error: true }));
    return;
  }
  saveDownload(name, bytes);
}

/** Open file input `input` and call `fn(files)` with what was chosen. */
function choose(input, fn) {
  input.onchange = () => {
    const files = [...(input.files ?? [])];
    input.value = "";
    if (files.length) fn(files);
  };
  input.click();
}

/** Rows dragged from pane `from` (`data`: `{path, names}`) dropped on `to` at `path`. */
async function dropFrom(from, data, to, path) {
  const p = from.place;
  if (!p || !to.place) return null;
  const listing = await (p.kind === "device" ? p.device.list(data.path) : p.folder.list(data.path)).catch(() => null);
  const entries = (listing?.entries ?? []).filter((e) => data.names.includes(e.name));
  if (!entries.length) return null;
  const items = p.kind === "folder" ? entries.map((e) => folderItem(p.folder, data.path, e)) : entries;
  return app.transfers.copy({ from: p, fromPath: data.path, items, to: to.place, toPath: path });
}

// A file dropped anywhere the panes do not take it is not opened in the tab
// (which would end every connection): outside the panes, and inside one
// that shows nothing usable (pane.js refuses it there).
document.addEventListener("dragover", (e) => {
  if ([...(e.dataTransfer?.types ?? [])].includes("Files")) e.preventDefault();
});
document.addEventListener("drop", (e) => {
  if ([...(e.dataTransfer?.types ?? [])].includes("Files")) e.preventDefault();
});

// ------------------------------------------------------------ the name question

/**
 * Ask for a name: `title`, `why` (a sentence), `suggested`, `action` (the
 * button), `check(name)` why it is refused or null (the calculator's rule
 * by default). Resolves to the name, or null.
 */
function askName({ title = "A name for it", why = "", suggested = "", action = "Put", check = null } = {}) {
  return new Promise((resolve) => {
    ui.nameTitle.textContent = title;
    ui.nameWhy.textContent = why;
    ui.nameOk.textContent = action;
    ui.nameInput.value = suggested;
    ui.nameProblem.textContent = "";
    const test = async () => {
      const name = ui.nameInput.value.trim();
      let problem;
      if (!name) problem = "A name is needed.";
      else if (check) problem = await check(name);
      else problem = (await helpers()).name_problem(name) ? "The calculator does not take this as a name." : null;
      ui.nameProblem.textContent = problem ?? "";
      return problem ? null : name;
    };
    const onClose = async () => {
      ui.nameDialog.removeEventListener("close", onClose);
      // Escape and Cancel leave returnValue as they find it: "" here.
      const ok = ui.nameDialog.returnValue === "ok";
      ui.nameDialog.returnValue = "";
      if (!ok) {
        resolve(null);
        return;
      }
      const name = await test();
      if (name) resolve(name);
      else {
        ui.nameDialog.addEventListener("close", onClose);
        ui.nameDialog.showModal();
        ui.nameInput.select();
      }
    };
    ui.nameDialog.returnValue = "";
    ui.nameDialog.addEventListener("close", onClose);
    ui.nameDialog.showModal();
    ui.nameInput.select();
  });
}

$("name-cancel").addEventListener("click", () => ui.nameDialog.close(""));

// ------------------------------------------------------------ the backup question

/**
 * The backup dialog: the free memory and what a backup needs; resolves to
 * "backup", "copyAll" (the folder copy, which needs no free memory) or
 * null. `folder` the other pane's folder title, null without one;
 * `otherFolders` whether the other pane may show one (the right pane).
 */
function backupDialog({ memory, hub = false, model, folder, otherFolders = true }) {
  const name = EMULATED_NAMES[model] ?? "calculator";
  // The emulator's own calculator has no free figure: its backup says so itself when it does not fit.
  const fits = memory?.fits ?? true;
  ui.backupText.textContent = hub
    ? `A backup is one file with everything in HOME. While it runs, the ${name} needs twice what HOME holds free, and 1 KB more. ${HUB_RESTORE}`
    : fits
      ? `A backup is one file with everything in HOME, which a restore puts back. The ${name} has room for it.`
      : `There is not room on the ${name} for a backup: while it runs, it needs twice what HOME holds free, and 1 KB more.`;
  ui.backupMemory.hidden = !memory;
  ui.backupMemory.replaceChildren(...(memory ? [
    el("dt", { text: "Free memory" }), el("dd", { text: bytesText(memory.free) }),
    el("dt", { text: "HOME holds" }), el("dd", { text: bytesText(memory.home) }),
    el("dt", { text: "A backup needs" }), el("dd", { text: bytesText(memory.needed) }),
  ] : []));
  ui.backupAlt.textContent = folder
    ? `Or copy everything to ${folder} as folders and files, one variable at a time: slower, and it needs no free memory.`
    : otherFolders
      ? "Or show a folder in the other pane and copy everything there, one variable at a time: slower, and it needs no free memory."
      : "Or show this calculator in the left pane and a folder in the right one, and copy everything there, one variable at a time: slower, and it needs no free memory.";
  ui.backupGo.disabled = !fits;
  ui.backupGo.textContent = folder ? `Back up to ${folder}` : "Back up as a download";
  ui.backupCopy.disabled = !folder;
  return new Promise((resolve) => {
    const done = (answer) => {
      ui.backupGo.onclick = null;
      ui.backupCopy.onclick = null;
      ui.backupCancel.onclick = null;
      ui.backupDialog.removeEventListener("close", onClose);
      if (ui.backupDialog.open) ui.backupDialog.close();
      resolve(answer);
    };
    const onClose = () => done(null);
    ui.backupGo.onclick = () => done("backup");
    ui.backupCopy.onclick = () => done("copyAll");
    ui.backupCancel.onclick = () => done(null);
    ui.backupDialog.addEventListener("close", onClose);
    ui.backupDialog.showModal();
    (fits ? ui.backupGo : folder ? ui.backupCopy : ui.backupCancel).focus();
  });
}

// ------------------------------------------------------------ connect

/** The pane the connect and emulated dialogs fill. */
let target = panes[0];

let connectModel = MODELS.includes(store.get(TRANSFER_KEYS.model)) ? store.get(TRANSFER_KEYS.model) : "48gx";

function renderConnectDialog() {
  const tabs = radioTabIndexes(MODELS, connectModel);
  ui.connectModels.replaceChildren(...MODELS.map((m, i) => el("button", { type: "button", role: "radio", "aria-checked": String(m === connectModel), tabindex: String(tabs[i]), "data-model": m, text: MODEL_NAMES[m].replace(/^HP /, "") })));
  const s = START_SERVER[connectModel];
  ui.connectSteps.replaceChildren(
    el("h3", { class: "caps", text: `On the ${s.title}` }),
    el("ol", {}, ...s.steps.map((t) => el("li", { text: t }))),
    el("p", { class: "keys", "aria-hidden": "true" }, ...s.keys.map((k) => el("kbd", { class: "calc-key", text: k }))),
    el("p", { class: "muted", text: s.keysNote }),
    el("p", { text: `${SERVER_SHOWN} Then:` }),
  );
}

function setConnectModel(m) {
  connectModel = m;
  store.set(TRANSFER_KEYS.model, m);
  renderConnectDialog();
  ui.connectModels.querySelector('[aria-checked="true"]')?.focus();
}

ui.connectModels.addEventListener("click", (e) => {
  const b = e.target.closest("[data-model]");
  if (b) setConnectModel(b.dataset.model);
});
ui.connectModels.addEventListener("keydown", (e) => {
  const next = radioStep(MODELS, connectModel, e.key);
  if (next) {
    e.preventDefault();
    setConnectModel(next);
  }
});

/** A port the page was given before (Chrome remembers it) that no pane holds, or null. */
async function knownPort() {
  try {
    const ports = await navigator.serial.getPorts();
    const held = new Set(panes.map((p) => p.place?.device?.client?.link?.port).filter(Boolean));
    return ports.find((p) => !held.has(p)) ?? null;
  } catch {
    return null;
  }
}

/**
 * The desktop app's port chooser (the browser's own in Chrome): the ports
 * with their adapters' names, the one used last chosen. Resolves to the
 * port, or null when cancelled.
 */
async function choosePort() {
  const fill = async () => {
    let ports = [];
    try {
      ports = await serialPorts();
    } catch (err) {
      say(String(err?.message ?? err), { error: true });
    }
    const last = store.get(TRANSFER_KEYS.port);
    ui.portSelect.replaceChildren(...ports.map((p) => el("option", { value: p.name, text: p.label, selected: p.name === last })));
    ui.portNone.hidden = ports.length > 0;
    ui.portSelect.disabled = ports.length === 0;
    ui.portGo.disabled = ports.length === 0;
    return ports;
  };
  let ports = await fill();
  ui.portAgain.onclick = async () => {
    ports = await fill();
  };
  ui.portDialog.returnValue = "";
  ui.portDialog.showModal();
  await new Promise((resolve) => ui.portDialog.addEventListener("close", resolve, { once: true }));
  const port = ui.portDialog.returnValue === "ok" ? ports.find((p) => p.name === ui.portSelect.value) ?? null : null;
  if (port) store.set(TRANSFER_KEYS.port, port.name);
  return port;
}
app.choosePort = () => choosePort();

/** The desktop app's port used last, if it is still there. */
async function lastNativePort() {
  const last = store.get(TRANSFER_KEYS.port);
  if (!last) return null;
  const held = new Set(panes.map((p) => p.place?.device?.client?.link?.port?.name).filter(Boolean));
  try {
    return (await serialPorts()).find((p) => p.name === last && !held.has(p.name)) ?? null;
  } catch {
    return null;
  }
}

/** The connect dialog in the desktop app: the port used last, or the app's chooser. */
async function openNativeConnect(pane) {
  target = pane;
  renderConnectDialog();
  const known = await lastNativePort();
  ui.connectKnown.hidden = !known;
  ui.connectKnown.textContent = known ? `The port used last: ${known.label}.` : "";
  ui.connectOther.hidden = !known;
  ui.connectGo.textContent = known ? "Connect" : "Choose the port";
  const go = (choosing) => connectPort(choosing, (port) => new TauriSerialLink(port));
  ui.connectGo.onclick = () => go(known ? Promise.resolve(known) : choosePort());
  ui.connectOther.onclick = () => go(choosePort());
  ui.connectDialog.showModal();
}

async function openConnectDialog(pane) {
  if (isTauri()) return openNativeConnect(pane);
  if (!hasWebSerial()) {
    say("This browser cannot reach a serial port: open the page in Chrome or Edge on a computer.", { error: true });
    return;
  }
  target = pane;
  renderConnectDialog();
  const known = await knownPort();
  ui.connectKnown.hidden = !known;
  ui.connectKnown.textContent = known ? "This browser already has a port for this page from last time." : "";
  ui.connectOther.hidden = !known;
  ui.connectGo.textContent = known ? "Connect" : "Choose the port";
  ui.connectGo.onclick = () => connectPort(known ? Promise.resolve(known) : navigator.serial.requestPort());
  ui.connectOther.onclick = () => connectPort(navigator.serial.requestPort());
  ui.connectDialog.showModal();
}

async function connectPort(choosing, makeLink = (port) => new WebSerialLink(port)) {
  let port;
  try {
    port = await choosing;
  } catch {
    return; // the chooser was cancelled
  }
  if (!port) return; // the app's chooser was cancelled
  ui.connectDialog.close();
  await target.connectLink(makeLink(port), { model: connectModel, settings: app.settings });
}

/** The emulator's calculator of kept ROM `r` (`keptRoms`) in `pane`; the emulator desktop app's ROM (`r.app`) as a private one. */
async function addKept(pane, r) {
  const bytes = await keptRomBytes(r.sha256);
  if (!bytes) {
    say(r.app ? "That ROM could not be read from the emulator app's files." : "That ROM could not be read from this browser's storage.", { error: true });
    return null;
  }
  // The desktop app's kept ROMs have no shared calculator: a private one, as a ROM file.
  return r.app ? pane.addEmulated(bytes, r.name, r.model) : pane.addShared(bytes, r.name, r.model);
}

/** The model the left pane is being connected to from the emulator (`connectFromEmulator`), or null. */
let connecting = null;

/**
 * The emulator's "Files" link (`?connect=` at start, or a
 * message to this tab, shared/site-links.js): the left pane on the
 * emulator's calculator of `model`, as "Add an emulated calculator" adds
 * it. Nothing when a pane shows that calculator already (that pane is
 * made the active one) or is joining it. A left pane on another
 * calculator (a cable, a private emulated one) is replaced only after a
 * question; an empty pane or a folder without one.
 */
async function connectFromEmulator(model) {
  if (!SERIAL_MODELS.includes(model)) return null;
  const left = panes[0];
  const showing = panes.find((pane) => pane.place?.kind === "device" && pane.place.hub && pane.place.model === model);
  if (showing) {
    setActive(showing.id);
    return null;
  }
  if (connecting === model) return null;
  connecting = model;
  try {
    const r = (await keptRoms()).find((k) => k.model === model);
    if (!r) {
      say(`This browser keeps no ${EMULATED_NAMES[model]} ROM for the emulator.`, { error: true });
      return null;
    }
    setActive(left.id);
    const p = left.place;
    if (p?.kind === "device") {
      const go = await ask({ title: `Connect the emulator's ${EMULATED_NAMES[model]}?`, body: `The emulator asks for its ${EMULATED_NAMES[model]} here. ${p.title} is disconnected from this pane.`, action: "Connect" });
      if (!go) return null;
    }
    return await addKept(left, r);
  } finally {
    if (connecting === model) connecting = null;
  }
}

/** Say why the emulator's `model` is not connected here (`why`, `connectRefusal`'s code), once the page has started. */
function sayRefused(model, why) {
  const text = refusalText(model, why);
  if (text) started.then(() => say(text), () => {});
}

/** `connectFromEmulator` once the page has started; a failure is said, never thrown. */
function connectWhenStarted(model) {
  return started.then(() => connectFromEmulator(model)).catch((err) => {
    say(String(err?.message ?? err), { error: true });
    return null;
  });
}

async function openEmulatedDialog(pane) {
  target = pane;
  const roms = await keptRoms();
  ui.emulatedRoms.replaceChildren(...roms.map((r) => {
    const b = el("button", { type: "button", class: "rom-choice" }, iconEl("calculator"), el("span", {}, el("strong", { text: EMULATED_NAMES[r.model] }), el("span", { class: "muted", text: r.name })));
    b.addEventListener("click", async () => {
      ui.emulatedDialog.close();
      await addKept(target, r);
    });
    return b;
  }));
  ui.emulatedNone.hidden = roms.length > 0;
  ui.emulatedDialog.showModal();
}

ui.emulatedFile.addEventListener("click", () => ui.romInput.click());
ui.romInput.addEventListener("change", async () => {
  const f = ui.romInput.files?.[0];
  ui.romInput.value = "";
  if (!f) return;
  ui.emulatedDialog.close();
  // Not read at all: larger than any model's ROM.
  if (f.size > MAX_ROM_FILE) {
    say(`${f.name} is larger than any calculator's ROM (${bytesText(MAX_ROM_FILE)} at most).`, { error: true });
    return;
  }
  // Its model is known once it boots.
  await target.addEmulated(new Uint8Array(await f.arrayBuffer()), f.name, null);
});

// ------------------------------------------------------------ settings

function renderSettings() {
  const now = $("set-now");
  const conn = panes.map((p) => (p.place?.kind === "device" && p.place.device.connection ? `${p.place.heading}, ${settingsText(p.place.device.connection.settings)}` : null)).filter(Boolean);
  now.hidden = !conn.length;
  now.textContent = conn.length ? `Connected now: ${conn.join("; ")}.` : "";
  ui.setFind.checked = app.settings.find;
  ui.setBaud.value = String(app.settings.baud);
  ui.setParity.value = app.settings.parity;
  const theme = themeOf(store.get(THEME_KEY));
  const tabs = radioTabIndexes(THEMES, theme);
  ui.setTheme.replaceChildren(...THEMES.map((t, i) => el("button", { type: "button", role: "radio", "aria-checked": String(t === theme), tabindex: String(tabs[i]), "data-theme": t, text: t[0].toUpperCase() + t.slice(1) })));
}

function saveSettings() {
  app.settings = { find: ui.setFind.checked, baud: Number(ui.setBaud.value), parity: ui.setParity.value };
  store.set(TRANSFER_KEYS.link, JSON.stringify(app.settings));
}

function setTheme(t) {
  store.set(KEYS.theme, t);
  applyTheme(t);
  renderSettings();
  ui.setTheme.querySelector('[aria-checked="true"]')?.focus();
}

function openSettings() {
  renderSettings();
  ui.settingsDialog.showModal();
}

for (const input of [ui.setFind, ui.setBaud, ui.setParity]) input.addEventListener("change", saveSettings);
ui.setTheme.addEventListener("click", (e) => {
  const b = e.target.closest("[data-theme]");
  if (b) setTheme(b.dataset.theme);
});
ui.setTheme.addEventListener("keydown", (e) => {
  const next = radioStep(THEMES, themeOf(store.get(THEME_KEY)), e.key);
  if (next) {
    e.preventDefault();
    setTheme(next);
  }
});
$("open-settings").addEventListener("click", openSettings);

// ------------------------------------------------------------ start

async function start() {
  loadIcons();
  // Toasts at the window's bottom: at the top they would cover the places' names and status.
  placeToasts("bottom");
  for (const p of panes) p.render();
  renderSwitch();
  renderQueue();
  // The folder on the right: the one picked last time (its permission
  // asked on a click), else the page's own files where no folder can be
  // picked; else the right pane offers both.
  const right = panes[1];
  // The desktop app: the computer's files, at the home folder. Not
  // waited for: the page is started whatever the native side answers.
  if (isTauri()) right.useComputer();
  else if (!(await right.offerKept()) && !canPickFolder()) await right.useFiles();
  // The installed page: the site adds the manifest and a service worker
  // (web/site.sh); served from web/ there is neither.
  if (document.querySelector('link[rel="manifest"]') && "serviceWorker" in navigator && window.isSecureContext && !window.__TAURI__) {
    navigator.serviceWorker.register("sw.js").catch(() => {});
  }
}

const started = start();

// The emulator's links find this tab and ask it to connect a calculator
// (`?connect=` when they open it): shared/site-links.js.
if (!inDesktopApp()) {
  nameTab("transfer");
  const asked = takeParam("connect");
  if (asked) connectWhenStarted(asked);
  // The emulator's calculator that cannot be connected here, and why (shared/site-links.js `connectRefusal`).
  const refused = takeParam("refused");
  const why = takeParam("why");
  if (refused) sayRefused(refused, why);
  onSiteMessage((m) => {
    if (typeof m.connect === "string") connectWhenStarted(m.connect);
    else if (typeof m.refused === "string") sayRefused(m.refused, m.why);
  });
  linkTab($("to-emulator"), "emulator");
}

// For the tests (web/test/tx-page.test.mjs, tx-e2e.test.mjs).
window.tx = {
  app,
  panes,
  ui,
  queue,
  started,
  setActive,
  helpers,
  links: { FakeLink, RecordingLink },
  serve,
  /** The emulator's request to connect `model` (`?connect=`, its tab's message). */
  connectFromEmulator: connectWhenStarted,
  KermitDevice,
  Folder,
  MemoryDirectory,
  fallbackFolder,
  fileItem,
  flatten,
  /** A folder in memory titled `title` in pane `i`, shown as a picked folder (the tests). */
  memoryFolder: (i, title = "hp-files") => panes[i].setFolder(new Folder(new MemoryDirectory(title), { kind: "picked", title })),
  /** Nothing running or waiting in pane `i` (0 by default), its list read. */
  idle: (i = 0) => {
    const p = panes[i];
    const place = p.place;
    if (!place || p.step) return false;
    if (place.kind === "device" && (!place.device.connection || place.device.running || place.device.queued)) return false;
    return !queue.uses(place.id) && Boolean(p.ui.list.listing);
  },
  /** The names in pane `i`'s list. */
  names: (i = 0) => (panes[i].ui.list.listing?.entries ?? []).map((e) => e.name),
  /** Select `name` in pane `i`'s list. */
  select: (name, i = 0) => panes[i].ui.list.select(name),
  /** An entry of pane `i`'s list. */
  entry: (name, i = 0) => panes[i].ui.list.listing?.entries.find((e) => e.name === name) ?? null,
};
