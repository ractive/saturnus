// DOM helpers shared by the pages' components.

/**
 * A new `tag` element: `attrs` set as attributes (`class` as the class
 * name, `text` as the text, true as an empty attribute; false, null and
 * undefined left out), then `children` appended (nodes or strings; null,
 * undefined and false skipped).
 */
export function el(tag, attrs = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === false || v === null || v === undefined) continue;
    if (k === "class") e.className = v;
    else if (k === "text") e.textContent = v;
    else e.setAttribute(k, v === true ? "" : v);
  }
  for (const c of children) if (c !== null && c !== undefined && c !== false) e.append(c);
  return e;
}
