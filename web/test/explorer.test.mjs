// The Variables list in the real page in headless Chrome over the
// DevTools protocol (no dependencies), with real mouse events and keys:
// a double-click on a directory row opens it, one on another variable
// selects it; the "New directory…" and Rename fields take the focus
// once and give up none they do not have; New directory gives way to
// Rename and Purge and keeps a name the calculator refused. The
// preview's actions (kb iteration 32): one primary button and a "⋯"
// menu that never wrap, the same menu for a directory chosen in the
// list or the tree, HOME's Make current, the menu's keys, the context
// menu, F2 and Delete, the writes off while one runs, the bar's "New"
// menu; the divider between the tree and the list. No ROM: the page's
// memory reads are answered by the test (a 48SX with HOME holding MYDIR
// and X). Skipped without Chrome (`SATURNUS_CHROME` names one) or the
// wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

/** Poll `expression` in the page until it is truthy, at most `ms`. */
async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

// The memory reads answered in the page: HOME holds the directory MYDIR
// (with A in it) and the real number X (42), the stack 42. `createDir`
// refuses "1A" as the host does and adds any other name to HOME;
// `changeDir` moves the calculator. The writes asked for are kept in
// `window.__writes`; `window.__tree` is the memory.
const FAKE_MEMORY = `(() => {
  const b = window.saturnus.backend;
  const v = (name, type, extra = {}) => ({ name, type, size: 16, checksum: 0x5B55, address: 0x7A000 + name.length, ...extra });
  const tree = { path: ["HOME"], variables: [v("MYDIR", "Directory", { variables: [v("A", "Real Number")] }), v("X", "Real Number")] };
  window.__tree = tree;
  window.__writes = [];
  b.watchMemory = async () => ({ supported: true });
  b.memoryTree = async () => structuredClone(tree);
  b.stack = async () => [{ type: "real", text: "42" }];
  b.flags = async () => ({ set: [] });
  b.objectAt = async () => ({ type: "real", text: "42" });
  b.createDir = async (dir, name) => {
    window.__writes.push(["createDir", dir, name]);
    await new Promise((r) => setTimeout(r, 50));
    if (name === "1A") throw new Error('"1A" is not a plain variable name');
    tree.variables.unshift(v(name, "Directory", { variables: [] }));
    window.saturnus.memory.refresh();
    return { emulatedMs: 1, keys: false };
  };
  b.changeDir = async (dir) => {
    window.__writes.push(["changeDir", dir]);
    tree.path = [...dir];
    window.saturnus.memory.refresh();
    return { emulatedMs: 1, keys: false };
  };
  window.saturnus.store.set({ booted: "48sx" });
  return window.saturnus.setLayer(true).then(() => window.saturnus.explorer.setTab("vars")).then(() => true);
})()`;

/**
 * The page with the fake memory in headless Chrome, or null when the
 * test skipped. `width`/`height`: the window; `mobile`: a phone's
 * (touch, coarse pointer).
 */
