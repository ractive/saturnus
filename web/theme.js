// The page's colour theme: "system" follows the device (prefers-color-
// scheme), "light" and "dark" are fixed. A fixed theme is `data-theme` on
// <html>, which style.css's token sets follow; the browser's own parts
// (`color-scheme`), the installed page's bar (theme-color) and the
// desktop window's title bar follow too. web/theme-boot.js applies the
// stored choice before the first paint; this module changes it later.
// Pure but for `applyTheme`: web/test/theme.test.mjs.

/** The choices, in the panel's order, and the stored preference's key. */
export const THEMES = ["system", "light", "dark"];
export const THEME_KEY = "saturnus.theme";
/** The page's bar colour per theme (the panel's colour, style.css `--panel`). */
export const THEME_COLORS = { light: "#f1efe9", dark: "#272724" };

/** The choice a stored preference names; "system" for anything else. */
export function themeOf(pref) {
  return THEMES.includes(pref) ? pref : "system";
}

/** The `data-theme` value of a choice, or null (System: no attribute). */
export function themeAttribute(theme) {
  return theme === "light" || theme === "dark" ? theme : null;
}

/**
 * Apply `theme` to `doc`: `data-theme`, the `color-scheme` meta, the
 * theme-color metas (fixed to the theme's colour, or back to their
 * per-scheme values), and the desktop window's title bar (`tauri`, the
 * app's `window.__TAURI__`).
 */
export function applyTheme(theme, doc = document, tauri = globalThis.__TAURI__) {
  const attr = themeAttribute(theme);
  const root = doc.documentElement;
  if (attr) root.setAttribute("data-theme", attr);
  else root.removeAttribute("data-theme");
  doc.querySelector('meta[name="color-scheme"]')?.setAttribute("content", attr ?? "light dark");
  for (const m of doc.querySelectorAll('meta[name="theme-color"]')) {
    // Each meta's own colour (by its media query) is kept the first time.
    if (!m.dataset.scheme) m.dataset.scheme = m.getAttribute("content") ?? "";
    m.setAttribute("content", attr ? THEME_COLORS[attr] : m.dataset.scheme);
  }
  tauri?.core?.invoke?.("set_theme", { theme: attr })?.catch?.(() => {});
}
