// The transfer page's folder (web/tx/folder.js) over fakes: the same
// semantics on all three roots (a picked folder on a case-insensitive
// file system with permissions, OPFS, memory), the collisions, the
// portable names, rename by case, purge with its count, the kept handle
// and the permission asked again, the fallback's choice, downloads and
// the zip of a directory or a selection.

import assert from "node:assert/strict";
import { test } from "node:test";
import { FOLDER_KEY, FOLDER_PICKER_ID, FOLDER_STORE, TRANSFER_DB, TRANSFER_OPFS_DIR } from "../shared/storage-names.js";
import {
  FILES_TITLE, Folder, FolderError, canPickFolder, canWriteOpfs, collisions, download, downloadBytes, fallbackFolder, folderError, foldName,
  forgetFolder, handleStore, keptFolder, nameProblem, pickFolder,
} from "../tx/folder.js";
import { MemoryDirectory } from "../tx/memory-handle.js";
import { crc32 } from "../tx/zip.js";
import { readZip } from "./zip-reader.mjs";

const enc = new TextEncoder();
const dec = new TextDecoder();
const bytes = (s) => enc.encode(s);

/** A clock for modification times. */
function clock() {
  let t = Date.UTC(2026, 9, 10, 12, 0, 0);
  return () => (t += 1000);
}

/**
 * A picked folder's handle: memory, case-insensitive like macOS, with the
 * permission calls. `requestPermission` without a user gesture throws a
 * SecurityError, as Chrome's does; with one it answers `answer`.
 */
function fakePicked(name = "hp-files", { state = "granted", answer = "granted" } = {}) {
  const root = new MemoryDirectory(name, { caseInsensitive: true, now: clock() });
  const perm = { state, answer, gesture: false, asked: [] };
  root.perm = perm;
  root.queryPermission = async ({ mode }) => {
    assert.equal(mode, "readwrite");
    return perm.state;
  };
  root.requestPermission = async ({ mode }) => {
    perm.asked.push(mode);
    if (!perm.gesture) throw new DOMException("Must be handling a user gesture", "SecurityError");
    perm.state = perm.answer;
    return perm.state;
  };
  return root;
}

/** A fake `navigator.storage` whose OPFS root is memory (case-sensitive, as Chrome's OPFS is). */
function fakeStorage() {
  const root = new MemoryDirectory("", { now: clock() });
  return { root, getDirectory: async () => root };
}

/** A fake handle store (a Map), with a switch to make it refuse. */
function fakeStore() {
  const map = new Map();
  return {
    map,
    refuse: false,
    async get() { if (this.refuse) throw new Error("refused"); return map.get(FOLDER_KEY); },
    async set(h) { if (this.refuse) throw new Error("refused"); map.set(FOLDER_KEY, h); },
    async clear() { map.delete(FOLDER_KEY); },
  };
}

const BACKENDS = {
  picked: async () => new Folder(fakePicked(), { kind: "picked" }),
  opfs: async () => fallbackFolder({ storage: fakeStorage(), writable: true }),
  memory: async () => fallbackFolder({ storage: undefined, writable: false }),
};

/** `promise` rejects with a FolderError of `kind` (and the fields in `detail`). */
async function rejectsKind(promise, kind, detail = {}) {
  await assert.rejects(promise, (err) => {
    assert.ok(err instanceof FolderError, `a FolderError, not ${err}`);
    assert.equal(err.kind, kind, err.message);
    for (const [k, v] of Object.entries(detail)) assert.deepEqual(err[k], v, k);
    return true;
  });
}

