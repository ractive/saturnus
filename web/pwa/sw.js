// The installed page's service worker. web/site.sh puts two constants in
// front of this file: BUILD, a hash of every file the site ships, and
// FILES, those files relative to this worker (the page, its modules, the
// wasm package, the manifest and the icons). Never registered in the
// desktop app (web/pwa.js), and absent from it (web/site.sh ships it, the
// app's build.rs does not).
//
// Install: every file of this build into the cache `saturnus-<BUILD>`,
// fetched past the HTTP cache, so the glue JS and the wasm always come
// from the same build. Then it waits: a page that runs the old build keeps
// it until the page asks for the new one (`skip`) or every page has
// closed. Activate: the other builds' caches go. Fetch: a GET of one of
// these files is answered from the cache, a navigation to the page (its
// directory or index.html) with the cached index.html; anything else goes to the network untouched and is not
// kept (the page asks for nothing else; ROMs and states are in IndexedDB).
/* global BUILD, FILES */

const PREFIX = "saturnus-";
const CACHE = `${PREFIX}${BUILD}`;
const SCOPE = new URL("./", self.location.href);
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
    for (const name of await caches.keys()) {
      if (name.startsWith(PREFIX) && name !== CACHE) await caches.delete(name);
    }
    await self.clients.claim();
  })());
});

self.addEventListener("message", (event) => {
  if (event.data?.type === "skip") self.skipWaiting();
  else if (event.data?.type === "build") event.source?.postMessage({ type: "build", build: BUILD });
});

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
