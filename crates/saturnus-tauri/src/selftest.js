// Debug builds only (SATURNUS_SELFTEST=<48SX ROM path>): drives the real
// page in the app's webview, through the components and the
// TauriBackend, and reports over the `selftest_log` command. Native file
// dialogs cannot be scripted: under the hook the Rust side takes the ROM
// from the variable and the state file from the temp directory; the page
// sends no paths, as ever.
(async () => {
  const SECS = __SECS__;
  const invoke = window.__TAURI__.core.invoke;
  const log = (line) => invoke("selftest_log", { line: String(line) });
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  try {
    while (!window.saturnus) await sleep(100);
    const s = window.saturnus;
    await log(`host ${s.store.state.host}, models ${s.store.state.models.join(",")}`);
    const r = await s.backend.request("boot", { model: "48sx" });
    await log(`boot ${JSON.stringify(r)}`);
    try {
      await s.backend.request("saveState", { path: "/tmp/saturnus-selftest-forbidden" });
      await log("FAILED: a page-supplied path was accepted");
    } catch (err) {
      await log(`page path refused: ${err}`);
    }
    await sleep(2500);
    const click = async (name) => {
      const g = s.skinKey(name);
      const opts = { bubbles: true, pointerId: 1, isPrimary: true };
      g.dispatchEvent(new PointerEvent("pointerdown", opts));
      await sleep(150);
      g.dispatchEvent(new PointerEvent("pointerup", opts));
      await sleep(400);
    };
    const key = async (k, code) => {
      document.body.dispatchEvent(new KeyboardEvent("keydown", { key: k, code, bubbles: true }));
      await sleep(80);
      document.body.dispatchEvent(new KeyboardEvent("keyup", { key: k, code, bubbles: true }));
      await sleep(250);
    };
    const screen = () => s.screenText().split("\n").map((r) => r.replaceAll(".", " ").trimEnd()).filter(Boolean).join("\n");
    await click("f");
    await sleep(2000);
    await log(`status: ${document.getElementById("status").textContent}`);
    for (const k of ["2", "enter", "3", "plus"]) await click(k);
    await sleep(800);
    await log("after mouse 2 ENTER 3 +:\n" + screen());
    for (const ch of "Ab") await key(ch, `Key${ch.toUpperCase()}`);
    await sleep(1500);
    await log("after typing A b on the keyboard:\n" + screen());
    await key("`", "Backquote");
    await sleep(800);
    await s.backend.request("saveState");
    const saved = s.screenText();
    for (const k of ["9", "enter"]) await click(k);
    await sleep(1000);
    const changed = s.screenText();
    await s.backend.request("loadState");
    await sleep(1000);
    await log(`state: changed ${changed !== saved}, loaded equals saved ${s.screenText() === saved}`);
    const acc = (x) => x.emulatedMs + x.owedMs;
    const measure = async (label) => {
      const a = await s.stats();
      await sleep(SECS * 1000);
      const b = await s.stats();
      await log(`${label}: emulated ${(acc(b) - acc(a)).toFixed(1)} ms over ${(b.nowMs - a.nowMs).toFixed(1)} ms wall = ${((acc(b) - acc(a)) / (b.nowMs - a.nowMs) * 100).toFixed(2)}%, work ${(b.workMs - a.workMs).toFixed(1)} ms, ${b.ticks - a.ticks} passes, loop ${b.loop}`);
    };
    await measure("idle at 1x");
    for (const c of "1 99999 START NEXT") {
      if (c === " ") await key(" ", "Space");
      else if (/\d/.test(c)) await key(c, `Digit${c}`);
      else await key(c, `Key${c}`);
    }
    await sleep(2500);
    await key("Enter", "Enter");
    await sleep(500);
    await measure("computing at 1x");
    s.setSpeed("4");
    await measure("computing at 4x");
    s.setSpeed("1");
    await log("done");
  } catch (err) {
    await log(`FAILED ${err?.stack ?? err}`);
    await log("done");
  }
})();
