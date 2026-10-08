// The installed page's service worker. web/site.sh puts two constants in
// front of this file: BUILD, a hash of every file the site ships, and
// FILES, those files relative to this worker (the page, its modules, the
// wasm package, the manifest and the icons). Never registered in the
// desktop app (web/pwa.js), and absent from it (web/site.sh ships it, the
// app's build.rs does not).
//
// Install: every file of this build into the cache
// `saturnus:<scope path>:<BUILD>`, fetched past the HTTP cache, so the glue
// JS and the wasm always come from the same build. Then it waits: pages
// that run the old build keep it until every one has closed. A page asks
// for the new build (`skip`) and gets it only when it is the sole page
// open; otherwise it hears `others` and the new build waits, as taking
// over would hand the other pages, mid-session, the next build's files.
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
const KNOWN = new Set(FILES.map(urlOf));

self.addEventListener("install", (event) => {
  event.waitUntil((async () => {
    const cache = await caches.open(CACHE);
    await cache.addAll(FILES.map((f) => new Request(urlOf(f), { cache: "reload" })));
  })());
});

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
