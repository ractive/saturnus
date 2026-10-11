// A place a pane of the transfer page shows: a calculator, real or
// emulated, or a folder on the computer, as the tree and the list see it
// (`<device-tree>`, `<device-list>`, web/shared/components/;
// kb/docs/saturnus-tx.md, "The shared device layer"). The interface,
// which the components only read through:
//
//   home                             the top directory as a path (HOME when absent)
//   list(path)                       -> {path, free, entries: [{name, kind,
//                                       size, checksum, directory, modified?}]}
//   "change" event, detail {path}    a directory's listing changed
//
// and what the page does through it: read, write, rename, purge, mkdir
// (`KermitDevice`, web/tx/kermit-device.js; `Folder`, web/tx/folder.js;
// the tests' fakes). `path` is a directory from the top, `["HOME", "D1"]`
// on a calculator, `["hp-files", "D1"]` in a folder. The pure helpers
// below are shared by the components and the page.

/** HOME. */
export const HOME = Object.freeze(["HOME"]);

/** The top directory of `place` (a device or a folder). */
export const homeOf = (place) => place?.home ?? HOME;

/** A path as one key (`HOME/D1`). */
export const pathKey = (path) => path.join("/");

/** Whether `a` and `b` name the same directory. */
export const samePath = (a, b) => a.length === b.length && a.every((p, i) => p === b[i]);

/** The directory above `path` (the top stays the top). */
export const parentPath = (path) => (path.length > 1 ? path.slice(0, -1) : [...path]);

/** `path` as the page shows it: `HOME › D1`. */
export const pathTitle = (path) => path.join(" › ");

/** The columns of a calculator's list, in order: the key of an entry and the heading. */
export const COLUMNS = [
  ["name", "Name"],
  ["kind", "Type"],
  ["size", "Size"],
  ["checksum", "Checksum"],
];

/** The columns of a folder's list: the modified date where a calculator has a checksum. */
export const FOLDER_COLUMNS = [
  ["name", "Name"],
  ["kind", "Type"],
  ["size", "Size"],
  ["modified", "Modified"],
];

/**
 * The entries of a listing in the list's order: directories first, then
 * by `key` (a column) ascending or not, names as a tie-breaker; numbers
 * compare as numbers, text as the calculator orders nothing (plain
 * code-point order, so `A` before `a`).
 */
export function sortEntries(entries, key = "name", ascending = true) {
  const cmp = (a, b) => {
    const x = a[key];
    const y = b[key];
    const c = typeof x === "number" && typeof y === "number" ? x - y : String(x) < String(y) ? -1 : String(x) > String(y) ? 1 : 0;
    return c || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
  };
  return [...entries].sort((a, b) => {
    if (a.directory !== b.directory) return a.directory ? -1 : 1;
    const c = cmp(a, b);
    return ascending ? c : -c;
  });
}

/** Directories of a listing, by name. */
export const directoriesOf = (listing) => sortEntries((listing?.entries ?? []).filter((e) => e.directory));

/** A size in bytes as the list shows it: `29.5`, `1,204`; nothing for a folder's directory. */
export function sizeCell(size) {
  if (size === null || size === undefined) return "";
  const n = Number(size);
  return Number.isInteger(n) ? n.toLocaleString("en-US") : n.toFixed(1);
}

/** A checksum as the calculator's BYTES shows it: `# 3A5Fh`; nothing where there is none (a folder). */
export const checksumCell = (sum) => (sum === null || sum === undefined ? "" : `# ${Number(sum).toString(16).toUpperCase()}h`);

/** A file's modified time (ms) as the list shows it: `2026-10-11 14:32`, local time. */
export function modifiedCell(ms) {
  if (!Number.isFinite(ms)) return "";
  const d = new Date(ms);
  const two = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}:${two(d.getMinutes())}`;
}
