// The page's questions before Start fresh and Remove ROMs, in the shared
// modal (components/confirm.js). Each resolves to the answer.

import { confirmAction } from "./components/confirm.js";

/**
 * Ask whether to cold-boot `title` (the model's title) with an empty
 * memory; resolves to true for "Start fresh".
 */
export function confirmFresh(title, doc = document) {
  return confirmAction({
    title: `Start the ${title} fresh?`,
    body: "Its stack, variables and settings go, as on a new calculator. Your saved state stays.",
    action: "Start fresh",
    doc,
  });
}

/**
 * Ask before Remove ROMs: the browser deletes the ROMs and the saved
 * 49G state (it holds the 49G's flash, the ROM); the app only takes the
 * files off its list (`app`).
 */
export function confirmForget(app, doc = document) {
  return confirmAction({
    title: "Remove all ROMs?",
    body: app
      ? "They are taken off the list. The files and the saved states stay."
      : "They are deleted from this browser, with the saved 49G state, as it contains the ROM. Other saved states and the files on your computer stay.",
    action: "Remove",
    doc,
  });
}