async function page(t, { width = 1280, height = 900, mobile = false } = {}) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  if (mobile) await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile });
  const load = async () => {
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
    await until(ev, "!!window.saturnus", 15_000, "the page started");
    await ev("window.saturnus.started");
    await ev(FAKE_MEMORY);
    await until(ev, row("MYDIR"), 5_000, "the list shows MYDIR");
  };
  await load();
  /** A real mouse click in the middle of `selector`'s element. */
  const click = async (selector, clickCount = 1) => {
    const [x, y] = await ev(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return [r.x + Math.min(30, r.width / 2), r.y + r.height / 2]; })()`);
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    for (let n = 1; n <= clickCount; n++) {
      await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: n });
      await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: n });
    }
    await sleep(150);
  };
  /** Text typed into the focused element, as the keyboard does. */
  const type = async (text) => {
    await send("Input.insertText", { text });
    await sleep(50);
  };
  /** A named key (Enter). */
  const press = async (key, code = key, vk = 0) => {
    await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key, code, windowsVirtualKeyCode: vk });
    await send("Input.dispatchKeyEvent", { type: "char", key, text: "\r" });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key, code, windowsVirtualKeyCode: vk });
    await sleep(150);
  };
  /** A key without the text it types (arrows, Escape, F2, a letter's jump); `modifiers` 8 is Shift. */
  const key = async (k, { code = k, vk = KEY_CODES[k] ?? 0, modifiers = 0, text } = {}) => {
    await send("Input.dispatchKeyEvent", { type: text ? "keyDown" : "rawKeyDown", key: k, code, windowsVirtualKeyCode: vk, modifiers, text });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, windowsVirtualKeyCode: vk, modifiers });
    await sleep(120);
  };
  /** The mouse at the middle of `selector`'s element (or `at`), as a button press. */
  const mouse = async (x, y, button = "left", clickCount = 1) => {
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button, clickCount });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button, clickCount });
    await sleep(150);
  };
  const center = (selector) => ev(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return [Math.round(r.x + r.width / 2), Math.round(r.y + r.height / 2)]; })()`);
  /** A drag with the mouse from `[x, y]` by `dx`. */
  const drag = async ([x, y], dx) => {
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
    for (let i = 1; i <= 4; i++) await send("Input.dispatchMouseEvent", { type: "mouseMoved", x: x + (dx * i) / 4, y, button: "left", buttons: 1 });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: x + dx, y, button: "left", clickCount: 1 });
    await sleep(150);
  };
  /** The open menu: its label, items (text, off, key hint), rules and note, or null. */
  const menu = () => ev(`(() => {
    const m = document.querySelector(".menu");
    if (!m) return null;
    const r = m.getBoundingClientRect();
    return {
      label: m.getAttribute("aria-label"),
      items: [...m.querySelectorAll("[role=menuitem]")].map((b) => b.querySelector(".menu-text").textContent),
      off: [...m.querySelectorAll("[role=menuitem][aria-disabled=true]")].map((b) => b.querySelector(".menu-text").textContent),
      hints: Object.fromEntries([...m.querySelectorAll("[role=menuitem]")].filter((b) => b.querySelector("kbd")).map((b) => [b.querySelector(".menu-text").textContent, b.querySelector("kbd").textContent])),
      shape: [...m.children].map((c) => c.getAttribute("role") === "separator" ? "-" : c.classList.contains("menu-note") ? "note" : c.querySelector(".menu-text")?.textContent),
      note: m.querySelector(".menu-note")?.textContent ?? null,
      danger: [...m.querySelectorAll(".danger .menu-text")].map((e) => e.textContent),
      focused: document.activeElement?.closest(".menu") === m ? document.activeElement.querySelector(".menu-text")?.textContent : null,
      rect: { left: r.left, top: r.top, right: r.right, bottom: r.bottom },
      view: [innerWidth, innerHeight],
    };
  })()`);
  /** The preview's head: its title, facts and buttons (text, off). */
  const head = (pane = "vars") => ev(`(() => {
    const h = document.querySelector(".pane-${pane} .preview-head");
    if (!h) return null;
    const bs = [...h.querySelectorAll(".preview-actions > button")];
    return {
      title: h.querySelector("h3").firstChild.textContent,
      current: !!h.querySelector("h3 .here-mark"),
      meta: h.querySelector(".preview-meta").textContent,
      buttons: bs.map((b) => b.classList.contains("more") ? "⋯" : b.textContent),
      off: bs.filter((b) => b.disabled).map((b) => b.classList.contains("more") ? "⋯" : b.textContent),
      tops: [...new Set(bs.map((b) => Math.round(b.getBoundingClientRect().top)))].length,
      height: Math.round(h.querySelector(".preview-actions").getBoundingClientRect().height),
    };
  })()`);
  /** What has the focus: a short name for the assertions. */
  const focused = () => ev(`(() => {
    const a = document.activeElement;
    if (a?.closest(".vars-making")) return "new";
    if (a?.matches(".vars-find")) return "find";
    if (a?.closest(".preview .edit-row")) return "rename";
    return a?.tagName ?? null;
  })()`);
  return { send, ev, click, type, press, key, mouse, center, drag, menu, head, focused, load };
}

const KEY_CODES = { Escape: 27, Enter: 13, ArrowDown: 40, ArrowUp: 38, Home: 36, End: 35, F2: 113, F10: 121, Delete: 46, Tab: 9, ArrowLeft: 37, ArrowRight: 39 };

