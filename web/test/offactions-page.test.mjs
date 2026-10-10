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

/**
 * Each of `selectors` as computed: its colour, its background, its top border's
 * style and colour, its top and left borders' style, width and colour, its text's contrast on the surface behind it (the
 * first ancestor with a background of its own), its outline's style.
 */
const looks = (selectors) => `(() => {
  const rgb = (c) => c.match(/[\\d.]+/g).map(Number);
  const lum = (c) => {
    const [r, g, b] = rgb(c).map((v) => v / 255).map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  };
  const opaque = (c) => c !== "transparent" && (rgb(c)[3] ?? 1) > 0;
  const look = (e) => {
    const s = getComputedStyle(e);
    let under = e.parentElement;
    while (under && !opaque(getComputedStyle(under).backgroundColor)) under = under.parentElement;
    const [a, b] = [lum(s.color), lum(getComputedStyle(under ?? document.body).backgroundColor)].sort((x, y) => y - x);
    return { color: s.color, background: s.backgroundColor, clear: !opaque(s.backgroundColor), border: s.borderTopStyle + " " + s.borderTopColor,
      borderColor: s.borderTopColor, top: s.borderTopStyle + " " + s.borderTopWidth,
      left: s.borderLeftStyle + " " + s.borderLeftWidth, leftColor: s.borderLeftColor,
      contrast: Math.round(((a + 0.05) / (b + 0.05)) * 100) / 100, outline: s.outlineStyle };
  };
  return Object.fromEntries(${JSON.stringify(selectors)}.map((q) => [q, look(document.querySelector(q))]));
})()`;

test("desktop: an off button sinks in every kind, in both themes: no fill, a very light border, its text readable", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { width: 1280, height: 860, mobile: false });
  if (!p) return;
  // The kinds no page state shows off without a ROM: a primary, menu items.
  await p.ev(`(() => {
    const panel = document.getElementById("panel");
    panel.insertAdjacentHTML("beforeend", '<button id="t-primary" class="primary" type="button" disabled>Go</button><button id="t-primary-on" class="primary" type="button">Go</button>' +
      '<div class="menu" style="position: static">' +
      '<button id="t-item-on" role="menuitem" type="button"><svg class="ic" aria-hidden="true"><use href="#ic-copy"/></svg><span class="menu-text">Copy</span></button>' +
      '<button id="t-item" role="menuitem" type="button" aria-disabled="true"><svg class="ic" aria-hidden="true"><use href="#ic-edit"/></svg><span class="menu-text">Edit</span></button>' +
      '<button id="t-danger" class="danger" role="menuitem" type="button" aria-disabled="true">Purge…</button></div>');
    return true;
  })()`);
  const off = ["#reset", "#load", "#cmdline-edit", "#bar-edit", "#t-primary", "#t-item", "#t-danger"];
  /** The off item's icon and label against the on item's: the same left edges. */
  const edges = `["#t-item-on", "#t-item"].map((q) => [...document.querySelectorAll(q + " > *")].map((e) => Math.round(e.getBoundingClientRect().left * 10) / 10))`;
  for (const theme of ["light", "dark"]) {
    await p.ev(`document.querySelector("sat-controls").setTheme(${JSON.stringify(theme)}); true`);
    await sleep(400);
    const l = await p.ev(looks([...off, "#rom-pick", "#t-primary-on", "#speed button[aria-checked=true]", "#contrast", "#contrast button", "#contrast button + button", "#speed", "#speed button + button"]));
    const on = l["#rom-pick"];
    assert.equal(on.clear, false, `${theme}: an enabled button has a fill`);
    for (const q of off) {
      assert.equal(l[q].clear, true, `${theme} ${q}: no fill (${l[q].background})`);
      assert.equal(l[q].top, "solid 1px", `${theme} ${q}: a border`);
      assert.notEqual(l[q].border, on.border, `${theme} ${q}: not the enabled border`);
      assert.ok(l[q].contrast >= 3, `${theme} ${q}: text ${l[q].contrast}:1 on its surface`);
    }
    // Segments: no fill, the divider light; the group's own border stays.
    assert.equal(l["#contrast button"].clear, true, `${theme}: an off segment has no fill`);
    assert.ok(l["#contrast button"].contrast >= 3, `${theme}: an off segment's text ${l["#contrast button"].contrast}:1`);
    assert.equal(l["#contrast button + button"].left, "solid 1px", `${theme}: the off segments' divider`);
    assert.equal(l["#contrast button + button"].leftColor, l["#t-item"].borderColor, `${theme}: the divider in the off border's colour`);
    assert.notEqual(l["#contrast button + button"].leftColor, l["#speed button + button"].leftColor, `${theme}: not the on divider's`);
    assert.equal(l["#contrast"].border, l["#speed"].border, `${theme}: the group of off segments keeps its border`);
    const [onEdges, offEdges] = await p.ev(edges);
    assert.deepEqual(offEdges, onEdges, `${theme}: an off menu item's icon and label line up with an on one's`);
    assert.equal(l["#t-primary-on"].clear, false, `${theme}: an enabled primary keeps its accent`);
    assert.notEqual(l["#t-primary"].background, l["#t-primary-on"].background, `${theme}: an off primary gives up its accent`);
    assert.equal(l["#t-danger"].color, l["#t-item"].color, `${theme}: an off Purge… is grey, not red`);
    assert.equal(l["#speed button[aria-checked=true]"].clear, false, `${theme}: a selected segment stays filled`);
    // The off Edit and an off menu item stay focusable: focused, the
    // ring shows and they keep their off look (a key pressed first, so
    // the focus counts as the keyboard's).
    for (const q of ["#cmdline-edit", "#t-item"]) {
      await p.send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: "Shift", code: "ShiftLeft", windowsVirtualKeyCode: 16 });
      await p.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Shift", code: "ShiftLeft", windowsVirtualKeyCode: 16 });
      await p.ev(`document.querySelector(${JSON.stringify(q)}).focus(); true`);
      assert.deepEqual(await p.ev(`["#" + document.activeElement.id, document.activeElement.matches(":focus-visible")]`), [q, true]);
      const f = (await p.ev(looks([q])))[q];
      assert.equal(f.outline, "solid", `${theme} ${q}: the focus ring`);
      assert.equal(f.clear, true, `${theme} ${q}: focused, still no fill`);
      assert.equal(f.top, "solid 1px", `${theme} ${q}: focused, its border`);
      assert.equal(f.borderColor, l[q].borderColor, `${theme} ${q}: focused, the off border's colour`);
      await p.ev(`document.activeElement.blur(); true`);
    }
  }
});

