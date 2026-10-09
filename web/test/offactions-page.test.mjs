// Controls and actions that cannot act now, in the real page in headless
// Chrome with no ROM (the backend's reads stubbed): an icon button that
// is off is greyed in both themes; a tap on the off Edit says why under
// it; Search lists its off actions greyed with the reason, and Enter on
// one says why; Remove the ROM asks the ROM table's question. Skipped
// without Chrome or web/pkg.
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

async function page(t, { width, height, mobile }) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: mobile ? 2 : 1, mobile });
  if (mobile) await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started.then(() => true)");
  /** A tap (touch) or a click at the middle of `selector`. */
  const press = async (selector) => {
    const [x, y] = await ev(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return [Math.round(r.x + r.width / 2), Math.round(r.y + r.height / 2)]; })()`);
    if (mobile) {
      await send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
      await send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    } else {
      await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
      await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
    }
    await sleep(150);
  };
  return { ...c, press };
}

/** `[button colour, the --ink-disabled colour, the --ink colour]` as computed. */
const colours = (selector) => `(() => {
  const probe = (v) => { const p = document.createElement("span"); p.style.color = "var(" + v + ")"; document.body.append(p); const c = getComputedStyle(p).color; p.remove(); return c; };
  return [getComputedStyle(document.querySelector(${JSON.stringify(selector)})).color, probe("--ink-disabled"), probe("--ink")];
})()`;

for (const [label, size, edit, live] of [
  ["desktop", { width: 1280, height: 860, mobile: false }, "#cmdline-edit", "#palette-show"],
  ["phone", { width: 390, height: 844, mobile: true }, "#bar-edit", "#bar-palette"],
]) {
  test(`${label}: an off icon button is greyed in both themes`, { timeout: 120_000 }, async (t) => {
    const p = await page(t, size);
    if (!p) return;
    for (const theme of ["light", "dark"]) {
      await p.ev(`document.querySelector("sat-controls").setTheme(${JSON.stringify(theme)}); true`);
      // The colours' transition.
      await sleep(400);
      assert.equal(await p.ev(`document.querySelector(${JSON.stringify(edit)}).getAttribute("aria-disabled")`), "true");
      const [off, disabled, ink] = await p.ev(colours(edit));
      assert.notEqual(disabled, ink, `${theme}: the two tokens differ`);
      assert.equal(off, disabled, `${theme}: the off Edit is greyed`);
      const [on] = await p.ev(colours(live));
      assert.equal(on, ink, `${theme}: Search, which is on, is not`);
    }
  });
}

test("phone: a tap on the off Edit says why under it; the next tap closes it, and it goes by itself", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { width: 390, height: 844, mobile: true });
  if (!p) return;
  const note = `(() => { const n = document.querySelector(".note"); if (!n) return null; const r = n.getBoundingClientRect(), b = document.querySelector("#bar-edit").getBoundingClientRect(); return { text: n.textContent, role: n.getAttribute("role"), inside: r.left >= 0 && r.right <= innerWidth, below: r.top >= b.bottom }; })()`;
  assert.equal(await p.ev(note), null);
  await p.press("#bar-edit");
  assert.deepEqual(await p.ev(note), { text: "Start the calculator first", role: "status", inside: true, below: true });
  await p.press(".stage");
  assert.equal(await p.ev(note), null, "the next tap closes it");
  await p.ev(`window.saturnus.store.set({ booted: "42s", model: "42s" }); true`);
  await sleep(400);
  await p.press("#bar-edit");
  assert.equal((await p.ev(note))?.text, "The HP 42S has no editor. Editing works on the 48SX, 48GX and 49G.");
  await sleep(4300);
  assert.equal(await p.ev(note), null, "gone after 4 s");
});

test("Search: off actions greyed with the reason, Enter says why; Remove asks the ROM table's question; the twins only for a query", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { width: 1280, height: 860, mobile: false });
  if (!p) return;
  const open = async (query) => {
    await p.ev(`(async () => {
      // Opened again: the actions follow the state as it opens.
      const pal = window.saturnus.palette;
      pal.close();
      await new Promise((r) => setTimeout(r, 50));
      await pal.open();
      const i = pal.querySelector("input");
      i.value = ${JSON.stringify(query)};
      i.dispatchEvent(new Event("input"));
      return true;
    })()`);
    await sleep(200);
    return p.ev(`[...document.querySelectorAll(".palette .prow-action")].map((r) => ({ name: r.querySelector(".prow-name").textContent, desc: r.querySelector(".prow-desc")?.textContent ?? "", off: r.getAttribute("aria-disabled") === "true", kind: r.querySelector(".prow-kind").textContent }))`);
  };
  /** Enter on the first action row (`RE`, a command, ranks above Remove). */
  const enter = () => p.ev(`(() => {
    const m = window.saturnus.palette.model;
    m.select(m.rows.findIndex((r) => r.kind === "action"));
    document.querySelector(".palette input").dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    return true;
  })()`);
  // No ROM: Edit and Remove are listed, off, saying why.
  const edit = (await open("edit")).find((r) => r.name === "Edit");
  assert.deepEqual(edit, { name: "Edit", desc: "Start the calculator first", off: true, kind: "action" });
  const rows = await open("remove");
  assert.deepEqual(rows[0], { name: "Remove the HP 48SX ROM…", desc: "No HP 48SX ROM to remove", off: true, kind: "action" });
  await enter();
  await sleep(200);
  assert.equal(await p.ev(`window.saturnus.palette.isOpen()`), true, "Enter on an off action leaves Search open");
  assert.equal(await p.ev(`document.querySelector(".palette-notice").textContent`), "No HP 48SX ROM to remove.");

  // A kept ROM: Remove is on and asks the ROM table's question.
  await p.ev(`(() => { const s = window.saturnus.store; const r = s.state.roms; s.set({ roms: { ...r, slots: r.slots.map((x) => (x.model === "48sx" ? { ...x, fileName: "sxrom-j" } : x)) } }); return true; })()`);
  const on = await open("remove");
  assert.deepEqual(on[0], { name: "Remove the HP 48SX ROM…", desc: "Deletes it from this browser. The file on your computer stays.", off: false, kind: "action" });
  await enter();
  let q = null;
  for (let i = 0; i < 30 && !q; i++) {
    await sleep(100);
    q = await p.ev(`(() => { const d = document.querySelector("dialog.confirm[open]"); return d ? [d.querySelector("h2, [id$=title]")?.textContent, [...d.querySelectorAll("button")].map((b) => b.textContent)] : null; })()`);
  }
  assert.deepEqual(q, ["Remove the HP 48SX ROM?", ["Cancel", "Remove"]]);
  assert.equal(await p.ev(`window.saturnus.palette.isOpen()`), false, "Search closed first");
  await p.ev(`document.querySelector("dialog.confirm").dispatchEvent(new Event("cancel")); true`);

  // A calculator running: the screen images' twins only for a query, no kind badge before one.
  await p.ev(`window.saturnus.store.set({ booted: "48sx", model: "48sx" }); true`);
  const all = await open("");
  const names = all.map((r) => r.name);
  assert.ok(names.includes("Copy screen") && names.includes("Save screen"), names.join());
  assert.ok(!names.some((n) => /black on white|LCD colours/.test(n)), names.join());
  assert.ok(all.every((r) => r.kind === ""), "no kind badge with an empty query");
  assert.ok((await open("copy screen black")).some((r) => r.name === "Copy screen (black on white)"));
});