/** Click the open menu's item reading `text` (a real click). */
async function chooseItem(p, text) {
  const [x, y] = await p.ev(`(() => { const b = [...document.querySelectorAll(".menu [role=menuitem]")].find((b) => b.querySelector(".menu-text").textContent === ${JSON.stringify(text)}); const r = b.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
  await p.mouse(x, y);
}

/** Open the bar's "New" menu with the mouse and choose `text` in it. */
async function fromNew(p, text) {
  await p.click(".vars-new");
  await chooseItem(p, text);
}

/** Open the preview's "⋯" menu with the mouse and choose `text` in it. */
async function fromMore(p, text) {
  await p.click(".pane-vars .preview button.more");
  await chooseItem(p, text);
}

const row = (name) => `document.querySelector('.list tbody tr[data-name="${name}"]')`;
const rowSel = (name) => `.list tbody tr[data-name="${name}"] td`;

test("a double-click on a directory row opens it; on a variable it selects it", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const browse = () => p.ev("window.saturnus.explorer.browse");
  // The first click draws the list again: the second lands on a new row.
  await p.click(rowSel("MYDIR"), 2);
  assert.deepEqual(await browse(), ["HOME", "MYDIR"], "MYDIR opened");
  assert.ok(await p.ev(row("A")), "the list shows what MYDIR holds");

  await p.ev(`window.saturnus.explorer.go(["HOME"])`);
  await p.click(rowSel("X"), 2);
  assert.deepEqual(await browse(), ["HOME"], "a variable opens nothing");
  assert.equal(await p.ev("window.saturnus.explorer.selected?.name"), "X", "X selected");
});

test("the New directory field takes the focus once and steals none", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await fromNew(p, "New directory in HOME…");
  assert.equal(await p.focused(), "new", "the field has the focus when it opens");
  await p.type("AB");

  // Typing in the search field while the field is open: the letters stay there.
  await p.click(".vars-find");
  await p.type("my");
  assert.equal(await p.focused(), "find");
  assert.equal(await p.ev(`document.querySelector(".vars-find").value`), "my");
  await p.ev(`document.querySelector(".vars-find").value = ""; document.querySelector(".vars-find").dispatchEvent(new Event("input")); true`);
  // A memory refresh and a row selected leave the focus where it is.
  await p.ev("window.saturnus.memory.refresh().then(() => true)");
  await p.click(rowSel("X"));
  assert.equal(await p.focused(), "find", "neither took the focus into the field");
  assert.equal(await p.ev(`document.querySelector(".vars-making input").value`), "AB", "the name kept");

  // Back in the field, a render keeps its focus and its caret.
  await p.click(".vars-making input");
  await p.ev(`document.querySelector(".vars-making input").setSelectionRange(1, 1); true`);
  await p.ev("window.saturnus.explorer.renderVars(); true");
  assert.equal(await p.focused(), "new");
  assert.deepEqual(await p.ev(`(() => { const i = document.querySelector(".vars-making input"); return [i.value, i.selectionStart]; })()`), ["AB", 1]);
});

test("Rename and New directory close each other; Rename gets the focus", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await fromNew(p, "New directory in HOME…");
  await p.click(rowSel("X"));
  await fromMore(p, "Rename…");
  await sleep(100);
  assert.equal(await p.focused(), "rename", "the rename field has the focus");
  assert.equal(await p.ev(`document.querySelector(".vars-making").children.length`), 0, "New directory closed");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row input").value`), "X");

  await fromNew(p, "New directory in HOME…");
  assert.equal(await p.focused(), "new");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row")`), null, "the rename closed");
});

test("a name the calculator refuses stays in the field with the reason", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await fromNew(p, "New directory in HOME…");
  await p.type("1A");
  await p.press("Enter", "Enter", 13);
  await until(p.ev, "!window.saturnus.store.state.writing && window.saturnus.store.state.writeMessage", 5_000, "the refusal");
  await sleep(100);
  const field = await p.ev(`(() => { const i = document.querySelector(".vars-making input"); return i && { value: i.value, readOnly: i.readOnly, error: document.querySelector(".vars-making .edit-error")?.textContent }; })()`);
  assert.deepEqual(field, { value: "1A", readOnly: false, error: 'Creating 1A failed: "1A" is not a plain variable name' });
  assert.equal(await p.focused(), "new", "the field has the focus again");

  // Corrected, it is created and the field closes.
  await p.ev(`(() => { const i = document.querySelector(".vars-making input"); i.select(); return true; })()`);
  await p.type("NEWD");
  await p.press("Enter", "Enter", 13);
  await until(p.ev, row("NEWD"), 5_000, "NEWD listed");
  await until(p.ev, `document.querySelector(".vars-making").children.length === 0`, 2_000, "the field closed");
  assert.equal(await p.ev("window.saturnus.explorer.selected?.name"), "NEWD");
});

test("the Rename field takes the focus once and steals none", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.click(rowSel("X"));
  await fromMore(p, "Rename…");
  await sleep(100);
  assert.equal(await p.focused(), "rename", "the field has the focus when it opens");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row input").selectionStart`), 1, "the caret after the name");
  await p.type("2");

  // Typing in the search field while Rename is open: the letters stay there.
  await p.click(".vars-find");
  await p.type("x");
  assert.equal(await p.focused(), "find");
  assert.equal(await p.ev(`document.querySelector(".vars-find").value`), "x");
  // A memory refresh leaves the focus where it is.
  await p.ev("window.saturnus.memory.refresh().then(() => true)");
  await p.ev("window.saturnus.explorer.renderVars(); true");
  assert.equal(await p.focused(), "find", "the refresh took no focus into the field");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row input").value`), "X2", "the new name kept");

  // Back in the field, a render keeps its focus and its caret.
  await p.click(".preview .edit-row input");
  await p.ev(`document.querySelector(".preview .edit-row input").setSelectionRange(1, 1); true`);
  await p.ev("window.saturnus.explorer.renderVars(); true");
  assert.equal(await p.focused(), "rename");
  assert.deepEqual(await p.ev(`(() => { const i = document.querySelector(".preview .edit-row input"); return [i.value, i.selectionStart]; })()`), ["X2", 1]);
});

