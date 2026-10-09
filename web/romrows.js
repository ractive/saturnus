// The ROM table's rows ("ROMs of every model"): one button per row, a menu
// where there is more than one thing to do, and the words of removing a
// ROM. Pure (no DOM): web/test/romrows.test.mjs.

/** The models that use one ROM file: removing one removes the other too. */
const SHARED = [["39g", "40g"]];

/**
 * What a row's one button does: `{kind: "choose"}` (a plain Choose…), or
 * `{kind: "menu", label, items}` with `items` of `{id, text, danger?}`
 * and `"-"`. `app`: the desktop app, which downloads from hpcalc.org
 * itself (the page links there in the row instead).
 */
export function rowAction(slot, app) {
  const download = Boolean(app && slot.download);
  if (!slot.fileName) {
    if (!download) return { kind: "choose", label: "Choose…" };
    return {
      kind: "menu",
      label: "Add",
      items: [
        { id: "download", text: "Download from hpcalc.org…" },
        { id: "choose", text: "Choose a file…" },
      ],
    };
  }
  return {
    kind: "menu",
    label: "Change",
    items: [
      { id: "choose", text: "Choose another file…" },
      ...(download ? [{ id: "download", text: "Download again…" }] : []),
      "-",
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

/** The question before removing `models`' ROM (`title` names a model). */
export function removeQuestion(models, title, app) {
  const both = models.length > 1 ? " They use the same file, so both go." : "";
  if (app) return `Remove the ${names(models, title)} ROM from the list?${both} The file stays.`;
  const state = models.includes("49g") ? " Its saved state goes too, as it contains the ROM." : "";
  return `Remove the ${names(models, title)} ROM from this browser?${both}${state} The file on your computer stays.`;
}

/** What the status line says once it is done. */
export function removedMessage(models, title, app) {
  if (app) return `The ${names(models, title)} ROM is removed from the list.`;
  return `The ${names(models, title)} ROM is removed from this browser${models.includes("49g") ? ", with its saved state" : ""}.`;
}
