// <sat-calculator>: the calculator itself. Draws the selected model's skin
// and the LCD from the store's frames, and turns pointer presses and the
// computer keyboard into backend commands. With no ROM running for the
// model shown, the LCD holds an empty state with a "Choose ROM…" button
// (event `sat-choose-rom`), which a key press makes pulse. Light
// DOM (display: contents), so the page's stylesheet applies.

import { contrastDarkness, offTint } from "../contrast.js";
import { isLive, keyAction, noRomText } from "../norom.js";

const ANN_H = 8;
const SVG_NS = "http://www.w3.org/2000/svg";
/** The skin may shrink this much so the LCD lands on whole device pixels. */
const SNAP_LOSS = 0.08;

export const MODEL_TITLES = {
  "48sx": "HP 48SX",
  "48gx": "HP 48GX",
  "38g": "HP 38G",
  "49g": "HP 49G",
  "39g": "HP 39G",
  "40g": "HP 40G",
  "42s": "HP 42S",
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
};
/** Shortcuts that depend on the model's keys: the first name present wins. */
const KEYMAP_ANY = {
  "[": ["leftshift", "shift"],
  "]": ["rightshift"],
  // The menu keys; the 42S's top row keeps its own names.
  F1: ["a", "sigmaplus"], F2: ["b", "inv"], F3: ["c", "sqrt"],
  F4: ["d", "log"], F5: ["e", "ln"], F6: ["f", "xeq"],
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

/** The 42S's seven annunciators, in the order of its LCD. */
const ANNUNCIATORS_42S = [
  ["updown", "▲▼"],
  ["leftshift", "⇧"],
  ["transmit", "((•))"],
  ["busy", "⌛"],
  ["battery", "BAT"],
  ["g", "G"],
  ["rad", "RAD"],
];

const TEMPLATE = `
  <section class="skin" aria-label="Calculator">
    <svg xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Calculator keyboard"></svg>
    <canvas width="131" height="72" aria-label="Calculator display"></canvas>
  </section>
    <div class="no-rom" hidden>
      <p></p>
      <button type="button">Choose ROM…</button>
    </div>
  </section>`;

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

function mix(a, b, t) {
  return [0, 1, 2].map((i) => Math.round(a[i] + (b[i] - a[i]) * t));
}

/** A frame's pixels as one byte per pixel, 1 = dark. */
export function decodeFrame(frame) {
  const { width: w, height: h } = frame;
  const bin = atob(frame.pixels);
  const row = Math.ceil(w / 8);
  const px = new Uint8Array(w * h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      px[y * w + x] = (bin.charCodeAt(y * row + (x >> 3)) >> (7 - (x & 7))) & 1;
    }
  }
  return px;
}

export class SatCalculator extends HTMLElement {
  constructor() {
    super();
    this.backend = null;
    this.store = null;
    /** Drawn skin: the model's skin data, its key groups and display window. */
    this.skinData = null;
    this.skinModel = null;
    this.skinKeys = new Map();
    this.skinWindow = null;
    this.skinWindowFill = "";
    /** Text elements to squeeze into a width once the skin is laid out. */
    this.skinFits = [];
    /** The drawn model's key names and typing rules (letter map, shifts). */
    this.keyNames = new Set();
    this.typing = null;
    this.down = new Set();
    /** The last frame, decoded. */
    this.pixels = null;
    this.drawPending = 0;
    /** Bumped on every render request, so a late skin fetch is dropped. */
    this.renderSeq = 0;
    /** LCD rows of the drawn model while no ROM runs (64, 16 on the 42S). */
    this.idleRows = 64;
    /** Calculator keys held down from the computer keyboard. */
    this.keyboardDown = new Set();
  }

