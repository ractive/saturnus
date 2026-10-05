// saturnus web UI: drives the WebAssembly core from requestAnimationFrame
// while the calculator is busy, and from a timer while it sleeps.
// No framework, no bundler. See web/README.md.

import init, { Emulator, model_names, rom_fits, skin as skinFor } from "./pkg/saturnus_web.js";

const W = 131;
const H = 64;
/** Height of the annunciator strip above the pixels, in LCD pixels. */
const ANN_H = 8;
const ROWS = H + ANN_H;
/** Longest stretch of wall time one animation frame may make up for. */
const MAX_FRAME_MS = 100;
/** Frames run in slices of this many emulated ms, so key timing is fine. */
const SLICE_MS = 10;
/** At "Max" speed, the wall time a frame may spend emulating. */
const MAX_BUDGET_MS = 11;
/** At "Max" speed, the emulated time one frame may run at most. */
const MAX_EMULATED_PER_FRAME_MS = 1000;
/** Emulated time per wall time while sleeping at "Max" (the wake timer). */
const MAX_RATE = 60;
/** Shortest key press the ROM sees, in emulated ms (its debounce needs >10). */
const MIN_HOLD_MS = 60;
/** Shortest pause between two queued key presses, in emulated ms. */
const GAP_MS = 30;
/**
 * A queued press also waits for the ROM to go idle (SHUTDN) after the
 * previous key, since the 48SX ROM drops a key pressed while it still
 * handles the last one (70-230 ms); while the ROM stays busy, as in a
 * running program, it waits at most this long.
 */
const BUSY_GAP_MS = 300;
/**
 * Quiet time before a typed letter reads the alpha annunciator while
 * the ROM stays busy, in emulated ms: the 48SX ROM blinks it while it
 * redraws the command line, up to about 250 ms after a key is released.
 */
const LETTER_SETTLE_MS = 400;
const DB_NAME = "saturnus";
const DB_STORE = "states";
const PREF_MODEL = "saturnus.model";
const PREF_VIEW = "saturnus.view";
const PREF_SPEED = "saturnus.speed";
const PREF_PANEL = "saturnus.panel";
const SVG_NS = "http://www.w3.org/2000/svg";
/** The skin may shrink this much so the LCD lands on whole device pixels. */
const SNAP_LOSS = 0.08;
const SPEEDS = ["1", "2", "4", "max"];

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
  Escape: "on", Tab: "alpha",
  F1: "a", F2: "b", F3: "c", F4: "d", F5: "e", F6: "f",
};
/** Shortcuts that depend on the model's keys: the first name present wins. */
const KEYMAP_ANY = {
  "[": ["leftshift", "shift"],
  "]": ["rightshift"],
};
/** KeyboardEvent.code shortcuts (layout-independent). */
const CODEMAP = { Backquote: "on" };

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
  stage: $("stage"),
  speed: $("speed"),
  speedHint: $("speed-hint"),
  fullscreen: $("fullscreen"),
  barFullscreen: $("bar-fullscreen"),
  leaveFullscreen: $("leave-fullscreen"),
  panelHide: $("panel-hide"),
  panelShow: $("panel-show"),
  barMenu: $("bar-menu"),
};

let emu = null;
let modelName = null;
let romName = "";
/** The user's Run/Pause switch. */
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
/** Speed setting: "1", "2", "4" or "max". */
let speedSetting = "1";

// Key handling: `active` keys are down in the emulator; `pending` presses
// (or letters, expanded when their turn comes) wait for a fast-typed
// predecessor to be released first.
let active = [];
let pending = [];
let lastReleaseMs = -Infinity;
/**
 * Alpha is known to be off: a typed letter pressed alpha and its key used
 * it up, and no alpha press came since. Letters then skip reading the
 * annunciator and the settling wait.
 */
let alphaSpent = false;

// The run loop: an animation frame while the calculator computes, a wake
// timer while it sleeps in SHUTDN with nothing queued.
let frameId = 0;
let lastFrame = null;
let wakeTimer = 0;
/** Wall clock when the loop went to sleep, for catching up on wake. */
let sleptAt = 0;
let sleepSpanMs = 0;

const lcdOff = document.createElement("canvas");
lcdOff.width = W;
lcdOff.height = H;
const lcdOffCtx = lcdOff.getContext("2d");
const lcdImage = lcdOffCtx.createImageData(W, H);

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