for (const [backend, make] of Object.entries(BACKENDS)) {
  test(`${backend}: write, list, read, and nothing replaced without asking`, async () => {
    const f = await make();
    const home = f.home;
    assert.equal(home.length, 1);
    assert.equal(f.kind, backend);
    assert.equal(f.kept, backend !== "memory");
    assert.deepEqual((await f.list(home)).entries, []);
    const changes = [];
    f.addEventListener("change", (e) => changes.push(e.detail.path));

    assert.equal(await f.write(home, "A→B.hp", bytes("HPHP48-R one")), "A→B.hp");
    assert.equal(await f.write(home, "NOTES.txt", bytes("two")), "NOTES.txt");
    await f.mkdir(home, "D1");
    const listing = await f.list(home);
    assert.deepEqual(listing.path, home);
    assert.deepEqual(listing.entries.map((e) => [e.name, e.kind, e.directory, e.size]), [
      ["D1", "Folder", true, null], ["A→B.hp", "File", false, 12], ["NOTES.txt", "File", false, 3],
    ]);
    assert.ok(listing.entries.every((e) => e.checksum === null));
    assert.ok(Number.isFinite(listing.entries[1].modified));
    assert.deepEqual(listing.collisions, []);

    const r = await f.read(home, "A→B.hp");
    assert.equal(dec.decode(r.data), "HPHP48-R one");
    assert.equal(r.fileName, "A→B.hp");
    assert.equal(r.size, 12);

    // The same name: only with replace; a folder never.
    await rejectsKind(f.write(home, "NOTES.txt", bytes("three")), "exists", { name: "NOTES.txt", directory: false });
    assert.equal(dec.decode((await f.read(home, "NOTES.txt")).data), "two", "untouched");
    await f.write(home, "NOTES.txt", bytes("three"), { replace: true });
    assert.equal(dec.decode((await f.read(home, "NOTES.txt")).data), "three");
    await rejectsKind(f.write(home, "D1", bytes("x"), { replace: true }), "exists", { directory: true });
    await rejectsKind(f.mkdir(home, "D1"), "exists");
    await rejectsKind(f.mkdir(home, "NOTES.txt"), "exists", { directory: false });

    assert.deepEqual(await f.existing(home, "NOTES.txt"), { name: "NOTES.txt", directory: false, exact: true });
    assert.deepEqual(await f.existing(home, "d1"), { name: "D1", directory: true, exact: false });
    assert.equal(await f.existing(home, "OTHER"), null);

    // Into a directory below.
    await f.write([...home, "D1"], "X.hp", bytes("x"));
    assert.deepEqual((await f.list([...home, "D1"])).entries.map((e) => e.name), ["X.hp"]);
    await rejectsKind(f.list([...home, "NOPE"]), "missing");
    await rejectsKind(f.list(["elsewhere"]), "missing");
    await rejectsKind(f.read(home, "NOPE.hp"), "missing");
    await rejectsKind(f.read(home, "D1"), "type");

    assert.deepEqual(changes, [home, home, home, home, [...home, "D1"]]);
  });

  test(`${backend}: names differing only in case collide, whatever the file system`, async () => {
    const f = await make();
    const home = f.home;
    await f.write(home, "A.hp", bytes("upper"));
    await rejectsKind(f.write(home, "a.hp", bytes("lower")), "collision", { name: "a.hp", other: "A.hp" });
    await rejectsKind(f.write(home, "a.hp", bytes("lower"), { replace: true }), "collision", { other: "A.hp" });
    await rejectsKind(f.mkdir(home, "A.HP"), "collision");
    // NFC and NFD spellings are one name too (macOS compares them so).
    await f.write(home, "ÉTÉ.hp", bytes("nfc"));
    await rejectsKind(f.write(home, "ÉTÉ.hp", bytes("nfd")), "collision");
    assert.equal(dec.decode((await f.read(home, "A.hp")).data), "upper", "never replaced through another case");
    assert.deepEqual((await f.list(home)).entries.map((e) => e.name), ["A.hp", "ÉTÉ.hp"]);
  });

  test(`${backend}: only portable names are made; escaped file names pass`, async () => {
    const f = await make();
    const home = f.home;
    for (const name of ["", ".", "..", "a/b", "a\\b", "a:b", "a*b", "a?b", 'a"b', "a<b", "a>b", "a|b", "a\u0001b", "DOT.", "SP ", "AUX", "aux.hp", "COM1.txt", "lpt9"]) {
      await rejectsKind(f.write(home, name, bytes("x")), "bad-name", {});
      await rejectsKind(f.mkdir(home, name), "bad-name");
    }
    // What names.rs's escape makes of those (`file_name`) is fine.
    for (const name of ["%41UX.hp", "A%2FB.hp", "a%3Fb.txt", "DOT%2E.hp", "50%25.hp", "COM10.hp", "Σx².txt", ".hidden"]) {
      assert.equal(await f.write(home, name, bytes(name)), name);
      assert.equal(dec.decode((await f.read(home, name)).data), name);
    }
  });

  test(`${backend}: rename a file, a directory with its contents, and by case alone`, async () => {
    const f = await make();
    const home = f.home;
    await f.write(home, "a.hp", bytes("lower a"));
    await f.write(home, "B.hp", bytes("b"));
    await f.mkdir(home, "D1");
    await f.mkdir([...home, "D1"], "D2");
    await f.write([...home, "D1"], "X.hp", bytes("x"));
    await f.write([...home, "D1", "D2"], "Y.hp", bytes("y"));

    assert.equal(await f.rename(home, "B.hp", "C.hp"), "C.hp");
    assert.equal(dec.decode((await f.read(home, "C.hp")).data), "b");
    await rejectsKind(f.read(home, "B.hp"), "missing");

    await f.rename(home, "D1", "DIR");
    assert.deepEqual((await f.list([...home, "DIR"])).entries.map((e) => e.name), ["D2", "X.hp"]);
    assert.equal(dec.decode((await f.read([...home, "DIR", "D2"], "Y.hp")).data), "y");
    await rejectsKind(f.list([...home, "D1"]), "missing");

    // By case alone: through a temporary name, the content kept, nothing left behind.
    assert.equal(await f.rename(home, "a.hp", "A.hp"), "A.hp");
    assert.deepEqual((await f.list(home)).entries.map((e) => e.name), ["DIR", "A.hp", "C.hp"]);
    assert.equal(dec.decode((await f.read(home, "A.hp")).data), "lower a");
    await f.rename(home, "DIR", "dir");
    assert.deepEqual((await f.list(home)).entries.map((e) => e.name), ["dir", "A.hp", "C.hp"]);
    assert.equal(dec.decode((await f.read([...home, "dir", "D2"], "Y.hp")).data), "y");

    // Never over another entry.
    await rejectsKind(f.rename(home, "C.hp", "A.hp"), "exists", { name: "A.hp" });
    await rejectsKind(f.rename(home, "C.hp", "a.HP"), "collision", { other: "A.hp" });
    await rejectsKind(f.rename(home, "C.hp", "dir"), "exists", { directory: true });
    await rejectsKind(f.rename(home, "C.hp", "C:hp"), "bad-name");
    await rejectsKind(f.rename(home, "NOPE", "X"), "missing");
    await rejectsKind(f.rename(home, "c.hp", "X"), "missing", { name: "c.hp" });
    assert.equal(await f.rename(home, "C.hp", "C.hp"), "C.hp", "the same name: nothing to do");
    assert.equal(dec.decode((await f.read(home, "C.hp")).data), "b");
  });

  test(`${backend}: purge asks for recursive with the count, and says how many went`, async () => {
    const f = await make();
    const home = f.home;
    await f.write(home, "F.hp", bytes("f"));
    await f.mkdir(home, "EMPTY");
    await f.mkdir(home, "D1");
    await f.mkdir([...home, "D1"], "D2");
    await f.write([...home, "D1"], "X.hp", bytes("x"));
    await f.write([...home, "D1", "D2"], "Y.hp", bytes("y"));
    await f.write([...home, "D1", "D2"], "Z.hp", bytes("z"));

    assert.equal(await f.count(home, "D1"), 4, "D2, X.hp, Y.hp, Z.hp");
    assert.equal(await f.count(home, "F.hp"), 0);
    assert.equal(await f.count(home, "EMPTY"), 0);
    await rejectsKind(f.count(home, "NOPE"), "missing");

    await rejectsKind(f.purge(home, "D1"), "not-empty", { name: "D1", count: 4 });
    assert.equal(await f.count(home, "D1"), 4, "nothing removed");
    assert.equal(await f.purge(home, "D1", { recursive: true }), 4);
    assert.equal(await f.purge(home, "EMPTY"), 0, "an empty directory needs no question");
    assert.equal(await f.purge(home, "F.hp"), 0);
    assert.deepEqual((await f.list(home)).entries, []);
    await rejectsKind(f.purge(home, "F.hp"), "missing");
  });

  test(`${backend}: the zip of a directory, of a selection, of everything`, async () => {
    const f = await make();
    const home = f.home;
    await f.write(home, "A.hp", bytes("a"));
    await f.mkdir(home, "D1");
    await f.mkdir([...home, "D1"], "EMPTY");
    await f.write([...home, "D1"], "X.hp", bytes("xx"));

    const one = await f.zip(home, ["D1"]);
    assert.equal(one.name, "D1.zip");
    const entries = readZip(one.data);
    assert.deepEqual(entries.map((e) => e.name).sort(), ["D1/", "D1/EMPTY/", "D1/X.hp"]);
    const x = entries.find((e) => e.name === "D1/X.hp");
    assert.equal(dec.decode(x.data), "xx");
    assert.equal(x.crc, crc32(bytes("xx")));
    assert.deepEqual(one.collisions, []);

    const selection = await f.zip(home, ["A.hp", "D1"]);
    assert.equal(selection.name, `${home[0]}.zip`);
    assert.deepEqual(readZip(selection.data).map((e) => e.name).sort(), ["A.hp", "D1/", "D1/EMPTY/", "D1/X.hp"]);

    const all = await f.zip(home);
    assert.deepEqual(readZip(all.data).map((e) => e.name).sort(), ["A.hp", "D1/", "D1/EMPTY/", "D1/X.hp"]);
    const below = await f.zip([...home, "D1"]);
    assert.equal(below.name, "D1.zip");
    assert.deepEqual(readZip(below.data).map((e) => e.name).sort(), ["EMPTY/", "X.hp"]);
    await rejectsKind(f.zip(home, ["NOPE"]), "missing");
  });
}

