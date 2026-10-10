// The ROM table's rows ("ROMs of every model"): a button that acts at
// once, a "⋯" menu for the rest (as the memory view's preview head), and
// the words of removing a ROM. Pure (no DOM): web/test/romrows.test.mjs.

/** The models that use one ROM file: removing one removes the other too. */
const SHARED = [["39g", "40g"]];

/**
 * A row's actions: `{primary: {id, label}, menu}`, the primary acting at
 * once and `menu` the "⋯" menu's `{id, text, danger?}` and `"-"` (empty:
 * no "⋯"). A filled row keeps its "⋯" for Remove… alone, so that a
 * destructive action is never a bare button. `app`: the desktop app,
 * which downloads from hpcalc.org itself (the page links there in the
 * row instead).
 */
export function rowAction(slot, app) {
  const download = Boolean(app && slot.download);
  if (!slot.fileName) {
    if (!download) return { primary: { id: "choose", label: "Choose…" }, menu: [] };
    return { primary: { id: "download", label: "Download…" }, menu: [{ id: "choose", text: "Choose a file…" }] };
  }
  return {
    primary: { id: "choose", label: "Change…" },
    menu: [
      ...(download ? [{ id: "download", text: "Download again…" }, "-"] : []),
      { id: "remove", text: "Remove…", danger: true },
    ],
  };
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