/** The room inside the stage, in CSS pixels. */
function stageRoom() {
  const st = getComputedStyle(ui.stage);
  return {
    w: ui.stage.clientWidth - parseFloat(st.paddingLeft) - parseFloat(st.paddingRight),
    h: ui.stage.clientHeight - parseFloat(st.paddingTop) - parseFloat(st.paddingBottom),
  };
}

/**
 * Size the skin and the LCD canvas. The skin fills the stage's height (or
 * its width, on a narrow screen). The skin then shrinks by up to
 * `SNAP_LOSS` so each LCD pixel is a whole number of device pixels and
 * the display stays crisp; below two device pixels per LCD pixel it is
 * not snapped.
 */
function fitCanvas() {
  const dpr = window.devicePixelRatio || 1;
  let css;
  if (useSkin && skinData) {
    const s = skinData;
    const [lx, ly, lw] = s.lcd;
    const room = stageRoom();
    const availW = Math.max(200, room.w);
    const availH = Math.max(240, room.h);
    let f = Math.min(availW / s.width, availH / s.height);
    const unit = lw / W;
    const dev = f * unit * dpr;
    const snapped = Math.floor(dev);
    if (snapped >= 2 && snapped / dev >= 1 - SNAP_LOSS) f = snapped / (unit * dpr);
    css = f * unit;
    ui.skin.style.width = `${s.width * f}px`;
    const snap = (v) => Math.round(v * dpr) / dpr;
    ui.lcd.style.left = `${snap(lx * f)}px`;
    ui.lcd.style.top = `${snap(ly * f)}px`;
    ui.lcd.style.width = `${W * css}px`;
    ui.lcd.style.height = `${ROWS * css}px`;
  } else {
    const frame = ui.lcd.parentElement;
    const avail = Math.max(W * 2, frame.clientWidth - 16);
    css = Math.max(2, Math.min(4, Math.floor(avail / W)));
    ui.lcd.style.left = "";
    ui.lcd.style.top = "";
    ui.lcd.style.width = `${W * css}px`;
    ui.lcd.style.height = `${ROWS * css}px`;
  }
  ui.lcd.width = Math.max(W, Math.round(W * css * dpr));
  ui.lcd.height = Math.max(ROWS, Math.round(ROWS * css * dpr));
  draw();
}

