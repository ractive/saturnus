// The ARIA radio group's keyboard (the panel's Speed control): the group
// is one tab stop, the checked radio; the arrow keys move the selection
// and wrap around, Home and End go to the ends. Pure, so it is tested in
// Node: `radioStep` gives the value a key selects, or null for any other
// key.

/** The value `key` selects in `values` from `current`, or null. */
export function radioStep(values, current, key) {
  const n = values.length;
  if (!n) return null;
  const i = Math.max(0, values.indexOf(current));
  switch (key) {
    case "ArrowRight":
    case "ArrowDown":
      return values[(i + 1) % n];
    case "ArrowLeft":
    case "ArrowUp":
      return values[(i - 1 + n) % n];
    case "Home":
      return values[0];
    case "End":
      return values[n - 1];
    default:
      return null;
  }
}

/** The `tabindex` of each radio: 0 for the checked one (or the first), -1 for the rest. */
export function radioTabIndexes(values, current) {
  const checked = values.includes(current) ? current : values[0];
  return values.map((v) => (v === checked ? 0 : -1));
}
