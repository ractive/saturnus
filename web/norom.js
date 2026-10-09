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
    return `No ROM for the ${title}. HP never published it: read the 64 KB ROM out of your own 42S, then choose the file.`;
  }
  return `No ROM for the ${title}.`;
}

/**
 * Where the page sends the user for `download` (a ROM slot's, null for
 * the 42S): its hpcalc.org page, around the file name to expect, `{before,
 * link, href, after}`; null without one. The app downloads it itself
 * instead (`romSource` "dialog").
 */
export function getRomLink(download, romSource) {
  if (!download || romSource === "dialog") return null;
  return {
    before: `Download ${download.file} from `,
    link: "hpcalc.org",
    href: download.page,
    after: ", unzip it and drop the file here.",
  };
}

/** The order the page lists models in; the host's own order may differ. */
export const MODEL_ORDER = ["48sx", "48gx", "49g", "38g", "39g", "40g", "42s"];

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

/**
 * What the page says about files `names` dropped on it outside the
 * memory view, or null when it has nothing to add. `r` is the page's
 * `chooseRom` result for them: a ROM is always assigned or offered, so
 * when nothing booted, failed to boot or is offered, none of them is a
 * ROM (the notice beside the ROMs then says so in the core's words);
 * `r` is null after a failed `chooseRom` (its error is shown already).
 * In the app (`romSource` "dialog") a drop takes no ROM at all.
 * `writable`: the running calculator takes files in its memory view,
 * which the second sentence points to.
 */
export function dropNotice(names, r, romSource, writable) {
  if (!names.length) return null;
  const list = names.length === 1 ? names[0] : `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`;
  let text;
  if (romSource === "dialog") {
    text = `${list} ${names.length === 1 ? "was" : "were"} not used. In the app, choose ROMs with Choose… in the controls.`;
  } else {
    if (!r || r.booted || r.bootError || r.offers?.length) return null;
    text = names.length === 1 ? `${list} is not a ROM.` : `${list} are not ROMs.`;
  }
  return writable ? `${text} To put a file on the calculator, drop it on the memory view.` : text;
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
