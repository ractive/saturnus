// The calculator without a ROM for the model shown: what the page says,
// what a key press does, and what a model switch asks of the backend.
// Pure (no DOM), so `web/test/norom.test.mjs` runs it under Node.

/** Whether the model shown is the one running (the store's state). */
export function isLive(state) {
  return Boolean(state.booted) && state.booted === state.model;
}

/** The empty state's text for `title` (the model's title) and `model`. */
export function noRomText(model, title) {
  if (model === "42s") {
    return `No ROM for the ${title}. HP never released it: dump the ROM from your own calculator, then choose the file.`;
  }
  return `No ROM for the ${title}.`;
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

/**
 * The selected model changed to `model`: resume it if it is the machine
 * that runs (paused when another model was chosen), else boot it from its
 * remembered ROM (`slot`, iteration 20), else pause the machine of another
 * model and say which ROM is missing. Resolves to what was done: "resumed",
 * "boot" or "no-rom".
 */
export async function switchModel(backend, state, model, slot) {
  if (state.booted === model) {
    await backend.pause(false);
    return "resumed";
  }
  if (slot && slot.state !== "empty") return "boot";
  if (state.booted && state.running) await backend.pause(true);
  return "no-rom";
}
