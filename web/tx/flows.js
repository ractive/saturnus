// The transfer page's decisions and words, without the DOM: what a model
// is called, what the connect dialog says per model, what a put asks,
// what a preview fetches, how progress and failures read. Plain English,
// no protocol words in the main flows (kb/docs/saturnus-tx.md, "Goals").
// Pure: web/test/tx-flows.test.mjs.


import "../shared/models.js";

/** The models the page connects to, in its order (web/shared/models.js). */
export const MODELS = globalThis.SATURNUS_SERIAL;

/** A model as the page names it: the detected family. */
export const MODEL_NAMES = { "48sx": "HP 48S/SX", "48gx": "HP 48G/GX", "49g": "HP 49G", unknown: "Calculator" };

/** A model's title as the emulator names it: "HP 48GX" (web/shared/models.js); an unknown one as it is. */
export const modelTitle = (model) => globalThis.SATURNUS_MODEL_TITLES[model] ?? String(model ?? "calculator").toUpperCase();

/** The emulated calculator's names (one exact model), the emulator's: "HP 48GX". */
export const EMULATED_NAMES = Object.fromEntries(MODELS.map((m) => [m, modelTitle(m)]));

/**
 * What to do on the calculator to start its server, per model: `keys`
 * is a sequence of key caps (drawn as keys), `or` the typed alternative.
 */
export const START_SERVER = {
  "48sx": {
    title: "HP 48S or 48SX",
    steps: ["Type SERVER and press ENTER."],
    keys: ["S", "E", "R", "V", "E", "R", "ENTER"],
    keysNote: "Alpha mode for the letters (α, then the letter keys).",
  },
  "48gx": {
    title: "HP 48G, 48G+ or 48GX",
    steps: ["Press right-shift, then → (the right arrow)."],
    keys: ["↱", "→"],
    keysNote: "Or type SERVER and press ENTER.",
  },
  "49g": {
    title: "HP 49G",
    steps: ["Press right-shift and then → (the right arrow) quickly, without a pause."],
    keys: ["↱", "→"],
    keysNote: "With a pause between them the 49G starts a different server. Or type SERVER and press ENTER.",
  },
};

/** What every model shows once its server runs. */
export const SERVER_SHOWN = "The display says “Awaiting Server Cmd.”";

/** The connection settings stored as `text` (JSON): `{find, baud, parity}`, the defaults for anything else. */
export function linkSettings(text) {
  let v = null;
  try {
    v = JSON.parse(text ?? "null");
  } catch {
    v = null;
  }
  const baud = [9600, 4800, 2400, 1200].includes(v?.baud) ? v.baud : 9600;
  const parity = ["none", "odd", "even"].includes(v?.parity) ? v.parity : "none";
  return { find: v?.find !== false, baud, parity };
}

/** A size in bytes, short: `840 bytes`, `1.2 KB`, `31 KB`. */
export function bytesText(n) {
  const b = Math.max(0, Number(n) || 0);
  if (b < 1000) return `${Math.round(b)} ${Math.round(b) === 1 ? "byte" : "bytes"}`;
  const kb = b / 1024;
  return `${kb < 10 ? kb.toFixed(1) : Math.round(kb)} KB`;
}

/** The status line of an operation in progress: `Getting PRG… 1.2 KB of 3.4 KB`. */
export function progressText({ label, done = 0, total = null }) {
  if (!done) return label;
  if (total) return `${label} ${bytesText(Math.min(done, total))} of ${bytesText(total)}`;
  return `${label} ${bytesText(done)}`;
}

/** A link's settings as words: `9600 baud`, `2400 baud, odd parity`. */
export function settingsText({ baud, parity = "none" }) {
  return parity === "none" ? `${baud} baud` : `${baud} baud, ${parity} parity`;
}

/** The speed search's event as the status line says it. */
export function searchText(e) {
  if (e?.type === "listening") return `Listening at ${settingsText(e.settings)}…`;
  if (e?.type === "found") return `The calculator talks at ${settingsText(e.settings)}.`;
  return "";
}

/** The name of a connected place: "HP 48G/GX on USB serial port", "Emulated HP 49G". */
export function placeTitle({ model, emulated = false, linkLabel = "serial port" }) {
  if (emulated) return `Emulated ${EMULATED_NAMES[model] ?? "calculator"}`;
  return `${MODEL_NAMES[model] ?? MODEL_NAMES.unknown} on ${linkLabel}`;
}

