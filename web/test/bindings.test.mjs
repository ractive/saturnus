// The keyboard shortcuts' model (web/bindings.js): physical-key storage,
// matching, labels, the warnings and the defaults' reachability on five
// layouts. `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  ACTIONS, Bindings, DEAD_KEYS, calculatorTypes, comboLabel, comboOf, defaultNumberModifier, normalize, numberDigit, numberLabel, parseCombo, reserved,
} from "../bindings.js";

const ev = (code, o = {}) => ({ code, key: "", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...o });

test("a binding is the physical key plus modifiers, written in one order", () => {
  assert.equal(normalize("Shift+Alt+KeyM"), "Alt+Shift+KeyM");
  assert.equal(normalize("Mod+KeyK", true), "Meta+KeyK");
  assert.equal(normalize("Mod+KeyK", false), "Ctrl+KeyK");
  assert.equal(normalize("Hyper+KeyK"), null);
  assert.equal(normalize("Alt+ShiftLeft"), null, "a modifier alone is not a key");
  assert.deepEqual(parseCombo("Ctrl+Escape"), { code: "Escape", ctrl: true, alt: false, shift: false, meta: false });
  assert.equal(comboOf(ev("KeyO", { altKey: true, key: "ø" })), "Alt+KeyO", "the character a layout gives does not matter");
  assert.equal(comboOf(ev("AltLeft", { altKey: true })), null);
  assert.equal(comboOf(ev("BracketLeft", { key: "ü" })), "BracketLeft");
});

test("matching: the defaults, Cmd on a Mac and Ctrl elsewhere for the palette, groups", () => {
  const mac = new Bindings({ isMac: true });
  const pc = new Bindings({ isMac: false });
  assert.equal(mac.match(ev("KeyK", { metaKey: true })), "palette");
  assert.equal(mac.match(ev("KeyK", { ctrlKey: true })), null);
  assert.equal(pc.match(ev("KeyK", { ctrlKey: true })), "palette");
  assert.equal(pc.match(ev("KeyK", { ctrlKey: true, shiftKey: true })), null, "exact modifiers");
  assert.equal(pc.match(ev("Escape")), "on");
  assert.equal(pc.match(ev("Escape"), "app"), null);
  assert.equal(pc.match(ev("KeyO", { altKey: true })), "on");
  assert.equal(pc.match(ev("KeyM", { altKey: true }), "app"), "layerFocus");
  assert.equal(pc.match(ev("KeyM", { altKey: true, shiftKey: true }), "app"), "layer");
  assert.equal(pc.is("palette", ev("KeyK", { ctrlKey: true })), true);
  // Edit: Cmd+E on a Mac, Ctrl+E elsewhere, free of the browser's and the system's keys.
  assert.equal(mac.match(ev("KeyE", { metaKey: true }), "app"), "edit");
  assert.equal(pc.match(ev("KeyE", { ctrlKey: true }), "app"), "edit");
  assert.equal(pc.match(ev("KeyE", { altKey: true }), "app"), null);
  assert.deepEqual(pc.warnings("edit", "Ctrl+KeyE"), []);
  assert.deepEqual(mac.warnings("edit", "Meta+KeyE"), []);
  for (const a of ACTIONS) assert.ok(pc.keys(a.id).length > 0, `${a.id} has a default`);
});

test("rebinding is stored as the changes only, survives a reload, and reset restores", () => {
  const b = new Bindings({ isMac: false });
  let calls = 0;
  b.onChange(() => calls++);
  b.set("on", ["KeyQ"]);
  b.add("leftshift", "Alt+KeyJ");
  b.setNumberModifier("ctrl");
  assert.equal(calls, 3);
  const saved = JSON.parse(JSON.stringify(b));
  assert.deepEqual(saved, { version: 1, keys: { on: ["KeyQ"], leftshift: ["Alt+KeyL", "Alt+KeyJ"] }, numberModifier: "ctrl" });
  const again = new Bindings({ isMac: false, saved });
  assert.equal(again.match(ev("KeyQ")), "on");
  assert.equal(again.match(ev("Escape")), null, "the old key is free");
  assert.equal(again.match(ev("KeyJ", { altKey: true })), "leftshift");
  assert.equal(again.numberModifier, "ctrl");
  again.remove("leftshift", "Alt+KeyJ");
  assert.equal(again.isDefault("leftshift"), true);
  assert.deepEqual(again.toJSON().keys, { on: ["KeyQ"] });
  again.reset();
  assert.equal(again.match(ev("Escape")), "on");
  assert.deepEqual(again.toJSON(), { version: 1, keys: {} });
  // Rubbish in storage is dropped, not thrown.
  const odd = new Bindings({ saved: { keys: { on: ["Nope+X", 3], nothing: ["KeyA"] }, numberModifier: "hyper" } });
  assert.deepEqual(odd.keys("on"), []);
  assert.equal(odd.numberModifier, "alt");
});