// ------------------------------------------------------------ actions

const isMore = `document.activeElement?.matches(".pane-vars .preview button.more")`;
const keysHere = `document.querySelector(".layer-keys-text").textContent === "Keys here" && !document.querySelector(".layer-keys").classList.contains("visually-hidden")`;

for (const [width, height, mobile] of [[1280, 900, false], [360, 780, true]]) {
  test(`the head at ${width} px: one primary button and a "⋯" on one line`, { timeout: 120_000 }, async (t) => {
    const p = await page(t, { width, height, mobile });
    if (!p) return;
    await p.click(rowSel("X"));
    await until(p.ev, `!document.querySelector(".pane-vars .preview button.edit")?.disabled`, 3_000, "X read");
    const h = await p.head();
    assert.deepEqual(h.buttons, ["Edit", "⋯"]);
    assert.equal(h.meta, "Real number · 16 bytes · # 5B55h");
    assert.equal(h.tops, 1, "the buttons on one line");
    assert.ok(h.height <= (mobile ? 46 : 34), `the buttons ${h.height} px tall`);
    await p.click(rowSel("MYDIR"));
    const d = await p.head();
    assert.deepEqual([d.buttons, d.tops], [["Open", "⋯"], 1]);
    await p.click(".pane-vars .preview button.more");
    const m = await p.menu();
    assert.ok(m.rect.left >= 0 && m.rect.right <= m.view[0] && m.rect.bottom <= m.view[1], `the menu inside the window: ${JSON.stringify(m.rect)}`);
    assert.equal(await p.ev("document.documentElement.scrollWidth - document.documentElement.clientWidth"), 0, "no page overflow");
    if (mobile) {
      const small = await p.ev(`[...document.querySelectorAll(".menu [role=menuitem]")].filter((b) => b.getBoundingClientRect().height < 44).length`);
      assert.equal(small, 0, "items a finger can hit");
      assert.equal(await p.ev(`[...document.querySelectorAll(".menu kbd")].filter((k) => k.checkVisibility()).length`), 0, "no key hints on a phone");
    }
  });
}

test("an object's menu: Copy text, Save as file, Rename, Purge last; its keys", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.click(rowSel("X"));
  await until(p.ev, `!document.querySelector(".pane-vars .preview button.edit")?.disabled`, 3_000, "X read");
  // By the keyboard: the "⋯" focused, Enter opens with the first item.
  await p.ev(`document.querySelector(".pane-vars .preview button.more").focus()`);
  await p.press("Enter", "Enter", 13);
  let m = await p.menu();
  assert.equal(m.label, "Actions for X");
  assert.deepEqual(m.shape, ["Copy text", "Save as file…", "Rename…", "-", "Purge…"]);
  assert.deepEqual(m.danger, ["Purge…"]);
  assert.equal(m.hints["Rename…"], "F2");
  assert.equal(m.hints["Purge…"], "Del");
  assert.match(m.hints["Copy text"], /^(⌘C|Ctrl\+C)$/);
  assert.equal(m.focused, "Copy text");
  assert.equal(await p.ev(`document.querySelector(".pane-vars .preview button.more").getAttribute("aria-expanded")`), "true");
  // Arrows wrap, Home and End, a letter jumps.
  await p.key("ArrowUp");
  assert.equal((await p.menu()).focused, "Purge…", "up from the first: the last");
  await p.key("ArrowDown");
  assert.equal((await p.menu()).focused, "Copy text", "down from the last: the first");
  await p.key("End");
  assert.equal((await p.menu()).focused, "Purge…");
  await p.key("Home");
  assert.equal((await p.menu()).focused, "Copy text");
  await p.key("r", { code: "KeyR", vk: 82 });
  assert.equal((await p.menu()).focused, "Rename…");
  // Escape: the menu closes, the focus is back on "⋯", the view stays open with the keys.
  await p.key("Escape");
  assert.equal(await p.menu(), null);
  assert.ok(await p.ev(isMore), "the focus back on ⋯");
  assert.ok(await p.ev(keysHere), "the keys still here");
  assert.equal(await p.ev("window.saturnus.store.state.layer"), true, "the memory view still open");
  // ArrowUp on the button opens with the last item; Enter runs it.
  await p.key("ArrowUp");
  m = await p.menu();
  assert.equal(m.focused, "Purge…");
  await p.press("Enter", "Enter", 13);
  assert.equal(await p.menu(), null);
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row span").textContent`), "Purge X from HOME?");
  assert.ok(await p.ev(`document.activeElement.matches(".edit-row button.danger")`), "the purge button has the focus");
  assert.equal(await p.ev("window.__writes.length"), 0, "nothing purged yet");
});