test("Chrome's swap files are never shown, counted or zipped", async () => {
  const root = fakePicked();
  const f = new Folder(root, { kind: "picked" });
  await f.write(f.home, "A.hp", bytes("a"));
  (await root.getFileHandle("A.hp.crswap", { create: true })).data = bytes("partial");
  assert.deepEqual((await f.list(f.home)).entries.map((e) => e.name), ["A.hp"]);
  assert.equal(await f.existing(f.home, "A.hp.crswap"), null);
  assert.deepEqual(readZip((await f.zip(f.home)).data).map((e) => e.name), ["A.hp"]);
});

test("a folder that already holds names differing in case (a Linux folder) lists them as collisions", async () => {
  const root = new MemoryDirectory("linux");
  for (const n of ["A.hp", "a.hp", "B.hp"]) (await root.getFileHandle(n, { create: true })).data = bytes(n);
  const f = new Folder(root, { kind: "picked" });
  const listing = await f.list(f.home);
  assert.deepEqual(listing.collisions, [["A.hp", "a.hp"]]);
  // Both read as themselves; neither can be replaced through the other.
  assert.equal(dec.decode((await f.read(f.home, "a.hp")).data), "a.hp");
  await f.write(f.home, "A.hp", bytes("new"), { replace: true });
  assert.equal(dec.decode((await f.read(f.home, "a.hp")).data), "a.hp");
  const z = await f.zip(f.home);
  assert.deepEqual(z.collisions, [["A.hp", "a.hp"]], "the zip says what an unzip on macOS or Windows would merge");
});

