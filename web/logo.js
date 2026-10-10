// The saturnus logo in each model's colours. The colours are one table in
// style.css (`[data-logo=<model>]`: `--logo-planet` and `--logo-ring`,
// each with the skin colour it comes from); the plain logo's (logo.svg)
// are the fallback for no model. The headers' logo follows `data-logo` on
// <html>: web/theme-boot.js sets it before the first paint from the
// remembered model, `watchLogo` after that from the running model, else
// the selected one. The case draws the same mark with `data-logo` of its
// own model (`drawLogo`). Static places (the favicon, the icons, About)
// keep logo.svg. Pure but for `watchLogo`: web/test/logo.test.mjs.

/** The models with their own colours (style.css's table; theme-boot.js's list). */
export const LOGO_MODELS = ["48sx", "48gx", "49g", "38g", "39g", "40g", "42s"];

/** The `data-logo` value of `model`, or null (the plain logo). */
export function logoAttribute(model) {
  return LOGO_MODELS.includes(model) ? model : null;
}

/** The mark in a 32-unit box: the ring's back half, the planet, the ring's front half (as logo.svg). */
export const LOGO_PARTS = [
  ["path", { class: "ring", d: "M 1.6 16 A 14.4 4.6 0 0 1 30.4 16", transform: "rotate(-24 16 16)" }],
  ["circle", { class: "planet", cx: 16, cy: 16, r: 8.6 }],
  ["path", { class: "ring", d: "M 30.4 16 A 14.4 4.6 0 0 1 1.6 16", transform: "rotate(-24 16 16)" }],
];

/**
 * The mark into SVG `parent` at `[x, y, w, h]` in `model`'s colours: a
 * `g.logo` (its colours from style.css through `data-logo`). `make(tag,
 * attrs, parent)` creates an SVG element.
 */
export function drawLogo(make, parent, [x, y, w, h], model) {
  const attrs = { class: "logo", transform: `translate(${x} ${y}) scale(${w / 32} ${h / 32})` };
  const attr = logoAttribute(model);
  if (attr) attrs["data-logo"] = attr;
  const g = make("g", attrs, parent);
  for (const [tag, a] of LOGO_PARTS) make(tag, a, g);
  return g;
}

/** Keep `data-logo` on `doc`'s <html> on the running model, else the selected one. */
export function watchLogo(store, doc = document) {
  const apply = (s) => {
    const attr = logoAttribute(s.booted ?? s.model);
    if (attr) doc.documentElement.setAttribute("data-logo", attr);
    else doc.documentElement.removeAttribute("data-logo");
  };
  apply(store.state);
  return store.watch(["booted", "model"], apply);
}