test("a directory: the same set from the list and the tree; HOME's Make current", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.click(rowSel("MYDIR"));
  assert.deepEqual((await p.head()).buttons, ["Open", "⋯"]);
  await p.click(".pane-vars .preview button.more");
  const list = await p.menu();
  assert.deepEqual(list.shape, ["Make current", "-", "Store file here…", "New directory here…", "Rename…", "-", "Purge…"]);
  await p.click(".layer-title");
  assert.equal(await p.menu(), null, "a click outside closes it");

  await p.click(`.tree .node[data-path='["HOME","MYDIR"]'] .name`);
  assert.deepEqual(await p.ev("window.saturnus.explorer.browse"), ["HOME", "MYDIR"]);
  const h = await p.head();
  assert.deepEqual([h.title, h.meta, h.buttons], ["MYDIR", "Directory in HOME · 1 variable", ["Make current", "⋯"]]);
  await p.click(".pane-vars .preview button.more");
  const tree = await p.menu();
  assert.deepEqual(tree.shape, ["Store file here…", "New directory here…", "Rename…", "-", "Purge…"]);
  assert.deepEqual(tree.off, []);
  assert.deepEqual(new Set(["Make current", ...tree.items]), new Set(list.items), "the same actions either way, but Open");
  await p.click(".layer-title");

  // Make current from the head: the calculator moves there.
  await p.click(".pane-vars .preview button.act-cd");
  await until(p.ev, `window.__writes.some((w) => w[0] === "changeDir")`, 3_000, "the change asked for");
  assert.deepEqual(await p.ev("window.__writes.at(-1)"), ["changeDir", ["HOME", "MYDIR"]]);
  await until(p.ev, `!window.saturnus.store.state.writing && document.querySelector(".pane-vars .preview-head h3 .here-mark")`, 3_000, "the current badge");
  assert.deepEqual((await p.head()).buttons, ["⋯"], "no Make current for the current directory");
  // The status line has no "Change to this one" any more.
  await p.click(`.tree .node[data-path='["HOME"]'] .name`);
  assert.equal(await p.ev(`document.querySelector(".vars-where").textContent`), "The calculator is in HOME › MYDIR. Show it");
  // HOME: Make current first, no Rename or Purge.
  const home = await p.head();
  assert.deepEqual([home.title, home.buttons], ["HOME", ["Make current", "⋯"]]);
  await p.click(".pane-vars .preview button.more");
  const hm = await p.menu();
  assert.deepEqual(hm.shape, ["Store file here…", "New directory here…", "note"]);
  assert.equal(hm.note, "HOME cannot be renamed or purged.");
  await p.click(".layer-title");
  await p.click(".pane-vars .preview button.act-cd");
  await until(p.ev, `JSON.stringify(window.__writes.at(-1)) === '["changeDir",["HOME"]]'`, 3_000, "HOME made current");
});

test("the context menu: right-click and Shift+F10 on a row or a node, the same items", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const [x, y] = await p.center(rowSel("X"));
  await p.mouse(x, y, "right");
  assert.equal(await p.ev("window.saturnus.explorer.selected?.name"), "X", "the row selected");
  await until(p.ev, `[...document.querySelectorAll(".menu .menu-text")].some((e) => e.textContent === "Copy text")`, 3_000, "the menu follows the read object");
  let m = await p.menu();
  assert.deepEqual(m.shape, ["Edit", "Copy text", "Save as file…", "Rename…", "-", "Purge…"]);
  assert.ok(Math.abs(m.rect.left - x) <= 1 && Math.abs(m.rect.top - y) <= 1, `at the pointer: ${JSON.stringify(m.rect)} vs ${x},${y}`);
  await p.key("Escape");
  assert.equal(await p.menu(), null);
  assert.ok(await p.ev(`document.activeElement === ${row("X")}`), "the focus on the row");

  // Shift+F10 on the focused row: under the row.
  await p.key("F10", { modifiers: 8 });
  m = await p.menu();
  const r = await p.ev(`${row("X")}.getBoundingClientRect().bottom`);
  assert.deepEqual(m.items, ["Edit", "Copy text", "Save as file…", "Rename…", "Purge…"]);
  assert.ok(m.rect.top >= r && m.rect.top <= r + 8, "under the row");
  await p.key("Escape");

  // A tree node: its directory is shown, its menu opens.
  const [nx, ny] = await p.center(`.tree .node[data-path='["HOME","MYDIR"]'] .name`);
  await p.mouse(nx, ny, "right");
  assert.deepEqual(await p.ev("window.saturnus.explorer.browse"), ["HOME", "MYDIR"]);
  m = await p.menu();
  assert.deepEqual(m.items, ["Make current", "Store file here…", "New directory here…", "Rename…", "Purge…"]);
  await p.key("Escape");
  assert.ok(await p.ev(`document.activeElement.matches(".tree .node.shown")`), "the focus on the node");
});

