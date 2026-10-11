// The pages' icons: one SVG sprite, `shared/icons.svg` (`<symbol
// id="ic-…">`, stroked in the current colour), fetched once and put at the
// top of the page's body by `loadIcons`, then used by reference so every
// icon is drawn once. One file for both pages (the emulator and saturnus
// tx) and the desktop app, which embeds it with the page's files; the
// service worker precaches it, so an installed page has it offline.
// `icon` for the components' template strings, `iconEl` for DOM built in
// code.

const NS = "http://www.w3.org/2000/svg";

/** The sprite's URL: shared/icons.svg, beside this module's folder. */
export const ICONS_URL = new URL("../icons.svg", import.meta.url);

let loading = null;

/**
 * Fetch the sprite once and put it into `doc`'s body (hidden), where
 * every `<use href="#ic-…">` of the page finds its symbol; resolves when
 * it is there. A failed fetch leaves the icons empty, says so in the
 * console, and is tried again on the next call.
 */
export function loadIcons(doc = document, url = ICONS_URL) {
  loading ??= fetch(url)
    .then((r) => (r.ok ? r.text() : Promise.reject(new Error(`HTTP ${r.status}`))))
    .then((text) => {
      const holder = doc.createElement("div");
      holder.innerHTML = text;
      const svg = holder.querySelector("svg");
      if (!svg) throw new Error("no <svg> in it");
      svg.setAttribute("hidden", "");
      svg.setAttribute("aria-hidden", "true");
      svg.classList.add("icon-sprite");
      doc.body.prepend(svg);
    })
    .catch((err) => {
      loading = null;
      console.warn(`saturnus: the icons did not load (${err?.message ?? err})`);
    });
  return loading;
}

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