test("phone: a tap on the off Edit says why under it; the next tap closes it, and it goes by itself", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { width: 390, height: 844, mobile: true });
  if (!p) return;
  const note = `(() => { const n = document.querySelector(".note"); if (!n) return null; const r = n.getBoundingClientRect(), b = document.querySelector("#bar-edit").getBoundingClientRect(); return { text: n.textContent, role: n.getAttribute("role"), inside: r.left >= 0 && r.right <= innerWidth, below: r.top >= b.bottom }; })()`;
  assert.equal(await p.ev(note), null);
  // The text arrives after the live region is in the page (announced).
  await p.ev(`(() => {
    window.__noteAtInsert = [];
    new MutationObserver((ms) => { for (const m of ms) for (const n of m.addedNodes) if (n.classList?.contains("note")) window.__noteAtInsert.push(n.textContent); })
      .observe(document.body, { childList: true });
    return true;
  })()`);
  await p.press("#bar-edit");
  assert.deepEqual(await p.ev("window.__noteAtInsert"), [""], "inserted empty");
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

test("the memory view's keys chip never wraps the tab row, even in a wider font (a first click would move what it hits)", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { width: 1280, height: 860, mobile: false });
  if (!p) return;
  await p.ev("window.saturnus.setLayer(true)");
  await sleep(300);
  const row = `Math.round(document.querySelector(".layer-tabs").getBoundingClientRect().height)`;
  const before = await p.ev(row);
  // Linux's fonts run wider than macOS's: letter spacing stands in for them.
  await p.ev(`(() => { const s = document.createElement("style"); s.textContent = ".layer-keys-back, .tabs button { letter-spacing: 1.2px; }"; document.head.append(s); return true; })()`);
  await p.ev(`(() => { const e = window.saturnus.explorer; e.querySelector("#tab-flags").focus(); e.showKeys(); return true; })()`);
  await sleep(300);
  assert.deepEqual(await p.ev(`[document.querySelector(".layer-keys").className, document.querySelector(".layer-keys-back").hidden]`), ["layer-keys own", false], "the keys are in the view");
  assert.equal(await p.ev(row), before, "one line, as before the keys came");
});