test("F2 renames and Delete asks to purge, from a row or a node", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.click(rowSel("X"));
  await p.ev(`${row("X")}.focus()`);
  await p.key("F2");
  assert.equal(await p.focused(), "rename");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row input").value`), "X");
  await p.key("Escape");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row")`), null, "Escape in the field cancels");
  await p.ev(`${row("X")}.focus()`);
  await p.key("Delete");
  assert.ok(await p.ev(`document.activeElement.matches(".edit-row button.danger")`), "the purge question");
  assert.equal(await p.ev(`document.activeElement.textContent`), "Purge X");

  // On the tree: the directory shown.
  await p.ev(`window.saturnus.explorer.go(["HOME", "MYDIR"]); document.querySelector(".tree .node.shown").focus()`);
  await p.key("Delete");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row span").textContent`), "Purge the directory MYDIR and the 1 variable in it?");
  // HOME has neither.
  await p.ev(`window.saturnus.explorer.go(["HOME"]); document.querySelector(".tree .node.shown").focus()`);
  await p.key("F2");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row")`), null);
});

test("while a write runs: the menu closes, its writes are off, New is off", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.click(rowSel("X"));
  await until(p.ev, `!document.querySelector(".pane-vars .preview button.edit")?.disabled`, 3_000, "X read");
  await p.click(".pane-vars .preview button.more");
  assert.ok(await p.menu());
  await p.ev(`window.saturnus.store.set({ writing: "Storing…" })`);
  await sleep(100);
  assert.equal(await p.menu(), null, "the menu closed");
  const off = await p.ev(`[...document.querySelectorAll(".pane-vars button.write")].map((b) => [b.textContent, b.disabled])`);
  assert.ok(off.every(([, d]) => d), JSON.stringify(off));
  assert.equal(await p.ev(`document.querySelector(".pane-vars .preview button.more").disabled`), false, "⋯ stays on");
  // The calculator typing: the menu opens with its writes off in place.
  await p.ev(`window.saturnus.store.set({ writing: null, busy: true })`);
  await sleep(100);
  await p.ev(`document.querySelector(".pane-vars .preview button.more").click()`);
  const m = await p.menu();
  assert.deepEqual(m.items, ["Copy text", "Save as file…", "Rename…", "Purge…"], "the same shape");
  assert.deepEqual(m.off, ["Save as file…", "Rename…", "Purge…"]);
  await p.ev(`window.saturnus.store.set({ busy: false })`);
  await sleep(100);
  assert.deepEqual((await p.menu()).off, [], "on again when it is done");
});

test("the bar's New menu, and New directory here for a directory in the list", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.click(".vars-new");
  const m = await p.menu();
  assert.deepEqual(m.shape, ["Store file in HOME…", "New directory in HOME…", "note"]);
  assert.equal(m.note, "Or drop files on a directory.");
  assert.equal(await p.ev(`document.querySelector(".vars-new").getAttribute("aria-expanded")`), "true");
  await p.click(".vars-new");
  assert.equal(await p.menu(), null, "its button closes it again");
  assert.equal(await p.ev(`document.querySelector(".vars-new").getAttribute("aria-expanded")`), "false");

  await p.click(rowSel("MYDIR"));
  await fromMore(p, "New directory here…");
  assert.equal(await p.focused(), "new");
  assert.equal(await p.ev(`document.querySelector(".vars-making .edit-label").textContent`), "New directory in HOME › MYDIR:");
  await p.type("SUB");
  await p.press("Enter", "Enter", 13);
  await until(p.ev, `window.__writes.some((w) => w[0] === "createDir")`, 3_000, "created");
  assert.deepEqual(await p.ev("window.__writes[0]"), ["createDir", ["HOME", "MYDIR"], "SUB"]);
});

test("without writes: an object has Edit and Copy text inline, HOME no buttons", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(`window.saturnus.explorer.writes = null; window.saturnus.explorer.renderVars(); true`);
  assert.equal(await p.ev(`document.querySelector(".vars-new").hidden`), true, "no New");
  assert.deepEqual((await p.head()).buttons, [], "HOME: a head, no buttons");
  await p.click(rowSel("X"));
  await until(p.ev, `[...document.querySelectorAll(".pane-vars .preview-actions > button")].length === 2`, 3_000, "X read");
  assert.deepEqual((await p.head()).buttons, ["Edit", "Copy text"]);
  await p.click(rowSel("MYDIR"));
  assert.deepEqual((await p.head()).buttons, ["Open"]);
  // The stack: Edit and Copy text, as before.
  await p.ev(`window.saturnus.explorer.setTab("stack")`);
  await until(p.ev, `document.querySelector(".pane-stack .preview-head")`, 3_000, "the stack");
  assert.deepEqual((await p.head("stack")).buttons, ["Edit", "Copy text"]);
});

