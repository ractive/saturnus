// The calculator without a ROM for the model shown: what the page says,
// what a key press does, what a dropped file that is no ROM is told.
// Pure (no DOM), so `web/test/norom.test.mjs` runs it under Node.

/** Whether the model shown is the one running (the store's state). */
import "./shared/models.js";

/** The Pioneer models: no download, no serial port, a 131x16 display (web/shared/models.js). */
export const PIONEERS = globalThis.SATURNUS_PIONEERS;

/** The models HP never published a ROM for: the Pioneers and the 28S (web/shared/models.js). */
export const DUMP_ONLY = globalThis.SATURNUS_DUMP_ONLY;

/** The models whose ROM can be neither downloaded nor read out of the calculator: the 19BII (web/shared/models.js). */
export const NO_ROM_SOURCE = globalThis.SATURNUS_NO_ROM_SOURCE;

export function isLive(state) {
  return Boolean(state.booted) && state.booted === state.model;
}

/** The empty state when no model was asked for (`#/calculator`) and none runs. */
export const NO_CALCULATOR = "No calculator is running.";

/** The empty state's text for `title` (the model's title) and `model`, a model with no ROM. */
export function noRomText(model, title) {
  if (DUMP_ONLY.includes(model)) {
    return `No ROM for the ${title}. HP never published it: read the ROM out of your own calculator, then choose the file. The Emu42 page (hp.giesselink.com/emu42.htm) explains how.`;
  }
  if (NO_ROM_SOURCE.includes(model)) {
    return `No ROM for the ${title}. There is no download for it. If you have the file, choose it.`;
  }
  return `No ROM for the ${title}.`;
}

/**
 * The empty state of the model shown (nothing runs for it): none asked
 * for (`asked` false: the route names no model), its ROM kept but the
 * calculator not running (`kept`), or no ROM for it (`noRomText`).
 */
export function emptyText(model, title, { asked = false, kept = false } = {}) {
  if (!asked) return NO_CALCULATOR;
  if (kept) return `The ${title} is not running.`;
  return noRomText(model, title);
}

/** The order the page lists models in; the host's own order may differ (web/shared/models.js). */
export const MODEL_ORDER = globalThis.SATURNUS_MODEL_ORDER;

/** Each model's title, "HP 48SX" for "48sx" (web/shared/models.js). */
export const MODEL_TITLES = globalThis.SATURNUS_MODEL_TITLES;

/** `models` (the host's `hello.models`, or ROM slots by `model`) in [`MODEL_ORDER`]; models it does not know go last, in their order. */
export function orderModels(models, key = (m) => m) {
  const rank = (m) => {
    const i = MODEL_ORDER.indexOf(key(m));
    return i < 0 ? MODEL_ORDER.length : i;
  };
  return models.map((m, i) => [m, i]).sort((a, b) => rank(a[0]) - rank(b[0]) || a[1] - b[1]).map(([m]) => m);
}

/** The models whose memory view takes files: those with a Kermit server (48SX, 48GX, 49G). */
export const WRITABLE_MODELS = new Set(["48sx", "48gx", "49g"]);

/** "a.txt", "a.txt and b.png", "a.txt, b.png and c.pdf". */
function listed(names) {
  return names.length === 1 ? names[0] : `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`;
}

/**
 * What the home page says about files `names` dropped on it, or null
 * when it has nothing to add. `r` is its `chooseRom` result for them: a
 * ROM is always assigned or offered, so when nothing was kept to start
 * or offered, none of them is a ROM (the notice then says so in the
 * core's words); `r` is null after a failed `chooseRom` (its error is
 * shown already). In the app (`romSource` "dialog") a drop takes no ROM
 * at all.
 */
export function dropNotice(names, r, romSource) {
  if (!names.length) return null;
  if (romSource === "dialog") return `${listed(names)} ${names.length === 1 ? "was" : "were"} not used. In the app, choose ROMs with Choose….`;
  if (!r || r.start || r.booted || r.bootError || r.offers?.length) return null;
  return names.length === 1 ? `${listed(names)} is not a ROM.` : `${listed(names)} are not ROMs.`;
}

/** What Calculator says about dropped files `names` that are no calculator files (a ROM among them). */
export function notCalculatorFiles(names) {
  const are = names.length === 1 ? "is not a calculator file" : "are not calculator files";
  return `${listed(names)} ${are}. ROMs are chosen on the home page.`;
}

/**
 * A press of calculator key `name` (a computer key mapped to it, or a
 * drawn key): "press" sends it, "pulse" highlights the empty state (no ROM
 * runs for the model shown), "ignore" when the model has no such key.
 */
export function keyAction(state, name, keyNames) {
  if (!name || !keyNames.has(name)) return "ignore";
  return isLive(state) ? "press" : "pulse";
}
