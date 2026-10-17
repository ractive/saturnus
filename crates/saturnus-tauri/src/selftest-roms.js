// Debug builds only (SATURNUS_SELFTEST=<a ROM path>,
// SATURNUS_SELFTEST_SCRIPT=roms, SATURNUS_SELFTEST_PHASE=...): the
// remembered ROMs, driven through the home page in the app's webview
// (iteration 53: calculators get their ROM there, and open in
// Calculator). The ROM dialog is answered by the hook with the ROM of the
// variable; the page never sends a path. Run the phases in order on one
// SATURNUS_SETTINGS_DIR (kb: iterations/iteration-20-remember-roms):
// `choose` (a ROM chosen in a folder that holds the others), `restart`
// (a relaunch opens the last calculator), `missing` (after a ROM was
// moved away, opening its calculator reports it), and `hold38g` (open
// the 38G, then wait to be killed: the choice is already saved). Apart:
// `download` (iteration 20b), which downloads from hpcalc.org.
(async () => {
  const PHASE = "__PHASE__";
  const invoke = window.__TAURI__.core.invoke;
  const log = (line) => invoke("selftest_log", { line: String(line) });
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  try {
    while (!window.saturnusApp) await sleep(100);
    const app = window.saturnusApp;
    const home = app.home;
    const messages = [];
    for (const store of [home.store]) store.watch(["message"], (state) => state.message && messages.push(state.message));
    // Calculator, mounted on its first visit: its store says what runs.
    const calc = async () => {
      const v = await app.mounted("calculator");
      await v.ready;
      return window.saturnus;
    };
    const booted = () => window.saturnus?.store.state.booted ?? null;
    const status = () => document.getElementById("status")?.textContent ?? "";
    const roms = () => home.store.state.roms;
    const slots = () => roms().slots.map((x) => `${x.model}=${x.fileName ?? "-"}:${x.state}`).join(" ");
    const goHome = async () => {
      location.hash = "#/";
      for (let i = 0; i < 50 && app.view !== "home"; i++) await sleep(100);
      await home.render();
    };
    const open = async (model) => {
      location.hash = `#/calculator/${model}`;
      await calc();
      for (let i = 0; i < 50 && booted() !== model; i++) await sleep(100);
      await sleep(500);
    };
    /** Click a home page button, then wait for the calculator it opens. */
    const press = async (selector, model) => {
      document.querySelector(selector).click();
      for (let i = 0; i < 1200 && app.view !== "calculator" && !home.store.state.messageError; i++) await sleep(100);
      if (app.view === "calculator") {
        await calc();
        for (let i = 0; i < 50 && booted() !== model; i++) await sleep(100);
        await sleep(500);
      }
    };
    await home.render();
    if (app.view === "calculator") await calc();
    await log(`phase ${PHASE}: at start ${location.hash}, booted ${booted()}, status "${status()}"`);
    await log(`slots: ${slots()}; lastModel ${roms().lastModel}, bootLast ${roms().bootLast}, remembered ${roms().remembered}`);
    if (PHASE === "choose") {
      await goHome();
      document.getElementById("home-new").click();
      await sleep(300);
      await press('button[data-act="choose"][data-model="48sx"]', "48sx");
      await log(`chose for the 48SX: booted ${booted()}, status "${status()}"`);
      await goHome();
      await log(`notice: ${home.store.state.romNotice}`);
      await log(`offers: ${JSON.stringify(roms().offers)}`);
      await log(`slots: ${slots()}`);
      // The page learns file names, never a path or a folder.
      const names = [...roms().slots.map((x) => x.fileName), ...roms().offers.map((o) => o.fileName)];
      await log(`a path in what the page learnt: ${names.some((n) => /[\\/]/.test(n ?? ""))}`);
      for (const m of ["48gx", "49g", "39g"]) {
        await open(m);
        await log(`opened ${m}: booted ${booted()}, status "${status()}"`);
      }
      // The Lewis models' dumps are offered (no download names them).
      for (const m of ["42s", "27s", "17bii", "17b", "28s", "19bii"]) {
        await goHome();
        const offer = roms().offers.find((o) => o.models.includes(m));
        if (!offer) continue;
        await press(`button[data-offer="${offer.id}"][data-model="${m}"]`, m);
        await log(`took the offer for the ${m.toUpperCase()}: booted ${booted()}, status "${status()}"`);
      }
      await open("48gx");
      for (const msg of [{ cmd: "bootModel", model: "48sx", romPath: "/etc/passwd" }, { cmd: "chooseRom", model: "48sx", path: "/tmp" }]) {
        try {
          await window.saturnus.backend.request(msg.cmd, msg);
          await log(`FAILED: ${msg.cmd} with a path was accepted`);
        } catch (err) {
          await log(`${msg.cmd} with a path refused: ${err}`);
        }
      }
    } else if (PHASE.startsWith("hold")) {
      // `hold38g`: open the model, then stay up to be killed.
      await open(PHASE.slice(4));
      await log(`opened ${PHASE.slice(4)}: booted ${booted()}; holding`);
      for (;;) await sleep(1000);
    } else if (PHASE === "download") {
      // Iteration 20b, by hand only (it downloads from hpcalc.org): on an
      // empty SATURNUS_SETTINGS_DIR and a scratch SATURNUS_DATA_DIR, each
      // model's Download… under New calculator (the confirmation is
      // answered by the hook).
      for (const m of ["48sx", "48gx", "38g", "49g", "39g"]) {
        await goHome();
        if (home.ui.newSection.hidden) document.getElementById("home-new").click();
        await sleep(300);
        const selector = `button[data-act="download"][data-model="${m}"]`;
        if (!document.querySelector(selector)) {
          await log(`FAILED: no Download button for ${m}`);
          continue;
        }
        await press(selector, m);
        await log(`downloaded ${m}: booted ${booted()}, status "${status()}"`);
      }
      await goHome();
      await log(`slots: ${slots()}`);
      await log(`Download buttons left: ${document.querySelectorAll('button[data-act="download"]').length}`);
    } else if (PHASE === "missing") {
      await open("48gx");
      await log(`opened 48gx: booted ${booted()}, status "${status()}"`);
      await goHome();
      await log(`slots: ${slots()}`);
    }
    if (window.saturnus) messages.push(...(window.saturnus.store.state.message ? [window.saturnus.store.state.message] : []));
    await log(`messages shown: ${JSON.stringify(messages)}`);
    await log(`end: booted ${booted()}, lastModel ${roms().lastModel}`);
    await log("done");
  } catch (err) {
    await log(`FAILED ${err?.stack ?? err}`);
    await log("done");
  }
})();
