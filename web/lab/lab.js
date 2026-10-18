// The lab: a throwaway prototype for kb/research/modern-ui-on-the-rom.md.
// A terminal-and-stack UI over the real ROM: one command line (the
// palette and the keyboard in one), the whole stack rendered from the
// objects the host decodes out of RAM (web/protocol.md, "Typed objects"),
// graphs plotted by the ROM at as many points as the canvas needs, and
// the ROM's own screen as a fallback for forms, menus and apps. The page
// reuses the hub Worker (one calculator per model, shared with the app
// if it is open in the same document; here it is alone), the shared
// tokens, the object layout helpers and the command reference's index.
// Nothing in the app changes: this page is its own document.

import { HubClient } from "../shared/calc-hub.js";
import { graphicDrawable, graphicRgba } from "../shared/graphic.js";
import { indentRpl, summary, textOf, typeTitle } from "../shared/objects.js";
import { keptRomBytes, keptRoms } from "../tx/kept-roms.js";
import { buildIndex, search } from "../reference.js";

const $ = (id) => document.getElementById(id);
const ui = {
  model: $("lab-model"), status: $("lab-status"), screen: $("lab-screen"), keys: $("lab-keys"), rom: $("lab-rom"), rom2: $("lab-rom-2"),
  file: $("lab-file"), stack: $("lab-stack"), empty: $("lab-empty"), kept: $("lab-kept"), levels: $("lab-levels"), results: $("lab-results"),
  lcd: $("lab-lcd"), ann: $("lab-ann"), lcdClose: $("lab-lcd-close"), canvas: $("lab-canvas"), softkeys: $("lab-softkeys"), keygrid: $("lab-keygrid"),
  log: $("lab-log"), suggest: $("lab-suggest"), form: $("lab-form"), input: $("lab-input"), line: $("lab-line"),
};
const TITLES = globalThis.SATURNUS_MODEL_TITLES;

/** Make an element: `el("span", {class, text, ...attrs}, ...children)`. */
function el(tag, attrs = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === null || v === undefined) continue;
    if (k === "text") e.textContent = v;
    else if (k === "class") e.className = v;
    else e.setAttribute(k, v);
  }
  for (const c of children) if (c !== null && c !== undefined) e.append(c);
  return e;
}

// ------------------------------------------------------------------ state

const state = {
  calc: null,
  model: null,
  booted: false,
  busy: false,
  layout: null,
  index: null,
  stack: null,
  frame: null,
  lcdWanted: false,
  lcdAuto: false,
  history: [],
  historyAt: -1,
  suggestions: [],
  selected: -1,
  speed: "1",
  loop: null,
};

function status(text, kind = "") {
  ui.status.textContent = text ?? "";
  ui.status.className = `lab-status ${kind}`;
}

// ------------------------------------------------------------- connection

/** The hub's connection, made once; its events drive the page. */
function connection() {
  if (state.calc) return state.calc;
  const calc = new HubClient({ who: "tx" });
  calc.addEventListener("frame", (e) => onFrame(e.detail));
  calc.addEventListener("status", (e) => onStatus(e.detail));
  calc.addEventListener("memoryChanged", () => scheduleRead());
  calc.addEventListener("error", (e) => status(e.detail.message, "error"));
  calc.addEventListener("gone", (e) => {
    state.booted = false;
    status(String(e.detail ?? "The calculator stopped."), "error");
  });
  state.calc = calc;
  return calc;
}

/** The model a ROM file most likely is, from its size (web/protocol.md, ROM identification is the host's; this is only the preference it takes). */
function guessModel(bytes, name = "") {
  const n = name.toLowerCase();
  if (n.includes("49")) return "49g";
  if (n.includes("sx")) return "48sx";
  if (bytes.length >= 1024 * 1024) return "49g";
  if (bytes.length <= 300 * 1024) return "48sx";
  return "48gx";
}