test("the divider between the tree and the list: drag, keys, kept, reset, limits", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const treeW = () => p.ev(`Math.round(document.querySelector(".pane-vars .tree").getBoundingClientRect().width)`);
  const handle = ".pane-vars .resize-tree";
  const w0 = await treeW();
  assert.ok(await p.ev(`document.querySelector("${handle}").checkVisibility()`), "a handle");
  const [hx, hy] = await p.center(handle);
  assert.ok(Math.abs(hx - (await p.ev(`document.querySelector(".pane-vars .tree").getBoundingClientRect().right`))) <= 2, "on the tree's edge");
  await p.drag([hx, hy], 60);
  assert.equal(await treeW(), w0 + 60, "dragged 60 px wider");
  assert.equal(await p.ev(`localStorage.getItem("saturnus.treeWidth")`), String(w0 + 60), "kept");
  await p.ev(`document.querySelector("${handle}").focus()`);
  await p.key("ArrowRight");
  assert.equal(await treeW(), w0 + 76, "a step to the right");
  await p.key("ArrowLeft");
  await p.key("ArrowLeft");
  assert.equal(await treeW(), w0 + 44);
  // Kept across a reload.
  await p.load();
  assert.equal(await treeW(), w0 + 44, "the same width after a reload");
  // Neither side unusably narrow.
  let [x, y] = await p.center(handle);
  await p.drag([x, y], -1000);
  assert.equal(await treeW(), 80, "the tree at least 80 px");
  [x, y] = await p.center(handle);
  await p.drag([x, y], 2000);
  const list = await p.ev(`Math.round(document.querySelector(".pane-vars .list-wrap").getBoundingClientRect().width)`);
  assert.ok(list >= 159, `the list keeps ${list} px`);
  // A double-click: the default again, forgotten.
  [x, y] = await p.center(handle);
  await p.mouse(x, y, "left", 1);
  await p.mouse(x, y, "left", 2);
  assert.equal(await treeW(), w0, "the default width");
  assert.equal(await p.ev(`localStorage.getItem("saturnus.treeWidth")`), null);
});

test("no divider on a phone", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { width: 360, height: 780, mobile: true });
  if (!p) return;
  assert.equal(await p.ev(`document.querySelector(".pane-vars .resize-tree").checkVisibility()`), false);
});

test("keys on a tree node act on its directory, not on a row still selected in the list", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(`window.saturnus.explorer.go(["HOME", "MYDIR"])`);
  await p.click(rowSel("A"));
  assert.equal(await p.ev("window.saturnus.explorer.selected?.name"), "A");
  // Tab to the tree: the focus on MYDIR's node, A still selected.
  await p.ev(`document.querySelector(".tree .node.shown").focus()`);
  await p.key("F2");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row input")?.value`), "MYDIR", "MYDIR renamed, not A");
  assert.equal(await p.ev("window.saturnus.explorer.selected"), null, "the node's directory shown");
  await p.key("Escape");
  await p.click(rowSel("A"));
  await p.ev(`document.querySelector(".tree .node.shown").focus()`);
  await p.key("F10", { modifiers: 8 });
  assert.equal((await p.menu())?.label, "Actions for MYDIR");
  await p.key("Escape");
  assert.ok(await p.ev(`document.activeElement.matches(".tree .node.shown")`), "the focus back on the node");
});

test("a menu stays open through scrolls that do not move its row", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.click(rowSel("X"));
  // Opened, and at once a scroll of the list (as a redraw clamps it).
  assert.ok(await p.ev(`(() => {
    const x = window.saturnus.explorer;
    x.openActions(x.subject(), { at: { x: 600, y: 300 }, context: true, returnFocus: () => x.ui.listBody.querySelector('[aria-selected="true"]') });
    document.querySelector(".pane-vars .list-wrap").dispatchEvent(new Event("scroll"));
    return !!document.querySelector(".menu");
  })()`), "open after the scroll that came with it");
  await sleep(100);
  await p.ev(`document.querySelector(".pane-vars .preview").dispatchEvent(new Event("scroll"))`);
  assert.ok(await p.menu(), "a scroll of the preview leaves it");
  await p.ev(`document.querySelector(".pane-vars .list-wrap").dispatchEvent(new Event("scroll"))`);
  assert.equal(await p.menu(), null, "a scroll of the list closes it");
});

test("only the action keys select a row", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(`${row("X")}.focus()`);
  await p.key("F10");
  await p.key("F2", { modifiers: 1 });
  await p.key("Delete", { modifiers: 8 });
  assert.equal(await p.ev("window.saturnus.explorer.selected"), null, "F10, Alt+F2 and Shift+Delete do nothing");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row")`), null);
  await p.key("F2");
  assert.equal(await p.ev("window.saturnus.explorer.selected?.name"), "X", "F2 selects the row and renames it");
  assert.equal(await p.focused(), "rename");
});

