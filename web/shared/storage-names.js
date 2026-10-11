// Every name the emulator page stores under on its origin, in one place:
// its IndexedDB databases, its localStorage keys and the prefix of its
// service worker's caches. The modules that store import their names
// from here (states.js, romstore.js, app.js, theme.js, editor.js);
// theme-boot.js, a classic script, repeats two of them. The other pages
// of ractive.ch/saturnus/ (the landing page, saturnus tx at /saturnus/transfer/)
// must not share any of these: web/test/storage-names.test.mjs checks
// that the names here are the ones the code uses and that each page's
// set is disjoint from the others'.

/** The saved states: database `saturnus`, store `states` (web/states.js). */
export const STATE_DB = "saturnus";
/** The kept ROMs: database `saturnus-roms`, stores `slots` and `images` (web/romstore.js). */
export const ROM_DB = "saturnus-roms";
/** The IndexedDB databases, all of them. */
export const DATABASES = [STATE_DB, ROM_DB];

/** The localStorage keys, by preference. */
export const KEYS = {
  model: "saturnus.model",
  speed: "saturnus.speed",
  panel: "saturnus.panel",
  layer: "saturnus.layer",
  layerTab: "saturnus.layerTab",
  panelWidth: "saturnus.panelWidth",
  layerWidth: "saturnus.layerWidth",
  treeWidth: "saturnus.treeWidth",
  keys: "saturnus.keys",
  storageAsk: "saturnus.storageAsk",
  screenLook: "saturnus.screenLook",
  theme: "saturnus.theme",
  /** The palette editor's history (web/editor.js). */
  sent: "saturnus.sent",
  /** Whether the tour was offered (web/tour.js). */
  tour: "saturnus.tour",
  /** The old view setting, removed on load (web/app.js). */
  view: "saturnus.view",
};
/** The localStorage keys, all of them. */
export const LOCAL_STORAGE_KEYS = Object.values(KEYS);

/**
 * The prefix of the service worker's cache names for the worker at
 * `scopePath` (web/pwa/sw.js, which cannot import and repeats the rule):
 * `saturnus:/saturnus/emulator/:` on the site. The colon after the path
 * keeps `/saturnus/` apart from `/saturnus/emulator/`.
 */
export const cachePrefix = (scopePath) => `saturnus:${scopePath}:`;

/** The emulator's names, as a page on the origin: for the disjointness check. */
export const EMULATOR = {
  databases: DATABASES,
  localStorage: LOCAL_STORAGE_KEYS,
  cachePrefix: cachePrefix("/saturnus/emulator/"),
};

/**
 * The transfer page's (saturnus tx, /saturnus/transfer/) localStorage
 * keys, all under `saturnus.transfer.`. It opens no database of its own;
 * it reads the emulator's kept ROMs (`ROM_DB`, read only, web/tx/kept-roms.js)
 * and the theme (`KEYS.theme`, the one key both pages read and set).
 */
export const TRANSFER_KEYS = {
  /** The connection settings: find automatically, speed, parity (web/tx/app.js). */
  link: "saturnus.transfer.link",
  /** The model last chosen in the connect dialog. */
  model: "saturnus.transfer.model",
};

/**
 * The other pages of /saturnus/ and what they store. The landing page
 * stores nothing.
 */
export const OTHER_PAGES = {
  landing: { databases: [], localStorage: [], cachePrefix: cachePrefix("/saturnus/") },
  transfer: { databases: [], localStorage: Object.values(TRANSFER_KEYS), cachePrefix: cachePrefix("/saturnus/transfer/") },
};
