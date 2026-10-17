// The home page's ROM actions (iteration 53, web/home.js): a kept
// calculator's "⋯" menu, a new calculator's buttons, and the words of
// removing a ROM. Pure (no DOM): web/test/romrows.test.mjs.

/** The models that use one ROM file: removing one removes the other too. */
const SHARED = [["39g", "40g"]];

/**
 * A kept calculator's "⋯" menu: `{id, text, danger?}` and `"-"`.
 * `app`: the desktop app, which downloads from hpcalc.org itself.
 */
export function keptMenu(slot, app) {
  return [
    { id: "choose", text: "Change ROM…" },
    ...(app && slot.download ? [{ id: "download", text: "Download again…" }] : []),
    "-",
    { id: "remove", text: "Remove…", danger: true },
  ];
}

/**
 * A new calculator's buttons, `{id, label}`, the last the primary:
 * Choose…, after Download… in the app where hpcalc.org has the ROM (the
 * page links there instead: a browser cannot fetch it).
 */
export function newActions(slot, app) {
  const choose = { id: "choose", label: "Choose…" };
  return app && slot.download ? [{ id: "download", label: "Download…" }, choose] : [choose];
}

/** `model` and the models sharing its file (the 39G and 40G with the same one). */
export function sharedModels(slots, model) {
  const mine = slots.find((s) => s.model === model);
  const group = SHARED.find((g) => g.includes(model)) ?? [model];
  return group.filter((m) => m === model || (mine?.fileName && slots.find((s) => s.model === m)?.fileName === mine.fileName));
}

/** "HP 39G and HP 40G". */
function names(models, title) {
  return models.map(title).join(" and ");
}

/**
 * The question before removing `models`' ROM (`title` names a model), as
 * the shared modal takes it: `{title, body, action}`. `running`: the
 * model that runs (null for none), which stops when its ROM goes.
 */
export function removeQuestion(models, title, app, running = null) {
  const both = models.length > 1 ? "They use the same file, so both go. " : "";
  const stops = running && models.includes(running) ? " and the calculator stops" : "";
  if (app) {
    return { title: `Remove the ${names(models, title)} ROM?`, body: `${both}It is taken off the list${stops}. The file stays.`, action: "Remove" };
  }
  const state = models.includes("49g") ? ", with its saved state, as that contains the ROM," : "";
  const end = state && !stops ? state.slice(0, -1) : state;
  return { title: `Remove the ${names(models, title)} ROM?`, body: `${both}It is deleted from this browser${end}${stops}. The file on your computer stays.`, action: "Remove" };
}

/** What the status line says once it is done. */
export function removedMessage(models, title, app) {
  if (app) return `The ${names(models, title)} ROM is removed from the list.`;
  return `The ${names(models, title)} ROM is removed from this browser${models.includes("49g") ? ", with its saved state" : ""}.`;
}

/**
 * Why `name`'s ROM was not removed (`what`: "its ROM", "the ROMs"): the
 * engine's refusal (`err`) while a send types or a transfer runs, in
 * plain words.
 */
export function removeRefusal(err, name, what) {
  const why = `${err?.message ?? err}`;
  if (/transfer is in progress/.test(why)) return `The ${name} is in a transfer; remove ${what} when it's done.`;
  if (/typing is in progress/.test(why)) return `The ${name} is typing a send; remove ${what} when it's done.`;
  return `Could not remove ${what}: ${why}`;
}
