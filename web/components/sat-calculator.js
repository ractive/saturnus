// <sat-calculator>: the calculator itself. Draws the selected model's skin
// and the LCD from the store's frames, and turns pointer presses and the
// computer keyboard into backend commands. With no ROM running for the
// model shown, the LCD holds an empty state with a "Choose ROM…" button
// (event `sat-choose-rom`), which a key press makes pulse. Light
// DOM (display: contents), so the page's stylesheet applies.

import { contrastDarkness, offTint } from "../contrast.js";
import { action } from "../bindings.js";
import { edgeLayout } from "../edge.js";
import { getRomLink, isLive, keyAction, noRomText } from "../norom.js";
import { ContextClicks, ModifierGlow, clickHints, glowSide, modifierOf, shiftFor } from "../shiftclick.js";
import { ANN_H, LCD_BG, LCD_INK, NO_SCREEN, copyPng, hasScreen, pngBlob, screenFileName, screenRgba } from "../screenshot.js";
import { closeMenu, openMenu, openMenuKey } from "./menu.js";

const SVG_NS = "http://www.w3.org/2000/svg";
/** The skin may shrink this much so the LCD lands on whole device pixels. */
const SNAP_LOSS = 0.08;
/** Edge to edge the screen's width counts more: it may shrink only this much. */
const SNAP_LOSS_EDGE = 0.03;
/**
 * Where fullscreen crops the case to the face: a finger's screen (the
 * page's touch sizing, style.css), where the keys want all the width.
 * With a mouse the whole calculator is shown, scaled to the screen.
 */
const CROP = "(pointer: coarse)";

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
};
/** Shortcuts that depend on the model's keys: the first name present wins. */
const KEYMAP_ANY = {
  // The menu keys; the 42S's top row keeps its own names.
  F1: ["a", "sigmaplus"], F2: ["b", "inv"], F3: ["c", "sqrt"],
  F4: ["d", "log"], F5: ["e", "ln"], F6: ["f", "xeq"],
};
/**
 * The calculator keys of the rebindable actions (web/bindings.js): ON,
 * alpha and the shifts, the first name the model has.
 */
/**
 * Whether an open dialog is modal. Engines without `:modal` (older
 * WebKitGTK, the desktop app on Linux) throw on the selector: there an open
 * dialog counts as modal, as every dialog of this page is opened with
 * `showModal`.
 */
function isModal(dialog) {
  try {
    return dialog.matches(":modal");
  } catch {
    return dialog.open;
  }
}