/** Boot (or join) the calculator of `rom` (bytes) named `name`, as `model` or the one it fits. */
async function boot(rom, name, model = guessModel(rom, name)) {
  const calc = connection();
  status(`Starting the ${TITLES[model] ?? model}…`, "busy");
  await calc.request("hello");
  const booted = await calc.request("attach", { model, rom, romName: name });
  state.model = booted?.model ?? model;
  state.booted = true;
  ui.model.textContent = `${TITLES[state.model] ?? state.model} · ${name}`;
  const watch = await calc.request("watchMemory", { on: true });
  if (watch?.supported === false) throw new Error(watch.reason ?? "The calculator's memory cannot be read.");
  state.layout = await calc.request("layout", { model: state.model }).catch(() => null);
  renderKeys();
  loadIndex(state.model);
  status(booted?.attached ? "Joined the running calculator." : booted?.restored ? "Started as it was left." : "Started.");
  ui.empty.hidden = true;
  scheduleRead();
  ui.input.focus();
  // A cold boot: the ROM is still busy for a moment after its first
  // questions were answered, and a send typed into that fails (the
  // keyboard cannot be brought to alpha). Wait until the CPU sleeps.
  if (!booted?.attached && !booted?.restored) {
    await readable(calc, 15000);
    await idle(6000);
    // Not enough on the 49G: a send within about 3 s of the boot still
    // fails ("could not bring the keyboard to AlphaLocked"). Open question
    // for the host's typing engine; the lab waits it out.
    await new Promise((r) => setTimeout(r, 3000));
  }
  // The 49G boots in algebraic mode; this is an RPN terminal.
  if (state.model === "49g") {
    const flags = await calc.request("flags").catch(() => null);
    if (flags?.set?.includes(-95)) await runLab(":rpn");
  }
  return state.model;
}

/** Resolves once the user memory can be read (the ROM's first questions answered), or after `ms`. */
async function readable(calc, ms) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (await calc.request("stackTop").then(() => true, () => false)) return;
    await new Promise((r) => setTimeout(r, 200));
  }
}

/** Resolves once the machine sleeps (`status.loop === "sleep"`), or after `ms`. */
function idle(ms) {
  return new Promise((resolve) => {
    if (state.loop === "sleep") { resolve(); return; }
    const timer = setTimeout(done, ms);
    function done() {
      clearTimeout(timer);
      document.removeEventListener("lab-status", check);
      resolve();
    }
    function check() { if (state.loop === "sleep") done(); }
    document.addEventListener("lab-status", check);
  });
}

/** The command reference's index for `model` (web/commands.json). */
async function loadIndex(model) {
  try {
    const res = await fetch(new URL("../commands.json", import.meta.url));
    const data = await res.json();
    state.index = buildIndex(data, model);
  } catch (err) {
    console.warn("lab: no command reference", err);
  }
}

// -------------------------------------------------------------- the stack

let readTimer = 0;
function scheduleRead() {
  clearTimeout(readTimer);
  readTimer = setTimeout(readStack, 60);
}

/** Read the stack and the command line; draw what changed. */
async function readStack() {
  if (!state.booted || !state.calc) return;
  try {
    const [stack, line] = await Promise.all([state.calc.request("stack"), state.calc.request("commandLine")]);
    const was = state.stack ? JSON.stringify(state.stack) : null;
    state.stack = stack;
    renderLine(line);
    if (was !== JSON.stringify(stack)) renderStack(stack, was);
  } catch (err) {
    // Not readable yet (the ROM's first question), or typing in progress: try again soon.
    const m = String(err?.message ?? err);
    if (/typing|transfer/.test(m)) setTimeout(scheduleRead, 300);
    else if (!/Recover|Memory|before/i.test(m)) status(m, "error");
  }
}

/** The ROM's own command line, when one is open (a form, a program entered with the keys): shown under the input. */
function renderLine(line) {
  const open = Boolean(line?.active);
  ui.line.hidden = !open;
  ui.line.textContent = open ? `The calculator's line: ${line.text}` : "";
  // Something other than the stack is going on: the screen shows it.
  if (open && !state.lcdWanted) showLcd(true, true);
  if (!open && state.lcdAuto) showLcd(false);
}