test("the pure helpers: collisions, folding, portable names, errors", () => {
  assert.deepEqual(collisions(["A.hp", "B.hp", "a.hp", "A.HP", "b.txt", "A.hp"]), [["A.hp", "a.hp", "A.HP"]]);
  assert.deepEqual(collisions([]), []);
  assert.equal(foldName("ÄBC"), foldName("äbc"));
  assert.equal(nameProblem("A.hp"), null);
  assert.equal(nameProblem("a:b"), "The name cannot hold :.");
  assert.equal(nameProblem("a\u0007"), "The name holds a control character.");
  assert.equal(nameProblem("PRN.txt"), "PRN is a name Windows keeps for a device.");
  assert.equal(nameProblem("X."), "The name cannot end with a dot or a space.");
  assert.equal(nameProblem("é".repeat(128)), "The name is too long.");
  assert.equal(nameProblem("é".repeat(127)), null);
  const kinds = {
    NotFoundError: "missing", NotAllowedError: "permission", SecurityError: "permission", InvalidModificationError: "not-empty",
    TypeMismatchError: "type", QuotaExceededError: "full", NoModificationAllowedError: "busy",
  };
  for (const [name, kind] of Object.entries(kinds)) {
    const err = folderError(new DOMException("x", name), "A.hp");
    assert.equal(err.kind, kind);
    assert.ok(err.cause instanceof DOMException);
  }
  assert.equal(folderError(new TypeError("Name is not allowed.")).kind, "bad-name");
  assert.equal(folderError(new Error("disk on fire"), "A.hp").message, "A.hp: disk on fire");
  const same = new FolderError("exists", "x");
  assert.equal(folderError(same), same);
});