  /** Attach the backend and the store; renders and starts listening. */
  attach(backend, store) {
    this.backend = backend;
    this.store = store;
    this.innerHTML = TEMPLATE;
    this.ui = {
      skin: this.querySelector(".skin"),
      skinSvg: this.querySelector(".skin svg"),
      lcd: this.querySelector("canvas"),
      noRom: this.querySelector(".no-rom"),
    };
    this.ui.noRom.querySelector("button").addEventListener("click", (e) => {
      e.currentTarget.blur();
      this.dispatchEvent(new CustomEvent("sat-choose-rom", { bubbles: true, detail: this.shownModel() }));
    });
    this.ui.lcd.id = "lcd";
    const off = document.createElement("canvas");
    this.lcdOff = off;

    store.watch(["frame"], (s) => {
      this.pixels = s.frame ? decodeFrame(s.frame) : null;
      if (s.frame && s.frame.height !== this.lcdHeight()) this.fit();
      this.scheduleDraw();
    });
    store.watch(["keysDown"], (s) => this.showKeys(s.keysDown));
    // The skin follows the selected model at once, ROM or not.
    store.watch(["booted", "model"], () => this.onModel());

    store.watch(["busy"], (s) => this.classList.toggle("typing", s.busy));
    document.addEventListener("paste", (e) => this.onPaste(e));
    document.addEventListener("keydown", (e) => this.onKeyDown(e));
    document.addEventListener("keyup", (e) => this.onKeyUp(e));
    window.addEventListener("blur", () => {
      this.keyboardDown.clear();
      this.backend.keyUpAll();
    });
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => this.draw());
    new ResizeObserver(() => {
      this.fit();
      this.fitTexts();
    }).observe(this.parentElement ?? this);
    this.onModel();
  }

  /** The model drawn: the selected one (the running one once booted). */
  shownModel() {
    return this.store.state.model ?? this.store.state.booted;
  }

  /** Draw the shown model and show or hide the empty state. */
  onModel() {
    const model = this.shownModel();
    if (model && this.skinModel !== model) this.renderSkin(model);
    const empty = !isLive(this.store.state);
    this.ui.noRom.hidden = !empty;
    if (empty) {
      this.ui.noRom.querySelector("p").textContent = noRomText(model, MODEL_TITLES[model] ?? model);
    }
    this.fit();
  }

  /** A key pressed without a ROM: the empty state pulses. */
  pulseNoRom() {
    const n = this.ui.noRom;
    n.classList.remove("pulse");
    void n.offsetWidth; // restart the animation
    n.classList.add("pulse");
  }

  lcdWidth() {
    return this.store.state.frame?.width ?? 131;
  }

  /** LCD rows: the running machine's, else the drawn model's. */
  lcdHeight() {
    const f = this.store.state.frame;
    return isLive(this.store.state) && f ? f.height : this.idleRows;
  }

  // ---------------------------------------------------------- display

  lcdColors() {
    // The drawn calculator keeps its real LCD colours in both themes.
    return { bg: [183, 194, 162], ink: [16, 20, 12] };
  }

  /** Pixel darkness from the contrast register (contrast.js). */
  darkness() {
    const f = this.store.state.frame;
    if (!f) return 1;
    return contrastDarkness(f.contrast, f.contrastRange, f.contrastDefault);
  }

  /** The room inside the stage, in CSS pixels. */
  stageRoom() {
    const stage = this.parentElement;
    const st = getComputedStyle(stage);
    return {
      w: stage.clientWidth - parseFloat(st.paddingLeft) - parseFloat(st.paddingRight),
      h: stage.clientHeight - parseFloat(st.paddingTop) - parseFloat(st.paddingBottom),
    };
  }

  /**
   * Size the skin and the LCD canvas. The skin fills the stage's height
   * (or its width, on a narrow screen). The skin then shrinks by up to
   * `SNAP_LOSS` so each LCD pixel is a whole number of device pixels and
   * the display stays crisp; below two device pixels per LCD pixel it is
   * not snapped.
   */
  fit() {
    if (!this.ui || !this.skinData) return;
    const W = this.lcdWidth();
    const ROWS = this.lcdHeight() + ANN_H;
    const lcd = this.ui.lcd;
    const dpr = window.devicePixelRatio || 1;
    const s = this.skinData;
    const [lx, ly, lw] = s.lcd;
    const room = this.stageRoom();
    const availW = Math.max(200, room.w);
    const availH = Math.max(240, room.h);
    let f = Math.min(availW / s.width, availH / s.height);
    const unit = lw / W;
    const dev = f * unit * dpr;
    const snapped = Math.floor(dev);
    if (snapped >= 2 && snapped / dev >= 1 - SNAP_LOSS) f = snapped / (unit * dpr);
    const css = f * unit;
    this.ui.skin.style.width = `${s.width * f}px`;
    const snap = (v) => Math.round(v * dpr) / dpr;
    lcd.style.left = `${snap(lx * f)}px`;
    lcd.style.top = `${snap(ly * f)}px`;
    lcd.style.width = `${W * css}px`;
    lcd.style.height = `${ROWS * css}px`;
    for (const k of ["left", "top", "width", "height"]) this.ui.noRom.style[k] = lcd.style[k];
    lcd.width = Math.max(W, Math.round(W * css * dpr));
    lcd.height = Math.max(ROWS, Math.round(ROWS * css * dpr));
    this.draw();
  }

  scheduleDraw() {
    if (this.drawPending) return;
    this.drawPending = requestAnimationFrame(() => {
      this.drawPending = 0;
      this.draw();
    });
  }

  draw() {
    if (!this.ui) return;
    const lcd = this.ui.lcd;
    const W = this.lcdWidth();
    const H = this.lcdHeight();
    const ROWS = H + ANN_H;
    const ctx = lcd.getContext("2d");
    const { bg, ink } = this.lcdColors();
    const d = this.darkness();
    const on = mix(bg, ink, d);
    const off = mix(bg, ink, offTint(d));
    const cw = lcd.width;
    const ch = lcd.height;
    const sx = cw / W;
    const sy = ch / ROWS;
    ctx.fillStyle = `rgb(${off})`;
    ctx.fillRect(0, 0, cw, ch);
    if (this.skinWindow && this.skinWindowFill !== ctx.fillStyle) {
      this.skinWindowFill = ctx.fillStyle;
      this.skinWindow.setAttribute("fill", this.skinWindowFill);
    }
    const frame = this.store.state.frame;
    if (!frame || !this.pixels || !isLive(this.store.state)) return;

    const offCanvas = this.lcdOff;
    if (offCanvas.width !== W || offCanvas.height !== H) {
      offCanvas.width = W;
      offCanvas.height = H;
      this.lcdImage = null;
    }
    const offCtx = offCanvas.getContext("2d");
    this.lcdImage ??= offCtx.createImageData(W, H);
    const px = this.lcdImage.data;
    const fb = this.pixels;
    for (let i = 0; i < W * H; i++) {
      const c = fb[i] ? on : off;
      px[i * 4] = c[0];
      px[i * 4 + 1] = c[1];
      px[i * 4 + 2] = c[2];
      px[i * 4 + 3] = 255;
    }
    offCtx.putImageData(this.lcdImage, 0, 0);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(offCanvas, 0, Math.round(ANN_H * sy), Math.round(W * sx), Math.round(H * sy));

    const ann = frame.annunciators;
    ctx.fillStyle = `rgb(${on})`;
    ctx.font = `${Math.round(6.5 * sy)}px ${getComputedStyle(document.body).fontFamily}`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    const glyphs = this.store.state.booted === "42s" ? ANNUNCIATORS_42S : ANNUNCIATORS;
    const slot = cw / glyphs.length;
    glyphs.forEach(([name, glyph], i) => {
      if (ann[name]) ctx.fillText(glyph, slot * (i + 0.5), (ANN_H / 2) * sy);
    });
  }

  /** The display as text, `#` dark and `.` light, for automated checks. */
  screenText() {
    const f = this.store.state.frame;
    if (!f || !this.pixels) return "";
    let s = "";
    for (let y = 0; y < f.height; y++) {
      for (let x = 0; x < f.width; x++) s += this.pixels[y * f.width + x] ? "#" : ".";
      s += "\n";
    }
    return s;
  }

  // ---------------------------------------------------------- keys

  showDown(name, down) {
    const g = this.skinKeys.get(name);
    if (g && g.classList.contains("down") !== down) {
      g.classList.toggle("down", down);
      g.querySelector(".cap")?.setAttribute("transform", down ? "translate(0 3)" : "");
      g.querySelector(".shadow")?.setAttribute("fill-opacity", down ? "0.16" : "0.38");
      const press = g.querySelector(".press");
      press?.setAttribute("fill-opacity", down ? "0.26" : "0");
      press?.setAttribute("stroke-opacity", down ? "0.26" : "0");
    }
  }

  /** Draw the keys of `names` down and every other key up. */
  showKeys(names, all = false) {
    const next = new Set(names);
    for (const n of all ? new Set(this.skinKeys.keys()) : this.down) {
      if (!next.has(n)) this.showDown(n, false);
    }
    for (const n of next) this.showDown(n, true);
    this.down = next;
  }

  pressKey(name) {
    const action = keyAction(this.store.state, name, this.keyNames);
    if (action === "pulse") this.pulseNoRom();
    if (action !== "press") return false;
    this.backend.keyDown(name);
    return true;
  }

  releaseKey(name) {
    if (!isLive(this.store.state)) return;
    this.backend.keyUp(name);
  }

  /** The script key a physical key maps to on this model, or null. */
  keyFor(e) {
    const any = KEYMAP_ANY[e.key];
    if (any) return any.find((n) => this.keyNames.has(n)) ?? null;
    const name = KEYMAP[e.key] ?? CODEMAP[e.code];
    if (name === "space" && !this.keyNames.has("space") && this.typing?.letters[" "]) return null;
    return name && this.keyNames.has(name) ? name : null;
  }

  onKeyDown(e) {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const t = e.target;
    if (t instanceof HTMLSelectElement || t instanceof HTMLInputElement || t instanceof HTMLButtonElement) return;
    // A dialog and the memory view keep the keys while the focus is inside
    // them (the event's path, as a handler there may have redrawn its target).
    if (e.composedPath().some((n) => n instanceof Element && n.matches("dialog[open], sat-explorer"))) return;
    const name = this.keyFor(e);
    if (name) {
      e.preventDefault();
      if (!e.repeat && this.pressKey(name)) this.keyboardDown.add(name);
      return;
    }
    if (!isLive(this.store.state)) {
      if (/^[a-z]$/i.test(e.key)) {
        e.preventDefault();
        if (!e.repeat) this.pulseNoRom();
      }
      return;
    }
    const typing = this.typing;
    if (/^[a-z]$/i.test(e.key) || (e.key === " " && typing?.letters[" "])) {
      e.preventDefault();
      if (!e.repeat && typing && e.key.toUpperCase() in typing.letters) this.backend.typeLetter(e.key);
    } else if (e.key === " " && typing?.space.length) {
      // No space key and none in alpha mode: the model's shifted space (38G).
      e.preventDefault();
      if (!e.repeat) this.backend.typeKeys(typing.space.filter((n) => this.keyNames.has(n)));
    }
  }

  /**
   * Pasting with the calculator focused (no text field, no dialog) types
   * the clipboard's text into the command line (`insert`).
   */
  onPaste(e) {
    if (!isLive(this.store.state)) return;
    const t = e.target;
    if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement) return;
    if (t instanceof HTMLElement && t.isContentEditable) return;
    if (t instanceof Element && t.closest("dialog[open]")) return;
    const text = e.clipboardData?.getData("text/plain");
    if (!text) return;
    e.preventDefault();
    // The calculator's newline is one character.
    this.backend.insert(text.replace(/\r\n?/g, "\n")).catch((err) => {
      this.store.set({ message: `paste: ${err?.message ?? err}`, messageError: true });
    });
  }

  onKeyUp(e) {
    if (!isLive(this.store.state)) return;
    const name = this.keyFor(e);
    // Only a key this handler pressed: the release of a key typed into
    // the memory view or a dialog is not the calculator's.
    if (!name || !this.keyboardDown.delete(name)) return;
    e.preventDefault();
    this.releaseKey(name);
  }

  // ---------------------------------------------------------- skin

  /** Remember `t` to squeeze to `max` units wide once it can be measured. */
  fitLater(t, max) {
    this.skinFits.push([t, max]);
  }

  /** Squeeze texts wider than their room (needs the skin on screen). */
  fitTexts() {
    for (const [t, max] of this.skinFits) {
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
  drawShiftLabels(layer, s, k) {
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
      this.fitLater(a, (room - 6) * (la / (la + lb)));
      this.fitLater(b, (room - 6) * (lb / (la + lb)));
    } else if (k.left || k.right) {
      const t = svg("text", {
        x: x + w / 2, y: base, "font-size": size, "text-anchor": "middle",
        fill: k.left ? s.leftInk : s.rightInk,
      }, layer, k.left || k.right);
      this.fitLater(t, w + 34);
    }
  }

  drawAlpha(layer, s, k) {
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
  drawKey(keys, s, k) {
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
      this.fitLater(t, room);
    }
    g.addEventListener("pointerdown", (e) => {
      e.preventDefault();
      try { g.setPointerCapture(e.pointerId); } catch { /* not capturable */ }
      this.pressKey(k.name);
    });
    const up = () => this.releaseKey(k.name);
    g.addEventListener("pointerup", up);
    g.addEventListener("pointercancel", up);
    this.skinKeys.set(k.name, g);
  }

  /** Draw the skin of `model` into the SVG. */
  async renderSkin(model) {
    if (!model) return;
    const seq = ++this.renderSeq;
    let s;
    try {
      s = await this.backend.skin(model);
    } catch (err) {
      if (seq !== this.renderSeq) return;
      this.skinData = null;
      this.store.set({ message: String(err), messageError: true });
      return;
    }
    if (seq !== this.renderSeq) return;
    this.skinKeys.clear();
    this.skinFits = [];
    this.skinWindow = null;
    this.skinWindowFill = "";
    this.skinData = s;
    this.skinModel = model;
    // Without a running ROM the canvas takes the drawn model's rows.
    this.idleRows = s.lcdRows;
    this.keyNames = new Set(s.keys.map((k) => k.name));
    // The 42S has no typing data (`typing: null`): it types letters from
    // its ALPHA menus, so computer-keyboard letters are not mapped there.
    this.typing = s.typing ? { ...s.typing, letters: s.letters } : null;
    const root = this.ui.skinSvg;
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
    this.skinWindow = svg("rect", { x: lx - 6, y: ly - 6, width: lw + 12, height: lh + 12, rx: 5, fill: s.lcdFill }, root);
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
      this.drawShiftLabels(print, s, k);
      this.drawAlpha(print, s, k);
      if (k.below) {
        const [x, y, w, h] = wellRect(k);
        svg("text", { x: x + w / 2, y: y + h + s.small + 4, "font-size": s.small, fill: s.belowInk, "text-anchor": "middle" }, print, k.below);
      }
    }
    const keys = svg("g", { class: "keys" }, root);
    for (const k of s.keys) this.drawKey(keys, s, k);
    this.showKeys(this.store.state.keysDown, true);
    this.fit();
    this.fitTexts();
  }

  /** The drawn key group of `name`, for automated checks. */
  skinKey(name) {
    return this.skinKeys.get(name) ?? null;
  }
}

customElements.define("sat-calculator", SatCalculator);
