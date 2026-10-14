// The order the page lists models in (the host's own order may differ)
// and each model's title. Globals, so both the boot script (theme-boot.js,
// a classic script in index.html's head that cannot import: the first
// model is the one shown with none remembered) and the modules (web/norom.js
// imports this file and exports them as `MODEL_ORDER` and `MODEL_TITLES`)
// read one list.
globalThis.SATURNUS_MODEL_ORDER = Object.freeze(["48sx", "48gx", "49g", "38g", "39g", "40g", "42s", "27s", "17bii", "17b", "28s", "19bii"]);
// The Pioneer models (one Lewis chip): no download, no serial port, a
// 131x16 display, letters only from ALPHA menus.
globalThis.SATURNUS_PIONEERS = Object.freeze(["42s", "27s", "17bii", "17b"]);
// The models whose ROM HP never published, read out of one's own
// calculator: the Pioneers and the 28S (two Lewis chips).
globalThis.SATURNUS_DUMP_ONLY = Object.freeze(["42s", "27s", "17bii", "17b", "28s"]);
// The models whose ROM was never published and cannot be read out of the
// calculator either (the 19BII has no memory scanner): no download, no
// way for saturnus to get it; the ROM is chosen by file.
globalThis.SATURNUS_NO_ROM_SOURCE = Object.freeze(["19bii"]);
// The models without a serial port (IR printer only): the Pioneers, the
// 28S and the 19BII.
globalThis.SATURNUS_NO_SERIAL = Object.freeze([...globalThis.SATURNUS_DUMP_ONLY, ...globalThis.SATURNUS_NO_ROM_SOURCE]);
// The models with a serial port and a Kermit server: saturnus tx connects
// to them (web/tx/kept-roms.js's SERIAL_MODELS).
globalThis.SATURNUS_SERIAL = Object.freeze(["48sx", "48gx", "49g"]);
// The aplet models: they drive transfers themselves, saturnus tx serves
// a folder to them (web/tx/kept-roms.js's APLET_MODELS).
globalThis.SATURNUS_APLETS = Object.freeze(["38g", "39g", "40g"]);
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
  "19bii": "HP 19BII",
});
