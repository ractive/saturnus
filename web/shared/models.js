// The order the page lists models in (the host's own order may differ)
// and each model's title. Globals, so both the boot script (theme-boot.js,
// a classic script in index.html's head that cannot import: the first
// model is the one shown with none remembered) and the modules (web/norom.js
// imports this file and exports them as `MODEL_ORDER` and `MODEL_TITLES`)
// read one list.
globalThis.SATURNUS_MODEL_ORDER = Object.freeze(["48sx", "48gx", "49g", "38g", "39g", "40g", "42s", "27s", "17bii", "17b", "28s"]);
// The Pioneer models (one Lewis chip): no download, no serial port, a
// 131x16 display, letters only from ALPHA menus.
globalThis.SATURNUS_PIONEERS = Object.freeze(["42s", "27s", "17bii", "17b"]);
// The models whose ROM HP never published, read out of one's own
// calculator: the Pioneers and the 28S (two Lewis chips). None of them
// has a serial port.
globalThis.SATURNUS_DUMP_ONLY = Object.freeze(["42s", "27s", "17bii", "17b", "28s"]);
globalThis.SATURNUS_MODEL_TITLES = Object.freeze({
  "48sx": "HP 48SX",
  "48gx": "HP 48GX",
  "38g": "HP 38G",
  "49g": "HP 49G",
  "39g": "HP 39G",
  "40g": "HP 40G",
  "42s": "HP 42S",
  "27s": "HP 27S",
  "17bii": "HP 17BII",
  "17b": "HP 17B",
  "28s": "HP 28S",
});
