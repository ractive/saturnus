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
//
// The emulator and saturnus tx open each other in a tab of their own,
// named "saturnus-emulator" and "saturnus-tx" (`TAB_NAMES`; each page also
// names its own tab, `nameTab`). A link to the other page (`linkTab`)
// first looks for that tab (`window.open("", name)`, which opens nothing
// new when the tab is there): found showing the page, the tab is brought
// forward and told what to do in a message (`openSiteTab`: "connect" the
// emulator's calculator in tx, "show" a model in the emulator; received
// with `onSiteMessage`), so neither page is reloaded and both keep their
// state; not found (or showing another page), it opens or loads the URL,
// which carries the same request (`?connect=`, `?model=`), read once by
// the page (`takeParam`). A told tab answers (`ack`); one that does not
// within `ACK_MS` while it was still loading (its listener may not have
// been there) is loaded with the URL instead, so the request is not lost;
// a loaded tab that does not answer (an older build) is left alone, so a
// request is never run twice. Browsers find a named tab only among the tabs
// that opened one another: a tab opened by hand is not found, and the
// link opens a new one. A message from the opener rather than a
// BroadcastChannel: only the one tab brought forward acts on it; and a
// tab takes requests only from the tab that opened it or one it opened.

import "./models.js";

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

/** The window names of the pages' tabs: a second click brings the open tab forward. */
export const TAB_NAMES = Object.freeze({ emulator: "saturnus-emulator", transfer: "saturnus-tx" });

/** The `type` of the pages' messages to each other's tab. */
export const SITE_MESSAGE = "saturnus-site";

/** How long a told tab has to answer before it is loaded with the URL instead. */
export const ACK_MS = 1000;

/**
 * The model saturnus tx connects to from the emulator's link, or null:
 * the running calculator `booted`, shared with tx (`shared`: the
 * calculators' SharedWorker, and its ROM kept: `kept`), of a model with a
 * serial port and a Kermit server.
 */
export function connectModel({ booted, shared, kept }) {
  return booted && shared && kept && globalThis.SATURNUS_SERIAL.includes(booted) ? booted : null;
}

/** The href of saturnus tx from the page at `pathname`, its left pane on `model`'s shared calculator when given (`?connect=`). */
export function transferHref(pathname, model = null) {
  const href = siteHref("transfer", pathname);
  return model ? `${href}?connect=${encodeURIComponent(model)}` : href;
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
  return takeParam("model", win);
}

/**
 * The model in the page's parameter `name` (`?model=`, `?connect=`), or
 * null; the parameter is taken off the address at once, so a reload
 * starts as the page always does.
 */
export function takeParam(name, win = globalThis) {
  let url;
  try {
    url = new URL(win.location.href);
  } catch {
    return null;
  }
  const model = url.searchParams.get(name);
  if (model === null) return null;
  url.searchParams.delete(name);
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
    if (TAB_NAMES[a.dataset.site]) a.target = TAB_NAMES[a.dataset.site];
    a.hidden = false;
  }
}

/** Name this page's tab after page `to` ("emulator", "transfer"), so the other page's links find it; not in the desktop app. */
export function nameTab(to, win = globalThis) {
  if (inDesktopApp(win) || !TAB_NAMES[to]) return;
  try {
    win.name = TAB_NAMES[to];
  } catch {
    // A window whose name cannot be set: the links open a new tab.
  }
}

/** The page of address `href`: its path, "index.html" taken off. */
function pagePath(href) {
  return new URL(href).pathname.replace(/\/index\.html$/, "/");
}

/**
 * Open page `to` at `url` in its named tab. When that tab is open on the
 * same page, it is brought forward and sent `request` (`{connect}`,
 * `{show}`; null for none) rather than loaded again; otherwise `url`,
 * which carries the same request, is loaded there. A browser that opens
 * no tab (a blocker) loads `url` in this one, or, with `here: false`,
 * nothing. Returns what was done: "told", "opened", "here" or "blocked".
 */
export function openSiteTab(to, url, request = null, { here = true } = {}, win = globalThis) {
  let tab = null;
  try {
    tab = win.open("", TAB_NAMES[to]);
  } catch {
    tab = null;
  }
  if (!tab) {
    if (!here) return "blocked";
    win.location.assign(url);
    return "here";
  }
  let same = false;
  try {
    // Another origin's page throws here: loaded with ours.
    same = tab.location.href !== "about:blank" && pagePath(tab.location.href) === pagePath(new URL(url, win.location.href).href);
  } catch {
    same = false;
  }
  if (!same) {
    tab.location.href = new URL(url, win.location.href).href;
    return "opened";
  }
  if (request) {
    const nonce = Math.random().toString(36).slice(2);
    const target = new URL(url, win.location.href).href;
    let timer = 0;
    const onAck = (e) => {
      if (e.source !== tab || e.origin !== win.location.origin || e.data?.type !== SITE_MESSAGE || e.data.ack !== nonce) return;
      win.clearTimeout?.(timer);
      win.removeEventListener?.("message", onAck);
    };
    win.addEventListener?.("message", onAck);
    const loading = () => {
      try {
        return tab.document.readyState !== "complete";
      } catch {
        return false;
      }
    };
    const wasLoading = loading();
    timer = (win.setTimeout ?? setTimeout)(() => {
      win.removeEventListener?.("message", onAck);
      // Not taken by a tab that was loading: the URL carries the request.
      if (!wasLoading && !loading()) return;
      try {
        tab.location.href = target;
      } catch {
        // A tab that went away.
      }
    }, ACK_MS);
    tab.postMessage({ type: SITE_MESSAGE, ...request, nonce }, win.location.origin);
  }
  tab.focus();
  return "told";
}

/**
 * Link `a` opens its page in that page's tab (`openSiteTab`) with
 * `request()`'s request; a click with a modifier (a new tab, a new
 * window) or another button is the browser's own.
 */
export function linkTab(a, to, request = () => null, win = globalThis) {
  a.addEventListener("click", (e) => {
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    openSiteTab(to, a.href, request(), {}, win);
  });
}

/**
 * Call `fn(message)` for each request another saturnus tab sends this one
 * (`openSiteTab`): of this origin, and from the tab that opened this one
 * or one this one opened, so another page of the origin cannot drive it.
 */
export function onSiteMessage(fn, win = globalThis) {
  win.addEventListener("message", (e) => {
    if (e.origin !== win.location.origin || e.data?.type !== SITE_MESSAGE || !related(e.source, win)) return;
    // An answer to a request of this tab's (`openSiteTab`) is not a request.
    if (e.data.ack !== undefined) return;
    // Taken: the sender does not load this tab again.
    if (typeof e.data.nonce === "string") {
      try {
        e.source.postMessage({ type: SITE_MESSAGE, ack: e.data.nonce }, win.location.origin);
      } catch {
        // The sender went away.
      }
    }
    fn(e.data);
  });
}

/** Whether window `w` opened `win`, or `win` opened it. */
function related(w, win) {
  if (!w || w === win) return false;
  try {
    return w === win.opener || w.opener === win;
  } catch {
    return false;
  }
}

// Loaded by the page (a module script in tx.html, app.js's import in the
// emulator), it sets the page's links at once; not in Node's tests.
if (typeof document !== "undefined") applySiteLinks();
