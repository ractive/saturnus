// The words of "Serve a folder to a 38G, 39G or 40G", without the DOM:
// what to press on each calculator, how a transfer reads in the list,
// what "Transfer Failed" means each time. Plain English, no protocol
// words (kb/docs/saturnus-tx.md, "Aplets"; wiki protocols/aplet-transfer).
// Pure: web/test/tx-serve.test.mjs.

import { bytesText } from "./flows.js";

/** The calculators a folder is served to, as the page names them. */
export const SERVE_NAMES = { "38g": "HP 38G", "39g": "HP 39G or 40G", "40g": "HP 40G" };

/** Short names of the emulated ones. */
export const SERVE_EMULATED = { "38g": "38G", "39g": "39G", "40g": "40G" };

/**
 * What to press, per calculator family: the aplet library key, where
 * "Disk drive…" sits in the SEND and RECEIVE lists, and the wire.
 */
export const SERVE_STEPS = {
  "38g": {
    title: "HP 38G",
    library: "LIB",
    send: ["Press LIB and choose an aplet.", "Press SEND, choose “Disk drive…”, then “Send now”."],
    receive: ["Press LIB, then RECV.", "Choose “Disk drive…”, then the aplet."],
    sendKeys: ["LIB", "SEND", "Disk drive…", "Send now"],
    receiveKeys: ["LIB", "RECV", "Disk drive…"],
    wire: null,
  },
  "39g": {
    title: "HP 39G or 40G",
    library: "APLET",
    send: ["Press APLET and choose an aplet.", "Press SEND, choose “Disk drive…” (the third entry), then “Send now”."],
    receive: ["Press APLET, then RECV.", "Choose “Disk drive…”, then the aplet."],
    sendKeys: ["APLET", "SEND", "Disk drive…", "Send now"],
    receiveKeys: ["APLET", "RECV", "Disk drive…"],
    wire: ["Press APLET, then RECV.", "Choose “HP39/40 (Wire)”."],
  },
};

/** The steps for `model` ("40g" reads as the 39G's). */
export const stepsFor = (model) => SERVE_STEPS[model === "38g" ? "38g" : "39g"];

/** A note the lists carry under the steps. */
export const SERVE_NOTE = "Lists, matrices, programs and notes go the same way from their own catalogs. Pressing ON on the calculator stops a transfer.";

/** The title a file's header gives, else its name: `Function (FUNCTION.000)`. */
export function fileLabel(name, title = null) {
  return title && title !== name ? `${title} (${name})` : name;
}

/**
 * One line of the transfers list for an event of the server, or null for
 * an event the list does not show. `last` is the file the calculator last
 * asked for that was not there (for "Transfer Failed"). Lines:
 * `{kind: "in" | "out" | "info" | "error", text}`.
 */
export function eventLine(e, { last = null } = {}) {
  switch (e?.type) {
    case "model":
      return { kind: "info", text: `The calculator is an ${SERVE_NAMES[e.model] ?? "aplet calculator"}.` };
    case "received":
      if (e.directory) return { kind: "info", text: `The calculator wrote its list of files (${e.name}).` };
      return { kind: "in", text: `Received ${fileLabel(e.name, e.title)}, ${bytesText(e.size ?? e.data?.length ?? 0)}.` };
    case "sent":
      if (e.directory) return null;
      return { kind: "out", text: `Sent ${e.name}.` };
    case "discarded":
      return { kind: "error", text: `The calculator stopped sending ${e.name}; nothing was saved.` };
    case "notFound":
      return { kind: "error", text: `The calculator asked for ${e.name}, which is not in the folder.` };
    case "calculatorSays":
      return { kind: "error", text: transferFailedText(e.text, last) };
    case "failed":
      return { kind: "error", text: `The transfer broke off: ${failedText(e.message)}` };
    case "wireStarted":
      return { kind: "info", text: "The calculator is receiving by wire." };
    case "wireDone":
      return { kind: "out", text: "Sent by wire. If the calculator says “Transfer Failed”, the file is not a binary aplet: send it with RECV, “Disk drive…” instead." };
    case "wireFailed":
      return { kind: "error", text: wireFailedText(e.message) };
    default:
      return null;
  }
}

/** What the calculator's own error message means here. */
export function transferFailedText(text, last = null) {
  const said = text ? `“${text}”` : "an error";
  if (last) return `The calculator says ${said}: ${last} is not in the folder. It offers to get another file.`;
  if (/transfer failed/i.test(text ?? "")) return `The calculator says ${said}: it stopped the transfer (ON pressed, or it could not take the file).`;
  return `The calculator says ${said}.`;
}

/** A failure on this side, in words. */
export function failedText(message = "") {
  if (/timed out|retries/i.test(message)) return "the calculator stopped answering. Check the cable, then start again on the calculator.";
  if (/cancel/i.test(message)) return "stopped here.";
  return `${message}.`;
}

/** The wire send's failure, in words. */
export function wireFailedText(message = "") {
  if (/timed out|retries/i.test(message)) return "The calculator did not start receiving: choose RECV, then “HP39/40 (Wire)” on it, then send again. It shows “Transfer Failed” when it gives up waiting.";
  if (/cancelled by the peer/i.test(message)) return "The calculator stopped receiving (“Transfer Failed”): ON was pressed, or it waited too long. Choose RECV, “HP39/40 (Wire)” on it again.";
  if (/cancel/i.test(message)) return "The wire send was stopped.";
  return `The wire send failed: ${message}.`;
}

/** The status line while serving. */
export function serveStatus({ model = null, current = null, wire = false } = {}) {
  if (wire) return "Waiting for the calculator to receive by wire…";
  if (current) return current;
  return model ? `Waiting for the ${SERVE_NAMES[model] ?? "calculator"}…` : "Waiting for the calculator…";
}

/** A transfer in progress, for the status line. */
export function progressLine(e) {
  if (e?.type === "sending") return `Sending ${e.name}… ${bytesText(e.done)} of ${bytesText(e.total)}`;
  if (e?.type === "receiving") return `Receiving ${e.name}… ${bytesText(e.done)}`;
  if (e?.type === "wireProgress") return `Sending by wire… ${bytesText(Math.min(e.done, e.total))} of ${bytesText(e.total)}`;
  return null;
}