const BOUND_KEYS = {
  on: ["on"], alpha: ["alpha"], leftshift: ["leftshift", "shift"], rightshift: ["rightshift"],
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
    <div class="glass"></div>
    <div class="no-rom" hidden>
      <p class="no-rom-text"></p>
      <p class="no-rom-get" hidden></p>
      <div class="no-rom-actions">
        <button type="button" class="no-rom-choose">Choose ROM…</button>
        <button type="button" class="no-rom-download" hidden>Download…</button>
      </div>
    </div>
  </section>`;

/** The glass around the LCD pixels, in skin units on every side. */
const GLASS = 6;
/** Room around the face edge to edge, in skin units: its plates' outer edges. */
const EDGE_MARGIN = 4;
/** How far down a finger moves on the display to open the palette, in CSS pixels. */
const SWIPE = 40;
/** How long a finger rests on the display to open its menu, in ms, and how far it may wander. */
const LONG_PRESS = 500;
const LONG_PRESS_SLOP = 10;

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
 * Shared gradients and filters for the relief, one light from the top
 * left (the decision log, "Skin depth"): a light falling onto every key
 * cap, a rim lit above and shaded below, the light across the case, an
 * edge lit on the top left and shaded on the bottom right (a raised
 * surface's edge; stroked the other way round it is a recess's), the
 * grain of the case's plastic and the shadow the case casts on the page.
 * Each is defined once and referenced by every element (gradients stretch
 * to the element's own box).
 */
function drawDefs(root, s) {
  const defs = svg("defs", {}, root);
  const grad = (id, stops, dir = { x1: 0, y1: 0, x2: 0, y2: 1 }) => {
    const g = svg("linearGradient", { id, ...dir }, defs);
    for (const [offset, color, opacity] of stops) {
      svg("stop", { offset, "stop-color": color, "stop-opacity": opacity }, g);
    }
    return g;
  };
  const diagonal = { x1: 0, y1: 0, x2: 1, y2: 1 };
  grad("cap-light", [[0, "#fff", 0.26], [0.5, "#fff", 0.05], [1, "#000", 0.16]]);
  grad("cap-rim", [[0, "#fff", 0.45], [1, "#000", 0.4]], { x1: 0, y1: 0, x2: 0.3, y2: 1 });
  // A well is a recess: its upper lip is in shade, its lower lip catches light.
  grad("well-rim", [[0, "#000", 0.55], [0.55, "#000", 0.0], [1, "#fff", 0.2]]);
  grad("case-light", [[0, "#fff", 0.1], [0.45, "#fff", 0.0], [1, "#000", 0.14]], diagonal);
  grad("edge-lit", [[0, "#fff", 0.6], [0.5, "#fff", 0.0], [0.5, "#000", 0.0], [1, "#000", 0.55]], diagonal);
  grad("edge-shade", [[0, "#000", 0.7], [0.5, "#000", 0.0], [0.5, "#fff", 0.0], [1, "#fff", 0.3]], diagonal);
  // Grain: grey noise laid over the case with the overlay blend, so a
  // mid-grey leaves the colour alone and the speckles lighten or darken
  // it; cut to the shape it is applied to.
  if (s.texture > 0) {
    const grain = svg("filter", { id: "grain", x: 0, y: 0, width: 1, height: 1, "color-interpolation-filters": "sRGB" }, defs);
    svg("feTurbulence", { type: "fractalNoise", baseFrequency: 0.55, numOctaves: 2, seed: 7, result: "noise" }, grain);
    svg("feColorMatrix", { in: "noise", type: "matrix", values: "0.4 0.4 0.4 0 -0.1  0.4 0.4 0.4 0 -0.1  0.4 0.4 0.4 0 -0.1  0 0 0 0 1", result: "grey" }, grain);
    svg("feComposite", { in: "grey", in2: "SourceAlpha", operator: "in" }, grain);
  }
  const drop = svg("filter", { id: "drop", x: -0.1, y: -0.1, width: 1.2, height: 1.2 }, defs);
  svg("feGaussianBlur", { stdDeviation: 5 }, drop);
  // The lit shift labels (`showGlow`): a light ink (on a dark case) turns
  // lighter in a soft halo of its own colour; a dark one (the 49G's, on a
  // light face) keeps its colour on a pale halo. The text itself is drawn
  // sharp over the halo; the other labels fade. Halfway between the first
  // glow (iteration 30) and the faint one of PR 76, as the owner asked.
  for (const [side, ink] of [["left", s.leftInk], ["right", s.rightInk]]) {
    const f = svg("filter", { id: `glow-${side}`, x: -0.5, y: -0.5, width: 2, height: 2, "color-interpolation-filters": "sRGB" }, defs);
    const dark = isDark(ink);
    svg("feMorphology", { in: "SourceAlpha", operator: "dilate", radius: dark ? 2 : 0.6, result: "thick" }, f);
    svg("feGaussianBlur", { in: "thick", stdDeviation: dark ? 1.8 : 2.6, result: "blur" }, f);
    svg("feFlood", { "flood-color": dark ? tint(ink, 0.92) : ink, "flood-opacity": dark ? 0.9 : 0.7 }, f);
    svg("feComposite", { in2: "blur", operator: "in", result: "halo" }, f);
    svg("feFlood", { "flood-color": dark ? ink : tint(ink, 0.45) }, f);
    svg("feComposite", { in2: "SourceGraphic", operator: "in", result: "text" }, f);
    const merge = svg("feMerge", {}, f);
    for (const n of ["halo", "text"]) svg("feMergeNode", { in: n }, merge);
  }
  return defs;
}

/**
 * The edge of a panel: a stroke along its outline, the half inside the
 * shape kept by a clip, so `width` units show. Lit on the top left and
 * shaded on the bottom right for a raised surface (`edge-lit`), the other
 * way round for a recess (`edge-shade`).
 */
function drawEdge(parent, defs, id, d, bands) {
  const clip = svg("clipPath", { id }, defs);
  svg("path", { d }, clip);
  const g = svg("g", { "clip-path": `url(#${id})` }, parent);
  for (const [grad, width, opacity] of bands) {
    svg("path", { d, fill: "none", stroke: `url(#${grad})`, "stroke-width": 2 * width, opacity }, g);
  }
}

/**
 * One panel of the case. The first is the body: the shadow it casts on the
 * page, the light across it, a rounded outer edge (a wide soft band and a
 * crisp line, both lit on the top left and shaded on the bottom right) and
 * the grain of its plastic. A raised panel gets a thin lit edge, a sunk
 * one a shaded edge with the shadow of its upper lip.
 */
function drawPanel(parent, defs, s, p, i) {
  const d = roundedPath(p.rect, p.radius, p.bottomRadius, p.bow);
  if (i === 0) {
    svg("path", { d, fill: "#000", "fill-opacity": 0.3, transform: "translate(0 5)", filter: "url(#drop)" }, parent);
  }
  svg("path", { d, fill: p.fill }, parent);
  if (i === 0) {
    svg("path", { d, fill: "url(#case-light)" }, parent);
    drawEdge(parent, defs, "edge-case", d, [["edge-lit", 14, 0.22], ["edge-lit", 5, 0.35], ["edge-lit", 1.5, 0.9]]);
    if (s.texture > 0) {
      svg("path", { d, fill: "#808080", filter: "url(#grain)", opacity: s.texture / 100, style: "mix-blend-mode: overlay" }, parent);
    }
  } else if (p.relief === "raised") {
    drawEdge(parent, defs, `edge-${i}`, d, [["edge-lit", 4, 0.3], ["edge-lit", 1.5, 0.8]]);
  } else if (p.relief === "sunk") {
    drawEdge(parent, defs, `edge-${i}`, d, [["edge-shade", 10, 0.3], ["edge-shade", 1.5, 0.8]]);
  }
}

/** A `#rrggbb` colour as [r, g, b], or null. */
function rgb(hex) {
  return /^#[0-9a-f]{6}$/i.test(hex ?? "") ? [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) : null;
}

/** Whether ink `hex` is dark (relative luminance under 0.18), as the 49G's shift labels are. */
function isDark(hex) {
  const c = rgb(hex);
  if (!c) return false;
  const v = c.map((x) => x / 255).map((x) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
  return 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2] < 0.18;
}

/** Ink `hex` mixed toward white by `t`. */
function tint(hex, t) {
  const c = rgb(hex);
  return c ? `rgb(${mix(c, [255, 255, 255], t)})` : hex;
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
    /** Calculator keys held down from the computer keyboard, by the physical key that pressed them. */
    this.keyboardDown = new Map();
    /** Calculator keys held down by a finger or the mouse, by pointer id. */
    this.pointerDown = new Map();
    /** Which key's contextmenu belongs to a pointer press, and which is Firefox's Ctrl+click. */
    this.contextClicks = new ContextClicks();
    /** Edge to edge (`setEdge`), and the face's boxes once measured (`edgeBoxes`). */
    this.edge = false;
    /** Edge to edge, whether the case is cropped to the face (`CROP`). */
    this.crop = false;
    this.faceBoxes = null;
  }

  /**
   * Attach the backend, the store and the keyboard shortcuts
   * (`Bindings`, web/bindings.js); renders and starts listening.
   */
  attach(backend, store, bindings = null) {
    this.backend = backend;
    this.store = store;
    this.bindings = bindings;
    this.isMac = bindings?.isMac ?? /Mac|iPhone|iPad/.test(navigator.platform ?? "");
    this.innerHTML = TEMPLATE;
    this.ui = {
      skin: this.querySelector(".skin"),
      skinSvg: this.querySelector(".skin svg"),
      lcd: this.querySelector("canvas"),
      glass: this.querySelector(".glass"),
      noRom: this.querySelector(".no-rom"),
    };
    this.ui.noRom.querySelector(".no-rom-choose").addEventListener("click", (e) => {
      e.currentTarget.blur();
      this.dispatchEvent(new CustomEvent("sat-choose-rom", { bubbles: true, detail: this.shownModel() }));
    });
    // Cut short, the message's text shows the whole of it (`noRomMore`).
    const shortText = this.ui.noRom.querySelector(".no-rom-text");
    shortText.addEventListener("click", () => this.noRomMore());
    shortText.addEventListener("keydown", (e) => {
      if (e.key !== "Enter" && e.key !== " ") return;
      e.preventDefault();
      this.noRomMore();
    });
    this.ui.noRom.querySelector(".no-rom-download").addEventListener("click", (e) => {
      e.currentTarget.blur();
      this.dispatchEvent(new CustomEvent("sat-download-rom", { bubbles: true, detail: this.shownModel() }));
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
    store.watch(["booted", "model", "roms"], () => this.onModel());

    store.watch(["busy"], (s) => this.classList.toggle("typing", s.busy));
    document.addEventListener("paste", (e) => this.onPaste(e));
    document.addEventListener("keydown", (e) => this.onKeyDown(e));
    document.addEventListener("keyup", (e) => this.onKeyUp(e));
    // Keys held when the window loses the focus or the page goes to the
    // background (a phone's app switcher) would never see their release.
    const releaseAll = () => {
      this.keyboardDown.clear();
      this.pointerDown.clear();
      this.backend.keyUpAll();
    };
    window.addEventListener("blur", releaseAll);
    document.addEventListener("visibilitychange", () => {
      if (document.hidden) releaseAll();
    });
    this.watchModifiers();
    // A long press on the calculator is a held key, not a context menu;
    // on the display it is the display's menu (`displayMenu`).
    this.ui.skin.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      if (this.onDisplay(e)) this.displayMenu({ x: e.clientX, y: e.clientY });
    });
    this.swipeOnDisplay();
    this.longPressOnDisplay();
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => this.draw());
    this.cropQuery = window.matchMedia(CROP);
    this.cropQuery.addEventListener("change", () => this.setEdge(this.edge));
    // Labels squeezed first: fullscreen measures the print it crops to.
    new ResizeObserver(() => {
      this.fitTexts();
      this.fit();
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
    if (empty) this.showNoRom(model);
    this.fit();
  }

  /**
   * The empty state of `model`: what is missing and where to get it (the
   * page links to hpcalc.org, the app offers its download).
   */
  showNoRom(model) {
    const n = this.ui.noRom;
    // "HP 48SX" stays on one line.
    const text = noRomText(model, (MODEL_TITLES[model] ?? model).replace(" ", "\u00a0"));
    n.querySelector(".no-rom-text").textContent = text;
    n.dataset.text = text;
    const download = this.store.state.roms?.slots.find((s) => s.model === model)?.download ?? null;
    const get = getRomLink(download, this.backend.romSource);
    const p = n.querySelector(".no-rom-get");
    p.hidden = !get;
    if (get) {
      const a = document.createElement("a");
      a.href = get.href;
      a.target = "_blank";
      a.rel = "noopener noreferrer";
      a.textContent = get.link;
      p.replaceChildren(get.before, a, get.after);
    }
    n.querySelector(".no-rom-download").hidden = !(download && this.backend.romSource === "dialog");
    this.noRomGet = get;
    this.fitNoRom();
  }

  /**
   * The whole empty state, for when the display shows it cut short: a
   * tap or click on its text opens it in a small menu (a phone has no
   * tooltip), with the hpcalc.org page and Choose ROM.
   */
  noRomMore() {
    const n = this.ui.noRom;
    if (!n.classList.contains("short")) return;
    const text = n.querySelector(".no-rom-text");
    const get = this.noRomGet;
    openMenu({
      anchor: text,
      align: "start",
      key: "no-rom",
      label: "No ROM",
      returnFocus: () => text,
      items: [
        { note: n.title },
        ...(get ? [{ text: `Open ${get.link}`, run: () => window.open(get.href, "_blank", "noopener,noreferrer") }] : []),
        { text: "Choose ROM…", run: () => this.dispatchEvent(new CustomEvent("sat-choose-rom", { bubbles: true, detail: this.shownModel() })) },
      ],
    });
  }

  /**
   * Fit the empty state into the display: the whole message if it fits,
   * else smaller print (`compact`), else its first sentence without the
   * download hint (`short`; the panel's ROM list keeps the links), else
   * "No ROM" beside the buttons (`row`): the 42S's display is a strip,
   * and a phone on its side leaves every display small. Cut short, the
   * whole message (the hint's words too) is the box's tooltip, and its
   * text opens it in a menu (`noRomMore`).
   */
  fitNoRom() {
    const n = this.ui.noRom;
    if (n.hidden || !n.dataset.text) return;
    const text = n.querySelector(".no-rom-text");
    const fits = () => {
      if (n.scrollHeight > n.clientHeight + 1 || n.scrollWidth > n.clientWidth + 1) return false;
      const box = n.getBoundingClientRect();
      return [...n.querySelectorAll("p, button")].every((e) => {
        if (e.closest("[hidden]") || !e.getClientRects().length) return true;
        const r = e.getBoundingClientRect();
        return r.left >= box.left - 0.5 && r.right <= box.right + 0.5 && r.top >= box.top - 0.5 && r.bottom <= box.bottom + 0.5;
      });
    };
    const full = n.dataset.text;
    const first = full.replace(/^(.*?\.)\s.*$/s, "$1");
    const getP = n.querySelector(".no-rom-get");
    const whole = [full, getP.hidden ? "" : getP.textContent].filter(Boolean).join(" ");
    const levels = [
      ["", full],
      ["compact", full],
      ["compact short", first],
      ["compact short row", "No ROM"],
    ];
    for (const [cls, words] of levels) {
      n.classList.remove("compact", "short", "row");
      if (cls) n.classList.add(...cls.split(" "));
      text.textContent = words;
      // Cut short: the whole message as the tooltip, and the text a button that shows it.
      const cut = n.classList.contains("short");
      n.title = cut ? whole : "";
      if (cut) {
        text.setAttribute("tabindex", "0");
        text.setAttribute("role", "button");
        text.setAttribute("aria-label", `${words} More about it`);
      } else {
        for (const a of ["tabindex", "role", "aria-label"]) text.removeAttribute(a);
      }
      if (fits()) return;
    }
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
    return { bg: LCD_BG, ink: LCD_INK };
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
   * Edge to edge (the fullscreen view). On a finger's screen (`CROP`) the
   * case is dropped and the skin cropped to its face (the plates, the
   * window, the logo, the print and the keys), so the keys take the
   * screen's width; else the whole calculator fills the screen.
   */
  setEdge(on) {
    this.edge = on;
    this.crop = on && !!this.cropQuery?.matches;
    this.classList.toggle("edge", on);
    this.classList.toggle("crop", this.crop);
    // The stage too: cropped, it takes the case's colour and no margin.
    this.parentElement?.classList.toggle("fs-crop", this.crop);
    this.fit();
  }

  /**
   * Edge to edge, a swipe down on the display asks for the command palette
   * (`sat-palette`): there is no bar and no keyboard there to open it.
   */
  swipeOnDisplay() {
    const lcd = this.ui.lcd;
    let start = null;
    lcd.addEventListener("pointerdown", (e) => {
      if (!this.edge) return;
      start = { id: e.pointerId, x: e.clientX, y: e.clientY };
      try { lcd.setPointerCapture(e.pointerId); } catch { /* not capturable */ }
    });
    lcd.addEventListener("pointermove", (e) => {
      if (!start || e.pointerId !== start.id) return;
      const dy = e.clientY - start.y;
      if (dy >= SWIPE && dy > 2 * Math.abs(e.clientX - start.x)) {
        start = null;
        this.dispatchEvent(new CustomEvent("sat-palette", { bubbles: true }));
      }
    });
    for (const type of ["pointerup", "pointercancel"]) lcd.addEventListener(type, () => { start = null; });
  }

  /** Whether pointer event `e` is on the display. */
  onDisplay(e) {
    const r = this.ui.lcd.getBoundingClientRect();
    return e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
  }

  /**
   * A finger resting on the display opens its menu (iOS sends no
   * `contextmenu` for a long press; where a browser does, the menu is
   * open already and it is ignored). A move or the finger lifting first
   * cancels it, as does a swipe.
   */
  longPressOnDisplay() {
    const lcd = this.ui.lcd;
    let press = null;
    const cancel = () => {
      if (press) clearTimeout(press.timer);
      press = null;
    };
    lcd.addEventListener("pointerdown", (e) => {
      cancel();
      if (e.pointerType === "mouse") return;
      const at = { x: e.clientX, y: e.clientY };
      press = { id: e.pointerId, at, timer: setTimeout(() => { press = null; this.displayMenu(at); }, LONG_PRESS) };
    });
    lcd.addEventListener("pointermove", (e) => {
      if (press && e.pointerId === press.id && Math.hypot(e.clientX - press.at.x, e.clientY - press.at.y) > LONG_PRESS_SLOP) cancel();
    });
    for (const type of ["pointerup", "pointercancel"]) lcd.addEventListener(type, cancel);
  }

  /**
   * The display's menu at `at` (the pointer): copy or save the screen as
   * an image, in either look, the shortcut beside the look it takes. Only
   * while a ROM runs, once.
   */
  displayMenu(at) {
    if (!this.screenReady() || openMenuKey() === "display") return;
    // The shortcut beside the look it takes (the one chosen in the panel).
    const look = this.store.state.screenLook;
    const item = (text, icon, id, l, run) => ({ text, icon, hint: l === look ? this.bindings?.labelOf(id) || undefined : undefined, run: () => run() });
    openMenu({
      at,
      key: "display",
      label: "Display",
      items: [
        item("Copy screen", "copy-screen", "copyScreen", "lcd", () => this.copyScreen("lcd")),
        item("Copy screen (black on white)", "copy-screen", "copyScreen", "bw", () => this.copyScreen("bw")),
        "-",
        item("Save screen…", "save-screen", "saveScreen", "lcd", () => this.saveScreen("lcd")),
        item("Save screen (black on white)…", "save-screen", "saveScreen", "bw", () => this.saveScreen("bw")),
      ],
    });
  }

  /** Whether there is a screen to take: a ROM runs for the model shown and has drawn. */
  screenReady() {
    return hasScreen(this.store.state) && Boolean(this.pixels);
  }

  /** Whether there is a screen to take; if not, the status line says so (the palette, the keys). */
  screenOrSay() {
    if (this.screenReady()) return true;
    this.store.set({ message: NO_SCREEN, messageError: true });
    return false;
  }

  /** The screen as a PNG blob in `look` ("lcd", "bw"), as shown now (screenshot.js). */
  screenPng(look) {
    const s = this.store.state;
    return pngBlob(screenRgba(s.frame, this.pixels, s.booted, look));
  }

  /**
   * Copy the screen to the clipboard as a PNG in `look`; where images
   * cannot be copied, save it instead and say so. Call it inside the
   * click or key that asks (the clipboard wants that).
   */
  async copyScreen(look) {
    if (!this.screenOrSay()) return;
    const blob = this.screenPng(look);
    if (await copyPng(blob)) {
      this.store.set({ message: "Screen copied as an image.", messageError: false });
      return;
    }
    await this.saveScreen(look, "This browser cannot copy images. ");
  }

  /** Save the screen as a PNG file in `look` (a download, or the app's dialog). */
  async saveScreen(look, why = "") {
    if (!this.screenOrSay()) return;
    closeMenu();
    const name = screenFileName(this.store.state.booted);
    try {
      const where = await this.backend.saveFile(name, await this.screenPng(look));
      if (where === null) return;
      this.store.set({ message: `${why}Screen saved as ${where}.`, messageError: false });
    } catch (err) {
      this.store.set({ message: `Could not save the screen: ${err?.message ?? err}`, messageError: true });
    }
  }

  /**
   * Edge to edge, the face's boxes in skin units, measured once per skin:
   * `face`, all of it (the plates, the window, the logo, the print and the
   * keys), and `core`, what must show: the window and, below it, the keys
   * with their print. Null until laid out (a hidden page).
   */
  edgeBoxes() {
    if (this.faceBoxes) return this.faceBoxes;
    // The face without the logo: where the logo sits on the case outside
    // the plates (the 42S), it is dropped rather than leave a band of case.
    const face = this.ui.skinSvg.querySelector("g.face");
    const logo = face?.querySelector("image.logo");
    logo?.classList.remove("off-face");
    logo?.setAttribute("display", "none");
    const b = face?.getBBox();
    logo?.removeAttribute("display");
    // Not laid out yet (a hidden page): the whole skin until it is.
    if (!b || !b.width) return null;
    const s = this.skinData;
    const m = EDGE_MARGIN;
    const faceBox = [b.x - m, b.y - m, b.width + 2 * m, b.height + 2 * m];
    const [gx, gy, gw, gh] = s.logo;
    const [fx, fy, fw, fh] = faceBox;
    const inside = gx >= fx && gy >= fy && gx + gw <= fx + fw && gy + gh <= fy + fh;
    logo?.classList.toggle("off-face", !inside);
    // The core: the window with its lip, the keys, and the print from the
    // window down (the shift labels, the letters beside the keys); the
    // lettering above the window may go.
    const [lx, ly, lw, lh] = s.lcd;
    const lip = GLASS + 2;
    let [x0, y0, x1, y1] = [lx - lip, ly - lip, lx + lw + lip, ly + lh + lip];
    const parts = [face.querySelector("g.keys"), ...face.querySelectorAll("g.print > *")];
    for (const e of parts) {
      const r = e?.getBBox();
      if (!r || !r.width || r.y < y0) continue;
      [x0, y0, x1, y1] = [Math.min(x0, r.x), Math.min(y0, r.y), Math.max(x1, r.x + r.width), Math.max(y1, r.y + r.height)];
    }
    // Within the face, with the margin the face has around its plates.
    x0 = Math.max(fx, x0 - m);
    y0 = Math.max(fy, y0 - m);
    x1 = Math.min(fx + fw, x1 + m);
    y1 = Math.min(fy + fh, y1 + m);
    this.faceBoxes = { face: faceBox, core: [x0, y0, x1 - x0, y1 - y0] };
    return this.faceBoxes;
  }

  /**
   * Size the skin and the LCD canvas. The skin fills the stage's height
   * (or its width, on a narrow screen); edge to edge it is placed clear of
   * the overlay buttons (`edgeLayout`, web/edge.js), whole, or cropped
   * (`crop`): the face's core fills the screen's width and the rest of
   * the face is cropped to the screen. The skin then shrinks by up to
   * `SNAP_LOSS` (`SNAP_LOSS_EDGE` edge to edge) so each LCD pixel is a
   * whole number of device pixels and the display stays crisp; below two
   * device pixels per LCD pixel it is not snapped.
   */
  fit() {
    if (!this.ui || !this.skinData) return;
    const W = this.lcdWidth();
    const ROWS = this.lcdHeight() + ANN_H;
    const lcd = this.ui.lcd;
    const dpr = window.devicePixelRatio || 1;
    const s = this.skinData;
    const [lx0, ly0, lw] = s.lcd;
    const unit = lw / W;
    const snapScale = (f, loss) => {
      const dev = f * unit * dpr;
      const snapped = Math.floor(dev);
      return snapped >= 2 && snapped / dev >= 1 - loss ? snapped / (unit * dpr) : f;
    };
    const room = this.stageRoom();
    const availW = Math.max(200, room.w);
    const availH = Math.max(240, room.h);
    const whole = [0, 0, s.width, s.height];
    const boxes = !this.edge ? null : this.crop ? this.edgeBoxes() : { face: whole, core: whole };
    const place = boxes
      ? edgeLayout(boxes.face, boxes.core, { w: availW, h: availH }, (v) => snapScale(v, SNAP_LOSS_EDGE))
      : null;
    const view = place?.view ?? [0, 0, s.width, s.height];
    const f = place?.f ?? snapScale(Math.min(availW / s.width, availH / s.height), SNAP_LOSS);
    const [bx, by, bw] = view;
    this.ui.skinSvg.setAttribute("viewBox", view.join(" "));
    // Edge to edge the skin is placed in the room; else the stage centres it.
    this.ui.skin.style.marginLeft = place ? `${place.x}px` : "";
    this.ui.skin.style.marginTop = place ? `${place.y}px` : "";
    const [lx, ly] = [lx0 - bx, ly0 - by];
    const css = f * unit;
    this.ui.skin.style.width = `${bw * f}px`;
    // One skin unit in CSS pixels, for the glass's shadow (style.css).
    this.ui.skin.style.setProperty("--u", `${f}px`);
    const snap = (v) => Math.round(v * dpr) / dpr;
    lcd.style.left = `${snap(lx * f)}px`;
    lcd.style.top = `${snap(ly * f)}px`;
    lcd.style.width = `${W * css}px`;
    lcd.style.height = `${ROWS * css}px`;
    for (const k of ["left", "top", "width", "height"]) this.ui.noRom.style[k] = lcd.style[k];
    this.fitNoRom();
    // The glass reaches beyond the pixels on every side; its sunk look is CSS.
    const glass = this.ui.glass.style;
    glass.left = `${snap(lx * f) - GLASS * f}px`;
    glass.top = `${snap(ly * f) - GLASS * f}px`;
    glass.width = `${W * css + 2 * GLASS * f}px`;
    glass.height = `${ROWS * css + 2 * GLASS * f}px`;
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

  /** Press key `name`; with `shift`, a shifted press (the host taps the shift unless it is on). */
  pressKey(name, shift = null) {
    const action = keyAction(this.store.state, name, this.keyNames);
    if (action === "pulse") this.pulseNoRom();
    if (action !== "press") return false;
    this.backend.keyDown(name, shift);
    this.tookInput();
    return true;
  }

  /**
   * Input went to the calculator (a key pressed by pointer, keyboard or
   * Firefox's Ctrl+click, a letter typed, a paste): `sat-key`, so the
   * page's edit shortcut follows it and the memory view gives the keys
   * back (app.js). Every path that sends input calls it.
   */
  tookInput() {
    this.dispatchEvent(new CustomEvent("sat-key", { bubbles: true }));
  }

  releaseKey(name) {
    if (!isLive(this.store.state)) return;
    this.backend.keyUp(name);
  }

  /** The script key a physical key maps to on this model, or null. */
  keyFor(e) {
    const any = KEYMAP_ANY[e.key];
    if (any) return any.find((n) => this.keyNames.has(n)) ?? null;
    const name = KEYMAP[e.key];
    if (name === "space" && !this.keyNames.has("space") && this.typing?.letters[" "]) return null;
    return name && this.keyNames.has(name) ? name : null;
  }

  onKeyDown(e) {
    if (e.defaultPrevented) return;
    const t = e.target;
    if (t instanceof HTMLSelectElement || t instanceof HTMLInputElement || t instanceof HTMLButtonElement) return;
    // A dialog and the memory view keep the keys while the focus is inside
    // them (the event's path, as a handler there may have redrawn its target).
    if (e.composedPath().some((n) => n instanceof Element && n.matches("dialog[open], sat-explorer"))) return;
    // A modal dialog (the palette, the shortcuts) keeps the keys even when nothing in it has the focus.
    if ([...document.querySelectorAll("dialog[open]")].some(isModal)) return;
    // A binding first: ON, alpha or a shift (it may be a combination);
    // a key bound to an app action is the page's (app.js).
    const id = this.bindings?.match(e) ?? null;
    if (id && action(id).group !== "calculator") return;
    if (!id && (e.ctrlKey || e.metaKey || e.altKey)) return;
    const name = id ? BOUND_KEYS[id].find((n) => this.keyNames.has(n)) ?? null : this.keyFor(e);
    if (id && !name) return;
    if (name) {
      e.preventDefault();
      if (!e.repeat && this.pressKey(name)) this.keyboardDown.set(e.code, name);
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
      if (!e.repeat && typing && e.key.toUpperCase() in typing.letters) {
        this.backend.typeLetter(e.key);
        this.tookInput();
      }
    } else if (e.key === " " && typing?.space.length) {
      // No space key and none in alpha mode: the model's shifted space (38G).
      e.preventDefault();
      if (!e.repeat) {
        this.backend.typeKeys(typing.space.filter((n) => this.keyNames.has(n)));
        this.tookInput();
      }
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
    this.tookInput();
    // The calculator's newline is one character.
    this.backend.insert(text.replace(/\r\n?/g, "\n")).catch((err) => {
      this.store.set({ message: `Could not paste: ${err?.message ?? err}`, messageError: true });
    });
  }

  onKeyUp(e) {
    if (!isLive(this.store.state)) return;
    // Only a key this handler pressed, by the physical key (its modifiers
    // may be up already): the release of a key typed into the memory view
    // or a dialog is not the calculator's.
    const name = this.keyboardDown.get(e.code);
    if (!name) return;
    this.keyboardDown.delete(e.code);
    e.preventDefault();
    this.releaseKey(name);
  }

  /**
   * Whether the calculator has the keys of keyboard event `e`: no text
   * field or editable element has the focus, no open dialog or the memory
   * view is on its path, and no modal dialog is open.
   */
  ownsKeys(e) {
    const t = e.target;
    if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement) return false;
    if (t instanceof HTMLElement && t.isContentEditable) return false;
    if (e.composedPath().some((n) => n instanceof Element && n.matches("dialog[open], sat-explorer"))) return false;
    return ![...document.querySelectorAll("dialog[open]")].some(isModal);
  }

  /**
   * The shift glow (web/shiftclick.js): Ctrl or Option/Alt held alone
   * lights the labels its click reaches. Heard before the page's own
   * handlers, so a chord they take still puts it out; a lone Alt's
   * release is kept from the menu bar (Windows, Linux) while the
   * calculator has the keys.
   */
  watchModifiers() {
    this.glow = new ModifierGlow({ onChange: (lit) => this.showGlow(lit) });
    window.addEventListener("keydown", (e) => this.glow.keydown(e, this.ownsKeys(e)), true);
    window.addEventListener("keyup", (e) => {
      if (this.glow.keyup(e) && this.ownsKeys(e)) e.preventDefault();
    }, true);
    for (const type of ["pointerdown", "pointermove"]) {
      window.addEventListener(type, (e) => this.glow.sync(e), { capture: true, passive: true });
    }
    window.addEventListener("blur", () => this.glow.clear());
    document.addEventListener("visibilitychange", () => {
      if (document.hidden) this.glow.clear();
    });
  }

  /** Light the labels of modifier `lit` ("ctrl", "alt") on the drawn skin, or none. */
  showGlow(lit) {
    const side = glowSide(lit, this.keyNames);
    const root = this.ui.skinSvg;
    root.classList.toggle("glow-left", side === "left");
    root.classList.toggle("glow-right", side === "right");
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
      const a = svg("text", { x: x - 3, y: base, "font-size": size, fill: s.leftInk, class: "shift-left" }, layer, k.left);
      const b = svg("text", { x: x + w + 3, y: base, "font-size": size, fill: s.rightInk, "text-anchor": "end", class: "shift-right" }, layer, k.right);
      // Each label keeps to its share of the room above the key.
      const room = w + 6;
      const la = Math.max(1, k.left.length);
      const lb = Math.max(1, k.right.length);
      this.fitLater(a, (room - 6) * (la / (la + lb)));
      this.fitLater(b, (room - 6) * (lb / (la + lb)));
    } else if (k.left || k.right) {
      const t = svg("text", {
        x: x + w / 2, y: base, "font-size": size, "text-anchor": "middle",
        fill: k.left ? s.leftInk : s.rightInk, class: k.left ? "shift-left" : "shift-right",
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
    svg("title", {}, g, [title, ...clickHints(k, this.keyNames, this.isMac)].join("\n"));
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
    // Held while the finger or button is down (captured, so sliding off
    // the key keeps it), released once on whichever end comes first. With
    // the mouse, Ctrl or Option/Alt asks for the shift first (web/shiftclick.js);
    // a Mac's Ctrl+click is a secondary click (button 2, its context menu
    // prevented on the skin), the same press.
    g.addEventListener("pointerdown", (e) => {
      e.preventDefault();
      try { g.setPointerCapture(e.pointerId); } catch { /* not capturable */ }
      const mod = e.pointerType === "mouse" ? modifierOf(e) : null;
      this.contextClicks.pointerdown(k.name, e);
      if (this.pressKey(k.name, shiftFor(mod, k.name, this.keyNames))) this.pointerDown.set(e.pointerId, k.name);
    });
    // Firefox on macOS sends a Ctrl+click as `contextmenu` without a
    // `pointerdown`: the shifted press, as a tap (the host holds it the
    // minimum time). A `contextmenu` that follows a `pointerdown` is that
    // press's (web/shiftclick.js, `ContextClicks`).
    g.addEventListener("contextmenu", (e) => {
      const mod = this.contextClicks.contextmenu(k.name, e);
      if (mod && this.pressKey(k.name, shiftFor(mod, k.name, this.keyNames))) this.releaseKey(k.name);
    });
    const up = (e) => {
      if (this.pointerDown.get(e.pointerId) !== k.name) return;
      this.pointerDown.delete(e.pointerId);
      this.releaseKey(k.name);
    };
    for (const type of ["pointerup", "pointercancel", "lostpointercapture"]) g.addEventListener(type, up);
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
    this.faceBoxes = null;
    // Edge to edge, the room around the face takes the case's colour.
    this.parentElement?.style.setProperty("--case", s.panels[0]?.fill ?? "#141413");
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
    const defs = drawDefs(root, s);
    // The body in its own group, so a view can drop it and keep the face
    // (the plates, the window and the keys): the edge-to-edge fullscreen.
    const body = svg("g", { class: "case" }, root);
    const face = svg("g", { class: "face" }, root);
    s.panels.forEach((p, i) => drawPanel(i === 0 ? body : face, defs, s, p, i));
    const [lx, ly, lw, lh] = s.lcd;
    // The window: the bezel's lip, then the glass a little larger than the
    // pixels; the glass's sunk look is the `.glass` element over the canvas.
    svg("rect", { x: lx - GLASS - 2, y: ly - GLASS - 2, width: lw + 2 * GLASS + 4, height: lh + 2 * GLASS + 4, rx: 6, fill: "#000", "fill-opacity": 0.35 }, face);
    this.skinWindow = svg("rect", { x: lx - GLASS, y: ly - GLASS, width: lw + 2 * GLASS, height: lh + 2 * GLASS, rx: 5, fill: s.lcdFill }, face);
    const [gx, gy, gw, gh] = s.logo;
    svg("image", { href: "logo.svg", x: gx, y: gy, width: gw, height: gh, class: "logo" }, face);
    const print = svg("g", { class: "print" }, face);
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
    const keys = svg("g", { class: "keys" }, face);
    for (const k of s.keys) this.drawKey(keys, s, k);
    this.showKeys(this.store.state.keysDown, true);
    this.showGlow(this.glow?.lit ?? null);
    // Labels squeezed first: fullscreen measures the print it crops to,
    // and caches it for the skin (edgeBoxes).
    this.fitTexts();
    this.fit();
  }

  /** The drawn key group of `name`, for automated checks. */
  skinKey(name) {
    return this.skinKeys.get(name) ?? null;
  }
}

customElements.define("sat-calculator", SatCalculator);