// ---- The picked folder: kept, and asked again per visit ----

test("pick a folder: read and write asked for, the handle kept, Cancel is no error", async () => {
  const root = fakePicked();
  const calls = [];
  const win = { showDirectoryPicker: async (options) => { calls.push(options); return root; } };
  const store = fakeStore();
  assert.ok(canPickFolder(win));
  assert.ok(!canPickFolder({}));
  const f = await pickFolder({ win, store });
  assert.deepEqual(calls, [{ id: FOLDER_PICKER_ID, mode: "readwrite" }]);
  assert.equal(f.kind, "picked");
  assert.equal(f.title, "hp-files");
  assert.deepEqual(f.home, ["hp-files"]);
  assert.equal(f.remembered, true);
  assert.equal(store.map.get(FOLDER_KEY), root);

  const cancel = { showDirectoryPicker: async () => { throw new DOMException("The user aborted a request.", "AbortError"); } };
  assert.equal(await pickFolder({ win: cancel, store }), null);
  const blocked = { showDirectoryPicker: async () => { throw new DOMException("system folder", "SecurityError"); } };
  await rejectsKind(pickFolder({ win: blocked, store }), "permission");
  await rejectsKind(pickFolder({ win: {}, store }), "unsupported");

  // A store that refuses: the folder works for this visit, not remembered.
  store.refuse = true;
  const once = await pickFolder({ win, store });
  assert.equal(once.remembered, false);
  assert.deepEqual((await once.list(once.home)).entries, []);
});