/** Draw the whole stack, level 1 at the bottom; levels new since `was` light up. */
function renderStack(levels, was) {
  const old = was ? JSON.parse(was) : [];
  const items = [];
  for (let n = levels.length; n >= 1; n--) {
    const obj = levels[n - 1];
    // Fresh when the same level held something else (or nothing) before.
    const fresh = was !== null && JSON.stringify(old[n - 1] ?? null) !== JSON.stringify(obj);
    items.push(el("li", { class: `lab-level${fresh ? " fresh" : ""}`, "data-level": n },
      el("span", { class: "n", text: `${n}` }),
      el("div", { class: "obj" }, renderObject(obj, n)),
      el("span", { class: "type", text: typeTitle(obj) })));
  }
  ui.levels.replaceChildren(...items);
  ui.stack.scrollTop = ui.stack.scrollHeight;
}

/** A number's text split into mantissa and exponent, for a lighter exponent. */
function numberNode(text) {
  const m = /^(.*?)(E[-+]?\d+)$/.exec(text);
  if (!m) return el("span", { class: "num", text });
  return el("span", { class: "num" }, m[1], el("span", { class: "unit", text: m[2] }));
}

/** An algebraic's source laid out a little: `^2` as a superscript, `*` as a dot. */
function algebraicNode(source) {
  const s = String(source).replace(/^'|'$/g, "");
  const frag = document.createDocumentFragment();
  const re = /\^(\(?[A-Za-z0-9.]+\)?)|\*/g;
  let at = 0;
  for (let m = re.exec(s); m; m = re.exec(s)) {
    frag.append(s.slice(at, m.index));
    if (m[0] === "*") frag.append("·");
    else frag.append(el("sup", { text: m[1] }));
    at = m.index + m[0].length;
  }
  frag.append(s.slice(at));
  return frag;
}

