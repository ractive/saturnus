// saturnus tx, the transfer page (/saturnus/transfer/): the composition
// root. One pane in this iteration: a calculator (on a serial cable
// through Web Serial, or emulated in the page), its directory tree and
// list (the shared <device-tree> and <device-list>), the preview, and
// get and put through downloads and the file input or a drop
// (kb/docs/saturnus-tx.md, kb iteration 37). The protocol runs in a
// dedicated Worker (protocol-worker.js); this page holds the link and the
// dialogs. `window.tx` is for the tests.

import init, { describe_file, file_kind, name_problem, text_of, var_name } from "./pkg/saturnus_tx_web.js";
import { el } from "../shared/dom.js";
import { HOME, parentPath, pathTitle, samePath } from "../shared/device.js";
import { radioStep, radioTabIndexes } from "../shared/radiogroup.js";
import { KEYS, TRANSFER_KEYS } from "../shared/storage-names.js";
import { THEMES, THEME_KEY, applyTheme, themeOf } from "../shared/theme.js";
import { confirmAction } from "../shared/components/confirm.js";
import { iconEl, loadIcons } from "../shared/components/icons.js";
import { openMenu } from "../shared/components/menu.js";
import { placeToasts, toast } from "../shared/components/toast.js";
import "../shared/components/device-tree.js";
import "../shared/components/device-list.js";
import { EmulatedCalculator } from "./emulator.js";
import { keptRomBytes, keptRoms } from "./kept-roms.js";
import { KermitDevice } from "./kermit-device.js";
import { EmulatorLink, FakeLink, RecordingLink, WebSerialLink, hasWebSerial } from "./links.js";
import { ProtocolClient } from "./protocol.js";
import { renderPreview, textObject } from "./preview.js";
import {
  EMULATED_NAMES, MODELS, MODEL_NAMES, SERVER_SHOWN, START_SERVER, disconnectNote, failureText, linkSettings, placeTitle,
  previewPlan, progressText, putPlan, replaceQuestion, searchText, settingsText, stringQuestion,
} from "./flows.js";

