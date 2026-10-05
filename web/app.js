// saturnus web UI: drives the WebAssembly core from requestAnimationFrame.
// No framework, no bundler. See web/README.md.

import init, { Emulator, model_names, rom_fits, skin as skinFor } from "./pkg/saturnus_web.js";

const W = 131;
const H = 64;
/** Height of the annunciator strip above the pixels, in LCD pixels. */
const ANN_H = 8;
/** Longest stretch of emulated time one animation frame may run. */
const MAX_FRAME_MS = 100;
/** Frames run in slices of this many emulated ms, so key timing is fine. */
const SLICE_MS = 10;
/** Shortest key press the ROM sees, in emulated ms (its debounce needs >10). */
const MIN_HOLD_MS = 60;
/** Pause between two queued key presses, in emulated ms. */
const GAP_MS = 30;
const DB_NAME = "saturnus";
const DB_STORE = "states";
const PREF_MODEL = "saturnus.model";
const PREF_VIEW = "saturnus.view";
const SVG_NS = "http://www.w3.org/2000/svg";
/** Share of the display window the LCD canvas may fill. */
const LCD_FILL = 0.94;

const MODEL_TITLES = {
  "48sx": "HP 48SX",
  "48gx": "HP 48GX",
  "38g": "HP 38G",
  "49g": "HP 49G",
  "39g": "HP 39G",
  "40g": "HP 40G",
};

/** Physical keyboard: KeyboardEvent.key -> script key name. */
const KEYMAP = {
  "0": "0", "1": "1", "2": "2", "3": "3", "4": "4",
  "5": "5", "6": "6", "7": "7", "8": "8", "9": "9",
  "+": "plus", "-": "minus", "*": "multiply", "/": "divide",
  ".": "point", ",": "point", " ": "space", "'": "quote", "^": "power",
  Enter: "enter", Backspace: "backspace", Delete: "del",
  ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right",
  Escape: "on",
  F1: "a", F2: "b", F3: "c", F4: "d", F5: "e", F6: "f",
};

/** Annunciator glyphs, generic marks in strip order. */
const ANNUNCIATORS = [
  ["leftshift", "↰"],
  ["rightshift", "↱"],
  ["alpha", "α"],
  ["alert", "((•))"],
  ["busy", "⌛"],
  ["transmit", "⇄"],
];

const $ = (id) => document.getElementById(id);
const ui = {
  model: $("model"),
  rom: $("rom"),
  run: $("run"),
  reset: $("reset"),
  save: $("save"),
  load: $("load"),
  lcd: $("lcd"),
  keyboard: $("keyboard"),
  status: $("status"),
  calc: $("calc"),
  skin: $("skin"),
  skinSvg: $("skin-svg"),
  viewSkin: $("view-skin"),
};

let emu = null;
let modelName = null;
let running = false;
let haltMessage = null;
let message = "";
let keyNames = new Set();
const keyButtons = new Map();
/** Drawn skin: the model's skin data, its key groups and display window. */
let useSkin = true;
let skinData = null;
let skinModel = null;
const skinKeys = new Map();
let skinWindow = null;
let skinWindowFill = "";
/** Text elements to squeeze into a width once the skin is laid out. */
let skinFits = [];

// Key handling: `active` keys are down in the emulator; `pending` presses
// wait for a fast-typed predecessor to be released first.
let active = [];
let pending = [];
let lastReleaseMs = -Infinity;

// Speed measurement over the last second of wall time.
let lastFrame = null;
let speedWindow = [];

const lcdOff = document.createElement("canvas");
lcdOff.width = W;
lcdOff.height = H;
const lcdOffCtx = lcdOff.getContext("2d");
const lcdImage = lcdOffCtx.createImageData(W, H);
let scale = 3;

// ---------------------------------------------------------------- storage

