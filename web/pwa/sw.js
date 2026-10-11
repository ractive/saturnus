// The installed page's service worker. web/site.sh puts two constants in
// front of this file: BUILD, a hash of every file the site ships, and
// FILES, those files relative to this worker (the page, its modules, the
// wasm package, the manifest and the icons), each as [path, SHA-256 in
// hex]. Never registered in the desktop app (web/pwa.js), and absent from
// it (web/site.sh ships it, the app's build.rs does not).
//
// Install: every file of this build, fetched past the HTTP cache, its
// body's SHA-256 checked against FILES, and only when every one matches
// all go into the cache `saturnus:<scope path>:<BUILD>`; so the glue JS
// and the wasm always come from the same build. A file that does not
// match (a deploy still uploading, the server serving an older copy) or
// does not come fails the install: nothing is cached, the browser keeps
// the worker it has and tries the new one again on a later visit (the
// deploy uploads sw.js before most files, kb/decision-log.md). Then it
// waits: pages that run the old build keep it until every one has closed.
// A page asks for the new build (`skip`) and gets it only when it is the
// sole page open; otherwise it hears `others` and the new build waits, as
// taking over would hand the other pages, mid-session, the next build's
// files.
// Activate: this scope's other builds' caches go (other deployments on
// the same origin keep theirs); the worker claims the open pages only on
// the first install, when no page runs an older build. Fetch: a GET of
// one of these files is answered from the cache, a navigation to the page
// (its directory or index.html) with the cached index.html; anything else
// goes to the network untouched and is not kept (the page asks for
// nothing else; ROMs and states are in IndexedDB).
/* global BUILD, FILES */

const SCOPE = new URL("./", self.location.href);
const PREFIX = `saturnus:${SCOPE.pathname}:`;
const CACHE = `${PREFIX}${BUILD}`;
const urlOf = (f) => new URL(f, SCOPE).href;
const KNOWN = new Set(FILES.map(([f]) => urlOf(f)));

self.addEventListener("install", (event) => {
  event.waitUntil(precache());
});

/** Every file of FILES, checked against its hash, into CACHE; throws, caching nothing, on the first that does not match. */
async function precache() {
  const checked = await Promise.all(FILES.map(async ([f, hash]) => {
    const res = await fetch(new Request(urlOf(f), { cache: "reload" }));
    if (!res.ok) throw new Error(`${f}: HTTP ${res.status}`);
    const got = hex(await crypto.subtle.digest("SHA-256", await res.clone().arrayBuffer()));
    if (got !== hash) throw new Error(`${f} is not this build's (SHA-256 ${got}, not ${hash})`);
    return [f, res];
  }));
  const cache = await caches.open(CACHE);
  await Promise.all(checked.map(([f, res]) => cache.put(urlOf(f), res)));
}

/** A digest as lowercase hex, as sha256sum prints it. */
const hex = (buf) => [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, "0")).join("");

self.addEventListener("activate", (event) => {
  event.waitUntil((async () => {
    let first = true;
    for (const name of await caches.keys()) {
      if (!name.startsWith(PREFIX) || name === CACHE) continue;
      first = false;
      await caches.delete(name);
    }
    if (first) await self.clients.claim();
  })());
});

self.addEventListener("message", (event) => {
  if (event.data?.type === "skip") event.waitUntil(skipIfAlone(event.source));
  else if (event.data?.type === "build") event.source?.postMessage({ type: "build", build: BUILD });
});

/** Take over only when `source` is the one page open; else tell it so. */
async function skipIfAlone(source) {
  const pages = await self.clients.matchAll({ type: "window" });
  if (pages.every((p) => p.id === source?.id)) await self.skipWaiting();
  else source?.postMessage({ type: "others" });
}

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);
  url.hash = "";
  // The page itself, with or without a query; every other URL as it is.
  const page = req.mode === "navigate" && url.origin === SCOPE.origin && [SCOPE.pathname, `${SCOPE.pathname}index.html`].includes(url.pathname);
  let key = null;
  if (page) key = urlOf("index.html");
  else if (KNOWN.has(url.href)) key = url.href;
  if (!key) return;
  event.respondWith((async () => {
    const hit = await caches.match(key, { cacheName: CACHE });
    return hit ?? fetch(req);
  })());
});
