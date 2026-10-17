// The desktop app's views on its one native calculator (web/native-hub.js,
// iteration 51), over the real TauriBackend with a fake app: the memory
// watch and the visibility merged across Calculator and Files (watched
// while any view watches, hidden only when every view is; the app's
// `hello` ending the watch, asked for again for the other view), Files'
// `attach` (the running model shown, none booted from the library,
// another refused), the *bytes* as base64 over invoke (`fetchFile` with
// `inline`), `gone` when Calculator turns the calculator off, starts
// another model or starts it afresh, `memoryChanged` to the attached
// views only, `peers` to Calculator, the serial port refused while Files
// is on, and one session numbered in call order.

import assert from "node:assert/strict";
import { test } from "node:test";

const { NativeHub, bootedModel } = await import("../native-hub.js");
const { TauriBackend } = await import("../backend.js");
const { HubDevice } = await import("../shared/hub-device.js");

/** A fake app: every message the page sends (`sent`), answered by `answer(msg)`; `emit(event)` an event of the app. */
function fakeApp(answer = () => null) {
  const sent = [];
  const all = [];
  let listener = null;
  const tauri = {
    core: {
      invoke: async (cmd, { msg }) => {
        assert.equal(cmd, "command");
        sent.push(msg);
        all.push(msg);
        const r = await answer(msg);
        if (r instanceof Error) throw r.message;
        return r;
      },
    },
    event: { listen: async (_, f) => { listener = f; } },
  };
  const backend = new TauriBackend(tauri);
  const hub = new NativeHub(backend);
  return {
    sent,
    all,
    hub,
    backend,
    emit: (payload) => listener({ payload }),
    /** The messages sent, without `v`, `session` and `seq`. */
    log: () => sent.map(({ v, session, seq, ...m }) => m),
    clear: () => { sent.length = 0; },
  };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

test("the memory watch: watched while any view watches; the app's hello asks again for the other view", async () => {
  const app = fakeApp((m) => (m.cmd === "watchMemory" ? { supported: true } : null));
  app.emit({ type: "status", model: "48gx", romName: "gx.rom" });
  const files = app.hub.client("tx");
  assert.deepEqual(await app.backend.watchMemory(true), { supported: true });
  await files.request("attach", { model: "48gx", rom: new Uint8Array(4), romName: "gx.rom" });
  await files.request("watchMemory", { on: true });
  await app.backend.watchMemory(false);
  await settle();
  assert.deepEqual(app.log(), [
    { cmd: "watchMemory", on: true },
    { cmd: "watchMemory", on: true },
    // Calculator stopped watching; Files still does.
    { cmd: "watchMemory", on: true },
  ]);
  app.clear();
  // Calculator's view started again: its hello ends the app's watch; Files' is asked for again, after it.
  await app.backend.hello();
  await settle();
  assert.deepEqual(app.log(), [{ cmd: "hello" }, { cmd: "watchMemory", on: true }]);
  app.clear();
  // Files leaves: nobody watches.
  await files.close();
  await settle();
  assert.deepEqual(app.log(), [{ cmd: "watchMemory", on: false }]);
  // One session, numbered in call order.
  assert.equal(new Set(app.all.map((m) => m.session)).size, 1);
  assert.deepEqual(app.all.map((m) => m.seq), [...app.all.keys()]);
});

test("the visibility: hidden only when every view is; Files never says so", async () => {
  const app = fakeApp();
  app.emit({ type: "status", model: "48gx" });
  app.backend.visibility(true);
  await settle();
  const files = app.hub.client("tx");
  await files.request("attach", { model: "48gx" });
  await settle();
  app.backend.visibility(true);
  await settle();
  await files.close();
  await settle();
  assert.deepEqual(app.log(), [
    { cmd: "visibility", hidden: true },
    // Files came: shown.
    { cmd: "visibility", hidden: false },
    { cmd: "visibility", hidden: false },
    // Files went: hidden again.
    { cmd: "visibility", hidden: true },
  ]);
});

test("attach: the running model is shown, none boots it from the library, another is refused", async () => {
  const app = fakeApp((m) => (m.cmd === "bootModel" ? { slots: [], booted: { model: m.model, romName: "sx.rom" } } : null));
  const files = app.hub.client("tx");
  app.emit({ type: "status", model: null });
  const booted = await files.request("attach", { model: "48sx", rom: new Uint8Array(4), romName: "sx.rom" });
  assert.deepEqual(booted, { model: "48sx", romName: "sx.rom" });
  assert.deepEqual(app.log(), [{ cmd: "bootModel", model: "48sx" }], "no ROM bytes to the app: it boots its own");
  app.emit({ type: "status", model: "48sx", romName: "sx.rom" });
  app.clear();
  const again = app.hub.client("tx");
  assert.deepEqual(await again.request("attach", { model: "48sx" }), { model: "48sx", romName: "sx.rom", restored: true, attached: true });
  assert.deepEqual(app.log(), [], "nothing booted");
  const other = app.hub.client("tx");
  await assert.rejects(other.request("attach", { model: "49g" }), /Calculator runs the HP 48SX: .*Switch to the HP 49G there first/);
});

test("the bytes travel as base64: storeFile's data out, fetchFile's (inline) and backup's back", async () => {
  const app = fakeApp((m) => {
    if (m.cmd === "fetchFile") return { name: m.name, size: 3, data: "AUH/" };
    if (m.cmd === "backup") return { size: 2, data: "AAE=" };
    return { name: m.name };
  });
  app.emit({ type: "status", model: "48gx" });
  const files = app.hub.client("tx");
  await files.request("attach", { model: "48gx" });
  await files.request("storeFile", { dir: ["HOME"], name: "A", data: new Uint8Array([1, 0x41, 0xff]), replace: false });
  const got = await files.request("fetchFile", { dir: ["HOME"], name: "A" });
  const backup = await files.request("backup");
  assert.deepEqual(app.log(), [
    { cmd: "storeFile", dir: ["HOME"], name: "A", data: "AUH/", replace: false },
    { cmd: "fetchFile", dir: ["HOME"], name: "A", inline: true },
    { cmd: "backup" },
  ]);
  assert.deepEqual(got.data, new Uint8Array([1, 0x41, 0xff]));
  assert.deepEqual(backup.data, new Uint8Array([0, 1]));
});

test("gone: the calculator turned off, another model, Start fresh; memoryChanged to the attached views only; peers", async () => {
  const app = fakeApp((m) => (m.cmd === "bootModel" ? { booted: { model: m.model } } : null));
  const peers = [];
  app.backend.addEventListener("peers", (e) => peers.push(e.detail.tx));
  app.emit({ type: "status", model: "48gx" });
  const join = async () => {
    const c = app.hub.client("tx");
    const heard = [];
    c.addEventListener("gone", (e) => heard.push(`gone: ${e.detail}`));
    c.addEventListener("memoryChanged", () => heard.push("memoryChanged"));
    await c.request("attach", { model: "48gx" });
    return heard;
  };
  const idle = app.hub.client("tx");
  const idleHeard = [];
  idle.addEventListener("memoryChanged", () => idleHeard.push("memoryChanged"));

  let heard = await join();
  app.emit({ type: "memoryChanged" });
  app.emit({ type: "status", model: null });
  assert.deepEqual(heard, ["memoryChanged", "gone: The calculator was turned off in Calculator."]);
  assert.deepEqual(idleHeard, [], "a client not attached hears nothing");

  app.emit({ type: "status", model: "48gx" });
  heard = await join();
  app.emit({ type: "status", model: "49g" });
  assert.deepEqual(heard, ["gone: Calculator runs the HP 49G now."]);

  app.emit({ type: "status", model: "48gx" });
  heard = await join();
  await app.backend.startFresh("48gx");
  await settle();
  assert.deepEqual(heard, ["gone: Calculator started this calculator afresh."]);
  assert.deepEqual(peers, [true, false, true, false, true, false]);
  // A cancelled choice boots nothing: nobody is dropped.
  assert.equal(bootedModel("chooseRom", null), null);
  assert.equal(bootedModel("boot", { model: "48sx" }), "48sx");
});

test("the serial port is refused while Files is on the calculator", async () => {
  const app = fakeApp();
  app.emit({ type: "status", model: "48gx" });
  const files = app.hub.client("tx");
  await files.request("attach", { model: "48gx" });
  await assert.rejects(app.backend.serialOpen(), /shared with another view/);
  await files.close();
  await app.backend.serialOpen();
  assert.deepEqual(app.log().filter((m) => m.cmd.startsWith("serial")), [{ cmd: "serialOpen" }]);
});

test("HubDevice on the native calculator: connect, list, get and put", async () => {
  const tree = { path: ["HOME"], variables: [{ name: "A", type: "real", size: 10.5, checksum: "#1h" }] };
  const app = fakeApp((m) => {
    if (m.cmd === "hello") return { protocol: 1, host: "tauri" };
    if (m.cmd === "watchMemory") return { supported: true };
    if (m.cmd === "memoryTree") return tree;
    if (m.cmd === "fetchFile") return { name: m.name, size: 3, data: "SFBI" };
    if (m.cmd === "storeFile") return { name: m.name };
    return null;
  });
  app.emit({ type: "status", model: "48gx" });
  const helpers = async () => ({ file_name: (n) => `${n}.hp`, file_kind: () => "binary", name_problem: () => null });
  const device = new HubDevice(app.hub.client("tx"), { helpers });
  assert.deepEqual(await device.connect({ model: "48gx", rom: new Uint8Array(1), romName: "gx.rom" }), { model: "48gx", settings: null, hub: true });
  assert.deepEqual((await device.list()).entries.map((e) => e.name), ["A"]);
  assert.deepEqual((await device.read(["HOME"], "A")).data, new TextEncoder().encode("HPH"));
  assert.equal(await device.write(["HOME"], "B", new Uint8Array([1, 2])), "B");
  const lost = [];
  device.addEventListener("lost", (e) => lost.push(e.detail));
  app.emit({ type: "status", model: null });
  assert.deepEqual(lost, ["The calculator was turned off in Calculator."]);
  assert.deepEqual(app.log().map((m) => m.cmd), ["hello", "watchMemory", "memoryTree", "fetchFile", "storeFile"]);
});

test("a reload straight into Files: the running model is asked for, never booted over", async () => {
  // The app runs a 49G; the page has heard no status yet. Its hello makes the app say so.
  let model = "49g";
  const app = fakeApp((m) => {
    if (m.cmd === "hello") {
      if (model) setTimeout(() => app.emit({ type: "status", model, romName: `${model}.rom` }), 0);
      return { protocol: 1, host: "tauri", models: [] };
    }
    return null;
  });
  const files = app.hub.client("tx");
  await assert.rejects(files.request("attach", { model: "48gx" }), /Calculator runs the/);
  assert.ok(!app.log().some((m) => m.cmd === "bootModel"), "nothing booted over the 49G");
  assert.equal(await app.hub.knownRunning(), "49g");
  // An app that never says: refused, still nothing booted.
  model = null;
  const quiet = fakeApp((m) => (m.cmd === "hello" ? { protocol: 1, host: "tauri", models: [] } : null));
  await assert.rejects(quiet.hub.client("tx").request("attach", { model: "48gx" }), /has not said/);
  assert.ok(!quiet.log().some((m) => m.cmd === "bootModel"));
});