function draw() {
  const ctx = ui.lcd.getContext("2d");
  const { bg, ink } = lcdColors();
  const d = darkness();
  const on = mix(bg, ink, d);
  const off = mix(bg, ink, 0.06 * d * d);
  const cw = ui.lcd.width;
  const ch = ui.lcd.height;
  const sx = cw / W;
  const sy = ch / ROWS;
  ctx.fillStyle = `rgb(${off})`;
  ctx.fillRect(0, 0, cw, ch);
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
  ctx.drawImage(lcdOff, 0, Math.round(ANN_H * sy), Math.round(W * sx), Math.round(H * sy));

  const ann = emu.annunciators();
  ctx.fillStyle = `rgb(${on})`;
  ctx.font = `${Math.round(6.5 * sy)}px ${getComputedStyle(document.body).fontFamily}`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  const slot = cw / ANNUNCIATORS.length;
  ANNUNCIATORS.forEach(([name, glyph], i) => {
    if (ann[name]) ctx.fillText(glyph, slot * (i + 0.5), (ANN_H / 2) * sy);
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
  if (g && g.classList.contains("down") !== down) {
    g.classList.toggle("down", down);
    g.querySelector(".cap")?.setAttribute("transform", down ? "translate(0 3)" : "");
    g.querySelector(".shadow")?.setAttribute("fill-opacity", down ? "0.16" : "0.38");
    const press = g.querySelector(".press");
    press?.setAttribute("fill-opacity", down ? "0.26" : "0");
    press?.setAttribute("stroke-opacity", down ? "0.26" : "0");
  }
}

function pressKey(name) {
  if (!emu || !keyNames.has(name)) return;
  pending.push({ name, up: false, downAt: 0 });
  wake();
  pumpKeys();
}

/** Type `ch` through the calculator's alpha mode, when its turn comes. */
function typeLetter(ch) {
  if (!emu || !skinData) return;
  const upper = ch.toUpperCase();
  if (!(upper in skinData.letters)) return;
  pending.push({ letter: upper, lower: ch !== upper });
  wake();
  pumpKeys();
}

/** Queue full presses of `names`, one after the other. */
function typeKeys(names) {
  if (!emu) return;
  for (const name of names) {
    if (keyNames.has(name)) pending.push({ name, up: true, downAt: 0, typed: true });
  }
  wake();
  pumpKeys();
}

/**
 * The key presses that type a queued letter now: the letter's key after
 * the alpha key, unless alpha is already on (one alpha press lasts for
 * the next key; two lock it on the 48 and 49G, cancel it on the others);
 * a lowercase letter adds the model's shift, before or after alpha as the
 * model wants. Alpha is read from the annunciator, except right after a
 * letter this page pressed alpha for: that alpha was spent by its key.
 */
function expandLetter(item) {
  const t = skinData.typing;
  const key = skinData.letters[item.letter];
  const alphaOn = alphaSpent ? false : Boolean(emu.annunciators().alpha);
  const seq = [];
  if (t.shiftFirst && item.lower) seq.push(t.lowerShift);
  if (!alphaOn) seq.push(t.alpha);
  if (!t.shiftFirst && item.lower) seq.push(t.lowerShift);
  seq.push(key);
  alphaSpent = !alphaOn;
  return seq.filter((n) => keyNames.has(n)).map((name) => ({ name, up: true, downAt: 0, typed: true }));
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
  const idle = emu.is_shutdown();
  while (pending.length > 0) {
    // Wait while a fast-typed key is still on its way up; a key the user
    // keeps holding (ON for a chord) does not block.
    if (active.some((k) => k.up)) break;
    const since = now - lastReleaseMs;
    if (since < GAP_MS || (!idle && since < BUSY_GAP_MS)) break;
    if (pending[0].letter) {
      // A letter waits for an empty keyboard, and, unless it follows a
      // letter whose alpha this page pressed, for the ROM to settle the
      // alpha annunciator it reads.
      if (active.length > 0) break;
      if (!alphaSpent && !idle && since < LETTER_SETTLE_MS) break;
      pending.splice(0, 1, ...expandLetter(pending[0]));
      continue;
    }
    const k = pending.shift();
    if (k.name === skinData?.typing.alpha && !k.typed) alphaSpent = false;
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

/** The script key a physical key maps to on this model, or null. */
function keyFor(e) {
  const any = KEYMAP_ANY[e.key];
  if (any) return any.find((n) => keyNames.has(n)) ?? null;
  const name = KEYMAP[e.key] ?? CODEMAP[e.code];
  if (name === "space" && !keyNames.has("space") && skinData?.letters[" "]) return null;
  return name && keyNames.has(name) ? name : null;
}

function onKeyDown(e) {
  if (!emu || e.ctrlKey || e.metaKey || e.altKey) return;
  const t = e.target;
  if (t instanceof HTMLSelectElement || t instanceof HTMLInputElement || t instanceof HTMLButtonElement) return;
  const name = keyFor(e);
  if (name) {
    e.preventDefault();
    if (!e.repeat) pressKey(name);
    return;
  }
  if (/^[a-z]$/i.test(e.key) || (e.key === " " && skinData?.letters[" "])) {
    e.preventDefault();
    if (!e.repeat) typeLetter(e.key);
  } else if (e.key === " " && skinData?.typing.space.length) {
    // No space key and none in alpha mode: the model's shifted space (38G).
    e.preventDefault();
    if (!e.repeat) typeKeys(skinData.typing.space);
  }
}

function onKeyUp(e) {
  if (!emu) return;
  const name = keyFor(e);
  if (!name) return;
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

/**
 * A rectangle path with top corners `rt` and bottom corners `rb`. With
 * `bow`, the bottom edge curves down by that much in the middle (the box
 * includes the curve).
 */
function roundedPath([x, y, w, h], rt, rb, bow = 0) {
  rt = Math.min(rt, w / 2, h / 2);
  rb = Math.min(rb, w / 2, h / 2);
  const b = y + h - bow;
  const bottom = bow
    ? `Q${x + w / 2} ${b + 2 * bow} ${x + rb} ${b}`
    : `H${x + rb}`;
  return `M${x + rt} ${y}H${x + w - rt}A${rt} ${rt} 0 0 1 ${x + w} ${y + rt}`
    + `V${b - rb}A${rb} ${rb} 0 0 1 ${x + w - rb} ${b}${bottom}`
    + `A${rb} ${rb} 0 0 1 ${x} ${b - rb}V${y + rt}A${rt} ${rt} 0 0 1 ${x + rt} ${y}Z`;
}

/** A key's rectangle grown by its well on every side. */
function wellRect(k) {
  const [x, y, w, h] = k.rect;
  const m = k.well || 0;
  return [x - m, y - m, w + 2 * m, h + 2 * m];
}

/**
 * Outline of a key: a rounded rectangle (corner radius `round` percent of
 * the shorter side, plus `grow` for the well around it), or a cursor-pad
 * trapezoid.
 */
function keyPath(shape, [x, y, w, h], round = 22, grow = 0) {
  if (shape === "key") {
    const r = (Math.min(w, h) - 2 * grow) * round / 100 + grow;
    return roundedPath([x, y, w, h], r, r);
  }
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

/**
 * Shared gradients for the relief: a light falling from the top onto every
 * key cap and the case, and a rim that is lit above and shaded below. Each
 * is defined once and referenced by every key (gradients stretch to the
 * element's own box).
 */
function drawDefs(root) {
  const defs = svg("defs", {}, root);
  const grad = (id, stops) => {
    const g = svg("linearGradient", { id, x1: 0, y1: 0, x2: 0, y2: 1 }, defs);
    for (const [offset, color, opacity] of stops) {
      svg("stop", { offset, "stop-color": color, "stop-opacity": opacity }, g);
    }
  };
  grad("cap-light", [[0, "#fff", 0.26], [0.5, "#fff", 0.05], [1, "#000", 0.16]]);
  grad("cap-rim", [[0, "#fff", 0.45], [1, "#000", 0.4]]);
  grad("case-light", [[0, "#fff", 0.08], [0.6, "#fff", 0.0], [1, "#000", 0.14]]);
  // A well is a recess: its upper lip is in shade, its lower lip catches light.
  grad("well-rim", [[0, "#000", 0.55], [0.55, "#000", 0.0], [1, "#fff", 0.2]]);
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
  const [x, y, w] = wellRect(k);
  // Wells leave a little more air between the key and its labels.
  const base = y - (k.well ? 8 : 5);
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
  const [x, y, w, h] = wellRect(k);
  const size = s.small * 0.95;
  if (s.alphaStyle === "below") {
    svg("text", { x: x + w + 6, y: y + h + size * 0.95, "font-size": size, fill: s.alphaInk, "text-anchor": "end", "font-style": "italic" }, layer, k.alpha);
  } else if (s.alphaStyle === "corner") {
    svg("text", { x: x + w + 4, y: y + h + size * 0.55, "font-size": size, fill: s.alphaInk }, layer, k.alpha);
  } else {
    svg("text", { x: x + w + 5, y: y + h + 1, "font-size": size, fill: s.alphaInk }, layer, k.alpha);
  }
}

/**
 * One key: its well (a dark recess, or the outline, around the cap), a
 * soft shadow under the cap, the cap in its colour with the shared light
 * and rim over it, a press tint, the label. Pressing moves the cap down
 * onto its shadow (see `showDown`).
 */
function drawKey(keys, s, k) {
  const [x, y, w, h] = k.rect;
  const g = svg("g", { class: "skey", "data-key": k.name }, keys);
  const title = k.alpha ? `${k.name} (alpha ${k.alpha})` : k.name;
  svg("title", {}, g, title);
  // Hit area a little larger than the key and its well, as the grid's buttons are.
  const [wx, wy, ww, wh] = wellRect(k);
  const pad = k.well ? 3 : 6;
  svg("rect", { x: wx - pad, y: wy - pad, width: ww + 2 * pad, height: wh + 2 * pad, fill: "#000", "fill-opacity": 0 }, g);
  const d = keyPath(k.shape, k.rect, s.round);
  const rounded = k.shape === "key";
  if (k.well && rounded) {
    const well = keyPath(k.shape, [wx, wy, ww, wh], s.round, k.well);
    svg("path", { d: well, class: "well", fill: s.wellFill, stroke: "url(#well-rim)", "stroke-width": 1.5 }, g);
  } else if (k.well) {
    svg("path", { d, class: "well", fill: s.wellFill, stroke: s.wellFill, "stroke-width": 10 + 2 * k.well, "stroke-linejoin": "round" }, g);
  }
  svg("path", { d, class: "shadow", fill: "#000", "fill-opacity": 0.38, transform: "translate(0 4)", ...keyStroke(k.shape, "#000") }, g);
  const cap = svg("g", { class: "cap" }, g);
  svg("path", { d, fill: k.fill, ...keyStroke(k.shape, k.fill) }, cap);
  if (rounded) {
    svg("path", { d, fill: "url(#cap-light)", stroke: "url(#cap-rim)", "stroke-width": 2 }, cap);
  } else {
    svg("path", { d, fill: "url(#cap-light)", stroke: "url(#cap-light)", "stroke-width": 10, "stroke-linejoin": "round" }, cap);
  }
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
    // Caps in wells are smaller than their keys, and print larger on them.
    let size = single ? h * (badge ? 0.5 : k.well ? 0.68 : 0.56) : h * (badge ? 0.38 : k.well ? 0.5 : 0.4);
    if (!rounded) size = Math.min(w, h) * 0.42;
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
  drawDefs(root);
  s.panels.forEach((p, i) => {
    const d = roundedPath(p.rect, p.radius, p.bottomRadius, p.bow);
    svg("path", { d, fill: p.fill }, root);
    // The case itself gets the light; the panel inside it a lit top edge.
    if (i === 0) svg("path", { d, fill: "url(#case-light)" }, root);
    if (i === 1) svg("path", { d, fill: "none", stroke: "#fff", "stroke-opacity": 0.07, "stroke-width": 2 }, root);
  });
  const [lx, ly, lw, lh] = s.lcd;
  // The window: the glass a little larger than the pixels, with a shaded rim.
  svg("rect", { x: lx - 8, y: ly - 8, width: lw + 16, height: lh + 16, rx: 6, fill: "#000", "fill-opacity": 0.35 }, root);
  skinWindow = svg("rect", { x: lx - 6, y: ly - 6, width: lw + 12, height: lh + 12, rx: 5, fill: s.lcdFill }, root);
  const [gx, gy, gw, gh] = s.logo;
  svg("image", { href: "logo.svg", x: gx, y: gy, width: gw, height: gh }, root);
  const print = svg("g", { class: "print" }, root);
  for (const m of s.marks) {
    const mark = svg("text", { x: m.x, y: m.y, "font-size": m.size, fill: m.fill, "text-anchor": "middle" }, print, m.text);
    if (m.italic) mark.setAttribute("font-style", "italic");
  }
  for (const l of s.lines) {
    svg("line", { x1: l.x1, y1: l.y1, x2: l.x2, y2: l.y2, stroke: l.stroke, "stroke-width": 2 }, print);
  }
  for (const k of s.keys) {
    drawShiftLabels(print, s, k);
    drawAlpha(print, s, k);
    if (k.below) {
      const [x, y, w, h] = wellRect(k);
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

/** Emulated ms per wall ms while asleep, for the wake timer. */
function rate() {
  return speedSetting === "max" ? MAX_RATE : Number(speedSetting);
}

/** Run `ms` of emulated time in slices, feeding the key queue. */
function runSlices(ms, budgetMs) {
  const start = performance.now();
  let left = ms;
  while (left > 0) {
    const step = Math.min(left, SLICE_MS);
    emu.run_ms(step);
    left -= step;
    pumpKeys();
    if (budgetMs !== undefined && performance.now() - start > budgetMs) break;
  }
}

function frame(t) {
  frameId = 0;
  const wall = lastFrame === null ? 0 : Math.min(t - lastFrame, MAX_FRAME_MS);
  lastFrame = t;
  if (!emu || !running) return;
  try {
    if (speedSetting === "max") {
      runSlices(MAX_EMULATED_PER_FRAME_MS, MAX_BUDGET_MS);
    } else {
      runSlices(wall * Number(speedSetting), MAX_BUDGET_MS * 2);
    }
  } catch (err) {
    haltMessage = String(err);
    setRunning(false);
    draw();
    return;
  }
  draw();
  scheduleNext();
}

/**
 * Keep animating while the calculator computes or keys are queued; once
 * it sleeps in SHUTDN with nothing to do, stop and set a timer for its
 * next timer event instead, so an idle page costs nothing.
 */
function scheduleNext() {
  if (!emu || !running) return;
  const idle = emu.idle_ms();
  if (idle < 0 || pending.length > 0 || active.length > 0) {
    if (!frameId) frameId = requestAnimationFrame(frame);
    return;
  }
  clearTimeout(wakeTimer);
  sleptAt = performance.now();
  sleepSpanMs = idle;
  lastFrame = null;
  wakeTimer = setTimeout(wake, Math.min(idle / rate(), 2 ** 31 - 1) + 1);
}

/**
 * Leave the sleep: run the emulated time that passed meanwhile (cheap
 * while the CPU sleeps; stops early once it wakes), then animate again.
 */
function wake() {
  if (!emu || !running || !wakeTimer) return;
  clearTimeout(wakeTimer);
  wakeTimer = 0;
  const slept = Math.min((performance.now() - sleptAt) * rate(), sleepSpanMs + 1);
  try {
    let left = slept;
    while (left > 0 && emu.is_shutdown()) {
      const step = Math.min(left, 1000);
      emu.run_ms(step);
      left -= step;
    }
  } catch (err) {
    haltMessage = String(err);
    setRunning(false);
  }
  lastFrame = null;
  // Still asleep (a timer event the ROM did not wake for): the display
  // cannot have changed, so sleep on without a frame.
  scheduleNext();
}

function stopLoop() {
  if (frameId) cancelAnimationFrame(frameId);
  frameId = 0;
  clearTimeout(wakeTimer);
  wakeTimer = 0;
  lastFrame = null;
}

function setRunning(on) {
  running = on && emu !== null && haltMessage === null;
  ui.run.textContent = running ? "Pause" : "Run";
  stopLoop();
  if (running) frameId = requestAnimationFrame(frame);
  updateStatus();
}

function updateStatus() {
  let text;
  if (!emu) {
    text = message || "Pick a model and a ROM file to start.";
  } else {
    const parts = [MODEL_TITLES[modelName] ?? modelName, romName];
    if (haltMessage) parts.push(haltMessage);
    else if (!running) parts.push("paused");
    if (message) parts.push(message);
    text = parts.filter(Boolean).join(" · ");
  }
  if (ui.status.textContent !== text) ui.status.textContent = text;
  ui.status.classList.toggle("error", Boolean(haltMessage) || (!emu && message.startsWith("Cannot")));
}

function setMessage(text, isError = false) {
  message = text;
  updateStatus();
  if (isError) ui.status.classList.add("error");
}

function setSpeed(value) {
  speedSetting = SPEEDS.includes(value) ? value : "1";
  prefSet(PREF_SPEED, speedSetting);
  for (const b of ui.speed.querySelectorAll("button")) {
    b.setAttribute("aria-checked", String(b.dataset.speed === speedSetting));
  }
  ui.speedHint.textContent = {
    "1": "Real time.",
    "2": "Twice real time; the calculator's clock runs twice as fast.",
    "4": "Four times real time; the calculator's clock runs four times as fast.",
    max: "As fast as this device can; the calculator's clock runs fast.",
  }[speedSetting];
  // A sleeping calculator wakes on the new schedule.
  if (running && wakeTimer) {
    wake();
  }
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
  stopLoop();
  try {
    const next = new Emulator(model, bytes);
    if (emu) emu.free();
    emu = next;
  } catch (err) {
    setMessage(`Cannot start: ${err}`, true);
    return;
  }
  modelName = model;
  romName = file.name;
  prefSet(PREF_MODEL, model);
  haltMessage = null;
  message = "";
  releaseAll();
  lastReleaseMs = -Infinity;
  alphaSpent = false;
  buildKeyboard();
  if (useSkin && skinModel !== model) renderSkin(model);
  for (const b of [ui.run, ui.reset, ui.save]) b.disabled = false;
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
    setMessage(`state saved ${new Date().toLocaleTimeString()}`);
    ui.load.disabled = false;
  } catch (err) {
    setMessage(`save failed: ${err}`, true);
  }
}

async function loadState() {
  if (!emu) return;
  try {
    const rec = await dbGet(modelName);
    if (!rec) {
      setMessage("no saved state for this model");
      return;
    }
    releaseAll();
    emu.load_state(rec.state);
    haltMessage = null;
    lastReleaseMs = -Infinity;
    alphaSpent = false;
    setMessage(`state from ${new Date(rec.saved).toLocaleString()} loaded`);
    draw();
    setRunning(running);
  } catch (err) {
    setMessage(`load failed: ${err}`, true);
  }
}

function blurAfter(fn) {
  return (e) => {
    e.currentTarget.blur();
    fn();
  };
}

// ---------------------------------------------------------------- chrome

function setPanelHidden(hidden) {
  document.body.classList.toggle("panel-hidden", hidden);
  ui.panelShow.hidden = !hidden;
  prefSet(PREF_PANEL, hidden ? "hidden" : "shown");
  fitCanvas();
  fitTexts();
}

function setSheetOpen(open) {
  document.body.classList.toggle("sheet-open", open);
  ui.barMenu.setAttribute("aria-expanded", String(open));
}

async function enterFullscreen() {
  if (document.fullscreenElement) return;
  try {
    await ui.stage.requestFullscreen({ navigationUI: "hide" });
  } catch (err) {
    setMessage(`fullscreen refused: ${err.message ?? err}`);
  }
}

async function onFullscreenChange() {
  const on = document.fullscreenElement === ui.stage;
  ui.fullscreen.textContent = on ? "Leave fullscreen" : "Fullscreen";
  // Keep Escape for the ON key where the browser allows it (Chromium's
  // keyboard lock; a held Escape still leaves fullscreen).
  try {
    if (on && navigator.keyboard?.lock) await navigator.keyboard.lock(["Escape"]);
    else if (!on) navigator.keyboard?.unlock?.();
  } catch { /* not granted */ }
  fitCanvas();
  fitTexts();
}

async function main() {
  await init();
  fillModels();
  ui.model.addEventListener("change", () => {
    prefSet(PREF_MODEL, ui.model.value);
    if (emu && ui.model.value !== modelName) setMessage("pick a ROM for the new model");
    if (!emu && useSkin) renderSkin(ui.model.value);
  });
  ui.rom.addEventListener("change", () => {
    const f = ui.rom.files?.[0];
    if (f) startWithRom(f);
    ui.rom.blur();
    setSheetOpen(false);
  });
  ui.run.addEventListener("click", blurAfter(() => setRunning(!running)));
  ui.reset.addEventListener("click", blurAfter(() => {
    if (!emu) return;
    releaseAll();
    emu.reset();
    alphaSpent = false;
    haltMessage = null;
    setMessage("");
    setRunning(true);
  }));
  ui.save.addEventListener("click", blurAfter(saveState));
  ui.load.addEventListener("click", blurAfter(loadState));
  ui.speed.addEventListener("click", (e) => {
    const b = e.target.closest("button[data-speed]");
    if (!b) return;
    b.blur();
    setSpeed(b.dataset.speed);
  });
  for (const b of [ui.fullscreen, ui.barFullscreen]) {
    b.addEventListener("click", blurAfter(() => {
      setSheetOpen(false);
      if (document.fullscreenElement) document.exitFullscreen();
      else enterFullscreen();
    }));
  }
  ui.leaveFullscreen.addEventListener("click", blurAfter(() => document.exitFullscreen()));
  document.addEventListener("fullscreenchange", onFullscreenChange);
  ui.panelHide.addEventListener("click", blurAfter(() => setPanelHidden(true)));
  ui.panelShow.addEventListener("click", blurAfter(() => setPanelHidden(false)));
  ui.barMenu.addEventListener("click", blurAfter(() => setSheetOpen(!document.body.classList.contains("sheet-open"))));
  ui.stage.addEventListener("pointerdown", () => setSheetOpen(false));
  document.addEventListener("keydown", onKeyDown);
  document.addEventListener("keyup", onKeyUp);
  window.addEventListener("blur", () => {
    for (const k of active.concat(pending)) k.up = true;
    pumpKeys();
  });
  window.addEventListener("resize", () => {
    fitCanvas();
    fitTexts();
  });
  ui.viewSkin.addEventListener("change", () => {
    ui.viewSkin.blur();
    setView(ui.viewSkin.checked);
  });
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", draw);
  setSpeed(prefGet(PREF_SPEED) ?? "1");
  setPanelHidden(prefGet(PREF_PANEL) === "hidden");
  setView(prefGet(PREF_VIEW) !== "grid");
  if (!document.fullscreenEnabled) {
    ui.fullscreen.disabled = true;
    ui.barFullscreen.disabled = true;
  }
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
    startWithRom,
    /** Whether the page is animating, sleeping on a timer, or stopped. */
    get loop() { return frameId ? "frame" : wakeTimer ? "sleep" : "stopped"; },
    get speed() { return speedSetting; },
    setSpeed,
    /** The drawn key group of `name`, for automated checks. */
    skinKey(name) { return skinKeys.get(name) ?? null; },
  };
}

main().catch((err) => setMessage(`Failed to start: ${err}`, true));
