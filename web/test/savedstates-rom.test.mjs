// One state file for the page, the desktop app and the command line
// (iteration 34), in the real page in headless Chrome with the real
// wasm core: a 48SX booted from `SATURNUS_ROM_DIR/sxrom-j`, `1 ENTER 2
// ENTER` typed, "Save state to a file…" (its download caught); the CLI
// (`saturnus run --load`, the desktop app's runner reads the same bytes)
// loads that file, presses +, and saves; the page loads the CLI's file
// through "Load state from a file…" and its question: the stack is 3.
// Save state and Load state through the browser's slot too. Skipped
// without the ROM, Chrome (`SATURNUS_CHROME`), the wasm package (`just
// web`) or cargo.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { WEB, session, sleep } from "./chrome.mjs";

async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

const HELPERS = `
  window.__idle = async (ms = 1500) => {
    const b = window.saturnus.backend;
    const t0 = (await b.stats()).emulatedMs;
    for (;;) {
      const s = await b.stats();
      if (s.emulatedMs - t0 >= ms && s.loop === "sleep" && !window.saturnus.store.state.keysDown.length) return true;
      await new Promise((r) => setTimeout(r, 50));
    }
  };
  window.__press = async (key) => {
    const b = window.saturnus.backend;
    b.keyDown(key);
    await new Promise((r) => setTimeout(r, 80));
    b.keyUp(key);
    await window.__idle(300);
  };
  // The download, caught: its name and bytes (base64).
  window.saturnus.backend.saveFile = async (name, blob) => {
    const bytes = new Uint8Array(await blob.arrayBuffer());
    let bin = "";
    for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    window.__download = { name, data: btoa(bin) };
    return name;
  };
  true`;

const STACK = "window.saturnus.backend.stack().then((s) => s.map((l) => l.text))";
const ANSWER = (text) => `[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === ${JSON.stringify(text)}).click(), true`;

test("a state file the page saves loads in the CLI, and the CLI's in the page", { timeout: 300_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "sxrom-j") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/sxrom-j not found");
    return;
  }
  const c = await session(t, { files: { "/__rom": rom } });
  if (!c) return;
  const tmp = mkdtempSync(join(tmpdir(), "saturnus-states-"));
  try {
    const { send, ev, port } = c;
    await send("Page.enable");
    await send("Runtime.enable");
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
    await until(ev, "!!window.saturnus", 15_000, "the page started");
    await ev("window.saturnus.started");
    await ev(HELPERS);
    await ev(`(async () => {
      const bytes = await (await fetch("/__rom")).arrayBuffer();
      await window.saturnus.startWithRom(new File([bytes], "sxrom-j"));
      return true;
    })()`);
    await until(ev, "window.saturnus.store.state.booted === '48sx'", 15_000, "booted");
    await ev("window.__idle()");
    await ev("window.__idle()");
    for (const k of ["1", "enter", "2", "enter"]) await ev(`window.__press(${JSON.stringify(k)})`);
    assert.deepEqual(await ev(STACK), ["2", "1"], "level 1 first");

    // The browser's slot: Save state, a change, Load state after its question.
    const controls = "document.querySelector('sat-controls')";
    await ev(`${controls}.saveState()`);
    assert.equal(await ev("window.saturnus.store.state.message"), "State saved.");
    await ev("window.__press('3')");
    await ev("window.__press('enter')");
    assert.deepEqual(await ev(STACK), ["3", "2", "1"]);
    await ev(`${controls}.loadState(); true`);
    await until(ev, "!!document.querySelector('dialog.confirm[open]')", 5_000, "the question");
    assert.match(await ev("document.querySelector('dialog.confirm h2').textContent"), /^Load the state saved on \d{1,2} [A-Z][a-z]{2} \d{4}, \d\d:\d\d\?$/);
    await ev(ANSWER("Load"));
    await until(ev, "/^Loaded the state saved on/.test(window.saturnus.store.state.message)", 10_000, "loaded");
    await ev("window.__idle()");
    assert.deepEqual(await ev(STACK), ["2", "1"], "the slot's state");

    // The page's file, into the CLI: loaded, +, saved.
    await ev(`${controls}.saveStateToFile()`);
    const { name, data } = await ev("window.__download");
    assert.match(name, /^48sx-\d{4}-\d\d-\d\d-\d{4}\.sat$/);
    const pageFile = join(tmp, name);
    writeFileSync(pageFile, Buffer.from(data, "base64"));
    const keys = join(tmp, "plus.txt");
    writeFileSync(keys, "plus\n");
    const cliFile = join(tmp, "from-cli.sat");
    const cli = spawnSync("cargo", ["run", "-q", "--locked", "-p", "saturnus", "--bin", "saturnus", "--",
      "run", "--model", "48sx", "--rom", rom, "--load", pageFile, "--keys", keys, "--save", cliFile], { cwd: join(WEB, ".."), encoding: "utf8", timeout: 240_000 });
    if (cli.error) {
      t.skip(`cargo cannot run: ${cli.error.message}`);
      return;
    }
    assert.equal(cli.status, 0, `the CLI loads the page's file: ${cli.stderr}`);
    assert.ok(readFileSync(cliFile).subarray(0, 8).equals(Buffer.from("SATURNUS")));

    // The CLI's file, into the page through the file input and its question.
    const b64 = readFileSync(cliFile).toString("base64");
    await ev(`(() => {
      const bin = atob(${JSON.stringify(b64)});
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      const dt = new DataTransfer();
      dt.items.add(new File([bytes], "from-cli.sat"));
      const input = document.getElementById("state-file");
      input.files = dt.files;
      input.dispatchEvent(new Event("change"));
      return true;
    })()`);
    await until(ev, "!!document.querySelector('dialog.confirm[open]')", 5_000, "the file's question");
    assert.equal(await ev("document.querySelector('dialog.confirm h2').textContent"), "Load the state in from-cli.sat?");
    await ev(ANSWER("Load"));
    await until(ev, "window.saturnus.store.state.message === 'Loaded the state in from-cli.sat.'", 10_000, "loaded the CLI's file");
    await ev("window.__idle()");
    assert.deepEqual(await ev(STACK), ["3"], "1 + 2, pressed by the CLI");
  } finally {
    rmSync(tmp, { recursive: true, force: true });
    await c.close();
  }
});
