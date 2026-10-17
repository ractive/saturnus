// The home page (`#/`, iteration 50d): the calculators this browser
// keeps (their ROMs, web/romstore.js, read through web/tx/kept-roms.js),
// each with Calculator and Files, those that run now marked (the hub's
// `engines`, web/protocol.md); "New calculator"; "Connect a calculator"
// (a calculator on a serial cable, in Files; where the browser has no Web
// Serial, the line that says why). Rendered each time it is shown.

import { el } from "./shared/dom.js";
import { HubClient, hubRunning } from "./shared/calc-hub.js";
import { iconEl } from "./shared/components/icons.js";
import { keptRoms } from "./tx/kept-roms.js";
import { hasWebSerial } from "./tx/links.js";
import "./shared/models.js";

/** How long the home page waits for the hub's running list before it leaves the marks out. */
const ENGINES_MS = 2000;

const ORDER = globalThis.SATURNUS_MODEL_ORDER;
const TITLES = globalThis.SATURNUS_MODEL_TITLES;
const SERIAL = globalThis.SATURNUS_SERIAL;

/** The logo (as in the headers), drawn in a model's colours by its `data-logo` (style.css). */
const LOGO = '<svg class="logo" viewBox="0 0 32 32" width="36" height="36" aria-hidden="true"><path class="ring" d="M 1.6 16 A 14.4 4.6 0 0 1 30.4 16" transform="rotate(-24 16 16)"/><circle class="planet" cx="16" cy="16" r="8.6"/><path class="ring" d="M 30.4 16 A 14.4 4.6 0 0 1 1.6 16" transform="rotate(-24 16 16)"/></svg>';

function logo() {
  const t = document.createElement("template");
  t.innerHTML = LOGO;
  return t.content.firstElementChild;
}

/** Why "Connect a calculator" is not offered in this browser. */
export const NO_SERIAL_PORT = "A calculator on a cable needs Chrome or Edge on a computer: this browser cannot reach a serial port.";

/**
 * The rows of the home page's list (pure; tested): one per kept ROM in
 * the page's model order, `{model, title, romName, running, files}`
 * (`files`: Files can reach it, a model with a serial port).
 */
export function homeRows(kept, engines = []) {
  const running = new Set(engines.map((e) => e.model));
  return ORDER.flatMap((model) => {
    const r = kept.find((k) => k.model === model);
    if (!r) return [];
    return [{ model, title: TITLES[model] ?? model, romName: r.name, running: running.has(model), files: SERIAL.includes(model) }];
  });
}

/**
 * The home page in `root` (`#home-view`): `render()` reads the kept ROMs
 * and the running calculators again and draws the list. `on.newCalculator`
 * and `on.connect` are the shell's (web/main.js).
 */
export class Home {
  constructor(root, { on = {}, serial = hasWebSerial } = {}) {
    this.root = root;
    this.on = on;
    this.serial = serial;
    this.list = root.querySelector("#home-list");
    this.empty = root.querySelector("#home-empty");
    this.connectLine = root.querySelector("#home-connect-why");
    this.hub = null;
    this.drawn = 0;
    root.querySelector("#home-new").addEventListener("click", () => this.on.newCalculator?.());
    const connect = root.querySelector("#home-connect");
    connect.addEventListener("click", () => this.on.connect?.());
    const can = this.serial();
    connect.hidden = !can;
    this.connectLine.hidden = can;
    this.connectLine.textContent = can ? "" : NO_SERIAL_PORT;
  }

  /** The running calculators: none while the hub has not started (no view needed one). */
  async engines() {
    if (!hubRunning()) return [];
    this.hub ??= new HubClient({ who: "home" });
    try {
      return await this.hub.request("engines");
    } catch {
      return [];
    }
  }

  async render() {
    const turn = ++this.drawn;
    const kept = await keptRoms(globalThis.indexedDB, ORDER, null);
    // A newer render started meanwhile: it draws.
    if (turn !== this.drawn) return;
    this.draw(kept, [], turn);
    // The running marks when the hub answers; a busy or stuck hub never holds the list.
    const engines = await Promise.race([this.engines(), new Promise((r) => setTimeout(() => r(null), ENGINES_MS))]);
    if (engines?.length && turn === this.drawn) this.draw(kept, engines, turn);
  }

  draw(kept, engines, turn) {
    const rows = homeRows(kept, engines);
    this.list.replaceChildren(...rows.map((r) => this.row(r)));
    this.list.hidden = rows.length === 0;
    this.empty.hidden = rows.length > 0;
    this.root.dataset.rendered = String(turn);
  }

  row(r) {
    const title = el("h3", { class: "home-title" }, el("span", { text: r.title }));
    if (r.running) title.append(el("span", { class: "home-running", text: "Running" }));
    const actions = el("div", { class: "home-actions" },
      el("a", { class: "button primary", href: `#/calculator/${r.model}`, "data-model": r.model, "data-go": "calculator" }, iconEl("calculator", "ic-sm"), el("span", { text: "Calculator" })),
    );
    if (r.files) {
      // Before Calculator, so Calculator lines up on the right in every row.
      actions.prepend(el("a", { class: "button", href: `#/files/${r.model}`, "data-model": r.model, "data-go": "files", title: `Files on the ${r.title}: move files between it and this computer` }, iconEl("copy-to", "ic-sm"), el("span", { text: "Files" })));
    }
    return el("li", { class: `home-calc${r.running ? " running" : ""}`, "data-model": r.model, "data-logo": r.model },
      logo(),
      el("div", { class: "home-place" }, title, el("p", { class: "home-rom muted", text: r.romName })),
      actions,
    );
  }
}