/** Whether an algebraic's source looks like a function of X. */
const ofX = (source) => /\bX\b/.test(String(source ?? "").replace(/'/g, ""));

/** The node for one object. */
function renderObject(obj, level) {
  const text = textOf(obj);
  switch (obj?.type) {
    case "real":
    case "integer":
      return numberNode(text ?? String(obj.value));
    case "complex":
      return el("span", { class: "num", text: text ?? `(${obj.re}, ${obj.im})` });
    case "binary":
      return el("span", { class: "num", text: text ?? `# ${obj.value}` });
    case "string":
      return el("span", { class: "str", text: text ?? `"${obj.value}"` });
    case "name":
    case "local_name":
      return el("span", { class: "name", text: text ?? `'${obj.value}'` });
    case "character":
      return el("span", { class: "str", text: text ?? obj.value });
    case "unit": {
      const m = /^(.*?)_(.*)$/.exec(text ?? "");
      if (!m) return el("span", { text: text ?? `${obj.value}_${obj.unit ?? "?"}` });
      return el("span", {}, numberNode(m[1]), el("span", { class: "unit", text: `_${m[2]}` }));
    }
    case "tagged":
      return el("span", {}, el("span", { class: "tag", text: `${obj.tag}:` }), renderObject(obj.object, level));
    case "list": {
      const items = obj.items ?? [];
      const shown = items.slice(0, 60);
      const box = el("div", { class: "lab-list" }, el("span", { class: "delim", text: "{" }));
      for (const it of shown) box.append(el("span", { class: "item", text: summary(it, 80).text }));
      if (items.length > shown.length) box.append(el("span", { class: "more", text: `… ${items.length - shown.length} more` }));
      box.append(el("span", { class: "delim", text: "}" }));
      return box;
    }
    case "array": {
      const dims = obj.dims ?? [];
      const rows = dims.length === 1 ? [obj.items ?? []] : (obj.items ?? []);
      const table = el("table", { class: "lab-matrix" });
      for (const row of rows.slice(0, 24)) {
        const tr = el("tr");
        for (const c of (Array.isArray(row) ? row : [row]).slice(0, 12)) tr.append(el("td", { text: Array.isArray(c) ? `[${c.map((x) => textOf(x) ?? "?").join(" ")}]` : (textOf(c) ?? "?") }));
        table.append(tr);
      }
      return table;
    }
    case "program": {
      if (obj.source == null) return el("span", { class: "partial", text: "« a program the ROM's tables do not name »" });
      const lines = indentRpl(obj.source);
      const pre = el("pre", { class: "lab-prog" });
      for (const l of lines.slice(0, 40)) {
        pre.append("  ".repeat(l.depth));
        for (const w of l.text.split(" ")) {
          const cls = /^[«»{}[\]()]$/.test(w) ? "delim" : /^(IF|THEN|ELSE|END|FOR|NEXT|STEP|START|DO|UNTIL|WHILE|REPEAT|CASE|IFERR|→)$/.test(w) ? "kw" : state.index?.byName?.has(w) ? "cmd" : /^"/.test(w) ? "str" : null;
          pre.append(cls ? el("span", { class: cls, text: w }) : w, " ");
        }
        pre.append("\n");
      }
      if (lines.length > 40) pre.append(`… ${lines.length - 40} more lines`);
      return pre;
    }
    case "algebraic": {
      if (obj.source == null) return el("span", { class: "partial", text: "'an expression the ROM's tables do not describe'" });
      const box = el("div", { class: "lab-alg" }, el("span", {}, el("span", { class: "delim", text: "'" }), algebraicNode(obj.source), el("span", { class: "delim", text: "'" })));
      if (ofX(obj.source)) {
        const b = el("button", { class: "small lab-plot-btn", type: "button", text: "Plot −2π…2π" });
        b.addEventListener("click", () => runLab(`:plot ${obj.source} -6.2832 6.2832`));
        box.append(b);
      }
      return box;
    }
    case "command":
      return el("span", { class: "cmd", text: text ?? obj.name ?? (obj.library !== undefined ? `XLIB ${obj.library} ${obj.command}` : `ROM object${obj.address !== undefined ? ` at #${obj.address.toString(16).toUpperCase()}` : ""}`) });
    default: {
      if (graphicDrawable(obj?.graphic)) return grobCanvas(obj.graphic);
      if (obj?.source) return el("span", { text: obj.source });
      return el("span", { class: "partial", text: `${obj?.kind ?? "Object"} · ${obj?.nibbles ?? "?"} nibbles` });
    }
  }
}

/** The screen images' colours from the tokens, as `{on, off}` RGB. */
function displayColors() {
  const probe = el("span");
  probe.style.cssText = "position:absolute;visibility:hidden";
  document.body.append(probe);
  const rgb = (v) => {
    probe.style.color = v;
    const m = /(\d+),\s*(\d+),\s*(\d+)/.exec(getComputedStyle(probe).color);
    return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : [0, 0, 0];
  };
  const out = { on: rgb("var(--display-ink)"), off: rgb("var(--display)") };
  probe.remove();
  return out;
}

/** A GROB drawn crisp: an integer scale in device pixels, at least 2 CSS px a pixel. */
function grobCanvas(g) {
  const dpr = window.devicePixelRatio || 1;
  const cssScale = g.width > 300 ? 1 : g.width > 131 ? 2 : 3;
  const scale = Math.max(1, Math.round(cssScale * dpr));
  const c = el("canvas", { class: "lab-grob", width: g.width * scale, height: g.height * scale, "aria-label": `Graphic ${g.width} × ${g.height}` });
  c.style.width = `${g.width * cssScale}px`;
  const ctx = c.getContext("2d");
  const { width, height, data } = graphicRgba(g, displayColors(), scale);
  ctx.putImageData(new ImageData(data, width, height), 0, 0);
  return c;
}

// ---------------------------------------------------------------- the LCD

function onStatus(s) {
  state.busy = Boolean(s.busy);
  state.loop = s.loop ?? null;
  document.dispatchEvent(new CustomEvent("lab-status"));
  if (s.model && !state.booted) state.booted = true;
  if (s.halted) status(`Halted: ${s.halted}`, "error");
  else if (state.busy) status("Typing…", "busy");
  else if (ui.status.textContent === "Typing…") status("");
  ui.form.classList.toggle("busy", state.busy);
}

function onFrame(f) {
  state.frame = f;
  if (!ui.lcd.hidden) drawFrame(f);
  const on = Object.entries(f.annunciators ?? {}).filter(([, v]) => v).map(([k]) => ({ leftshift: "⇧L", rightshift: "⇧R", alpha: "α", alert: "((•))", busy: "⌛", transmit: "⇄" }[k] ?? k));
  ui.ann.textContent = on.join(" ");
}

/** The frame at an integer multiple of the device pixel, nearest neighbour. */
function drawFrame(f) {
  const c = ui.canvas;
  const dpr = window.devicePixelRatio || 1;
  const cssW = c.getBoundingClientRect().width || 393;
  const scale = Math.max(1, Math.floor((cssW * dpr) / f.width));
  if (c.width !== f.width * scale) {
    c.width = f.width * scale;
    c.height = f.height * scale;
  }
  const bytes = Uint8Array.from(atob(f.pixels), (ch) => ch.charCodeAt(0));
  const rowBytes = Math.ceil(f.width / 8);
  const rows = [...bytes.subarray(0, rowBytes * f.height)].map((b) => b.toString(16).padStart(2, "0")).join("");
  const { width, height, data } = graphicRgba({ width: f.width, height: f.height, rows }, displayColors(), scale);
  c.getContext("2d").putImageData(new ImageData(data, width, height), 0, 0);
}

function showLcd(on, auto = false) {
  state.lcdAuto = on && auto;
  if (!auto) state.lcdWanted = on;
  ui.lcd.hidden = !on;
  ui.screen.setAttribute("aria-pressed", String(on));
  if (on && state.frame) drawFrame(state.frame);
}

/** A full press of key `name` on the calculator (the soft keys and the key grid). */
async function press(name, shift = null) {
  if (!state.calc || !state.booted) return;
  try {
    await state.calc.request("keyDown", { key: name, ...(shift ? { shift } : {}) });
    await state.calc.request("keyUp", { key: name });
  } catch (err) {
    status(String(err.message ?? err), "error");
  }
}

/** The six menu keys under the screen, and the whole keyboard as plain buttons from the host's layout (no artwork). */
function renderKeys() {
  const keys = state.layout?.keys ?? [];
  const soft = keys.filter((k) => ["a", "b", "c", "d", "e", "f"].includes(k.name)).sort((p, q) => p.x - q.x);
  ui.softkeys.replaceChildren(...soft.map((k, i) => {
    const b = el("button", { type: "button", text: `F${i + 1}`, title: `Menu key ${i + 1}` });
    b.addEventListener("click", () => press(k.name));
    return b;
  }));
  ui.softkeys.hidden = soft.length === 0;
  const grid = [];
  const columns = state.layout?.columns ?? 30;
  for (const k of keys) {
    const b = el("button", { type: "button", text: k.label, title: `${k.name}${k.alpha ? ` (α ${k.alpha})` : ""}`, "data-key": k.name });
    b.style.setProperty("--w", String(Math.round((k.w / columns) * 30)));
    if (k.name === "leftshift") b.classList.add("shift-l");
    if (k.name === "rightshift") b.classList.add("shift-r");
    b.addEventListener("click", () => press(k.name));
    grid.push(b);
  }
  ui.keygrid.replaceChildren(...grid);
}

// --------------------------------------------------------------- the line

/** Add a line to the log (the last five stay). */
function log(what, text, kind = "") {
  ui.log.append(el("li", { class: kind }, el("span", { class: "what", text: what }), el("span", { class: "text", text })));
  while (ui.log.children.length > 5) ui.log.firstChild.remove();
}

/** Run `text` on the calculator (`run`: typed, ENTER, settled); what came of it in the log and the status. */
async function runText(text) {
  if (!state.calc || !state.booted) {
    status("No calculator runs. Choose a ROM first.", "error");
    return null;
  }
  log("›", text);
  status("Running…", "busy");
  try {
    let r;
    try {
      r = await state.calc.request("run", { text });
    } catch (err) {
      // The typing engine could not get the keyboard into alpha (seen on
      // the 49G right after a boot, and once after such a failure): a
      // second try a moment later goes through.
      if (!/AlphaLocked/.test(String(err?.message))) throw err;
      await new Promise((res) => setTimeout(res, 1200));
      r = await state.calc.request("run", { text });
    }
    if (r.error) {
      log("!", r.error, "error");
      status(r.error, "error");
      // The ROM leaves the refused line open (the 48 with its message on
      // it): the next run would type into it. The line is in the history
      // here, so it is cancelled (ATTN) and the next command starts clean.
      if (r.commandLine?.active) await press("on");
    } else status(r.running ? "Still running…" : "");
    scheduleRead();
    return r;
  } catch (err) {
    const m = String(err.message ?? err);
    log("!", m, "error");
    status(m, "error");
    return null;
  }
}

/** The lab's own commands (`:screen`, `:keys`, `:plot`, `:rpn`, `:speed`, `:help`). */
async function runLab(text) {
  const [cmd, ...rest] = text.slice(1).trim().split(/\s+/);
  switch (cmd) {
    case "screen": showLcd(ui.lcd.hidden); return;
    case "keys": toggleKeys(); return;
    case "rpn": {
      // The algebraic mode's ENTER leaves its echo (the parsed line and NOVAL) on the stack: dropped by the count.
      const before = (await state.calc?.request("stackTop").catch(() => null))?.depth ?? 0;
      const r = await runText("CF(-95)");
      const after = (await state.calc?.request("stackTop").catch(() => null))?.depth ?? 0;
      if (r && !r.error && after > before) await runText(`${after - before} DROPN`);
      log("·", "The 49G is in RPN mode now.");
      return;
    }
    case "speed": {
      const s = ["1", "2", "4", "max"].includes(rest[0]) ? rest[0] : "1";
      state.speed = s;
      await state.calc?.request("setSpeed", { speed: s });
      log("·", `Speed ${s}.`);
      return;
    }
    case "plot": {
      const m = /^:plot\s+('[^']*'|[^\s]+)\s*(-?[\d.]+)?\s*(-?[\d.]+)?\s*(\d+)?/.exec(text);
      if (!m) { log("!", "Usage: :plot 'SIN(X)' -6.28 6.28 [points]", "error"); return; }
      await plot(m[1].startsWith("'") ? m[1] : `'${m[1]}'`, Number(m[2] ?? -10), Number(m[3] ?? 10), Number(m[4] ?? 0));
      return;
    }
    case "help":
    default:
      log("·", ":screen  :keys  :plot 'SIN(X)' -6.28 6.28  :rpn  :speed max  — anything else is sent to the calculator");
  }
}

function toggleKeys() {
  const on = ui.keygrid.hidden;
  ui.keygrid.hidden = !on;
  // The grid has the six menu keys in its first row already.
  ui.softkeys.hidden = on;
  ui.keys.setAttribute("aria-pressed", String(on));
  if (on) showLcd(true);
}

/**
 * Plot `expr` (a function of X) from `a` to `b` through the ROM: a
 * program stores X and evaluates the expression at as many points as
 * the canvas is wide (the ROM's own PLOT draws 131), reads the list of
 * values off the stack, draws it crisp, and drops the list again.
 */
async function plot(expr, a, b, points = 0) {
  if (!state.calc || !state.booted) return;
  const n = Math.min(Math.max(points || Math.round(Math.min(ui.stack.clientWidth - 48, 440) / 3), 16), 300);
  log("›", `:plot ${expr} ${a} ${b} (${n} points)`);
  status(`The calculator evaluates ${expr} at ${n} points…`, "busy");
  const prog = `« ${a} ${b} ${n} → a b n « 0 n 1 - FOR i a b a - i * n 1 - / + 'X' STO ${expr} →NUM NEXT n →LIST 'X' PURGE » »`;
  await state.calc.request("setSpeed", { speed: "max" }).catch(() => {});
  let ys = null;
  try {
    const r = await state.calc.request("run", { text: `${prog} EVAL` });
    if (r.error) {
      log("!", r.error, "error");
      status(r.error, "error");
      return;
    }
    const stack = await state.calc.request("stack");
    const top = stack[0];
    if (top?.type !== "list" || top.items.length !== n) {
      log("!", "The calculator did not leave the list of values on the stack.", "error");
      return;
    }
    ys = top.items.map((it) => (it.type === "real" && typeof it.value === "number" ? it.value : NaN));
    await state.calc.request("run", { text: "DROP" });
  } catch (err) {
    log("!", String(err.message ?? err), "error");
    return;
  } finally {
    await state.calc.request("setSpeed", { speed: state.speed }).catch(() => {});
  }
  status("");
  const card = graphCard(expr, a, b, ys);
  ui.results.append(el("li", { class: "lab-level fresh" }, el("span", { class: "n", text: "" }), el("div", { class: "obj" }, card), el("span", { class: "type", text: "Graph" })));
  while (ui.results.children.length > 3) ui.results.firstChild.remove();
  ui.stack.scrollTop = ui.stack.scrollHeight;
  scheduleRead();
}

/** A graph of `ys` over `[a, b]` on a canvas at the device's resolution, with axes. */
function graphCard(expr, a, b, ys) {
  const dpr = window.devicePixelRatio || 1;
  const cssW = Math.min(ui.stack.clientWidth - 48, 440);
  const cssH = Math.round(cssW * 0.45);
  const c = el("canvas", { class: "lab-graph", width: Math.round(cssW * dpr), height: Math.round(cssH * dpr), "aria-label": `Graph of ${expr} from ${a} to ${b}` });
  c.style.width = `${cssW}px`;
  c.style.height = `${cssH}px`;
  const ctx = c.getContext("2d");
  ctx.scale(dpr, dpr);
  const colors = displayColors();
  const css = getComputedStyle(document.documentElement);
  const finite = ys.filter((y) => Number.isFinite(y));
  let lo = Math.min(0, ...finite);
  let hi = Math.max(0, ...finite);
  if (hi === lo) { hi += 1; lo -= 1; }
  const pad = (hi - lo) * 0.08;
  lo -= pad;
  hi += pad;
  const X = (x) => ((x - a) / (b - a)) * cssW;
  const Y = (y) => cssH - ((y - lo) / (hi - lo)) * cssH;
  ctx.strokeStyle = css.getPropertyValue("--rule").trim() || "#ccc";
  ctx.lineWidth = 1;
  ctx.beginPath();
  if (lo < 0 && hi > 0) { ctx.moveTo(0, Y(0)); ctx.lineTo(cssW, Y(0)); }
  if (a < 0 && b > 0) { ctx.moveTo(X(0), 0); ctx.lineTo(X(0), cssH); }
  ctx.stroke();
  ctx.strokeStyle = css.getPropertyValue("--accent").trim() || `rgb(${colors.on})`;
  ctx.lineWidth = 2;
  ctx.lineJoin = "round";
  ctx.beginPath();
  let pen = false;
  ys.forEach((y, i) => {
    const x = a + ((b - a) * i) / (ys.length - 1);
    if (!Number.isFinite(y)) { pen = false; return; }
    if (pen) ctx.lineTo(X(x), Y(y));
    else ctx.moveTo(X(x), Y(y));
    pen = true;
  });
  ctx.stroke();
  const ink = css.getPropertyValue("--muted").trim() || "#666";
  ctx.fillStyle = ink;
  ctx.font = `11px ${css.getPropertyValue("--mono").trim() || "monospace"}`;
  ctx.fillText(String(a), 4, cssH - 6);
  ctx.textAlign = "right";
  ctx.fillText(String(b), cssW - 4, cssH - 6);
  ctx.textAlign = "left";
  ctx.fillText(hi.toPrecision(3), 4, 12);
  ctx.fillText(lo.toPrecision(3), 4, cssH - 18);
  return el("div", { class: "lab-graph-wrap" }, c, el("span", { class: "cap", text: `${expr} from ${a} to ${b}, ${ys.length} points evaluated by the calculator` }));
}

// ------------------------------------------------------------ suggestions

function suggest() {
  const q = ui.input.value;
  const rows = state.index && q.trim() && !q.startsWith(":") ? search(state.index, q, { typing: true, limit: 8 }).filter((r) => r.kind === "command" || r.kind === "menu") : [];
  state.suggestions = rows;
  state.selected = rows.length ? 0 : -1;
  renderSuggestions();
}

function renderSuggestions() {
  const rows = state.suggestions;
  ui.suggest.hidden = rows.length === 0;
  ui.suggest.replaceChildren(...rows.map((r, i) => {
    const row = el("div", { role: "option", "aria-selected": String(i === state.selected), "data-i": i },
      el("span", { class: "sname", text: r.name }),
      el("span", { class: "sdesc" }, r.kind === "command" ? el("span", { class: "fx", text: r.stack ?? "" }) : el("b", { text: "menu " }), r.kind === "command" ? (r.longName ? `${r.longName}. ` : "") + (r.description ?? "") : r.menu?.title ?? ""));
    row.addEventListener("mousedown", (e) => { e.preventDefault(); accept(i); });
    return row;
  }));
  ui.input.setAttribute("aria-expanded", String(rows.length > 0));
}

/** Put suggestion `i`'s name in place of the word at the end of the line. */
function accept(i) {
  const r = state.suggestions[i];
  if (!r) return;
  const v = ui.input.value;
  const head = v.replace(/\S*$/, "");
  ui.input.value = `${head}${r.name} `;
  state.suggestions = [];
  renderSuggestions();
  ui.input.focus();
}

function onKey(e) {
  const n = state.suggestions.length;
  if (e.key === "Tab" && n) {
    e.preventDefault();
    accept(Math.max(state.selected, 0));
  } else if (e.key === "ArrowDown" && n) {
    e.preventDefault();
    state.selected = (state.selected + 1) % n;
    renderSuggestions();
  } else if (e.key === "ArrowUp" && n) {
    e.preventDefault();
    state.selected = (state.selected - 1 + n) % n;
    renderSuggestions();
  } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
    // The history, when nothing is suggested.
    const h = state.history;
    if (!h.length) return;
    e.preventDefault();
    state.historyAt = e.key === "ArrowUp" ? Math.min(state.historyAt + 1, h.length - 1) : Math.max(state.historyAt - 1, -1);
    ui.input.value = state.historyAt < 0 ? "" : h[h.length - 1 - state.historyAt];
  } else if (e.key === "Escape") {
    if (n) { state.suggestions = []; renderSuggestions(); } else ui.input.value = "";
  }
}

async function submit(e) {
  e?.preventDefault();
  const text = ui.input.value.trim();
  if (!text) {
    // Enter on an empty line is the calculator's ENTER (DUP, or the open line).
    await runText("");
    return;
  }
  state.history.push(text);
  state.historyAt = -1;
  ui.input.value = "";
  state.suggestions = [];
  renderSuggestions();
  if (text.startsWith(":")) await runLab(text);
  else await runText(text);
}

// -------------------------------------------------------------------- ROMs

async function listKept() {
  const roms = await keptRoms();
  ui.kept.replaceChildren(...roms.map((r) => {
    const b = el("button", { class: "small", type: "button", text: TITLES[r.model] ?? r.model, title: r.name });
    b.addEventListener("click", async () => {
      try {
        const bytes = await keptRomBytes(r.sha256);
        if (!bytes) throw new Error("The ROM's bytes are not there any more.");
        await boot(bytes, r.name, r.model);
      } catch (err) {
        status(String(err.message ?? err), "error");
      }
    });
    return b;
  }));
  if (!roms.length) ui.kept.textContent = "(none kept in this browser; the app keeps the ROMs you give it)";
}

async function chooseFile() {
  ui.file.value = "";
  ui.file.click();
}

ui.file.addEventListener("change", async () => {
  const f = ui.file.files?.[0];
  if (!f) return;
  try {
    await boot(new Uint8Array(await f.arrayBuffer()), f.name);
  } catch (err) {
    status(String(err.message ?? err), "error");
  }
});

// ------------------------------------------------------------------ wiring

ui.form.addEventListener("submit", submit);
ui.input.addEventListener("input", suggest);
ui.input.addEventListener("keydown", onKey);
ui.input.addEventListener("blur", () => setTimeout(() => { if (document.activeElement !== ui.input) { state.suggestions = []; renderSuggestions(); } }, 120));
ui.screen.addEventListener("click", () => showLcd(ui.lcd.hidden));
ui.lcdClose.addEventListener("click", () => showLcd(false));
ui.keys.addEventListener("click", toggleKeys);
ui.rom.addEventListener("click", chooseFile);
ui.rom2.addEventListener("click", chooseFile);
document.addEventListener("keydown", (e) => {
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
    e.preventDefault();
    ui.input.focus();
    ui.input.select();
  }
});
addEventListener("resize", () => { if (!ui.lcd.hidden && state.frame) drawFrame(state.frame); });
document.addEventListener("visibilitychange", () => state.calc?.request("visibility", { hidden: document.hidden }).catch(() => {}));
listKept();

// For the tests and the screenshots.
window.lab = {
  boot,
  run: runText,
  lab: runLab,
  press,
  get state() { return state; },
  get stack() { return state.stack; },
  showLcd,
};
