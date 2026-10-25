// The menu keys' tooltips on real ROMs, through the Worker and the wasm
// host: on the 48SX and 48GX, MTH's keys name its submenus, the ALGEBRA
// menu's its commands with their full names, the VAR menu the variables,
// and the keys' own lines stay under them. Skipped unless
// `SATURNUS_ROM_DIR` holds `sxrom-j` or `gxrom-r`, Chrome and web/pkg
// exist; web/test/menukeys.test.mjs checks the lines,
// crates/saturnus-host/tests/menu_keys.rs the keys read from RAM.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { session, sleep } from "./chrome.mjs";

for (const [model, file, algebra] of [["48sx", "sxrom-j", ["leftshift", "9"]], ["48gx", "gxrom-r", null]]) {
  test(`${model} ROM: the menu keys' tooltips name what the screen shows`, { timeout: 180_000 }, async (t) => {
    const dir = process.env.SATURNUS_ROM_DIR;
    const rom = dir ? join(dir, file) : null;
    if (!rom || !existsSync(rom)) {
      t.skip(`SATURNUS_ROM_DIR/${file} not found`);
      return;
    }
    const c = await session(t, { files: { "/__rom": rom } });
    if (!c) return;
    const { send, ev, port } = c;
    await send("Page.enable");
    await send("Runtime.enable");
    await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
    for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
    await ev("window.saturnus.started.then(() => true)");
    await ev(`(async () => {
      const bytes = await (await fetch("/__rom")).arrayBuffer();
      await window.saturnus.startWithRom(new File([bytes], ${JSON.stringify(file)}));
      return true;
    })()`);
    for (let i = 0; i < 150 && (await ev("window.saturnus.store.state.booted")) !== model; i++) await sleep(100);
    await ev(`window.__idle = async (ms = 1500) => {
      const b = window.saturnus.backend;
      const t0 = (await b.stats()).emulatedMs;
      for (;;) {
        const s = await b.stats();
        if (s.emulatedMs - t0 >= ms && s.loop === "sleep") return true;
        await new Promise((r) => setTimeout(r, 50));
      }
    }; true`);
    await ev("window.__idle()");
    await ev("window.saturnus.store.set({ storageOffer: null }); true");
    const b = "window.saturnus.backend";
    const title = (key) => `document.querySelector("sat-calculator").skinKey(${JSON.stringify(key)}).querySelector("title").textContent`;
    const until = async (expr, what, n = 100) => {
      let v = null;
      for (let i = 0; i < n; i++) {
        v = await ev(expr).catch(() => null);
        if (v) return v;
        await sleep(100);
      }
      assert.fail(`timed out: ${what} (${await ev(title("a")).catch(() => "?")})`);
    };
    const press = (keys) => ev(`${b}.typeKeys(${JSON.stringify(keys)}); window.__idle(800)`);

    await press(["mth"]);
    const first = model === "48sx" ? "PARTS" : "VECTR";
    await until(`${title("a")}.startsWith(${JSON.stringify(`${first} menu\n`)})`, "MTH's first key");
    assert.match(await ev(title("a")), /\nA \(alpha A\)/, "the key's own line stays");

    // The ALGEBRA menu: commands with their full names.
    if (algebra) await press(algebra);
    else await ev(`${b}.run("93 MENU").then(() => window.__idle(800))`);
    await until(`${title("b")}.startsWith("EXPAN · Expand products\\n")`, "EXPAN's full name");
    const shown = {};
    for (const k of ["a", "b", "c", "d", "e", "f"]) shown[k] = (await ev(title(k))).split("\n")[0];
    t.diagnostic(`${model} ALGEBRA: ${JSON.stringify(shown)}`);

    // The VAR menu: the variables, newest first.
    await ev(`${b}.run("1 'AA' STO \\"HI\\" 'BB' STO").then(() => window.__idle(800))`);
    await press(["var"]);
    await until(`${title("a")}.startsWith("BB · variable (string)\\n")`, "the newest variable");
    assert.ok((await ev(title("b"))).startsWith("AA · variable (real number)\n"));
    // A key past the variables keeps its own lines.
    await until(`!${title("f")}.includes("·")`, "a blank key");
  });
}
