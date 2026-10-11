// The transfer page as the desktop app runs it (crates/saturnus-tx-tauri,
// web/tx/tauri.js), in headless Chrome with a fake `window.__TAURI__`: its
// commands answered from a file system in memory and a serial port that
// records what it is sent. Covers what the page picks there:
//
// - The right pane is the computer's files, open at the home folder, no
//   picker needed; its card and Change menu offer "This computer's files"
//   and no page files; "Open a folder…" (the app's dialog) jumps there.
// - The folder works through the native handles: a file dropped is
//   written, read back, renamed in place, deleted.
// - "Add a calculator" lists the ports by their adapters' names, the one
//   used last chosen; the link opens the chosen port at the settings and
//   its bytes come back from the port's channel; a loss is said.
// - The links to the site's other pages are hidden (the brand stays).
// Skipped without Chrome or the wasm packages, as tx-page.test.mjs.

import assert from "node:assert/strict";
import { test } from "node:test";
import { session } from "./chrome.mjs";
import { txBuilt, txPage } from "./tx-chrome.mjs";

/** The fake app: `window.__TAURI__.core` and what the tests read (`window.fakeTauri`). */
const FAKE_TAURI = `(() => {
  const home = ["Users", "ann"];
  // A tree of names: a folder is a Map, a file {data, modified}.
  const root = new Map([["Users", new Map([["ann", new Map([
    ["hp", new Map([["OLD.hp", { data: new Uint8Array([72, 80]), modified: 1e12 }]])],
    ["notes.txt", { data: new TextEncoder().encode("hello"), modified: 1e12 }],
    [".hidden", { data: new Uint8Array(1), modified: 1e12 }],
  ])]])]]);
  const fail = (name, message) => { throw name + ": " + message; };
  const at = (names) => {
    let n = root;
    for (const part of names) {
      if (!(n instanceof Map) || !n.has(part)) fail("NotFoundError", "it is not there");
      n = n.get(part);
    }
    return n;
  };
  const parent = (names) => {
    const dir = at(names.slice(0, -1));
    if (!(dir instanceof Map)) fail("TypeMismatchError", "a file where a folder was expected");
    return dir;
  };
  const entry = (name, n) => (n instanceof Map ? { name, directory: true, size: null, modified: null } : { name, directory: false, size: n.data.length, modified: n.modified });
  const header = (o, k) => JSON.parse(decodeURIComponent(o.headers[k]));
  const fake = { calls: [], ports: [], channels: [], written: [], pick: null, root };
  const commands = {
    folder_home: () => home,
    folder_pick: () => fake.pick,
    folder_list: ({ path }) => [...at(path)].filter(([n]) => !n.startsWith(".")).map(([n, v]) => entry(n, v)),
    folder_stat: ({ path }) => entry(path.at(-1) ?? "", at(path)),
    folder_read: ({ path, start = 0, end }) => {
      const n = at(path);
      if (n instanceof Map) fail("TypeMismatchError", "it is not a file");
      return n.data.slice(start, end ?? n.data.length).buffer;
    },
    folder_create: ({ path }) => { const d = parent(path); if (!d.has(path.at(-1))) d.set(path.at(-1), { data: new Uint8Array(0), modified: Date.now() }); },
    folder_write: (body, o) => { const path = header(o, "x-path"); parent(path).set(path.at(-1), { data: new Uint8Array(body), modified: Date.now() }); fake.written.push(path.join("/")); },
    folder_mkdir: ({ path }) => { const d = parent(path); if (!d.has(path.at(-1))) d.set(path.at(-1), new Map()); },
    folder_remove: ({ path, recursive }) => {
      const d = parent(path); const n = at(path);
      if (n instanceof Map && n.size && !recursive) fail("InvalidModificationError", "the folder is not empty");
      d.delete(path.at(-1));
    },
    folder_rename: ({ dir, from, to }) => { const d = at(dir); const n = d.get(from); if (!n) fail("NotFoundError", "it is not there"); d.delete(from); d.set(to, n); },
    serial_ports: () => [
      { name: "/dev/cu.usbserial-A1", label: "FT232R USB UART (FTDI) on /dev/cu.usbserial-A1", usb: true },
      { name: "/dev/cu.Bluetooth-Incoming-Port", label: "Serial port on /dev/cu.Bluetooth-Incoming-Port", usb: false },
    ],
    serial_open: (a) => { fake.ports.push({ name: a.name, baud: a.baud, parity: a.parity, dataBits: a.dataBits }); fake.channels.push(a.onEvent); return fake.ports.length; },
    serial_write: (body, o) => { fake.written.push("port " + o.headers["x-port"] + ": " + [...new Uint8Array(body)].join(",")); },
    serial_close: ({ id }) => { fake.calls.push("close " + id); },
    save_file: (body, o) => { fake.written.push("saved " + decodeURIComponent(o.headers["x-name"])); return true; },
  };
  class Channel { constructor() { this.onmessage = () => {}; } }
  window.fakeTauri = fake;
  window.__TAURI__ = { core: {
    Channel,
    invoke: async (cmd, args, options = {}) => {
      fake.calls.push(cmd);
      const fn = commands[cmd];
      if (!fn) throw "no command " + cmd;
      return fn(args, options);
    },
  } };
})();`;

