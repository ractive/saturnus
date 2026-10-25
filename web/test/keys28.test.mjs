// The 28S's commands typed on its keys from Search (web/keys28.js): the
// keys for a name in each entry state, the menu labels read off a frame,
// and the palette model sending them: `node --test web/test/`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { fromCodes28, KEY_COMMANDS, keys28, labelsShown } from "../keys28.js";
import { PaletteModel } from "../palette.js";
import { buildIndex } from "../reference.js";

test("a plain name is a space, its letters and ENTER; insert leaves out ENTER", () => {
  assert.deepEqual(keys28("SIN", "run"), ["space", "letters", "letteri", "lettern", "enter"]);
  assert.deepEqual(keys28("SIN", "insert"), ["space", "letters", "letteri", "lettern"]);
  // → types as it is outside alpha mode.
  assert.deepEqual(keys28("R→C", "run"), ["space", "letterr", "shift", "letteru", "letterc", "enter"]);
});

test("a command that is a key is pressed when run, typed when inserted", () => {
  assert.deepEqual(keys28("DROP", "run"), ["drop"]);
  assert.deepEqual(keys28("√", "run"), ["shift", "minus"]);
  assert.deepEqual(keys28("SWAP", "run", { shift: true }), ["shift", "shift", "backspace"]);
  assert.deepEqual(keys28("DROP", "insert"), ["space", "letterd", "letterr", "lettero", "letterp"]);
  for (const k of Object.keys(KEY_COMMANDS)) assert.ok(keys28(k, "run").length >= 1, k);
});

test("an operator in a name needs alpha mode and its padding taken out, with the cursor menu", () => {
  assert.deepEqual(keys28("STO+", "run"), ["alpha", "space", "letters", "lettert", "lettero", "plus", "backspace", "e", "backspace", "f", "enter"]);
  // Alpha already on: not pressed; a menu shown makes way for the cursor menu and comes back.
  assert.deepEqual(keys28("Σ+", "run", { alpha: true, labels: true }), ["cursor", "space", "shift", "letterv", "plus", "backspace", "e", "backspace", "f", "cursor", "enter"]);
  // Inserted, alpha goes back off.
  assert.deepEqual(keys28("%T", "insert").slice(-2), ["lettert", "alpha"]);
  // In alpha mode → is padded too.
  assert.deepEqual(keys28("→LIST", "run", { alpha: true }).slice(0, 4), ["space", "shift", "letteru", "backspace"]);
});

test("lowercase, a pending shift and Menu Lock are set right for the name and given back", () => {
  assert.deepEqual(keys28("e", "run"), ["space", "lc", "lettere", "enter"]);
  assert.deepEqual(keys28("SIN", "insert", { lowercase: true }), ["space", "lc", "letters", "letteri", "lettern", "lc"]);
  assert.deepEqual(keys28("i", "insert", { lowercase: true }), ["space", "letteri"]);
  assert.deepEqual(keys28("ABS", "run", { menuLock: true }), ["shift", "alpha", "space", "lettera", "letterb", "letters", "enter", "shift", "alpha"]);
  assert.deepEqual(keys28("ABS", "run", { shift: true }).slice(0, 2), ["shift", "space"]);
  assert.throws(() => keys28("A_B", "run"), /no key/);
});

test("menu labels are six inverse boxes on the bottom line", () => {
  const frame = (dark) => {
    const row = 18;
    const bytes = new Uint8Array(row * 32);
    for (let y = 0; y < 32; y++) for (let x = 0; x < 137; x++) if (dark(x, y)) bytes[y * row + (x >> 3)] |= 0x80 >> (x & 7);
    return { width: 137, height: 32, pixels: Buffer.from(bytes).toString("base64") };
  };
  assert.equal(labelsShown(frame((x, y) => y >= 24 && x % 23 !== 22)), true);
  assert.equal(labelsShown(frame(() => false)), false);
  assert.equal(labelsShown(frame((x, y) => y >= 24)), false, "a dark line without the gaps is no menu");
  assert.equal(labelsShown(null), false);
});

test("the palette model sends a 28S command as keys, reading the state first", async () => {
  const data = JSON.parse(readFileSync(new URL("../commands.json", import.meta.url), "utf8"));
  const calls = [];
  const backend = {
    commandLine: async () => { throw new Error("the palette does not read the 28S's command line"); },
    entryModes: async () => ({ lowercase: false, menuLock: false }),
    typeKeys: (keys) => calls.push(keys),
    stats: async () => ({ loop: "sleep" }),
  };
  const store = { state: { model: "28s", booted: "28s", keysDown: [], frame: { width: 137, height: 32, pixels: "", annunciators: { alpha: false, leftshift: true } } } };
  const m = new PaletteModel(backend, store);
  m.pace = { first: 0, poll: 0, limit: 100 };
  m.setIndex(buildIndex(data, "28s"));
  await m.open();
  assert.equal(m.canType, true);
  assert.equal(m.noTyping, null);
  m.setQuery("SIN");
  const r = await m.choose(m.rows[0]);
  assert.equal(r.close, true);
  assert.deepEqual(calls, [["shift", "space", "letters", "letteri", "lettern", "enter"]]);
  await m.choose(m.rows[0], { opposite: true });
  assert.deepEqual(calls[1], ["shift", "space", "letters", "letteri", "lettern"]);
});

test("the 48's character codes the 28S has keys for become its characters; the others are refused", () => {
  assert.equal(fromCodes28("\\<< \\-> X '\\GS(I=1,X,\\v/I)' \\>>"), "« → X 'Σ(I=1,X,√I)' »");
  assert.equal(fromCodes28("\\pi \\<= \\>= \\=/ \\.S \\.d \\^o \\Gm"), "π ≤ ≥ ≠ ∫ ∂ ° µ");
  assert.equal(fromCodes28("« 1 2 + »"), "« 1 2 + »", "text without codes is unchanged");
  for (const t of ["\\oo", "\\Ga", "\\160", "\\\\", "a\\"]) assert.throws(() => fromCodes28(t), /28S cannot type .*no backslash key/, t);
});
