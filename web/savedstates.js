// The words and checks of the saved states (iteration 34), the same on
// both hosts: Save state and Load state use the host's slot for the
// model, the "⋯" menu's two commands use a `.sat` file. Pure (no DOM):
// web/test/savedstates.test.mjs.

/** A state file's extension; the desktop app's dialogs wrote `.state` before. */
export const STATE_EXTENSIONS = [".sat", ".state"];

/** Largest state the page reads (`MAX_STATE_BYTES` of the host crate). */
export const MAX_STATE_FILE = 4 * 1024 * 1024;

// The first bytes of every state (crates/saturnus-core/src/state.rs): the
// magic `SATURNUS`, a u16 version, then the model's code.
const MAGIC = "SATURNUS";
const MODEL_AT = 10;
const MODEL_CODES = ["48sx", "48gx", "49g", "38g", "39g", "40g", "42s", "27s", "17bii", "17b", "28s", "19bii"];

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const two = (n) => String(n).padStart(2, "0");

/**
 * "9 Oct 2026, 16:40" for the time `ms`, in the device's time zone;
 * without the year (`year: false`) when it is `now`'s year.
 */
export function savedDate(ms, { now = Date.now(), year = true } = {}) {
  const d = new Date(ms);
  const y = year || d.getFullYear() !== new Date(now).getFullYear() ? ` ${d.getFullYear()}` : "";
  return `${d.getDate()} ${MONTHS[d.getMonth()]}${y}, ${two(d.getHours())}:${two(d.getMinutes())}`;
}

/** `48gx-2026-10-09-1640.sat`: the file a state of `model` saved at `ms` is offered as. */
export function stateFileName(model, ms) {
  const d = new Date(ms);
  return `${model}-${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}-${two(d.getHours())}${two(d.getMinutes())}.sat`;
}

/** What the status line says after Save state; `replaced` the earlier save's time, or null. */
export function savedMessage(replaced, now = Date.now()) {
  return replaced ? `State saved; it replaces the one from ${savedDate(replaced, { now, year: false })}.` : "State saved.";
}

const LOAD_BODY = "It replaces the calculator's memory, stack and screen.";

/** The question before Load state, as the shared modal takes it: the slot's save at `saved`. */
export function loadQuestion(saved) {
  return { title: `Load the state saved on ${savedDate(saved)}?`, body: LOAD_BODY, action: "Load" };
}

/** The question before loading the file `name`. */
export function loadFileQuestion(name) {
  return { title: `Load the state in ${name}?`, body: LOAD_BODY, action: "Load" };
}

/** "an HP 48SX": every model's title starts with HP. */
const an = (title) => `an ${title}`;

/**
 * Why the state `bytes` (a Uint8Array) cannot load into `model`, in plain
 * words, or null when its header is right for it. `title` names a model.
 * The rest (another ROM, a damaged body) is the core's to tell
 * (`loadError`).
 */
export function stateRefusal(bytes, model, title) {
  if (!bytes || bytes.length > MAX_STATE_FILE || bytes.length <= MODEL_AT) return "This file is not a saturnus state.";
  for (let i = 0; i < MAGIC.length; i++) {
    if (bytes[i] !== MAGIC.charCodeAt(i)) return "This file is not a saturnus state.";
  }
  const of = MODEL_CODES[bytes[MODEL_AT]];
  if (!of) return "This file is not a saturnus state.";
  if (of !== model) return `This state is from ${an(title(of))}. Switch to the ${title(of)} first, then load it.`;
  return null;
}

/** A failed load's error (the core's words) as the status line says it. */
export function loadError(err, model, title) {
  const why = `${err?.message ?? err}`;
  if (/different ROM/.test(why)) return `This state was saved with another ROM of the ${title(model)}; it loads only with that one.`;
  if (/another model/.test(why)) return `This state is from another model, not the ${title(model)}.`;
  if (/state version/.test(why)) return "This state is from another version of saturnus and cannot be loaded.";
  if (/invalid saved state|bad magic|larger than/.test(why)) return "This file is damaged or not a saturnus state.";
  return `Could not load the state: ${why}`;
}