test("the next visit: the kept folder, the permission asked again on a click", async () => {
  const store = fakeStore();
  assert.equal(await keptFolder({ store }), null, "nothing kept yet");

  const root = fakePicked("hp-files", { state: "prompt" });
  await store.set(root);
  const f = await keptFolder({ store });
  assert.equal(f.title, "hp-files");
  assert.equal(f.remembered, true);
  assert.equal(await f.permission(), "prompt");
  // Nothing is touched before the permission.
  await rejectsKind(f.list(f.home), "permission");
  await rejectsKind(f.write(f.home, "A.hp", bytes("a")), "permission");
  assert.equal(root.children.size, 0);

  // Asked without a click: the browser refuses.
  await rejectsKind(f.requestAccess(), "permission");
  // On a click, the user allows it.
  root.perm.gesture = true;
  assert.equal(await f.requestAccess(), "granted");
  assert.deepEqual(root.perm.asked, ["readwrite", "readwrite"]);
  await f.write(f.home, "A.hp", bytes("a"));
  assert.deepEqual((await f.list(f.home)).entries.map((e) => e.name), ["A.hp"]);

  // The user says no: denied, and the folder stays shut.
  const denied = fakePicked("other", { state: "prompt", answer: "denied" });
  denied.perm.gesture = true;
  const g = new Folder(denied, { kind: "picked" });
  assert.equal(await g.requestAccess(), "denied");
  await rejectsKind(g.read(g.home, "A.hp"), "permission");

  // Forgotten: no folder next time; a store that fails reads as none.
  await forgetFolder({ store });
  assert.equal(await keptFolder({ store }), null);
  store.map.set(FOLDER_KEY, { kind: "file", name: "x" });
  assert.equal(await keptFolder({ store }), null, "not a directory handle");
  store.refuse = true;
  assert.equal(await keptFolder({ store }), null);
});

/** A small fake of IndexedDB: open with upgrade, one store, get, put, delete, structured values kept as they are. */
function fakeIndexedDB() {
  const dbs = new Map();
  const log = [];
  return {
    log,
    dbs,
    open(name, version) {
      log.push(["open", name, version]);
      const req = {};
      setTimeout(() => {
        let db = dbs.get(name);
        if (!db) dbs.set(name, (db = { version: 0, stores: new Map() }));
        const conn = {
          closed: false,
          close() { this.closed = true; },
          createObjectStore(s) { log.push(["createObjectStore", s]); db.stores.set(s, new Map()); },
          transaction(store, mode) {
            const map = db.stores.get(store);
            if (!map) throw new DOMException(`no store ${store}`, "NotFoundError");
            log.push(["transaction", store, mode]);
            const tx = {
              objectStore: (s) => {
                assert.equal(s, store);
                return {
                  get: (k) => ({ result: map.get(k) }),
                  put: (v, k) => { assert.equal(mode, "readwrite"); map.set(k, v); return { result: k }; },
                  delete: (k) => { assert.equal(mode, "readwrite"); map.delete(k); return { result: undefined }; },
                };
              },
            };
            setTimeout(() => tx.oncomplete?.(), 0);
            return tx;
          },
        };
        req.result = conn;
        if (db.version < version) {
          db.version = version;
          req.onupgradeneeded?.();
        }
        req.onsuccess?.();
      }, 0);
      return req;
    },
  };
}

test("the handle store: the transfer page's own database and store, read, kept, cleared", async () => {
  const idb = fakeIndexedDB();
  const store = handleStore(idb);
  assert.equal(await store.get(), undefined);
  const root = fakePicked();
  await store.set(root);
  assert.equal(await store.get(), root);
  await store.clear();
  assert.equal(await store.get(), undefined);
  assert.deepEqual([...idb.dbs.keys()], [TRANSFER_DB]);
  assert.deepEqual([...idb.dbs.get(TRANSFER_DB).stores.keys()], [FOLDER_STORE]);
  assert.deepEqual(idb.log.filter((l) => l[0] === "createObjectStore"), [["createObjectStore", FOLDER_STORE]], "made once");
  assert.ok(idb.log.filter((l) => l[0] === "open").every((l) => l[1] === TRANSFER_DB && l[2] === 1));

  // Picked and found again through the real store.
  const win = { showDirectoryPicker: async () => root };
  await pickFolder({ win, store });
  const again = await keptFolder({ store });
  assert.equal(again.root, root);
});

// ---- The fallback: OPFS where it writes, memory otherwise ----

