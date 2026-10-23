// Links to other sites, the same on the docs pages (web/docs/markdown.mjs,
// as HTML text), the landing page (by hand) and the app (as DOM): each
// opens in a new tab (target="_blank", rel="noopener noreferrer"), with
// a small ↗ after its text (hidden from screen readers) and " (opens in
// a new tab)" for them instead (`.visually-hidden`). A site named in the
// text ("hpcalc.org") is a link to it, at most twice per address on a
// page, counting the page's own links to that address (`LinkBudget`);
// a list of sources does not count and is always links. A site that is
// gone (DEAD_SITES) stays text. Pure functions, but `externalLink` and
// `linkSites`, which build DOM.

/** The site's own address: links under it stay in the tab. */
export const SITE = "https://ractive.ch/saturnus/";

/** How often one address is a link on a page, sources aside. */
export const MAX_LINKS = 2;

/** Sites named in the texts that no longer exist: their names stay text. */
export const DEAD_SITES = new Set(["hpcomm.org"]);

/** The address of a site named in a text, where it is not simply https://<name>/. */
const SITE_URLS = {
  "hpcalc.org": "https://www.hpcalc.org/",
  "hpmuseum.org": "https://www.hpmuseum.org/",
  "finseth.com": "https://www.finseth.com/",
  "kermitproject.org": "https://www.kermitproject.org/",
  "columbia.edu": "https://www.columbia.edu/",
  "hparchive.com": "https://www.hparchive.com/",
  "wikipedia.org": "https://www.wikipedia.org/",
};

/**
 * A site's name in running text, perhaps with a path
 * ("hp.giesselink.com/emu42.htm"): dot-separated labels ending in one of
 * these top-level domains, not part of an address, a file name or an
 * e-mail address (nothing word-like, "@", ".", "/" or ":" right before
 * it; no word or further label right after it). A sentence's full stop
 * after it is fine. Sticky: tests one place.
 */
const TLDS = "org|com|net|edu|io|ch|de|fr|uk|jp|rs|dev|gov";
const SITE_NAME = new RegExp(`(?<![\\w@./:-])(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\\.)+(?:${TLDS})(?:/(?:[\\w.~%/-]*[\\w~%/])?)?(?![\\w-]|\\.[a-z0-9])`, "iy");

/** The site name (and path) at `at` in `text`, or null. */
export function siteNameAt(text, at) {
  SITE_NAME.lastIndex = at;
  return SITE_NAME.exec(text)?.[0] ?? null;
}

/**
 * The address a site name links to: a name alone (lower case, "www."
 * off) its page in SITE_URLS or https://<name>/, a name with a path
 * https://<name><path>; null for a dead site.
 */
export function siteUrl(name) {
  const at = name.indexOf("/");
  const host = (at < 0 ? name : name.slice(0, at)).toLowerCase();
  const bare = host.replace(/^www\./, "");
  if (DEAD_SITES.has(bare)) return null;
  if (at >= 0 && at < name.length - 1) return `https://${host}${name.slice(at)}`;
  return SITE_URLS[bare] ?? `https://${bare}/`;
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