/**
 * What a put of a file holding `kind` (the transfer bindings' `file_kind`:
 * binary, text, other) does: send it as it is, or ask first whether to
 * store it as a string (the calculator would).
 */
export function putPlan(kind) {
  if (kind === "binary") return { ask: null, asString: false, how: "an exact copy" };
  if (kind === "text") return { ask: null, asString: false, how: "text the calculator reads in" };
  return { ask: "string", asString: true, how: "a string" };
}

/** The question before storing a file that is no calculator object. */
export function stringQuestion(fileName, name) {
  return {
    title: "Store it as a string?",
    body: `${fileName} is not a calculator file. The calculator can keep its contents as a string in ${name}.`,
    action: "Store as string",
    danger: false,
  };
}

/** The question before replacing a variable. */
export function replaceQuestion(name) {
  return {
    title: `Replace ${name}?`,
    body: `The calculator already has a variable named ${name} here. The file takes its place; the old contents are lost.`,
    action: "Replace",
    danger: true,
  };
}

/** Kinds whose text needs the calculator's command names: previewed as the calculator's text. */
const TEXT_KINDS = new Set(["Program", "Algebraic", "Unit"]);
/** Kinds with nothing to show. */
const NO_PREVIEW = new Set(["Directory", "Library", "Backup", "Library Data", "Code", "System Binary"]);
/** Above this many bytes a preview waits for a click (about 2 s at 9600 baud). */
export const AUTO_PREVIEW_BYTES = 2048;

/**
 * How the preview of a listed variable is fetched: `{fetch: "text" |
 * "binary" | null, auto}`. Programs, algebraics and units as the
 * calculator's text (one transfer, no ROM needed here); data objects and
 * GROBs as the binary file, decoded here; libraries, backups and code not
 * at all. `auto` when small enough to fetch on selection.
 */
export function previewPlan(entry) {
  if (!entry || NO_PREVIEW.has(entry.kind)) return { fetch: null, auto: false };
  return { fetch: TEXT_KINDS.has(entry.kind) ? "text" : "binary", auto: Number(entry.size) <= AUTO_PREVIEW_BYTES };
}

/** What the page says when its own files are from two builds (a deploy caught half way, a stale cache). */
export const MIXED_FILES = "This page's files are from different versions. Reload the page.";

/**
 * A row dragged out to the desktop as file `name` at blob URL `url`: the
 * DownloadURL `type:name:url`, or null for a name with a colon, which
 * that form cannot carry (Chrome would cut the name there).
 */
export function downloadUrl(name, url) {
  return name.includes(":") ? null : `application/octet-stream:${name}:${url}`;
}

/** A failure as a sentence for the user. */
export function failureText(err) {
  const kind = err?.kind;
  const message = String(err?.message ?? err ?? "");
  switch (kind) {
    case "noAnswer":
      return "No calculator answered. Is it in server mode, and is the cable in both ends? The settings let you try a fixed speed.";
    case "nameClash":
      return `${err.varName ?? "That name"} is already there.`;
    case "isDirectory":
      return `${err.varName ?? "It"} is a directory: a file cannot replace it.`;
    case "badName":
      return `The calculator does not take “${err.varName ?? ""}” as a name.`;
    case "notAnObject":
      return "The file is not a calculator object.";
    case "cancelled":
    case "aborted":
      return "Stopped.";
    case "closed":
      return "Not connected to a calculator.";
    case "busy":
      return "Another transfer is still running.";
    case "notConnected":
      return "Not connected to a calculator.";
    case "leftServer":
      return "The calculator left server mode for a fast transfer: start its server again and reconnect.";
    case "xmodemTimeout":
      return "Nothing came from the calculator in time.";
    default:
      // The wasm package does not know a request this page's modules send:
      // the two came from different builds (a deploy caught half way).
      if (/^no request "/.test(message)) return MIXED_FILES;
      return message ? message[0].toUpperCase() + message.slice(1) : "Something went wrong.";
  }
}

/**
 * What the page says after FINISH: the server has ended. A 49G whose
 * server was started from algebraic mode leaves the stack packed in one
 * list (kb/docs/saturnus-tx.md, Connect, step 4).
 */
export function disconnectNote(model) {
  const base = "The calculator has left server mode.";
  if (model === "49g") return `${base} If its server was started in algebraic mode, the 49G has now put what was on its stack into one list.`;
  return base;
}
