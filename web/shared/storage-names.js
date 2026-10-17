// Every name the app stores under on its origin, in one place: its
// IndexedDB databases, its localStorage keys, its directory of the origin
// private file system and the prefix of its service worker's caches. The
// modules that store import their names from here (states.js,
// romstore.js, app.js, theme.js, editor.js, tx/); theme-boot.js, a
// classic script, repeats two of them. The values stay what they were
// when Calculator and Files were two pages (iteration 50d), so the kept
// ROMs, states, settings and folders are found again:
// web/test/storage-names.test.mjs pins them and checks that the names
// here are the ones the code uses.

/** The saved states: database `saturnus`, store `states` (web/states.js). */
export const STATE_DB = "saturnus";
/** The kept ROMs: database `saturnus-roms`, stores `slots` and `images` (web/romstore.js). */
export const ROM_DB = "saturnus-roms";

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
  /** Set the clock on every start (web/clock.js). */
  clockOnStart: "saturnus.clockOnStart",
  /** The palette editor's history (web/editor.js). */
  sent: "saturnus.sent",
  /** Whether the tour was offered (web/tour.js). */
  tour: "saturnus.tour",
  /** The old view setting, removed on load (web/app.js). */
  view: "saturnus.view",
};

/**
 * Files' localStorage keys, all under `saturnus.transfer.`. Besides its
 * own database (`TRANSFER_DB`) and "Files" directory
 * (`TRANSFER_OPFS_DIR`) it reads the kept ROMs (`ROM_DB`, read only,
 * web/tx/kept-roms.js) and shows the calculators through the hub (which
 * alone writes their states, web/hub.js).
 */
export const TRANSFER_KEYS = {
  /** The connection settings: find automatically, speed, parity (web/tx/app.js). */
  link: "saturnus.transfer.link",
  /** The model last chosen in the connect dialog. */
  model: "saturnus.transfer.model",
  /** The serial port last used, by its name (the desktop app's port list). */
  port: "saturnus.transfer.port",
};

/**
 * Files' database: the folders the user picked with the File
 * System Access API, the last few of them kept in store `FOLDER_STORE`
 * (key an id, value `{handle, name, used}`), the pane's "Recent folders",
 * so the next visit only asks for the permission again (web/tx/folder.js).
 */
export const TRANSFER_DB = "saturnus-transfer";
/** The store of `TRANSFER_DB` that keeps the picked folders' handles. */
export const FOLDER_STORE = "folders";
/** The picker's id: the browser starts the next pick where this one ended (`showDirectoryPicker`). */
export const FOLDER_PICKER_ID = "saturnus-transfer";
/**
 * The directory of the origin private file system (shared by every page
 * of the origin) that holds Files' own "Files" where there is no
 * directory picker (web/tx/folder.js).
 */
export const TRANSFER_OPFS_DIR = "saturnus-transfer-files";

/** The IndexedDB databases, all of them. */
export const DATABASES = [STATE_DB, ROM_DB, TRANSFER_DB];
/** The localStorage keys, all of them. */
export const LOCAL_STORAGE_KEYS = [...Object.values(KEYS), ...Object.values(TRANSFER_KEYS)];

/**
 * The prefix of the service worker's cache names for the worker at
 * `scopePath` (web/pwa/sw.js, which cannot import and repeats the rule):
 * `saturnus:/saturnus/app/:` on the site. The colon after the path keeps
 * `/saturnus/` apart from `/saturnus/app/`.
 */
export const cachePrefix = (scopePath) => `saturnus:${scopePath}:`;
