// The links between the site's pages: the landing page, the emulator and
// saturnus tx. On the site they are siblings (/saturnus/,
// /saturnus/emulator/, /saturnus/transfer/, web/site.sh); served from
// web/ (README, "Build and serve", and the tests' server) the emulator is
// index.html, saturnus tx tx.html and the landing page landing/. The
// pages' HTML carries the site's hrefs; `applySiteLinks` rewrites them for
// web/ and hides the links in the desktop app (`window.__TAURI__`), where
// there is no other page to go to.
//
//   <a data-site="landing|emulator|transfer" href="../..." hidden>
//
// A link starts hidden and is shown here, so the desktop app never shows
// it; one with `data-site-keep` (the brand) is always shown and keeps its
// own href ("./", the page itself) in the desktop app.

/**
 * The href of page `to` ("landing", "emulator", "transfer") from the page
 * at `pathname`: the site's layout when the page sits in emulator/ or
 * transfer/, else web/'s.
 */
export function siteHref(to, pathname) {
  const site = /\/(emulator|transfer)\/[^/]*$/.test(pathname);
  if (site) return { landing: "../", emulator: "../emulator/", transfer: "../transfer/" }[to] ?? null;
  return { landing: "landing/", emulator: "./", transfer: "tx.html" }[to] ?? null;
}

/** Whether the page runs in the desktop app, where the links make no sense. */
export function inDesktopApp(win = globalThis) {
  return Boolean(win.__TAURI__);
}

/**
 * The model the emulator page was opened on (`?model=48gx`, saturnus tx's
 * "Open in the emulator"), or null; the parameter is taken off the
 * address at once, so a reload starts as the page always does.
 */
export function modelParam(win = globalThis) {
  let url;
  try {
    url = new URL(win.location.href);
  } catch {
    return null;
  }
  const model = url.searchParams.get("model");
  if (model === null) return null;
  url.searchParams.delete("model");
  try {
    win.history?.replaceState(win.history.state, "", url.href);
  } catch {
    // An address that cannot be changed: the model is still opened once.
  }
  return /^[a-z0-9]{2,8}$/.test(model) ? model : null;
}

/** Set each `[data-site]` link of `doc` for this layout, or hide it in the desktop app. */
export function applySiteLinks(doc = document, win = globalThis) {
  const desktop = inDesktopApp(win);
  for (const a of doc.querySelectorAll("a[data-site]")) {
    if (desktop) {
      if (!a.hasAttribute("data-site-keep")) a.hidden = true;
      continue;
    }
    const href = siteHref(a.dataset.site, win.location.pathname);
    if (href) a.setAttribute("href", href);
    a.hidden = false;
  }
}

// Loaded by the page (a module script in tx.html, app.js's import in the
// emulator), it sets the page's links at once; not in Node's tests.
if (typeof document !== "undefined") applySiteLinks();
