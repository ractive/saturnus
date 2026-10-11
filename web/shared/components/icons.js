// The page's icons: one inline SVG sprite in index.html (`<symbol
// id="ic-…">`, stroked in the current colour), used by reference so every
// icon is drawn once. `icon` for the components' template strings,
// `iconEl` for DOM built in code.

const NS = "http://www.w3.org/2000/svg";

/** `<svg class="ic"><use href="#ic-name"/></svg>` as a string. */
export const icon = (name, cls = "") => `<svg class="ic${cls ? ` ${cls}` : ""}" aria-hidden="true"><use href="#ic-${name}"/></svg>`;

/** The same as an element. */
export function iconEl(name, cls = "") {
  const s = document.createElementNS(NS, "svg");
  s.setAttribute("class", `ic${cls ? ` ${cls}` : ""}`);
  s.setAttribute("aria-hidden", "true");
  const u = document.createElementNS(NS, "use");
  u.setAttribute("href", `#ic-${name}`);
  s.append(u);
  return s;
}