/** A Chrome session on the page with the fake app, or null (skipped). */
async function open(t) {
  if (!txBuilt()) {
    if (process.env.SATURNUS_AUDIT) assert.fail("web/pkg or web/tx/pkg not built (just web tx-web)");
    t.skip("web/pkg or web/tx/pkg not built (just web tx-web)");
    return null;
  }
  const c = await session(t, { audit: true });
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await c.send("Page.addScriptToEvaluateOnNewDocument", { source: FAKE_TAURI });
  return c;
}

/** The texts of the right pane's "Change" menu, separators as "-", the menu closed again. */
const CHANGE_MENU = `(async () => {
  document.querySelector(".menu")?.remove();
  document.querySelector(".pane[data-pane=right] .pane-change").click();
  for (let i = 0; i < 50 && !document.querySelector(".menu"); i++) await new Promise((r) => setTimeout(r, 20));
  const items = [...document.querySelector(".menu").querySelectorAll("[role=menuitem], [role=separator]")].map((e) => (e.getAttribute("role") === "separator" ? "-" : e.textContent.trim()));
  document.querySelector(".menu").remove();
  return items;
})()`;

test("the app: the right pane is the computer's files at the home folder; Open a folder jumps; files go through the native commands", { timeout: 60_000 }, async (t) => {
  const c = await open(t);
  if (!c) return;
  const p = txPage(c);
  await p.open(c.port);
  await p.until(`tx.panes[1].place?.folder?.kind === "native" && !!tx.panes[1].ui.list.listing`, 10_000, "the computer's files");
  const shown = await c.ev(`({
    title: document.querySelector(".pane[data-pane=right] .pane-title").textContent,
    path: tx.panes[1].path,
    names: tx.names(1),
    crumbs: [...document.querySelectorAll(".pane[data-pane=right] .crumb")].map((b) => b.textContent),
    status: document.querySelector(".pane[data-pane=right] .pane-status-text").textContent,
  })`);
  assert.deepEqual(shown, {
    title: "This computer",
    path: ["This computer", "Users", "ann"],
    names: ["hp", "notes.txt"],
    crumbs: ["This computer", "Users", "ann"],
    status: "The files on this computer.",
  });
  // Up to the top and back: anywhere the user may go.
  await c.ev(`tx.panes[1].navigate(["This computer"]); true`);
  await p.until(`JSON.stringify(tx.names(1)) === '["Users"]'`, 5_000, "the top listed");
  // The Change menu: the app's dialog and the computer's files, no page files.
  assert.deepEqual(await c.ev(CHANGE_MENU), ["A folder on this computer…", "This computer's files", "-", "A calculator on a cable…", "An emulated calculator…", "-", "Serve a folder to a 38G, 39G or 40G…", "-", "Nothing (close the folder)"]);
  // "Open a folder…": the dialog's choice, shown in the same place.
  await c.ev(`fakeTauri.pick = ["Users", "ann", "hp"]; tx.panes[1].openFolder().then(() => true)`);
  await p.until(`JSON.stringify(tx.names(1)) === '["OLD.hp"]'`, 5_000, "the picked folder listed");
  assert.deepEqual(await c.ev(`tx.panes[1].path`), ["This computer", "Users", "ann", "hp"]);
  // A cancelled dialog changes nothing.
  await c.ev(`fakeTauri.pick = null; tx.panes[1].openFolder().then(() => true)`);
  assert.deepEqual(await c.ev(`tx.panes[1].path`), ["This computer", "Users", "ann", "hp"]);

  // A file dropped is written; it reads back; renamed in place; deleted.
  await c.ev(`tx.panes[1].drop([tx.fileItem(new File(["%%HP: T(3)A(R)F(.);\\r\\n1"], "ONE.txt"))]).then(() => true)`);
  await p.until(`tx.names(1).includes("ONE.txt")`, 5_000, "the dropped file listed");
  assert.ok((await c.ev(`fakeTauri.written`)).includes("Users/ann/hp/ONE.txt"));
  const folder = `tx.panes[1].place.folder`;
  const path = `["This computer", "Users", "ann", "hp"]`;
  assert.equal(await c.ev(`${folder}.read(${path}, "ONE.txt").then((f) => new TextDecoder().decode(f.data))`), "%%HP: T(3)A(R)F(.);\r\n1");
  assert.equal(await c.ev(`${folder}.head(${path}, "ONE.txt", 4).then((b) => new TextDecoder().decode(b))`), "%%HP");
  await c.ev(`${folder}.rename(${path}, "ONE.txt", "TWO.txt").then(() => true)`);
  assert.ok((await c.ev(`fakeTauri.calls`)).includes("folder_rename"), "renamed in place, not copied");
  await c.ev(`${folder}.purge(${path}, "TWO.txt").then(() => true)`);
  assert.deepEqual(await c.ev(`${folder}.list(${path}).then((l) => l.entries.map((e) => e.name))`), ["OLD.hp"]);
  // A folder that is not empty is refused without recursive, in the page's words.
  assert.match(await c.ev(`${folder}.purge(["This computer", "Users"], "ann").then(() => "", (e) => e.message)`), /holds \d+ items/);
  // A download goes through the save dialog.
  await c.ev(`tx.app.download("X.hp", new Uint8Array([1, 2])); true`);
  await p.until(`fakeTauri.written.includes("saved X.hp")`, 5_000, "the save dialog");
  // No links to the site's other pages; the brand stays.
  assert.equal(await c.ev(`document.getElementById("to-emulator").hidden`), true);
  assert.equal(await c.ev(`document.querySelector(".brand").hidden`), false);
  assert.deepEqual(c.errors, []);
});

