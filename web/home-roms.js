// The ROM commands of the home page (iteration 53), the one place where
// calculators get a ROM and lose it (web/protocol.md, "ROM slots"). In
// the browser through the calculators' hub (web/hub.js) as a connection
// of its own ("home"), which keeps a chosen ROM without booting it
// (`chooseRom` with `boot: false`, the result's `start` the model to
// open): the home page shows no calculator, and choosing a ROM stops no
// running one. In the desktop app through the document's one
// TauriBackend (web/native-hub.js): its dialogs choose and download, and
// it boots what it keeps (it runs one calculator). Both give `romSource`,
// `romSlots()`, `chooseRom(model, files)`, `takeOffer(model, offer)`,
// `forgetRom(model)` (null: every one), `romSettings(bootLast)` and
// `running()` (the models whose calculator runs); the app also
// `downloadRom(model)`.

import { HubClient, hubRunning } from "./shared/calc-hub.js";
import { MAX_ROM_FILE } from "./shared/limits.js";
import { ROM_HOLDING_STATES, autoKey, dbDelete } from "./states.js";

/** The browser's: the hub, joined on the first command (`started()` says whether it was). */
export function hubRoms({ client = () => new HubClient({ who: "home" }) } = {}) {
  let hub = null;
  const request = (cmd, args = {}) => {
    hub ??= client();
    return hub.request(cmd, args);
  };
  return {
    romSource: "file",
    started: () => hub !== null,
    romSlots: () => request("romSlots"),
    /** ROM files (Files) for `model`: read here, kept by the hub, booted by nobody. */
    async chooseRom(model, files) {
      const list = await Promise.all([...files].map(async (f) => ({
        name: f.name,
        rom: f.size > MAX_ROM_FILE ? new Uint8Array(0) : new Uint8Array(await f.arrayBuffer()),
      })));
      return request("chooseRom", { model, files: list, boot: false });
    },
    takeOffer: (model, offer) => request("chooseRom", { model, offer, boot: false }),
    /**
     * Forget `model`'s ROM (every ROM: null); the hub stops the calculators
     * that run from it first. The saved states that hold a copy of it go
     * too (`ROM_HOLDING_STATES`), the user's and the auto-saved one.
     */
    async forgetRom(model = null) {
      const r = await request("forgetRom", model ? { model } : {});
      for (const m of ROM_HOLDING_STATES) {
        if (model === null || model === m) {
          await dbDelete(m);
          await dbDelete(autoKey(m));
        }
      }
      return r;
    },
    romSettings: (bootLast) => request("romSettings", { bootLast }),
    /** The running calculators' models: none while the hub has not started (no view needed one). */
    async running() {
      if (!hubRunning()) return [];
      try {
        return (await request("engines")).map((e) => e.model);
      } catch {
        return [];
      }
    },
  };
}

/** The desktop app's: the native hub's TauriBackend (`hub`, web/native-hub.js). */
export function appRoms(hub) {
  const b = hub.backend;
  const running = async () => {
    const m = await hub.knownRunning();
    return m ? [m] : [];
  };
  return {
    romSource: "dialog",
    started: () => true,
    romSlots: () => b.romSlots(),
    /** The app's file dialog; null if cancelled. */
    chooseRom: (model) => b.chooseRom(model),
    downloadRom: (model) => b.downloadRom(model),
    takeOffer: (model, offer) => b.takeOffer(model, offer),
    /** The calculator stops first when it runs from it (refused while a send types: nothing is forgotten). */
    async forgetRom(model = null) {
      const now = await running();
      if (now.some((m) => model === null || m === model)) await b.unload();
      return b.forgetRom(model);
    },
    romSettings: (bootLast) => b.romSettings(bootLast),
    running,
  };
}
