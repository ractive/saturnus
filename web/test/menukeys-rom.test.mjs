// The menu keys' tooltips on real ROMs, through the Worker and the wasm
// host: on the 48SX and 48GX, MTH's keys name its submenus, the ALGEBRA
// menu's its commands with their full names, the VAR menu the variables,
// and the keys' own lines stay under them; on the 42S and the 17BII the
// labels from RAM with their names from the key reference, none while no
// menu shows; on the 28S STACK's commands from its table. Skipped unless
// `SATURNUS_ROM_DIR` holds `sxrom-j`, `gxrom-r`, `hp42s.rom`,
// `hp17bii.rom` or `hp28s.rom`, Chrome and web/pkg exist;
// web/test/menukeys.test.mjs checks the lines,
// crates/saturnus-host/tests/menu_keys.rs and menu_labels.rs the keys read
// from RAM.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { session, sleep } from "./chrome.mjs";

/** A page with the calculator of ROM `file` booted and idle, or null (skipped). */
async function boot(t, model, file) {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, file) : null;
  if (!rom || !existsSync(rom)) {
    t.skip(`SATURNUS_ROM_DIR/${file} not found`);
    return null;
  }
  const c = await session(t, { files: { "/__rom": rom } });
  if (!c) return null;
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
  const title = (key) => `document.querySelector("sat-calculator").skinKey(${JSON.stringify(key)}).querySelector("title").textContent`;
  const first = model === "42s" ? "sigmaplus" : "a";
  const until = async (expr, what, n = 100) => {
    let v = null;
    for (let i = 0; i < n; i++) {
      v = await ev(expr).catch(() => null);
      if (v) return v;
      await sleep(100);
    }
    assert.fail(`timed out: ${what} (${await ev(title(first)).catch(() => "?")})`);
  };
  const press = (keys) => ev(`window.saturnus.backend.typeKeys(${JSON.stringify(keys)}); window.__idle(800)`);
  return { send, ev, title, until, press };
}

for (const [model, file, algebra] of [["48sx", "sxrom-j", ["leftshift", "9"]], ["48gx", "gxrom-r", null]]) {
  test(`${model} ROM: the menu keys' tooltips name what the screen shows`, { timeout: 180_000 }, async (t) => {
    const c = await boot(t, model, file);
    if (!c) return;
    const { ev, title, until, press } = c;
    const b = "window.saturnus.backend";

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

    // The look-up line names the palette's key; with the pointer over
    // EXPAN's key, that key opens Search with EXPAN typed.
    const isMac = await ev("window.saturnus.bindings?.isMac ?? /Mac/.test(navigator.platform)");
    const mod = isMac ? "⌘K" : "Ctrl+K";
    assert.equal((await ev(title("b"))).split("\n")[1], `${mod}: look up EXPAN`);
    const r = await ev(`(() => { const b = document.querySelector("sat-calculator").skinKey("b").getBoundingClientRect(); return [b.x + b.width / 2, b.y + b.height / 2]; })()`);
    await c.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: r[0], y: r[1] });
    await until(`document.querySelector("sat-calculator").hoveredCommand() === "EXPAN"`, "the hovered key");
    const k = { key: "k", code: "KeyK", windowsVirtualKeyCode: 75, modifiers: isMac ? 4 : 2 };
    await c.send("Input.dispatchKeyEvent", { type: "rawKeyDown", ...k });
    await c.send("Input.dispatchKeyEvent", { type: "keyUp", ...k });
    await until(`window.saturnus.palette.isOpen() && document.querySelector("sat-palette input").value === "EXPAN"`, "Search open with EXPAN");
    await until(`[...document.querySelectorAll("sat-palette [role=option]")].some((o) => o.textContent.includes("Expand products"))`, "EXPAN among the results");
    await ev("window.saturnus.palette.close(); true");

    // The VAR menu: the variables, newest first.
    await ev(`${b}.run("1 'AA' STO \\"HI\\" 'BB' STO").then(() => window.__idle(800))`);
    await press(["var"]);
    await until(`${title("a")}.startsWith("BB · variable (string)\\n")`, "the newest variable");
    assert.ok((await ev(title("b"))).startsWith("AA · variable (real number)\n"));
    // A key past the variables keeps its own lines.
    await until(`!${title("f")}.includes("·")`, "a blank key");
  });
}

test("42s ROM: the top row's tooltips name the menu's labels while one shows", { timeout: 180_000 }, async (t) => {
  const c = await boot(t, "42s", "hp42s.rom");
  if (!c) return;
  const { ev, title, until, press } = c;
  const own = await ev(title("sigmaplus"));
  assert.ok(!own.includes(" · "), `no menu: the key's own tooltip (${own})`);
  // CONVERT: the labels from RAM, the full names from the key reference.
  await press(["shift", "5"]);
  await until(`${title("sigmaplus")}.startsWith("→DEG · To degrees\\n")`, "→DEG");
  assert.ok((await ev(title("sigmaplus"))).endsWith(own), "the key's own lines stay");
  assert.ok((await ev(title("xeq"))).startsWith("→POL · To polar\n"));
  const shown = {};
  for (const k of ["sigmaplus", "inv", "sqrt", "log", "ln", "xeq"]) shown[k] = (await ev(title(k))).split("\n")[0];
  t.diagnostic(`42s CONVERT: ${JSON.stringify(shown)}`);
  // EXIT: the menu goes, so do the lines (the labels stay in RAM).
  await press(["on"]);
  await until(`${title("sigmaplus")} === ${JSON.stringify(own)}`, "no menu after EXIT");
});

test("17bii ROM: the menu keys' tooltips name MAIN's and TVM's labels", { timeout: 180_000 }, async (t) => {
  const c = await boot(t, "17bii", "hp17bii.rom");
  if (!c) return;
  const { ev, title, until, press } = c;
  await press(["b"]); // ENGL at the language prompt
  await until(`${title("a")}.startsWith("FIN · Finance\\n")`, "MAIN's FIN");
  await press(["a", "a"]); // FIN TVM
  await until(`${title("a")}.startsWith("N · Number of periods\\n")`, "TVM's N");
  assert.ok((await ev(title("c"))).startsWith("PV · Present value\n"));
  const shown = {};
  for (const k of ["a", "b", "c", "d", "e", "f"]) shown[k] = (await ev(title(k))).split("\n")[0];
  t.diagnostic(`17bii TVM: ${JSON.stringify(shown)}`);
});

test("28s ROM: the menu keys' tooltips name STACK's commands from the 28S's table", { timeout: 180_000 }, async (t) => {
  const c = await boot(t, "28s", "hp28s.rom");
  if (!c) return;
  const { ev, title, until, press } = c;
  await press(["on"]); // ATTN: "Memory Lost" goes
  await press(["shift", "letterg"]); // STACK
  await until(`${title("a")}.startsWith("DUP · Duplicate\\n")`, "STACK's DUP");
  await press(["nxt"]);
  await until(`${title("a")}.startsWith("ROLLD")`, "STACK's second page");
  const shown = {};
  for (const k of ["a", "b", "c", "d", "e", "f"]) shown[k] = (await ev(title(k))).split("\n")[0];
  t.diagnostic(`28s STACK page 2: ${JSON.stringify(shown)}`);
});