test("the fallback: OPFS in its own directory where it can write, memory otherwise", async () => {
  const storage = fakeStorage();
  const f = await fallbackFolder({ storage, writable: true });
  assert.equal(f.kind, "opfs");
  assert.equal(f.title, FILES_TITLE);
  assert.deepEqual(f.home, ["Files"]);
  await f.write(f.home, "A.hp", bytes("a"));
  assert.deepEqual([...storage.root.children.keys()], [TRANSFER_OPFS_DIR], "one directory of the page's own in the origin's OPFS");
  // Kept: a second visit finds the file.
  const again = await fallbackFolder({ storage, writable: true });
  assert.deepEqual((await again.list(again.home)).entries.map((e) => e.name), ["A.hp"]);

  // OPFS that cannot write from the page (an older Safari), none at all, or one that refuses (a private window): memory.
  for (const [storage2, writable] of [[fakeStorage(), false], [undefined, true], [{ getDirectory: async () => { throw new DOMException("no", "SecurityError"); } }, true]]) {
    const m = await fallbackFolder({ storage: storage2, writable });
    assert.equal(m.kind, "memory");
    assert.equal(m.kept, false);
    assert.deepEqual(m.home, ["Files"]);
  }
  assert.equal(canWriteOpfs({ FileSystemFileHandle: { prototype: { createWritable() {} } } }), true);
  assert.equal(canWriteOpfs({}), false);
});

// ---- Downloads ----

/** A fake document and URL that record the downloads. */
function fakeDownloads() {
  const saved = [];
  const blobs = new Map();
  let n = 0;
  const doc = {
    body: { append: (a) => { a.parent = true; } },
    createElement: (tag) => {
      assert.equal(tag, "a");
      return { click() { saved.push({ name: this.download, blob: blobs.get(this.href), hidden: this.hidden, attached: this.parent }); }, remove() { this.parent = false; } };
    },
  };
  const revoked = [];
  const urls = { createObjectURL: (blob) => { const u = `blob:${++n}`; blobs.set(u, blob); return u; }, revokeObjectURL: (u) => revoked.push(u) };
  const timers = [];
  const later = (fn, ms) => timers.push([fn, ms]);
  return { saved, revoked, timers, deps: { doc, urls, later } };
}

test("downloads: one file as itself, a directory or a selection as a .zip", async () => {
  const f = await fallbackFolder({ writable: false });
  await f.write(f.home, "A→B.hp", bytes("HPHP48-R a"));
  await f.mkdir(f.home, "D1");
  await f.write([...f.home, "D1"], "X.hp", bytes("x"));
  const d = fakeDownloads();

  assert.deepEqual(await download(f, f.home, ["A→B.hp"], d.deps), { name: "A→B.hp", size: 10, collisions: [] });
  const dir = await download(f, f.home, ["D1"], d.deps);
  assert.equal(dir.name, "D1.zip");
  assert.deepEqual(dir.collisions, []);
  const sel = await download(f, f.home, ["A→B.hp", "D1"], d.deps);
  assert.equal(sel.name, "Files.zip");

  assert.deepEqual(d.saved.map((s) => [s.name, s.blob.type, s.hidden, s.attached]), [
    ["A→B.hp", "application/octet-stream", true, true],
    ["D1.zip", "application/zip", true, true],
    ["Files.zip", "application/zip", true, true],
  ]);
  assert.equal(dec.decode(new Uint8Array(await d.saved[0].blob.arrayBuffer())), "HPHP48-R a");
  assert.equal(dir.size, d.saved[1].blob.size);
  const zipped = readZip(new Uint8Array(await d.saved[2].blob.arrayBuffer()));
  assert.deepEqual(zipped.map((e) => e.name).sort(), ["A→B.hp", "D1/", "D1/X.hp"]);
  // Each object URL is let go a minute later.
  assert.ok(d.timers.every(([, ms]) => ms === 60_000));
  for (const [fn] of d.timers) fn();
  assert.deepEqual(d.revoked, ["blob:1", "blob:2", "blob:3"]);

  await rejectsKind(download(f, f.home, ["NOPE"], d.deps), "missing");
});

test("downloadBytes: the anchor clicked once and removed", () => {
  const d = fakeDownloads();
  downloadBytes("X.hp", new Uint8Array([1, 2]), d.deps);
  assert.equal(d.saved.length, 1);
  assert.equal(d.saved[0].name, "X.hp");
});
