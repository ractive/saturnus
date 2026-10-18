// What a pane offers, without the DOM: a row's actions (its "⋯", the
// context menu) and the pane's own "⋯", for a calculator and for a
// folder, and the keys (F5 copies to the other pane, F2 renames, Delete
// purges or deletes; kb/docs/saturnus-tx.md, "Layout"). The page turns
// each action's `id` into what it does (pane.js). Pure:
// web/test/tx-queue.test.mjs.

/** The key that runs an action, as the menus show it. */
export const KEYS = { copy: "F5", rename: "F2", remove: "Del" };

/** The action a key runs in a pane's list, or null. */
export function keyAction(e) {
  if (e.altKey || e.ctrlKey || e.metaKey) return null;
  if (e.key === "F5") return "copy";
  if (e.key === "F2") return "rename";
  if (e.key === "Delete") return "remove";
  return null;
}

/**
 * The actions of the selected rows of a pane: `[{id, text, icon, key?,
 * danger?, disabled?, title?}]` and `"-"` rules. `kind` the pane's place
 * ("device" or "folder"); `entries` the selection (the row's entry first
 * when it alone is meant); `other` the other pane's place kind ("device"
 * only while connected), null when it shows nothing usable;
 * `otherFolders` whether the other pane may show a folder (the right
 * one); `xmodem` the calculator (this pane's, or for a folder the other
 * pane's) has the fast transfer (a 48GX or 49G); `restore` the other
 * pane's calculator takes a restore (not the emulator's own).
 */
export function rowActions({ kind, entries, other = null, otherFolders = true, xmodem = false, restore = true }) {
  const n = entries.length;
  if (!n) return [];
  const one = n === 1 ? entries[0] : null;
  const items = [];
  const copy = { id: "copy", text: n === 1 ? "Copy to the other pane" : `Copy ${n} to the other pane`, icon: "copy-to", key: KEYS.copy };
  if (!other) Object.assign(copy, { disabled: true, title: otherFolders ? "Show a calculator or a folder in the other pane first." : "Show a calculator in the other pane first." });
  if (one?.directory) items.push({ id: "open", text: "Open", icon: "folder-open" });
  items.push(copy);
  if (kind === "device") {
    if (one?.directory) items.push({ id: "getOne", text: other === "folder" ? "Copy as one file to the other pane" : "Download as one file", icon: "save" });
    else if (one) {
      items.push({ id: "download", text: "Download", icon: "save" });
      items.push({ id: "downloadText", text: "Download as text", icon: "file" });
      if (xmodem) items.push({ id: "xmodemGet", text: other === "folder" ? "Fast transfer to the other pane (XModem)…" : "Fast transfer as a download (XModem)…", icon: "copy-to" });
    }
  } else {
    items.push({ id: "download", text: one && !one.directory ? "Download" : "Download as .zip", icon: "save" });
    if (restore && one && !one.directory && other === "device" && /\.hp$/i.test(one.name)) items.push({ id: "restoreFrom", text: "Restore the calculator from this file…", icon: "load" });
    if (one && !one.directory && other === "device" && xmodem && /\.hp$/i.test(one.name)) items.push({ id: "xmodemPut", text: "Fast transfer to the calculator (XModem)…", icon: "copy-to" });
  }
  items.push("-");
  if (one) items.push({ id: "rename", text: "Rename…", icon: "rename", key: KEYS.rename });
  items.push({ id: "remove", text: kind === "device" ? (n === 1 ? "Purge…" : `Purge ${n}…`) : n === 1 ? "Delete…" : `Delete ${n}…`, icon: "trash", key: KEYS.remove, danger: true });
  return items;
}

/** What the emulator's own calculator says instead of a restore (owner, 2026-10-14). */
export const HUB_RESTORE = "To go back to an earlier state, use Load state in Calculator.";

/**
 * The pane's own "⋯": for a calculator (`connected` false after a
 * restore, `emulated` the page's own emulated one, `hub` the emulator's,
 * which has no restore, graphics screen or fast transfer: a line says
 * where its restore is), or a folder (`picked` one the user chose,
 * `fallback` the page's own files). `other`, `otherFolders` and `xmodem`
 * as in `rowActions`.
 */
export function paneActions({ kind, connected = true, emulated = false, hub = false, picked = false, other = null, otherFolders = true, xmodem = false }) {
  if (kind === "device") {
    const off = !connected;
    return [
      { id: "mkdir", text: "New directory…", icon: "folder", disabled: off },
      ...(xmodem && !hub ? [{ id: "xmodemPut", text: "Fast transfer a file to it (XModem)…", icon: "load", disabled: off }] : []),
      { id: "backup", text: "Back up…", icon: "save", disabled: off },
      ...(hub
        ? [{ id: "restoreNote", text: HUB_RESTORE, icon: "load", disabled: true }]
        : [
          { id: "restore", text: "Restore from a backup file…", icon: "load", disabled: off },
          { id: "pict", text: "Graphics screen", icon: "screen-var", disabled: off },
        ]),
      { id: "copyAll", text: "Copy everything to a folder", icon: "copy-to", disabled: off || other !== "folder", title: other === "folder" ? null : otherFolders ? "Show a folder in the other pane first." : "A folder opens only in the right pane: show this calculator in the left one." },
      "-",
      ...(hub ? [] : [{ id: "settings", text: "Connection settings…", icon: "settings" }]),
      { id: "refresh", text: "Read everything again", icon: "refresh", disabled: off },
      "-",
      { id: "close", text: emulated ? "Turn the emulated calculator off" : "Disconnect", icon: "disconnect" },
    ];
  }
  return [
    { id: "mkdir", text: "New folder…", icon: "folder" },
    { id: "upload", text: "Add files from this computer…", icon: "load" },
    { id: "zipAll", text: "Download everything as .zip", icon: "save" },
    { id: "refresh", text: "Read the folder again", icon: "refresh" },
    "-",
    { id: "close", text: picked ? "Close this folder" : "Close the files", icon: "close" },
  ];
}