const $ = (id) => document.getElementById(id);
const ui = {
  title: $("pane-title"), status: $("pane-status"), progress: $("pane-progress"), empty: $("pane-empty"), body: $("pane-body"),
  tree: $("tree"), list: $("list"), crumbs: $("crumbs"), preview: $("preview"),
  putFile: $("put-file"), refresh: $("refresh"), more: $("pane-more"), cancelQueued: $("cancel-queued"),
  addSerial: $("add-serial"), addEmulated: $("add-emulated"), noSerial: $("no-serial"), lastNote: $("last-note"),
  fileInput: $("file-input"), romInput: $("rom-input"),
  connectDialog: $("connect-dialog"), connectModels: $("connect-models"), connectSteps: $("connect-steps"),
  connectGo: $("connect-go"), connectOther: $("connect-other"), connectKnown: $("connect-known"),
  emulatedDialog: $("emulated-dialog"), emulatedRoms: $("emulated-roms"), emulatedNone: $("emulated-none"), emulatedFile: $("emulated-file"),
  settingsDialog: $("settings-dialog"), setFind: $("set-find"), setBaud: $("set-baud"), setParity: $("set-parity"), setTheme: $("set-theme"),
  nameDialog: $("name-dialog"), nameWhy: $("name-why"), nameInput: $("name-input"), nameProblem: $("name-problem"),
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

let settings = linkSettings(store.get(TRANSFER_KEYS.link));

// ------------------------------------------------------------ the wasm helpers

let wasm = null;
/** The bindings in the page (files, names, the preview); the protocol's own copy runs in its Worker. */
const helpers = () => (wasm ??= init().then(() => ({ describe_file, file_kind, name_problem, text_of, var_name })));

// ------------------------------------------------------------ state

const state = {
  /** The KermitDevice shown, or null. */
  device: null,
  /** `{model, emulated, linkLabel}` of the connected place. */
  place: null,
  /** The emulated calculator behind the device, if any. */
  emulator: null,
  path: [...HOME],
  /** The selected entry and its preview. */
  selected: null,
  previews: new Map(),
  /** A step in progress that is not the device's (starting the emulator). */
  step: null,
  /** Options for the next device (the tests: a fixed seed, a short search). */
  deviceOptions: {},
  /** Wraps the next link (the tests: a RecordingLink). */
  wrapLink: null,
  lastDownload: null,
  downloads: [],
};

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function say(text, { error = false } = {}) {
  toast(text, { error });
}

/** The status line (a live region): one sentence a step, never per packet. */
function setStatus(text) {
  if (ui.status.textContent !== text) ui.status.textContent = text;
  setProgress("");
}

/** How far the running transfer is: beside the status, outside the live region (it changes with every packet). */
function setProgress(text) {
  ui.progress.textContent = text;
}

/** Show the pane for the state: the start card, or the device. */
function render() {
  const d = state.device;
  const on = Boolean(d?.connection);
  ui.empty.hidden = on;
  ui.body.hidden = !on;
  ui.putFile.hidden = !on;
  ui.refresh.hidden = !on;
  ui.more.hidden = !d;
  // One calculator at a time: no second connect while one connects or runs.
  const busy = Boolean(d || state.step);
  for (const b of [ui.addSerial, ui.addEmulated, ui.connectGo, ui.connectOther, ui.emulatedFile, ...ui.emulatedRoms.querySelectorAll("button")]) b.disabled = busy;
  ui.title.textContent = d || state.step ? placeTitle({ ...state.place, model: d?.connection?.model ?? state.place?.model }) : "No calculator";
  if (!d && !state.step) setStatus("");
  renderCrumbs();
}

function renderCrumbs() {
  const parts = [];
  state.path.forEach((name, i) => {
    if (i) parts.push(el("span", { class: "crumb-sep", "aria-hidden": "true", text: "›" }));
    const here = i === state.path.length - 1;
    const b = el("button", { type: "button", class: "link crumb", "aria-current": here ? "location" : null, text: name });
    if (!here) b.addEventListener("click", () => navigate(state.path.slice(0, i + 1)));
    parts.push(b);
  });
  ui.crumbs.replaceChildren(...parts);
}

function navigate(path) {
  state.path = [...path];
  ui.tree.current = path;
  ui.list.path = path;
  select(null);
  renderCrumbs();
}

// ------------------------------------------------------------ connect

/** Wire a device's events to the page. */
function watchDevice(device) {
  device.addEventListener("busy", (e) => {
    const { label, queued } = e.detail;
    if (state.device !== device) return;
    if (label) setStatus(queued ? `${label} (${queued} more waiting)` : label);
    else setStatus(device.connection ? `Connected at ${settingsText(device.connection.settings)}.` : "");
    ui.cancelQueued.hidden = !queued;
  });
  device.addEventListener("progress", (e) => {
    if (state.device !== device) return;
    const { label, done, total } = e.detail;
    if (ui.status.textContent !== label) ui.status.textContent = label;
    setProgress(progressText({ label: "", done, total }).trim());
  });
  device.addEventListener("search", (e) => {
    if (state.device === device) setStatus(searchText(e.detail));
  });
  device.addEventListener("lost", (e) => {
    if (state.device !== device) return;
    const why = e.detail;
    drop(device);
    say(`The calculator is gone: ${why}`, { error: true });
  });
}

/** Forget `device` (its link closed) and show the start card. */
function drop(device, note = "") {
  if (state.device === device) {
    state.device = null;
    ui.tree.device = null;
    ui.list.device = null;
    select(null);
  }
  device?.close().catch(() => {});
  state.emulator?.close();
  state.emulator = null;
  ui.lastNote.textContent = note;
  ui.lastNote.hidden = !note;
  render();
}

/**
 * Connect over `link`: the device, the search, the model; then HOME.
 * `emulated` for the emulated calculator. Resolves to the device, or
 * null when it failed (said in a toast).
 */
export async function connectLink(link, { emulated = false, model = null } = {}) {
  // A device still there (a connect not finished) lets go of its link first.
  const old = state.device;
  if (old) {
    state.device = null;
    ui.tree.device = null;
    ui.list.device = null;
    await old.close().catch(() => {});
  }
  link = state.wrapLink?.(link) ?? link;
  const client = new ProtocolClient(link, state.deviceOptions);
  const device = new KermitDevice(client, { emulated });
  watchDevice(device);
  state.device = device;
  state.place = { model, emulated, linkLabel: link.label };
  ui.lastNote.hidden = true;
  render();
  setStatus("Looking for the calculator…");
  try {
    const opts = emulated ? { find: true } : settings;
    const conn = await device.connect(opts);
    // A newer connect took over meanwhile: this device is closed, its tail is not run.
    if (state.device !== device) return null;
    state.place.model = conn.model;
  } catch (err) {
    if (state.device === device) {
      drop(device);
      say(failureText(err), { error: true });
    }
    return null;
  }
  state.path = [...HOME];
  ui.tree.device = device;
  ui.list.device = device;
  ui.tree.current = HOME;
  render();
  setStatus(`Connected at ${settingsText(device.connection.settings)}.`);
  return device;
}

/** End server mode and let go of the link. */
export async function disconnect() {
  const device = state.device;
  if (!device) return;
  const model = device.connection?.model ?? state.place?.model;
  const emulated = state.place?.emulated;
  if (device.connection) {
    try {
      await device.disconnect();
    } catch (err) {
      say(`The calculator did not answer the end of server mode: ${failureText(err)}`, { error: true });
    }
  }
  drop(device, emulated ? "The emulated calculator was turned off." : disconnectNote(model));
}

// The connect dialog: the model, what to do on it, the port.

let connectModel = MODELS.includes(store.get(TRANSFER_KEYS.model)) ? store.get(TRANSFER_KEYS.model) : "48gx";

function renderConnectDialog() {
  const tabs = radioTabIndexes(MODELS, connectModel);
  ui.connectModels.replaceChildren(...MODELS.map((m, i) => {
    const b = el("button", { type: "button", role: "radio", "aria-checked": String(m === connectModel), tabindex: String(tabs[i]), "data-model": m, text: MODEL_NAMES[m].replace(/^HP /, "") });
    return b;
  }));
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

/** A port the page was given before (Chrome remembers it), or null. */
async function knownPort() {
  try {
    const ports = await navigator.serial.getPorts();
    return ports[0] ?? null;
  } catch {
    return null;
  }
}

async function openConnectDialog() {
  if (!hasWebSerial()) {
    say("This browser cannot reach a serial port: open the page in Chrome or Edge on a computer.", { error: true });
    return;
  }
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

async function connectPort(choosing) {
  if (state.device || state.step) return;
  let port;
  try {
    port = await choosing;
  } catch {
    return; // the chooser was cancelled
  }
  ui.connectDialog.close();
  await connectLink(new WebSerialLink(port), { model: connectModel });
}

// The emulated calculator.

async function openEmulatedDialog() {
  const roms = await keptRoms();
  ui.emulatedRoms.replaceChildren(...roms.map((r) => {
    const b = el("button", { type: "button", class: "rom-choice" }, iconEl("calculator"), el("span", {}, el("strong", { text: EMULATED_NAMES[r.model] }), el("span", { class: "muted", text: r.name })));
    b.addEventListener("click", async () => {
      if (state.device || state.step) return;
      ui.emulatedDialog.close();
      const bytes = await keptRomBytes(r.sha256);
      if (!bytes) {
        say("That ROM could not be read from this browser's storage.", { error: true });
        return;
      }
      await addEmulated(bytes, r.name, r.model);
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
  if (!f || state.device || state.step) return;
  ui.emulatedDialog.close();
  await addEmulated(new Uint8Array(await f.arrayBuffer()), f.name, "48gx");
});

/**
 * Boot `rom` in an emulated calculator, start its server and connect to
 * it. `speed` the emulated calculator's ("1" as a real one; the tests
 * may pass "max").
 */
export async function addEmulated(rom, name, model = "48gx", { speed = "1" } = {}) {
  if (state.device) await disconnect();
  // An emulated calculator still starting is stopped.
  state.emulator?.close();
  state.emulator = null;
  const emu = new EmulatedCalculator();
  state.emulator = emu;
  state.step = "emulator";
  state.place = { model, emulated: true, linkLabel: "emulated serial port" };
  ui.lastNote.hidden = true;
  render();
  try {
    setStatus("Starting the emulated calculator…");
    const booted = await emu.boot(rom, name, model);
    state.place.model = booted;
    render();
    if (speed !== "1") await emu.setSpeed(speed);
    setStatus("Starting its server…");
    await emu.startServer();
  } catch (err) {
    emu.close();
    if (state.emulator !== emu) return null; // replaced meanwhile
    state.step = null;
    state.emulator = null;
    render();
    say(String(err?.message ?? err), { error: true });
    return null;
  }
  state.step = null;
  const device = await connectLink(new EmulatorLink(emu), { emulated: true, model: state.place.model });
  if (device) state.emulator = emu;
  return device;
}

// ------------------------------------------------------------ get

/** Save `bytes` as a download named `name`. */
function download(name, bytes) {
  state.lastDownload = { name, bytes };
  state.downloads.push(state.lastDownload);
  if (state.downloads.length > 20) state.downloads.shift();
  const url = URL.createObjectURL(new Blob([bytes], { type: "application/octet-stream" }));
  const a = el("a", { href: url, download: name, hidden: true });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

/** Failures that are no news: the user stopped it, or the link closed. */
const quiet = (err) => ["cancelled", "aborted", "closed"].includes(err?.kind);

const previewKey = (path, entry) => `${path.join("/")}/${entry.name}#${entry.checksum}`;

/** Get variable `entry` of the shown directory as a download (`text`: the calculator's text form). */
export async function getVariable(entry, { text = false } = {}) {
  const device = state.device;
  if (!device) return;
  const key = previewKey(state.path, entry);
  const kept = state.previews.get(key);
  try {
    let file;
    if (!text && kept?.binary) file = kept.binary;
    else file = await device.read(state.path, entry.name, { text, size: entry.size });
    download(file.fileName, file.data);
    say(`${file.fileName} saved to your downloads.`);
  } catch (err) {
    if (state.device === device && !quiet(err)) say(`${entry.name} was not got: ${failureText(err)}`, { error: true });
  }
}

// ------------------------------------------------------------ put

/** Ask for a variable name; resolves to it, or null. */
function askName(why, suggested) {
  return new Promise((resolve) => {
    ui.nameWhy.textContent = why;
    ui.nameInput.value = suggested;
    ui.nameProblem.textContent = "";
    const check = async () => {
      const h = await helpers();
      const name = ui.nameInput.value.trim();
      const problem = name ? h.name_problem(name) : "A name is needed.";
      ui.nameProblem.textContent = problem ? "The calculator does not take this as a name." : "";
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
      const name = await check();
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

/**
 * Put `files` (File objects) into directory `path` of the device, one
 * after the other: binary and `%%HP:` text files as they are, anything
 * else as a string after asking; a name in use after "Replace X?".
 */
export async function putFiles(files, path = state.path) {
  const device = state.device;
  if (!device?.connection) return;
  const h = await helpers();
  for (const file of files) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const plan = putPlan(h.file_kind(bytes));
    let name = null;
    try {
      name = h.var_name(file.name);
    } catch {
      name = null;
    }
    if (!name || h.name_problem(name)) {
      name = await askName(`The calculator does not take “${name ?? file.name}” as a name. What should ${file.name} be called there?`, (name ?? "").replace(/[^A-Za-z0-9]/g, "").replace(/^[0-9]+/, "") || "FILE");
      if (!name) continue;
    }
    if (plan.ask === "string" && !(await confirmAction(stringQuestion(file.name, name)))) continue;
    try {
      let stored;
      try {
        stored = await device.write(path, name, bytes, { asString: plan.asString });
      } catch (err) {
        if (err?.kind !== "nameClash") throw err;
        if (!(await confirmAction(replaceQuestion(name)))) continue;
        stored = await device.write(path, name, bytes, { asString: plan.asString, replace: true });
      }
      say(`${file.name} is now ${stored} on the calculator.`);
    } catch (err) {
      // A device that went away has said so already ("The calculator is gone").
      if (state.device !== device) return;
      if (!quiet(err)) say(`${file.name} was not put: ${failureText(err)}`, { error: true });
    }
  }
}

// ------------------------------------------------------------ preview

function previewState(entry) {
  if (!entry) return null;
  const kept = state.previews.get(previewKey(state.path, entry));
  if (kept) return kept;
  const plan = previewPlan(entry);
  if (entry.directory || !plan.fetch) return { status: "none" };
  return { status: plan.auto ? "loading" : "ask" };
}

async function fetchPreview(entry) {
  const device = state.device;
  const path = state.path;
  const key = previewKey(path, entry);
  const plan = previewPlan(entry);
  state.previews.set(key, { status: "loading" });
  showPreview();
  let result;
  try {
    const h = await helpers();
    // One preview at a time: a selection that moved on drops the one still waiting.
    device.cancelQueued("preview");
    const file = await device.read(path, entry.name, { text: plan.fetch === "text", size: entry.size, tag: "preview" });
    if (plan.fetch === "text") result = { status: "shown", object: textObject(entry.kind, h.text_of(file.data)) };
    else result = { status: "shown", object: h.describe_file(file.data), binary: file };
  } catch (err) {
    if (quiet(err) || state.device !== device) {
      // Not an answer about the object: asked again when selected again.
      state.previews.delete(key);
      if (state.selected?.name === entry.name && samePath(state.path, path)) showPreview();
      return;
    }
    result = { status: "error", error: failureText(err) };
  }
  state.previews.set(key, result);
  if (state.selected?.name === entry.name && samePath(state.path, path)) showPreview();
}

function showPreview() {
  const entry = state.selected;
  if (!entry) {
    renderPreview(ui.preview, null, null);
    return;
  }
  const st = previewState(entry);
  const getButton = el("button", { type: "button", class: "icon", title: `Get ${entry.name} as a file`, "aria-label": `Get ${entry.name}` }, iconEl("save"));
  getButton.addEventListener("click", () => getVariable(entry));
  const closeButton = el("button", { type: "button", class: "icon preview-close", title: "Close the preview", "aria-label": "Close the preview" }, iconEl("close"));
  closeButton.addEventListener("click", () => {
    ui.list.select(null);
  });
  renderPreview(ui.preview, entry, st, { onShow: () => fetchPreview(entry), actions: entry.directory ? [closeButton] : [getButton, closeButton] });
}

function select(entry) {
  state.selected = entry;
  showPreview();
  if (!entry) return;
  const st = previewState(entry);
  const key = previewKey(state.path, entry);
  if (st.status === "loading" && !state.previews.has(key)) fetchPreview(entry);
}

// ------------------------------------------------------------ the list

ui.list.emptyText = "";
ui.list.actions = (entry) => {
  if (entry.directory) return [{ text: "Open", icon: "folder-open", run: () => navigate([...state.path, entry.name]) }];
  return [
    { text: "Get", icon: "save", run: () => getVariable(entry) },
    { text: "Get as text", icon: "file", run: () => getVariable(entry, { text: true }) },
  ];
};
ui.list.addEventListener("select", (e) => select(e.detail.entry));
ui.list.addEventListener("open", (e) => {
  const entry = e.detail.entry;
  navigate(entry.up ? parentPath(state.path) : [...state.path, entry.name]);
});
ui.list.addEventListener("dropfiles", (e) => putFiles(e.detail.files, e.detail.path));
ui.tree.addEventListener("navigate", (e) => navigate(e.detail.path));

ui.putFile.addEventListener("click", () => ui.fileInput.click());
ui.fileInput.addEventListener("change", () => {
  const files = [...(ui.fileInput.files ?? [])];
  ui.fileInput.value = "";
  if (files.length) putFiles(files);
});
ui.refresh.addEventListener("click", () => {
  state.previews.clear();
  state.device?.refresh(null);
});
ui.cancelQueued.addEventListener("click", () => state.device?.cancelQueued());
ui.more.addEventListener("click", () => {
  const d = state.device;
  openMenu({
    anchor: ui.more,
    label: "Connection",
    returnFocus: () => ui.more,
    items: [
      { text: "Connection settings…", icon: "settings", run: () => openSettings() },
      { text: "Read everything again", icon: "refresh", disabled: !d?.connection, run: () => ui.refresh.click() },
      "-",
      { text: state.place?.emulated ? "Turn the emulated calculator off" : "Disconnect", icon: "disconnect", run: () => disconnect() },
    ],
  });
});

// A file dropped anywhere on the pane goes to the shown directory.
document.addEventListener("dragover", (e) => {
  if (state.device?.connection && [...(e.dataTransfer?.types ?? [])].includes("Files")) e.preventDefault();
});
document.addEventListener("drop", (e) => {
  if (!state.device?.connection || e.defaultPrevented) return;
  const files = [...(e.dataTransfer?.files ?? [])];
  if (!files.length) return;
  e.preventDefault();
  putFiles(files);
});

// ------------------------------------------------------------ settings

function renderSettings() {
  const conn = state.device?.connection;
  const now = $("set-now");
  now.hidden = !conn;
  now.textContent = conn ? `Connected now: ${placeTitle({ ...state.place, model: conn.model })}, ${settingsText(conn.settings)}.` : "";
  ui.setFind.checked = settings.find;
  ui.setBaud.value = String(settings.baud);
  ui.setParity.value = settings.parity;
  const theme = themeOf(store.get(THEME_KEY));
  const tabs = radioTabIndexes(THEMES, theme);
  ui.setTheme.replaceChildren(...THEMES.map((t, i) => el("button", { type: "button", role: "radio", "aria-checked": String(t === theme), tabindex: String(tabs[i]), "data-theme": t, text: t[0].toUpperCase() + t.slice(1) })));
}

function saveSettings() {
  settings = { find: ui.setFind.checked, baud: Number(ui.setBaud.value), parity: ui.setParity.value };
  store.set(TRANSFER_KEYS.link, JSON.stringify(settings));
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

ui.addSerial.addEventListener("click", () => {
  if (!state.device && !state.step) openConnectDialog();
});
ui.addEmulated.addEventListener("click", () => {
  if (!state.device && !state.step) openEmulatedDialog();
});

function start() {
  loadIcons();
  // Toasts at the window's bottom: at the top they would cover the place's name and status.
  placeToasts("bottom");
  const serial = hasWebSerial();
  ui.noSerial.hidden = serial;
  ui.addSerial.hidden = !serial;
  if (!serial) ui.addEmulated.classList.add("primary");
  render();
  // The installed page: the site adds the manifest and a service worker
  // (web/site.sh); served from web/ there is neither.
  if (document.querySelector('link[rel="manifest"]') && "serviceWorker" in navigator && window.isSecureContext && !window.__TAURI__) {
    navigator.serviceWorker.register("sw.js").catch(() => {});
  }
}

start();

// For the tests (web/test/tx-page.test.mjs, tx-e2e.test.mjs).
window.tx = {
  state,
  ui,
  connectLink,
  disconnect,
  addEmulated,
  putFiles,
  getVariable,
  navigate,
  select: (name) => ui.list.select(name),
  /** Nothing running or waiting, the list read. */
  idle: () => Boolean(state.device?.connection) && !state.device.running && !state.device.queued && Boolean(ui.list.listing) && !state.step,
  /** The names in the list. */
  names: () => (ui.list.listing?.entries ?? []).map((e) => e.name),
  helpers,
  sleep,
  links: { FakeLink, RecordingLink },
  KermitDevice,
  render,
};
