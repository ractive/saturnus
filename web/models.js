// The order the page lists models in; the host's own order may differ. A
// global, so both the boot script (theme-boot.js, a classic script in
// index.html's head that cannot import: the first model is the one shown
// with none remembered) and the modules (web/norom.js imports this file
// and exports it as `MODEL_ORDER`) read one list.
globalThis.SATURNUS_MODEL_ORDER = Object.freeze(["48sx", "48gx", "49g", "38g", "39g", "40g", "42s"]);
