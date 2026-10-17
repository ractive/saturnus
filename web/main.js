// The emulator page (web/index.html): a thin host for the calculator view
// (app.js `mountCalculator`), always shown.

import { mountCalculator } from "./app.js";

mountCalculator(document.getElementById("calculator-view"), { shown: () => true });