function prefGet(key) {
  try { return localStorage.getItem(key); } catch { return null; }
}
function prefSet(key, value) {
  try { localStorage.setItem(key, value); } catch { /* storage blocked */ }
}

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(DB_STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function dbGet(key) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const req = db.transaction(DB_STORE, "readonly").objectStore(DB_STORE).get(key);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  }).finally(() => db.close());
}

async function dbPut(key, value) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(DB_STORE, "readwrite");
    tx.objectStore(DB_STORE).put(value, key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  }).finally(() => db.close());
}

// ---------------------------------------------------------------- display

function lcdColors() {
  // The drawn calculator keeps its real LCD colours in both themes.
  const dark = !useSkin && window.matchMedia("(prefers-color-scheme: dark)").matches;
  return dark
    ? { bg: [150, 160, 132], ink: [12, 16, 10] }
    : { bg: [183, 194, 162], ink: [16, 20, 12] };
}

/** Pixel darkness from the contrast register, 0.3 (lightest) to 1. */
function darkness() {
  if (!emu) return 1;
  const [lo, hi] = emu.contrast_range();
  const t = Math.min(1, Math.max(0, (emu.contrast() - lo) / Math.max(1, hi - lo)));
  return 0.3 + 0.7 * t;
}

function mix(a, b, t) {
  return [0, 1, 2].map((i) => Math.round(a[i] + (b[i] - a[i]) * t));
}

/** Width available to the calculator inside `main`, in CSS pixels. */
function contentWidth() {
  const main = ui.skin.parentElement;
  const st = getComputedStyle(main);
  return main.clientWidth - parseFloat(st.paddingLeft) - parseFloat(st.paddingRight);
}

/**
 * Size the skin and the LCD canvas. On the skin, the largest integer LCD
 * scale (CSS pixels per LCD pixel, 2 or more) whose skin fits the width and
 * the window height wins, so the pixels stay crisp. Scale 2 is kept when only
 * the height is short (the page scrolls); when even 2 is too wide, the skin
 * fills the width and the scale is fractional.
 */
function fitCanvas() {
  const dpr = window.devicePixelRatio || 1;
  const rows = H + ANN_H;
  let css;
  if (useSkin && skinData) {
    const s = skinData;
    const [lx, ly, lw, lh] = s.lcd;
    const availW = Math.max(200, contentWidth());
    const availH = Math.max(480, window.innerHeight - 24);
    const unitsFor = (k) => Math.max((W * k) / (lw * LCD_FILL), (rows * k) / (lh * LCD_FILL));
    let f = null;
    for (let k = 6; k >= 2; k--) {
      const fk = unitsFor(k);
      if (s.width * fk <= availW && s.height * fk <= availH) {
        f = fk;
        css = k;
        break;
      }
    }
    // Scale 2 even if the page must scroll, as long as it fits the width.
    if (f === null && s.width * unitsFor(2) <= availW) {
      f = unitsFor(2);
      css = 2;
    }
    if (f === null) {
      f = availW / s.width;
      css = Math.min((lw * LCD_FILL * f) / W, (lh * LCD_FILL * f) / rows);
    }
    ui.skin.style.width = `${s.width * f}px`;
    const cw = W * css;
    const ch = rows * css;
    const snap = (v) => Math.round(v * dpr) / dpr;
    ui.lcd.style.left = `${snap((lx + lw / 2) * f - cw / 2)}px`;
    ui.lcd.style.top = `${snap((ly + lh / 2) * f - ch / 2)}px`;
    ui.lcd.style.width = `${cw}px`;
    ui.lcd.style.height = `${ch}px`;
  } else {
    const frame = ui.lcd.parentElement;
    const avail = Math.max(W * 2, frame.clientWidth - 16);
    css = Math.max(2, Math.min(4, Math.floor(avail / W)));
    ui.lcd.style.left = "";
    ui.lcd.style.top = "";
    ui.lcd.style.width = `${W * css}px`;
    ui.lcd.style.height = `${rows * css}px`;
  }
  scale = Math.max(1, Math.round(css * dpr));
  ui.lcd.width = W * scale;
  ui.lcd.height = rows * scale;
  draw();
}

