// The links between the site's pages: the landing page and the app
// (iteration 50d), and the docs (iteration 57). On the site the app sits in app/ under the landing
// page (/saturnus/, /saturnus/app/, web/site.sh); served from web/
// (README, "Build and serve", and the tests' server) the app is
// index.html and the landing page landing/. The app's HTML carries the
// site's hrefs; `applySiteLinks` rewrites them for web/ and hides them in
// the desktop app (`window.__TAURI__`), where there is no landing page.
//
//   <a data-site="landing" href="../" hidden>
//
// A link starts hidden and is shown here, so the desktop app never shows
// it. Inside the app the views link each other by their routes
// (web/router.js), in the same document.

import "./models.js";

/** The docs on the site (iteration 57), for a page that is not on it (web/ served directly). */
export const DOCS_URL = "https://ractive.ch/saturnus/docs/";

/**
 * The href of page `to` ("landing", "app", "docs", "commands": the docs'
 * command reference) from the page at `pathname`: the site's layout
 * when the page sits in app/, else web/'s, which has no docs and links
 * the site's.
 */
export function siteHref(to, pathname) {
  const site = /\/app\/[^/]*$/.test(pathname);
  if (site) return { landing: "../", app: "./", docs: "../docs/", commands: "../docs/commands/" }[to] ?? null;
  return { landing: "landing/", app: "./", docs: DOCS_URL, commands: `${DOCS_URL}commands/` }[to] ?? null;
}

/**
 * The model Files connects to from the Calculator view's link, or null:
 * the running calculator `booted`, its ROM kept (`kept`: only a kept
 * ROM's calculator is shared), of a model with a serial port and a Kermit
 * server.
 */
export function connectModel({ booted, kept }) {
  return booted && kept && globalThis.SATURNUS_SERIAL.includes(booted) ? booted : null;
}

/**
 * Why Files does not connect the calculator of `booted`: "aplet",
 * "noserial" or "unkept" (`kept`: its ROM is kept); null when it does
 * or nothing runs.
 */
export function connectRefusal({ booted, kept }) {
  if (!booted || connectModel({ booted, kept })) return null;
  if (globalThis.SATURNUS_APLETS.includes(booted)) return "aplet";
  if (!globalThis.SATURNUS_SERIAL.includes(booted)) return "noserial";
  return "unkept";
}

/** `connectRefusal`'s `why` for `model` in words, or null for a code it does not know. */
export function refusalText(model, why) {
  const title = globalThis.SATURNUS_MODEL_TITLES[model] ?? model;
  return {
    aplet: `The ${title} is not connected here: it sends and receives files itself, from a folder Files serves to it.`,
    noserial: `The ${title} has no serial port: Files cannot reach it.`,
    unkept: `The ${title} is not connected here: Files reaches only a calculator whose ROM this browser keeps. Choose its ROM again on the home page to keep it.`,
  }[why] ?? null;
}

/** Whether the page runs in the desktop app, where the links make no sense. */
export function inDesktopApp(win = globalThis) {
  return Boolean(win.__TAURI__);
}

/** Set each `[data-site]` link of `doc` for this layout, or hide it in the desktop app. */
export function applySiteLinks(doc = document, win = globalThis) {
  const desktop = inDesktopApp(win);
  for (const a of doc.querySelectorAll("a[data-site]")) {
    if (desktop) {
      // `data-site-keep` (the home page's logo) stays, as text.
      if ("siteKeep" in a.dataset) a.removeAttribute("href");
      else a.hidden = true;
      continue;
    }
    const href = siteHref(a.dataset.site, win.location.pathname);
    if (href) a.setAttribute("href", href);
    a.hidden = false;
  }
}