test("warnings: another action's key, the browser's and the system's, keys that type, dead keys", () => {
  const b = new Bindings({ isMac: false });
  b.add("darker", "Alt+KeyO");
  const kinds = (id, k) => b.warnings(id, k).map((w) => w.kind);
  assert.deepEqual(kinds("darker", "Alt+KeyO"), ["conflict"]);
  assert.match(b.warnings("darker", "Alt+KeyO")[0].text, /bound to “ON”; that one wins/);
  assert.match(b.warnings("on", "Alt+KeyO")[0].text, /bound to “Darker display”; this one wins/);
  assert.deepEqual(kinds("on", "Shift+Escape"), ["reserved"]);
  assert.deepEqual(kinds("on", "Escape"), ["reserved"], "Esc leaves fullscreen");
  assert.deepEqual(kinds("on", "Ctrl+Digit3"), ["reserved"]);
  assert.deepEqual(kinds("on", "Alt+Digit3"), ["reserved"]);
  assert.deepEqual(kinds("on", "KeyQ"), ["typing"]);
  assert.deepEqual(kinds("on", "Enter"), ["fixed"]);
  assert.deepEqual(kinds("on", "Equal"), ["typing", "dead"]);
  assert.deepEqual(kinds("on", "BracketRight"), ["typing", "dead"], "every character key may type one");
  // With the layout's map, the key's own character decides.
  b.layout = new Map([["BracketRight", "+"], ["BracketLeft", "ü"]]);
  assert.match(b.warnings("on", "BracketRight").find((w) => w.kind === "typing").text, /types “\+”/);
  assert.ok(!kinds("on", "BracketLeft").includes("typing"), "ü types nothing on the calculator");
  assert.deepEqual(kinds("leftshift", "Alt+KeyL"), [], "its own key");
  const mac = new Bindings({ isMac: true });
  assert.deepEqual(mac.warnings("on", "Meta+KeyQ").map((w) => w.kind), ["reserved"]);
  assert.deepEqual(mac.warnings("on", "Meta+KeyW").map((w) => w.kind), ["reserved"]);
  assert.deepEqual(mac.warnings("on", "Meta+Digit2").map((w) => w.kind), ["reserved"]);
  assert.equal(reserved("Meta+Digit2", { isMac: true, host: "tauri" }), null, "the app has no tabs");
  assert.ok(reserved("Meta+KeyQ", { isMac: true, host: "tauri" }), "but quits");
  // The browsers' inspect element: not the copy screen key, and said when bound.
  assert.deepEqual(b.keys("copyScreen"), ["Alt+Shift+KeyC"]);
  assert.deepEqual(kinds("copyScreen", "Alt+Shift+KeyC"), []);
  assert.deepEqual(kinds("copyScreen", "Ctrl+Shift+KeyC"), ["reserved"]);
  assert.match(b.warnings("copyScreen", "Ctrl+Shift+KeyC")[0].text, /inspect element/);
  assert.deepEqual(mac.warnings("copyScreen", "Shift+Meta+KeyC").map((w) => w.kind), ["reserved"]);
  assert.equal(reserved("Ctrl+Shift+KeyC", { host: "tauri" }), null, "the app has no inspector shortcut");
});

test("labels: the layout's character for the physical key, Mac symbols, US labels without a layout", () => {
  const de = new Map([["BracketLeft", "ü"], ["KeyY", "z"], ["Backquote", "^"]]);
  assert.equal(comboLabel("BracketLeft", { layout: de }), "Ü");
  assert.equal(comboLabel("Alt+KeyY", { layout: de }), "Alt+Z");
  assert.equal(comboLabel("BracketLeft"), "[");
  assert.equal(comboLabel("Alt+Shift+KeyM", { isMac: true }), "⌥⇧M");
  assert.equal(comboLabel("Meta+KeyK", { isMac: true }), "⌘K");
  assert.equal(comboLabel("Ctrl+KeyK"), "Ctrl+K");
  assert.equal(comboLabel("Escape", { layout: de }), "Esc");
  assert.equal(comboLabel("Alt+ArrowUp"), "Alt+↑");
});

test("row numbers: Cmd in the Mac app, Ctrl in Mac browsers, Alt in browsers elsewhere; changeable", () => {
  assert.equal(defaultNumberModifier("tauri", true), "meta");
  assert.equal(defaultNumberModifier("worker", true), "ctrl");
  assert.equal(defaultNumberModifier("worker", false), "alt");
  assert.equal(defaultNumberModifier("tauri", false), "alt");
  assert.equal(numberLabel("meta", true, 3), "⌘3");
  assert.equal(numberLabel("ctrl", true, 3), "⌃3");
  assert.equal(numberLabel("alt", false, 3), "Alt+3");
  const e = (o) => ev("Digit3", o);
  assert.equal(numberDigit(e({ altKey: true }), "alt"), 3);
  assert.equal(numberDigit(e({ ctrlKey: true }), "alt"), null);
  assert.equal(numberDigit(e({ ctrlKey: true }), "ctrl"), 3);
  assert.equal(numberDigit(e({ metaKey: true, ctrlKey: true }), "meta"), null);
  assert.equal(numberDigit(e({ altKey: true, shiftKey: true }), "alt"), null);
  assert.equal(numberDigit(ev("Digit0", { altKey: true }), "alt"), null);
  const b = new Bindings({ isMac: false, host: "worker" });
  assert.equal(b.numberWarning("ctrl") !== null, true);
  assert.equal(b.numberWarning("alt"), null);
  assert.equal(b.numberLabel(2), "Alt+2");
});

