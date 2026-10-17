// The kill switch for the service worker the emulator registered while it
// lived at /saturnus/ (before iteration 36d moved it to
// /saturnus/emulator/). That worker is scoped to /saturnus/, and a 404 on
// its script would not unregister it: left alone it would keep serving
// the old emulator at /saturnus/ from its cache. Shipped at
// /saturnus/sw.js by web/site.sh, so the browser's next update check
// installs this in its place. No fetch handler. Install: take over at
// once. Activate: delete the old worker's caches (the names starting with
// `saturnus:/saturnus/:`, the colon included, so the emulator's
// `saturnus:/saturnus/emulator/:` caches survive), unregister, and
// navigate the pages this worker controls to their own URL, so they load
// the landing page from the network. Kept ROMs and states are in the
// origin's IndexedDB and are not touched.
//
// Also shipped as /saturnus/emulator/sw.js and /saturnus/transfer/sw.js
// (iteration 50d), where it deletes those subsites' caches and sends their
// windows to /saturnus/app/.
//
// Remove on 2027-01-15 (kb/backlog/remove-kill-switch.md): take it out of
// web/site.sh, and the next deploy deletes /saturnus/sw.js from the server.
// Tested by web/test/killswitch.test.mjs.

const SCOPE = new URL("./", self.location.href);
// The same file serves as the old subsites' kill switch (emulator/sw.js,
// transfer/sw.js, iteration 50d): nothing is served there any more, so
// their windows go to the one app instead of reloading into a 404.
const SUBSITE = /\/(emulator|transfer)\/$/.test(SCOPE.pathname);
const PREFIX = `saturnus:${SCOPE.pathname}:`;

self.addEventListener("install", (event) => {
  event.waitUntil(self.skipWaiting());
});

self.addEventListener("activate", (event) => {
  event.waitUntil((async () => {
    for (const name of await caches.keys()) {
      if (name.startsWith(PREFIX)) await caches.delete(name);
    }
    await self.registration.unregister();
    // Only the windows this worker controls (matchAll's default), each
    // reloaded at its own URL.
    for (const client of await self.clients.matchAll({ type: "window" })) {
      await client.navigate(SUBSITE ? new URL("app/", new URL("..", SCOPE)).href : client.url).catch(() => null);
    }
  })());
});
