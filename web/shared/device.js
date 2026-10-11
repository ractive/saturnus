// A device: a calculator, real or emulated, as the transfer page's tree
// and list see it (`<device-tree>`, `<device-list>`,
// web/shared/components/; kb/docs/saturnus-tx.md, "The shared device
// layer"). The interface, which the components only read through:
//
//   list(path)                       -> {path, free, entries: [{name, kind,
//                                       size, checksum, directory}]}
//   read(path, name, {text})         -> {name, fileName, format, data}
//   write(path, name, bytes, {replace, asString}) -> the name stored
//   rename, purge, mkdir             (later; absent until a device has them)
//   "change" event, detail {path}    a directory's listing changed
//
// `path` is a directory from HOME, `["HOME", "D1"]`. Implementations:
// `KermitDevice` (web/tx/kermit-device.js, any link) and the tests' fakes.
// The pure helpers below are shared by the components and the page:
// web/test/tx-device.test.mjs.

/** HOME. */
export const HOME = Object.freeze(["HOME"]);

/** A path as one key (`HOME/D1`). */
export const pathKey = (path) => path.join("/");

/** Whether `a` and `b` name the same directory. */
export const samePath = (a, b) => a.length === b.length && a.every((p, i) => p === b[i]);

/** The directory above `path` (HOME stays HOME). */
export const parentPath = (path) => (path.length > 1 ? path.slice(0, -1) : [...HOME]);

/** `path` as the page shows it: `HOME › D1`. */
export const pathTitle = (path) => path.join(" › ");

/** The columns of the list, in order: the key of an entry and the heading. */
export const COLUMNS = [
  ["name", "Name"],
  ["kind", "Type"],
  ["size", "Size"],
  ["checksum", "Checksum"],
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

/** A size in bytes as the list shows it: `29.5`, `1,204`. */
export function sizeCell(size) {
  const n = Number(size);
  return Number.isInteger(n) ? n.toLocaleString("en-US") : n.toFixed(1);
}

/** A checksum as the calculator's BYTES shows it: `# 3A5Fh`. */
export const checksumCell = (sum) => `# ${Number(sum).toString(16).toUpperCase()}h`;
