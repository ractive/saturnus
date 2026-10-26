// Links to other sites, the same on the docs pages (web/docs/markdown.mjs,
// as HTML text), the landing page (by hand) and the app (as DOM): each
// opens in a new tab (target="_blank", rel="noopener noreferrer"), with
// a small ↗ after its text (hidden from screen readers) and " (opens in
// a new tab)" for them instead (`.visually-hidden`). In the desktop app
// a new tab may open nothing, so there the app's texts keep a site's
// name as text (their callers check `inDesktopApp`, site-links.js).
//
// A site named in the text ("hpcalc.org") is a link to it when it is one
// of SITES, the sites the texts name; any other word that looks like a
// domain (build.rs, lib.rs) stays text, and so does a site that is gone
// (hpcomm.org, not in SITES).
//
// The limit: a site named in the text is a link at most twice per
// address on a page, the page's own links to it counted (`LinkBudget`).
// A link written as one is always a link: it cites a source each time. Not counted, and always
// links: a list of sources (a docs page's "Sources" section, the landing
// page's dialogs' Sources lists, About's lists in the app) and the
// landing page's navigation in its header and footer (the same few
// links on every visit, not mentions in the text).
//
// Pure functions, but `externalLink` and `linkSites`, which build DOM.

/** The site's own address: links under it stay in the tab. */
export const SITE = "https://ractive.ch/saturnus/";

/** How often one address is a link on a page, sources and navigation aside. */
export const MAX_LINKS = 2;

/**
 * The sites a text may name and become a link, each with the address it
 * links to (a name with a path, "hp.giesselink.com/emu42.htm", links to
 * that page instead).
 */
export const SITES = {
  "hpcalc.org": "https://www.hpcalc.org/",
  "literature.hpcalc.org": "https://literature.hpcalc.org/",
  "hpmuseum.org": "https://www.hpmuseum.org/",
  "finseth.com": "https://www.finseth.com/",
  "kermitproject.org": "https://www.kermitproject.org/",
  "columbia.edu": "https://www.columbia.edu/",
  "hparchive.com": "https://www.hparchive.com/",
  "hp.giesselink.com": "https://hp.giesselink.com/",
  "thomasokken.com": "https://thomasokken.com/",
  "crates.io": "https://crates.io/",
  "github.com": "https://github.com/",
  "codeberg.org": "https://codeberg.org/",
  "wikipedia.org": "https://www.wikipedia.org/",
  "en.wikipedia.org": "https://en.wikipedia.org/",
};

/**
 * A domain-like name in running text, perhaps with a path: dot-separated
 * labels, not part of an address, a file name or an e-mail address
 * (nothing word-like, "@", ".", "/" or ":" right before it; no word or
 * further label right after it). A sentence's full stop after it is
 * fine. Sticky: tests one place. Only a name in SITES counts.
 */
const NAME = /(?<![\w@./:-])(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}(?:\/(?:[\w.~%/-]*[\w~%/])?)?(?![\w-]|\.[a-z0-9])/iy;

/** The host of a name ("www." off, lower case) and its path ("" or "/..."). */
function split(name) {
  const at = name.indexOf("/");
  const host = (at < 0 ? name : name.slice(0, at)).toLowerCase().replace(/^www\./, "");
  return { host, path: at < 0 ? "" : name.slice(at) };
}

/** The name of a site in SITES (with its path, if any) at `at` in `text`, or null. */
export function siteNameAt(text, at) {
  NAME.lastIndex = at;
  const name = NAME.exec(text)?.[0] ?? null;
  return name && Object.hasOwn(SITES, split(name).host) ? name : null;
}

/** The address a site's name links to: its entry in SITES, or with a path that page; null for a name not in SITES. */
export function siteUrl(name) {
  const { host, path } = split(name);
  if (!Object.hasOwn(SITES, host)) return null;
  if (path.length > 1) return `https://${name.slice(0, name.indexOf("/")).toLowerCase()}${path}`;
  return SITES[host];
}

/** Whether `href` is on another site: http(s), not under SITE. */
export function isExternal(href) {
  const h = String(href).trim();
  if (!/^https?:\/\//i.test(h)) return false;
  return !/^https?:\/\/(www\.)?ractive\.ch\/saturnus(\/|$|[?#])/i.test(h);
}

/** An address as the link limit counts it: host in lower case without "www.", no trailing "/". */
export function urlKey(href) {
  try {
    const u = new URL(href);
    return `${u.host.toLowerCase().replace(/^www\./, "")}${u.pathname.replace(/\/+$/, "")}${u.search}${u.hash}`;
  } catch {
    return String(href);
  }
}

/** How many links each address has had on one page: `take(href)` says whether one more may be a link (and counts it). */
export class LinkBudget {
  constructor(max = MAX_LINKS) {
    this.max = max;
    this.counts = new Map();
  }

  /** Whether `href` may be a link once more; counts it if so. */
  take(href) {
    const key = urlKey(href);
    const n = this.counts.get(key) ?? 0;
    if (n >= this.max) return false;
    this.counts.set(key, n + 1);
    return true;
  }

  /** Counts a link to `href` that is a link whatever the count. */
  add(href) {
    const key = urlKey(href);
    this.counts.set(key, (this.counts.get(key) ?? 0) + 1);
  }
}

/** The arrow after an external link's text. */
export const EXT_ARROW = "↗";
/** What a screen reader says instead of the arrow. */
export const NEW_TAB = " (opens in a new tab)";

/** The attributes of an external link, as HTML. */
export const EXT_ATTRS = 'target="_blank" rel="noopener noreferrer"';
/** The arrow and the hidden words, as HTML, after the link's text. */
export const EXT_MARKS_HTML = `<span class="ext" aria-hidden="true">${EXT_ARROW}</span><span class="visually-hidden">${NEW_TAB}</span>`;

/**
 * An <a> to another site: `children` (strings or nodes) as its text,
 * then the arrow and the hidden words. `attrs` adds attributes (a class,
 * a title); an `aria-label` gets ", opens in a new tab" (it replaces
 * the text for a screen reader).
 */
export function externalLink(href, children, attrs = {}) {
  const a = document.createElement("a");
  a.href = href;
  a.target = "_blank";
  a.rel = "noopener noreferrer";
  for (const [k, v] of Object.entries(attrs)) {
    if (v === null || v === undefined || v === false) continue;
    if (k === "class") a.className = v;
    else a.setAttribute(k, k === "aria-label" ? `${v}, opens in a new tab` : v);
  }
  a.append(...[children].flat());
  const arrow = document.createElement("span");
  arrow.className = "ext";
  arrow.setAttribute("aria-hidden", "true");
  arrow.textContent = EXT_ARROW;
  const hidden = document.createElement("span");
  hidden.className = "visually-hidden";
  hidden.textContent = NEW_TAB;
  a.append(arrow, hidden);
  return a;
}

/**
 * `text` as a DocumentFragment, each site named in it a link while
 * `budget` allows (one per call by default: a page's own LinkBudget
 * counts across its texts). The text is text nodes: nothing in it is
 * read as HTML.
 */
export function linkSites(text, budget = new LinkBudget()) {
  const frag = document.createDocumentFragment();
  let plain = "";
  for (let i = 0; i < text.length; ) {
    const name = siteNameAt(text, i);
    const url = name ? siteUrl(name) : null;
    if (name && url && budget.take(url)) {
      if (plain) frag.append(plain);
      plain = "";
      frag.append(externalLink(url, name));
      i += name.length;
    } else if (name) {
      plain += name;
      i += name.length;
    } else {
      plain += text[i++];
    }
  }
  if (plain) frag.append(plain);
  return frag;
}
