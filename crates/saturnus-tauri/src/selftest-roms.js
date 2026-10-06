// Debug builds only (SATURNUS_SELFTEST=<a ROM path>,
// SATURNUS_SELFTEST_SCRIPT=roms, SATURNUS_SELFTEST_PHASE=...): the
// remembered ROMs, driven through the page's controls in the app's
// webview. The ROM dialog is answered by the hook with the ROM of the
// variable; the page never sends a path. Run the phases in order on one
// SATURNUS_SETTINGS_DIR (kb: iterations/iteration-20-remember-roms):
// `choose` (a ROM chosen in a folder that holds the others), `restart`
// (a relaunch boots the last model), `missing` (after a ROM was moved
// away, selecting its model reports it and asks), and `hold38g` (select
// the 38G, then wait to be killed: the choice is already saved).
(async () => {
  const PHASE = "__PHASE__";
  const invoke = window.__TAURI__.core.invoke;
  const log = (line) => invoke("selftest_log", { line: String(line) });
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  try {
    while (!window.saturnus) await sleep(100);
    const s = window.saturnus;
    const messages = [];
    s.store.watch(["message"], (state) => state.message && messages.push(state.message));
    if (s.store.state.message) messages.push(s.store.state.message);
    await s.started;
    const st = () => s.store.state;
    const slots = () => st().roms.slots.map((x) => `${x.model}=${x.fileName ?? "-"}:${x.state}`).join(" ");
    const status = () => document.getElementById("status").textContent;
    const select = async (model) => {
      const sel = document.getElementById("model");
      sel.value = model;
      sel.dispatchEvent(new Event("change"));
      for (let i = 0; i < 50 && st().booted !== model; i++) await sleep(100);
      await sleep(500);
    };
    await log(`phase ${PHASE}: at start booted ${st().booted}, status "${status()}"`);
    await log(`slots: ${slots()}; lastModel ${st().roms.lastModel}, bootLast ${st().roms.bootLast}, remembered ${st().roms.remembered}`);
    if (PHASE === "choose") {
      document.getElementById("model").value = "48sx";
      s.store.set({ model: "48sx" });
      document.getElementById("rom-pick").click();
      for (let i = 0; i < 50 && !st().booted; i++) await sleep(100);
      await sleep(500);
      await log(`chose for the 48SX: booted ${st().booted}, status "${status()}"`);
      await log(`notice: ${st().romNotice}`);
      await log(`offers: ${JSON.stringify(st().roms.offers)}`);
      await log(`slots: ${slots()}`);
      // The page learns file names, never a path or a folder.
      const names = [...st().roms.slots.map((x) => x.fileName), ...st().roms.offers.map((o) => o.fileName)];
      await log(`a path in what the page learnt: ${names.some((n) => /[\\/]/.test(n ?? ""))}`);
      for (const m of ["48gx", "49g", "39g"]) {
        await select(m);
        await log(`selected ${m}: booted ${st().booted}, status "${status()}"`);
      }
      const offer = st().roms.offers.find((o) => o.models.includes("42s"));
      if (offer) {
        document.querySelector(`button[data-offer="${offer.id}"][data-model="42s"]`).click();
        for (let i = 0; i < 50 && st().booted !== "42s"; i++) await sleep(100);
        await log(`took the offer for the 42S: booted ${st().booted}, status "${status()}"`);
      }
      await select("48gx");
      for (const msg of [{ cmd: "bootModel", model: "48sx", romPath: "/etc/passwd" }, { cmd: "chooseRom", model: "48sx", path: "/tmp" }]) {
        try {
          await s.backend.request(msg.cmd, msg);
          await log(`FAILED: ${msg.cmd} with a path was accepted`);
        } catch (err) {
          await log(`${msg.cmd} with a path refused: ${err}`);
        }
      }
    } else if (PHASE.startsWith("hold")) {
      // `hold38g`: select the model, then stay up to be killed.
      await select(PHASE.slice(4));
      await log(`selected ${PHASE.slice(4)}: booted ${st().booted}; holding`);
      for (;;) await sleep(1000);
    } else if (PHASE === "missing") {
      await select("48gx");
      await log(`selected 48gx: booted ${st().booted}, status "${status()}"`);
      await log(`slots: ${slots()}`);
    }
    await log(`messages shown: ${JSON.stringify(messages)}`);
    await log(`end: booted ${st().booted}, lastModel ${st().roms.lastModel}`);
    await log("done");
  } catch (err) {
    await log(`FAILED ${err?.stack ?? err}`);
    await log("done");
  }
})();