function draw() {
  const ctx = ui.lcd.getContext("2d");
  const { bg, ink } = lcdColors();
  const d = darkness();
  const on = mix(bg, ink, d);
  const off = mix(bg, ink, 0.06 * d * d);
  ctx.fillStyle = `rgb(${off})`;
  ctx.fillRect(0, 0, ui.lcd.width, ui.lcd.height);
  if (skinWindow && skinWindowFill !== ctx.fillStyle) {
    skinWindowFill = ctx.fillStyle;
    skinWindow.setAttribute("fill", skinWindowFill);
  }
  if (!emu) return;

  const fb = emu.framebuffer();
  const px = lcdImage.data;
  for (let i = 0; i < W * H; i++) {
    const c = fb[i] ? on : off;
    px[i * 4] = c[0];
    px[i * 4 + 1] = c[1];
    px[i * 4 + 2] = c[2];
    px[i * 4 + 3] = 255;
  }
  lcdOffCtx.putImageData(lcdImage, 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(lcdOff, 0, ANN_H * scale, W * scale, H * scale);

  const ann = emu.annunciators();
  ctx.fillStyle = `rgb(${on})`;
  ctx.font = `${Math.round(6.5 * scale)}px system-ui, sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  const slot = (W * scale) / ANNUNCIATORS.length;
  ANNUNCIATORS.forEach(([name, glyph], i) => {
    if (ann[name]) ctx.fillText(glyph, slot * (i + 0.5), (ANN_H / 2) * scale);
  });
}

// ---------------------------------------------------------------- keyboard

function buildKeyboard() {
  ui.keyboard.replaceChildren();
  keyButtons.clear();
  keyNames = new Set();
  if (!emu) return;
  const layout = emu.keys();
  ui.keyboard.style.gridTemplateColumns = `repeat(${layout.columns}, 1fr)`;
  for (const k of layout.keys) {
    keyNames.add(k.name);
    const b = document.createElement("button");
    b.type = "button";
    b.tabIndex = -1;
    b.textContent = k.label;
    if (k.alpha) {
      const a = document.createElement("span");
      a.className = "alpha";
      a.textContent = k.alpha;
      b.append(a);
    }
    b.title = k.alpha ? `${k.name} (alpha ${k.alpha})` : k.name;
    b.dataset.key = k.name;
    if (/^[0-9]$/.test(k.label)) b.classList.add("digit");
    if (k.label.length > 4) b.classList.add("wide-label");
    b.style.gridRow = String(k.row + 1);
    b.style.gridColumn = `${k.x + 1} / span ${k.w}`;
    b.addEventListener("pointerdown", (e) => {
      e.preventDefault();
      b.setPointerCapture(e.pointerId);
      pressKey(k.name);
    });
    const up = () => releaseKey(k.name);
    b.addEventListener("pointerup", up);
    b.addEventListener("pointercancel", up);
    keyButtons.set(k.name, b);
    ui.keyboard.append(b);
  }
}

function showDown(name, down) {
  keyButtons.get(name)?.classList.toggle("down", down);
  const g = skinKeys.get(name);
  if (g) {
    g.classList.toggle("down", down);
    g.querySelector(".cap")?.setAttribute("transform", down ? "translate(0 3)" : "");
    const press = g.querySelector(".press");
    press?.setAttribute("fill-opacity", down ? "0.28" : "0");
    press?.setAttribute("stroke-opacity", down ? "0.28" : "0");
  }
}

function pressKey(name) {
  if (!emu || !keyNames.has(name)) return;
  pending.push({ name, up: false, downAt: 0 });
  pumpKeys();
}

function releaseKey(name) {
  // The newest press of `name` that is still held by the user.
  const all = active.concat(pending);
  for (let i = all.length - 1; i >= 0; i--) {
    if (all[i].name === name && !all[i].up) {
      all[i].up = true;
      break;
    }
  }
  pumpKeys();
}

/** Release keys held long enough, then start queued presses. */
function pumpKeys() {
  if (!emu) return;
  const now = emu.emulated_ms();
  active = active.filter((k) => {
    if (k.up && now - k.downAt >= MIN_HOLD_MS) {
      emu.key_up(k.name);
      showDown(k.name, active.some((o) => o !== k && o.name === k.name));
      lastReleaseMs = now;
      return false;
    }
    return true;
  });
  while (pending.length > 0) {
    // Wait while a fast-typed key is still on its way up; a key the user
    // keeps holding (ON for a chord) does not block.
    if (active.some((k) => k.up)) break;
    if (now - lastReleaseMs < GAP_MS) break;
    const k = pending.shift();
    if (active.some((o) => o.name === k.name)) {
      // Same key still down (held): count this press as the same one.
      if (k.up) {
        const o = active.find((a) => a.name === k.name);
        if (o) o.up = true;
      }
      continue;
    }
    try {
      emu.key_down(k.name);
    } catch (err) {
      setMessage(String(err), true);
      continue;
    }
    k.downAt = now;
    active.push(k);
    showDown(k.name, true);
  }
}

function releaseAll() {
  if (emu) emu.release_all();
  active = [];
  pending = [];
  for (const name of new Set([...keyButtons.keys(), ...skinKeys.keys()])) showDown(name, false);
}

function onKeyDown(e) {
  if (!emu || e.ctrlKey || e.metaKey || e.altKey) return;
  const t = e.target;
  if (t instanceof HTMLSelectElement || t instanceof HTMLInputElement) return;
  const name = KEYMAP[e.key];
  if (!name || !keyNames.has(name)) return;
  e.preventDefault();
  if (e.repeat) return;
  pressKey(name);
}

function onKeyUp(e) {
  const name = KEYMAP[e.key];
  if (!emu || !name || !keyNames.has(name)) return;
  e.preventDefault();
  releaseKey(name);
}

// ---------------------------------------------------------------- skin

function svg(name, attrs = {}, parent = null, text = null) {
  const e = document.createElementNS(SVG_NS, name);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  if (text !== null) e.textContent = text;
  if (parent) parent.append(e);
  return e;
}

/** A rectangle path with top corners `rt` and bottom corners `rb`. */
function roundedPath([x, y, w, h], rt, rb) {
  rt = Math.min(rt, w / 2, h / 2);
  rb = Math.min(rb, w / 2, h / 2);
  return `M${x + rt} ${y}H${x + w - rt}A${rt} ${rt} 0 0 1 ${x + w} ${y + rt}`
    + `V${y + h - rb}A${rb} ${rb} 0 0 1 ${x + w - rb} ${y + h}H${x + rb}`
    + `A${rb} ${rb} 0 0 1 ${x} ${y + h - rb}V${y + rt}A${rt} ${rt} 0 0 1 ${x + rt} ${y}Z`;
}

/** Outline of a key: a rounded rectangle, or a cursor-pad trapezoid. */
function keyPath(shape, [x, y, w, h]) {
  if (shape === "key") return roundedPath([x, y, w, h], Math.min(w, h) * 0.22, Math.min(w, h) * 0.22);
  // Trapezoids narrower on the side the arrow points to; rounded by a
  // stroke of the same colour (see `keyStroke`), so inset by its half.
  const i = 5;
  const [l, t, r, b] = [x + i, y + i, x + w - i, y + h - i];
  const pts = {
    up: [[l + (r - l) * 0.28, t], [r - (r - l) * 0.28, t], [r, b], [l, b]],
    down: [[l, t], [r, t], [r - (r - l) * 0.28, b], [l + (r - l) * 0.28, b]],
    left: [[l, t + (b - t) * 0.22], [r, t], [r, b], [l, b - (b - t) * 0.22]],
    right: [[l, t], [r, t + (b - t) * 0.22], [r, b - (b - t) * 0.22], [l, b]],
  }[shape];
  return `M${pts.map((p) => p.join(" ")).join("L")}Z`;
}

function keyStroke(shape, fill) {
  return shape === "key" ? {} : { stroke: fill, "stroke-width": 10, "stroke-linejoin": "round" };
}

/** Remember `t` to squeeze to `max` units wide once it can be measured. */
function fitLater(t, max) {
  skinFits.push([t, max]);
}

/** Squeeze texts wider than their room (needs the skin on screen). */
function fitTexts() {
  if (!useSkin) return;
  for (const [t, max] of skinFits) {
    t.removeAttribute("textLength");
    t.removeAttribute("lengthAdjust");
    let len = 0;
    try { len = t.getComputedTextLength(); } catch { len = 0; }
    if (len > max) {
      t.setAttribute("textLength", String(max));
      t.setAttribute("lengthAdjust", "spacingAndGlyphs");
    }
  }
}

/** The text of a shift label pair above key `k`. */
function drawShiftLabels(layer, s, k) {
  const [x, y, w] = k.rect;
  const base = y - 5;
  const size = s.small;
  if (k.left && k.right) {
    const a = svg("text", { x: x - 3, y: base, "font-size": size, fill: s.leftInk }, layer, k.left);
    const b = svg("text", { x: x + w + 3, y: base, "font-size": size, fill: s.rightInk, "text-anchor": "end" }, layer, k.right);
    // Each label keeps to its share of the room above the key.
    const room = w + 6;
    const la = Math.max(1, k.left.length);
    const lb = Math.max(1, k.right.length);
    fitLater(a, (room - 6) * (la / (la + lb)));
    fitLater(b, (room - 6) * (lb / (la + lb)));
  } else if (k.left || k.right) {
    const t = svg("text", {
      x: x + w / 2, y: base, "font-size": size, "text-anchor": "middle",
      fill: k.left ? s.leftInk : s.rightInk,
    }, layer, k.left || k.right);
    fitLater(t, w + 34);
  }
}

function drawAlpha(layer, s, k) {
  if (!k.alpha || s.alphaStyle === "badge") return;
  const [x, y, w, h] = k.rect;
  const size = s.small * 0.95;
  if (s.alphaStyle === "below") {
    svg("text", { x: x + w + 6, y: y + h + size * 0.95, "font-size": size, fill: s.alphaInk, "text-anchor": "end", "font-style": "italic" }, layer, k.alpha);
  } else {
    svg("text", { x: x + w + 2, y: y + h + size * 0.55, "font-size": size, fill: s.alphaInk }, layer, k.alpha);
  }
}

function drawKey(keys, s, k) {
  const [x, y, w, h] = k.rect;
  const g = svg("g", { class: "skey", "data-key": k.name }, keys);
  const title = k.alpha ? `${k.name} (alpha ${k.alpha})` : k.name;
  svg("title", {}, g, title);
  // Hit area a little larger than the cap, as the grid's buttons are.
  svg("rect", { x: x - 6, y: y - 6, width: w + 12, height: h + 12, fill: "#000", "fill-opacity": 0 }, g);
  const d = keyPath(k.shape, k.rect);
  svg("path", { d, fill: "#000", "fill-opacity": 0.38, transform: "translate(0 4)", ...keyStroke(k.shape, "#000") }, g);
  const cap = svg("g", { class: "cap" }, g);
  svg("path", { d, fill: k.fill, ...keyStroke(k.shape, k.fill) }, cap);
  svg("path", { d, fill: "#000", "fill-opacity": 0, class: "press", ...keyStroke(k.shape, "#000"), "stroke-opacity": 0 }, cap);
  const badge = s.alphaStyle === "badge" && k.alpha;
  if (badge) {
    const r = h * 0.3;
    const cx = x + w - r - h * 0.14;
    const cy = y + h * 0.54;
    svg("circle", { cx, cy, r, fill: s.alphaBadge }, cap);
    svg("text", { x: cx, y: cy + r * 0.5, "font-size": r * 1.45, fill: s.alphaInk, "text-anchor": "middle", "font-style": "italic" }, cap, k.alpha);
  }
  if (k.label) {
    const single = [...k.label].length === 1;
    let size = single ? h * 0.56 : h * (badge ? 0.34 : 0.4);
    if (k.shape !== "key") size = Math.min(w, h) * 0.42;
    // On a badge key the label keeps to the left of the disc.
    const room = badge ? w - 2 * h * 0.3 - h * 0.14 - w * 0.14 : w * 0.84;
    const cx = badge ? x + w * 0.07 + room / 2 : x + w / 2;
    const t = svg("text", {
      x: cx, y: y + h / 2 + size * 0.36, "font-size": size, fill: k.ink, "text-anchor": "middle",
    }, cap, k.label);
    fitLater(t, room);
  }
  g.addEventListener("pointerdown", (e) => {
    e.preventDefault();
    try { g.setPointerCapture(e.pointerId); } catch { /* not capturable */ }
    pressKey(k.name);
  });
  const up = () => releaseKey(k.name);
  g.addEventListener("pointerup", up);
  g.addEventListener("pointercancel", up);
  skinKeys.set(k.name, g);
}

/** Draw the skin of `model` into the SVG. */
function renderSkin(model) {
  skinKeys.clear();
  skinFits = [];
  skinWindow = null;
  skinWindowFill = "";
  try {
    skinData = skinFor(model);
  } catch (err) {
    skinData = null;
    setMessage(String(err), true);
    return;
  }
  skinModel = model;
  const s = skinData;
  const root = ui.skinSvg;
  root.replaceChildren();
  root.setAttribute("viewBox", `0 0 ${s.width} ${s.height}`);
  root.setAttribute("aria-label", `${MODEL_TITLES[model] ?? model} keyboard`);
  for (const p of s.panels) svg("path", { d: roundedPath(p.rect, p.radius, p.bottomRadius), fill: p.fill }, root);
  const [lx, ly, lw, lh] = s.lcd;
  skinWindow = svg("rect", { x: lx, y: ly, width: lw, height: lh, rx: 4, fill: s.lcdFill }, root);
  const [gx, gy, gw, gh] = s.logo;
  svg("image", { href: "logo.svg", x: gx, y: gy, width: gw, height: gh }, root);
  const print = svg("g", { class: "print" }, root);
  for (const m of s.marks) {
    svg("text", { x: m.x, y: m.y, "font-size": m.size, fill: m.fill, "text-anchor": "middle" }, print, m.text);
  }
  for (const l of s.lines) {
    svg("line", { x1: l.x1, y1: l.y1, x2: l.x2, y2: l.y2, stroke: l.stroke, "stroke-width": 2 }, print);
  }
  for (const k of s.keys) {
    drawShiftLabels(print, s, k);
    drawAlpha(print, s, k);
    if (k.below) {
      const [x, y, w, h] = k.rect;
      svg("text", { x: x + w / 2, y: y + h + s.small + 4, "font-size": s.small, fill: s.belowInk, "text-anchor": "middle" }, print, k.below);
    }
  }
  const keys = svg("g", { class: "keys" }, root);
  for (const k of s.keys) drawKey(keys, s, k);
  for (const a of active) showDown(a.name, true);
  fitCanvas();
  fitTexts();
}

/** Show the skin or the plain grid. */
function setView(skinView) {
  useSkin = skinView;
  ui.viewSkin.checked = skinView;
  prefSet(PREF_VIEW, skinView ? "skin" : "grid");
  ui.skin.hidden = !skinView;
  ui.calc.hidden = skinView;
  if (skinView) {
    ui.skin.append(ui.lcd);
    const model = modelName ?? ui.model.value;
    if (skinModel !== model) renderSkin(model);
  } else {
    ui.calc.querySelector(".lcd-frame").append(ui.lcd);
  }
  fitCanvas();
  fitTexts();
}

// ---------------------------------------------------------------- run loop

function frame(t) {
  requestAnimationFrame(frame);
  const wall = lastFrame === null ? 0 : t - lastFrame;
  lastFrame = t;
  if (!emu) return;
  if (running) {
    let left = Math.min(wall, MAX_FRAME_MS);
    const before = emu.emulated_ms();
    const start = performance.now();
    try {
      while (left > 0) {
        const step = Math.min(left, SLICE_MS);
        emu.run_ms(step);
        left -= step;
        pumpKeys();
      }
    } catch (err) {
      haltMessage = String(err);
      setRunning(false);
    }
    speedWindow.push({ t, wall, emu: emu.emulated_ms() - before, cpu: performance.now() - start });
    while (speedWindow.length > 0 && t - speedWindow[0].t > 1000) speedWindow.shift();
  }
  draw();
  updateStatus();
}

function speed() {
  let wall = 0, emulated = 0, cpu = 0;
  for (const s of speedWindow) {
    wall += s.wall;
    emulated += s.emu;
    cpu += s.cpu;
  }
  return wall > 0 ? { ratio: emulated / wall, load: cpu / wall } : null;
}

function updateStatus() {
  if (!emu) return;
  const parts = [
    MODEL_TITLES[modelName] ?? modelName,
    `${(emu.emulated_ms() / 1000).toFixed(1)} s emulated`,
    running ? "running" : "paused",
  ];
  const s = running ? speed() : null;
  if (s) parts.push(`speed ${Math.round(s.ratio * 100)}%`, `cpu ${Math.round(s.load * 100)}%`);
  if (emu.is_shutdown()) parts.push("idle");
  let text = parts.join(" · ");
  if (haltMessage) text += ` · ${haltMessage}`;
  else if (message) text += ` · ${message}`;
  if (ui.status.textContent !== text) ui.status.textContent = text;
  ui.status.classList.toggle("error", Boolean(haltMessage));
}

function setMessage(text, isError = false) {
  message = text;
  if (!emu) {
    ui.status.textContent = text;
    ui.status.classList.toggle("error", isError);
  }
}

function setRunning(on) {
  running = on && emu !== null && haltMessage === null;
  ui.run.textContent = running ? "Pause" : "Run";
  speedWindow = [];
}

// ---------------------------------------------------------------- setup

function fillModels() {
  const saved = prefGet(PREF_MODEL);
  for (const name of model_names()) {
    const o = document.createElement("option");
    o.value = name;
    o.textContent = MODEL_TITLES[name] ?? name;
    ui.model.append(o);
  }
  if (saved && model_names().includes(saved)) ui.model.value = saved;
}

/**
 * A model whose ROM fits `rom`, preferring the selected one. A 2 MB file
 * fits both the 49G (packed) and the 39G/40G (unpacked, every byte a
 * nibble); the bytes tell which.
 */
function modelForRom(rom, preferred) {
  const unpacked = rom.length === 2 * 1024 * 1024 && rom.every((b) => b < 16);
  const fits = (m) => rom_fits(m, rom.length) && (m !== "49g" || !unpacked)
    && (!["39g", "40g"].includes(m) || unpacked || rom.length !== 2 * 1024 * 1024);
  if (fits(preferred)) return preferred;
  return model_names().find(fits) ?? preferred;
}

async function startWithRom(file) {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const model = modelForRom(bytes, ui.model.value);
  if (model !== ui.model.value) ui.model.value = model;
  try {
    const next = new Emulator(model, bytes);
    if (emu) emu.free();
    emu = next;
  } catch (err) {
    setMessage(`Cannot start: ${err}`, true);
    return;
  }
  modelName = model;
  prefSet(PREF_MODEL, model);
  haltMessage = null;
  releaseAll();
  lastReleaseMs = -Infinity;
  buildKeyboard();
  if (useSkin && skinModel !== model) renderSkin(model);
  for (const b of [ui.run, ui.reset, ui.save]) b.disabled = false;
  message = `ROM ${file.name} loaded`;
  await refreshLoadButton();
  setRunning(true);
  fitCanvas();
}

async function refreshLoadButton() {
  if (!emu) {
    ui.load.disabled = true;
    return;
  }
  try {
    ui.load.disabled = !(await dbGet(modelName));
  } catch {
    ui.load.disabled = true;
  }
}

async function saveState() {
  if (!emu) return;
  try {
    await dbPut(modelName, { state: emu.save_state(), saved: Date.now(), cycles: emu.cycles() });
    message = `state saved ${new Date().toLocaleTimeString()}`;
    ui.load.disabled = false;
  } catch (err) {
    message = `save failed: ${err}`;
  }
}

async function loadState() {
  if (!emu) return;
  try {
    const rec = await dbGet(modelName);
    if (!rec) {
      message = "no saved state for this model";
      return;
    }
    releaseAll();
    emu.load_state(rec.state);
    haltMessage = null;
    lastReleaseMs = -Infinity;
    message = `state from ${new Date(rec.saved).toLocaleString()} loaded`;
    draw();
  } catch (err) {
    message = `load failed: ${err}`;
  }
}

function blurAfter(fn) {
  return (e) => {
    e.currentTarget.blur();
    fn();
  };
}

async function main() {
  await init();
  fillModels();
  ui.model.addEventListener("change", () => {
    prefSet(PREF_MODEL, ui.model.value);
    if (emu && ui.model.value !== modelName) {
      setMessage("pick a ROM for the new model");
      message = "pick a ROM for the new model";
    }
    if (!emu && useSkin) renderSkin(ui.model.value);
  });
  ui.rom.addEventListener("change", () => {
    const f = ui.rom.files?.[0];
    if (f) startWithRom(f);
    ui.rom.blur();
  });
  ui.run.addEventListener("click", blurAfter(() => setRunning(!running)));
  ui.reset.addEventListener("click", blurAfter(() => {
    if (!emu) return;
    releaseAll();
    emu.reset();
    haltMessage = null;
    setRunning(true);
  }));
  ui.save.addEventListener("click", blurAfter(saveState));
  ui.load.addEventListener("click", blurAfter(loadState));
  document.addEventListener("keydown", onKeyDown);
  document.addEventListener("keyup", onKeyUp);
  window.addEventListener("blur", () => {
    for (const k of active.concat(pending)) k.up = true;
  });
  window.addEventListener("resize", fitCanvas);
  ui.viewSkin.addEventListener("change", () => {
    ui.viewSkin.blur();
    setView(ui.viewSkin.checked);
  });
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", draw);
  setView(prefGet(PREF_VIEW) !== "grid");
  requestAnimationFrame(frame);
  // Read-only handle for debugging and automated checks.
  window.saturnus = {
    get emulator() { return emu; },
    screenText() {
      if (!emu) return "";
      const fb = emu.framebuffer();
      let s = "";
      for (let y = 0; y < H; y++) {
        for (let x = 0; x < W; x++) s += fb[y * W + x] ? "#" : ".";
        s += "\n";
      }
      return s;
    },
    speed,
    startWithRom,
    /** The drawn key group of `name`, for automated checks. */
    skinKey(name) { return skinKeys.get(name) ?? null; },
  };
}

main().catch((err) => setMessage(`Failed to start: ${err}`, true));
