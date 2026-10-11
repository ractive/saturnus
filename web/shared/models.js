// The order the page lists models in (the host's own order may differ)
// and each model's title. Globals, so both the boot script (theme-boot.js,
// a classic script in index.html's head that cannot import: the first
// model is the one shown with none remembered) and the modules (web/norom.js
// imports this file and exports them as `MODEL_ORDER` and `MODEL_TITLES`)
// read one list.
globalThis.SATURNUS_MODEL_ORDER = Object.freeze(["48sx", "48gx", "49g", "38g", "39g", "40g", "42s"]);
globalThis.SATURNUS_MODEL_TITLES = Object.freeze({
  "48sx": "HP 48SX",
  "48gx": "HP 48GX",
  "38g": "HP 38G",
  "49g": "HP 49G",
  "39g": "HP 39G",
  "40g": "HP 40G",
  "42s": "HP 42S",
});