test("the app: the port list names the adapters, remembers the last port, and the link moves bytes through the native port", { timeout: 60_000 }, async (t) => {
  const c = await open(t);
  if (!c) return;
  const p = txPage(c);
  await p.open(c.port);
  // The left card offers the cable in the app (no Web Serial needed).
  assert.equal(await c.ev(`!!document.querySelector('.pane[data-pane=left] [data-action="add-serial"]')`), true);
  await c.ev(`document.querySelector('.pane[data-pane=left] [data-action="add-serial"]').click(); true`);
  await p.until(`document.getElementById("connect-dialog").open`, 5_000, "the connect dialog");
  assert.equal(await c.ev(`document.getElementById("connect-go").textContent`), "Choose the port");
  await c.ev(`document.getElementById("connect-go").click(); true`);
  await p.until(`document.getElementById("port-dialog").open && document.getElementById("port-select").options.length === 2`, 5_000, "the port list");
  assert.deepEqual(await c.ev(`[...document.getElementById("port-select").options].map((o) => o.textContent)`), [
    "FT232R USB UART (FTDI) on /dev/cu.usbserial-A1",
    "Serial port on /dev/cu.Bluetooth-Incoming-Port",
  ]);
  // Cancel: no port opened, the connect dialog stays.
  await c.ev(`document.querySelector("#port-dialog button[value=cancel]").click(); true`);
  await p.until(`!document.getElementById("port-dialog").open`, 5_000, "the port list closed");
  assert.deepEqual(await c.ev(`fakeTauri.ports`), []);
  assert.equal(await c.ev(`document.getElementById("connect-dialog").open`), true);
  await c.ev(`document.getElementById("connect-dialog").close(); true`);

  // The link itself, with the remembered port: open at the settings, bytes in, bytes out, the loss.
  await c.ev(`localStorage.setItem("saturnus.transfer.port", "/dev/cu.usbserial-A1"); true`);
  const got = await c.ev(`import("./tx/tauri.js").then(async (m) => {
    const ports = await m.serialPorts();
    const link = new m.TauriSerialLink(ports[0]);
    const seen = [];
    link.addEventListener("data", (e) => seen.push([...e.detail].join(",")));
    link.addEventListener("lost", (e) => seen.push("lost: " + e.detail));
    await link.open({ baud: 9600, parity: "even", dataBits: 7 });
    fakeTauri.channels[0].onmessage(new Uint8Array([6, 1, 2]).buffer);
    await link.write(new Uint8Array([1, 35, 32]));
    fakeTauri.channels[0].onmessage({ lost: "unplugged" });
    return { label: link.label, ports: fakeTauri.ports, written: fakeTauri.written, seen };
  })`);
  assert.deepEqual(got, {
    label: "USB serial port",
    ports: [{ name: "/dev/cu.usbserial-A1", baud: 9600, parity: "even", dataBits: 7 }],
    written: ["port 1: 1,35,32"],
    seen: ["6,1,2", "lost: unplugged"],
  });
  // The connect dialog offers the port used last directly.
  await c.ev(`document.querySelector('.pane[data-pane=left] [data-action="add-serial"]').click(); true`);
  await p.until(`document.getElementById("connect-dialog").open && !document.getElementById("connect-known").hidden`, 5_000, "the last port offered");
  assert.equal(await c.ev(`document.getElementById("connect-go").textContent`), "Connect");
  assert.match(await c.ev(`document.getElementById("connect-known").textContent`), /FT232R USB UART \(FTDI\)/);
  assert.equal(await c.ev(`document.getElementById("connect-other").hidden`), false);
  await c.ev(`document.getElementById("connect-dialog").close(); true`);
  assert.deepEqual(c.errors, []);
});