/**
 * The physical keys that type a dead key, unshifted or shifted, on each
 * layout checked (a key held with Alt/Option is a combination the page
 * receives with its code, so only plain and shifted keys count). From
 * the layouts' charts: US and UK have none; German ^ (Backquote) and ´ `
 * (Equal); Swiss German ^ ` (Equal) and ¨ (BracketRight); French ^ ¨
 * (BracketLeft).
 */
const LAYOUT_DEAD = {
  US: [],
  UK: [],
  German: ["Backquote", "Equal"],
  "Swiss German": ["Equal", "BracketRight"],
  French: ["BracketLeft"],
};

test("every action has a default without a dead key on the US, UK, German, Swiss German and French layouts", () => {
  for (const [layout, dead] of Object.entries(LAYOUT_DEAD)) {
    for (const code of dead) assert.ok(DEAD_KEYS[code]?.includes(layout), `${layout}'s ${code} is in DEAD_KEYS`);
    for (const isMac of [true, false]) {
      const b = new Bindings({ isMac });
      for (const a of ACTIONS) {
        const ok = b.keys(a.id).filter((k) => {
          const c = parseCombo(k, isMac);
          const plain = !c.ctrl && !c.alt && !c.meta;
          return !(plain && dead.includes(c.code));
        });
        assert.ok(ok.length > 0, `${a.id} on ${layout} (${isMac ? "Mac" : "PC"}): ${b.keys(a.id).join(", ")}`);
      }
    }
  }
});

/**
 * What the character keys type, unshifted and shifted, on the layouts the
 * defaults are checked against (from the layouts' charts; a dead key is
 * its accent). A default binding without Ctrl, Alt or Cmd must not be a
 * key that types a character the calculator maps: on German the key
 * right of Ü types +, on Dvorak [ and ] type / and =.
 */
const LAYOUT_CHARS = {
  US: { Backquote: "`~", Minus: "-_", Equal: "=+", BracketLeft: "[{", BracketRight: "]}", Backslash: "\\|", Semicolon: ";:", Quote: "'\"", Comma: ",<", Period: ".>", Slash: "/?" },
  UK: { Backquote: "`¬", Minus: "-_", Equal: "=+", BracketLeft: "[{", BracketRight: "]}", Backslash: "#~", Semicolon: ";:", Quote: "'@", Comma: ",<", Period: ".>", Slash: "/?", IntlBackslash: "\\|" },
  German: { Backquote: "^°", Minus: "ß?", Equal: "´`", BracketLeft: "ü", BracketRight: "+*", Backslash: "#'", Semicolon: "ö", Quote: "ä", Comma: ",;", Period: ".:", Slash: "-_", IntlBackslash: "<>" },
  "Swiss German": { Backquote: "§°", Minus: "'?", Equal: "^`", BracketLeft: "üè", BracketRight: "¨!", Backslash: "$£", Semicolon: "öé", Quote: "äà", Comma: ",;", Period: ".:", Slash: "-_", IntlBackslash: "<>" },
  French: { Backquote: "²", Minus: ")°", Equal: "=+", BracketLeft: "^¨", BracketRight: "$£", Backslash: "*µ", Semicolon: "mM", Quote: "ù%", Comma: ";.", Period: ":/", Slash: "!§", IntlBackslash: "<>" },
  Dvorak: { Backquote: "`~", Minus: "[{", Equal: "]}", BracketLeft: "/?", BracketRight: "=+", Backslash: "\\|", Semicolon: "sS", Quote: "-_", Comma: "wW", Period: "vV", Slash: "zZ" },
};

test("no default without a modifier types a character the calculator maps, on six layouts", () => {
  assert.ok(calculatorTypes("+") && calculatorTypes("a") && calculatorTypes("7") && !calculatorTypes("ü") && !calculatorTypes("§"));
  for (const isMac of [true, false]) {
    const b = new Bindings({ isMac });
    for (const a of ACTIONS) {
      for (const k of b.keys(a.id)) {
        const c = parseCombo(k, isMac);
        if (c.ctrl || c.alt || c.meta) continue;
        assert.ok(!/^(Key[A-Z]|Digit\d|Numpad|Space)/.test(c.code), `${a.id}: ${k} types on every layout`);
        for (const [layout, chars] of Object.entries(LAYOUT_CHARS)) {
          for (const ch of chars[c.code] ?? "") assert.ok(!calculatorTypes(ch), `${a.id}: ${k} types ${ch} on ${layout}`);
        }
      }
    }
  }
  // The old shifts would fail: [ and ] type + on German and / = on Dvorak.
  assert.ok([...LAYOUT_CHARS.German.BracketRight].some(calculatorTypes));
  assert.ok([...LAYOUT_CHARS.Dvorak.BracketLeft].some(calculatorTypes));
});