test("the divider says its width and limits from the start", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const values = () => p.ev(`(() => {
    const h = document.querySelector(".pane-vars .resize-tree");
    return [h.getAttribute("aria-valuemin"), h.getAttribute("aria-valuenow"), h.getAttribute("aria-valuemax"),
      String(Math.round(document.querySelector(".pane-vars .tree").getBoundingClientRect().width))];
  })()`);
  // Set once the split is laid out (a ResizeObserver, the next frame).
  const handle = `document.querySelector(".pane-vars .resize-tree")`;
  await until(p.ev, `${handle}.hasAttribute("aria-valuenow")`, 2_000, "the width set");
  let [min, now, max, width] = await values();
  assert.deepEqual([min, now], ["80", width], "the default width");
  assert.ok(Number(max) > 160, `a maximum: ${max}`);
  await p.ev(`localStorage.setItem("saturnus.treeWidth", "200")`);
  await p.load();
  await until(p.ev, `${handle}.hasAttribute("aria-valuenow")`, 2_000, "the kept width set");
  [min, now, max, width] = await values();
  assert.deepEqual([min, now, width], ["80", "200", "200"], "the kept width");
});

test("Cmd/Ctrl+E follows the keys: the view's selection, or after the calculator took them its command line or level 1", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(`(() => {
    window.__edited = [];
    window.saturnus.palette.openEditor = async (target) => { window.__edited.push(target.kind === "variable" ? target.name : target.kind === "level" ? "level " + target.level : target.kind); };
    window.__line = false;
    window.saturnus.backend.commandLine = async () => ({ active: window.__line, text: "", cursor: 0 });
    return true;
  })()`);
  const mod = (await p.ev("window.saturnus.bindings.isMac")) ? 4 : 2;
  const edit = async () => {
    await p.ev("window.__edited.length = 0");
    await p.key("e", { code: "KeyE", vk: 69, modifiers: mod });
    await sleep(200);
    return p.ev("window.__edited.join()");
  };
  const inView = () => p.ev(`window.saturnus.explorer.hasFocus()`);
  // A row clicked in the view (the keys stay the calculator's): its object.
  await p.click(rowSel("X"));
  await until(p.ev, `!document.querySelector(".pane-vars .preview button.edit")?.disabled`, 3_000, "X read");
  assert.equal(await edit(), "X");
  // The owner's case: an arrow typed to the calculator, then Cmd+E: the calculator's level 1.
  await p.key("ArrowUp");
  assert.equal(await edit(), "level 1");
  await p.click(rowSel("X"));
  assert.equal(await edit(), "X");
  // A click on the calculator's display: the calculator's level 1, then its open command line.
  const [x, y] = await p.center("sat-calculator canvas");
  await p.mouse(x, y);
  assert.equal(await edit(), "level 1");
  await p.ev("window.__line = true");
  assert.equal(await edit(), "cmdline");
  // Alt+M: the keys in the view, the indicator shows the way back; Cmd+E edits the selection.
  await p.key("m", { code: "KeyM", vk: 77, modifiers: 1 });
  assert.equal(await inView(), true);
  assert.deepEqual(await p.ev(`(() => { const k = document.querySelector(".layer-keys"); const b = k.querySelector(".layer-keys-back"); return [k.classList.contains("visually-hidden"), b.hidden, b.textContent, k.getAttribute("aria-live")]; })()`),
    [false, false, "Give back (Esc)", "polite"]);
  assert.equal(await edit(), "X");
  // The indicator's button gives the keys back.
  await p.click(".layer-keys-back");
  assert.equal(await inView(), false);
  assert.equal(await p.ev(`document.querySelector(".layer-keys").classList.contains("visually-hidden")`), true);
  assert.equal(await edit(), "cmdline");
  // So does Escape; and a click on a key gives them back while it presses the key.
  await p.key("m", { code: "KeyM", vk: 77, modifiers: 1 });
  assert.equal(await inView(), true);
  await p.key("Escape");
  assert.equal(await inView(), false);
  assert.equal(await edit(), "cmdline");
  await p.key("m", { code: "KeyM", vk: 77, modifiers: 1 });
  await p.ev(`(() => { window.__keys = []; const b = window.saturnus.backend; const d = b.keyDown.bind(b); b.keyDown = (k, s) => { window.__keys.push(k); d(k, s); }; return true; })()`);
  const [kx, ky] = await p.ev(`(() => { const r = document.querySelector("sat-calculator").skinKey("enter").querySelector(".cap").getBoundingClientRect(); return [Math.round(r.x + r.width / 2), Math.round(r.y + r.height / 2)]; })()`);
  await p.mouse(kx, ky);
  assert.equal(await inView(), false);
  assert.deepEqual(await p.ev("window.__keys"), ["enter"]);
});
