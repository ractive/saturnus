// A fake calculator for the transfer page's tests (web/test/
// tx-page.test.mjs), loaded into the page: it stands in for the protocol
// Worker (`app.deviceOptions.worker`) and answers the transfer bindings'
// requests from a tree held in memory, with saturnus-tx's rules where the
// page relies on them: a name in use is `nameClash` (`namesInUse` for a
// tree) until `replace`, a directory that holds variables is `notEmpty`
// with its count until a purge names it, a restore restarts the
// calculator (no connection, `restarted` until a connect), a fast
// transfer (XModem) ends the server (its plan as the `xmodem` event, no
// connection after it, `leftServer` until a connect). Not the protocol:
// the recorded traces test that (tx-driver, tx-bindings).
//
// `fakeCalc({model, delay, holdXmodem})` gives `{worker, requests, root,
// release}`: `worker` for `deviceOptions`, every request seen, the tree
// (`Map` name -> `{kind, data}` or `{kind: "Directory", dir: Map}`);
// with `holdXmodem` a fast transfer waits after its plan until
// `release()` (the calculator's user typing the command).

const enc = new TextEncoder();
const dec = new TextDecoder();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** A checksum the list can show (not the calculator's CRC). */
const sum = (bytes) => bytes.reduce((s, b) => (s * 31 + b) & 0xffff, 7);

export function fakeCalc({ model = "48gx", delay = () => 10, free = 100_000, holdXmodem = false } = {}) {
  const header = model === "49g" ? "HPHP49-C" : "HPHP48-E";
  const root = new Map();
  const requests = [];
  const conn = { model, settings: { baud: 9600, parity: "none", dataBits: 8 }, version: null, userMode: "binary" };
  let connected = false;
  let restarted = false;
  let leftServer = false;
  /** Posts a Worker event (set by the worker for the running request). */
  let emit = () => {};
  let held = null;
  const release = () => held?.();
  const dirAt = (path) => {
    let d = root;
    for (const p of path.slice(1)) {
      const e = d.get(p);
      if (!e?.dir) throw fail("notFound", `${p} is not there`, p);
      d = e.dir;
    }
    return d;
  };
  const fail = (kind, message, name = null, extra = {}) => Object.assign(new Error(message), { tx: { kind, message, name, ...extra } });
  const binary = (kind, body) => {
    const b = enc.encode(`${header}${kind}:${body}`);
    return b;
  };
  /** What a put stores: the kind and the binary form. */
  const stored = (data, asString) => {
    const text = dec.decode(data);
    if (text.startsWith("HPHP4")) {
      const kind = text.slice(8).split(":")[0] || "Program";
      return { kind, data };
    }
    if (text.startsWith("%%HP:")) {
      const body = text.split(/\r?\n/).slice(1).join("\n").trim();
      const kind = body.startsWith("\\<<") || body.startsWith("«") ? "Program" : body.startsWith("GROB") ? "Graphic" : "Real Number";
      return { kind, data: binary(kind, body) };
    }
    if (!asString) throw fail("notAnObject", "the file is not a calculator object");
    return { kind: "String", data: binary("String", text) };
  };
  const entry = (name, e) => (e.dir
    ? { name, kind: "Directory", size: 5 + [...e.dir.values()].reduce((n, x) => n + (x.data?.length ?? 5), 0), checksum: 0x1111, directory: true }
    : { name, kind: e.kind, size: e.data.length, checksum: sum(e.data), directory: false });
  const count = (d) => [...d.values()].reduce((n, e) => n + 1 + (e.dir ? count(e.dir) : 0), 0);
  const fsName = (name, dir) => (dir ? name : `${name}.hp`);
  const varName = (fs) => fs.replace(/\.[^.]*$/, "");
  const treeItems = (d, below = []) => {
    const out = [];
    for (const [name, e] of d) {
      const path = [...below, name];
      if (e.dir) {
        out.push({ path, fsPath: path.map((p, i) => (i === path.length - 1 ? p : p)), directory: true });
        out.push(...treeItems(e.dir, path));
      } else out.push({ path, fsPath: [...below, fsName(name, false)], directory: false, data: e.data });
    }
    return out;
  };
  const serialize = (d) => [...d].map(([n, e]) => (e.dir ? [n, null, serialize(e.dir)] : [n, e.kind, [...e.data]]));
  const deserialize = (list) => new Map(list.map(([n, kind, x]) => (kind === null ? [n, { kind: "Directory", dir: deserialize(x) }] : [n, { kind, data: new Uint8Array(x) }])));
  const homeSize = () => [...root.values()].reduce((n, e) => n + (e.data?.length ?? 40), 0);

  async function answer(req, data) {
    requests.push(req);
    await sleep(delay(req));
    if (req.type === "connect") {
      connected = true;
      restarted = false;
      leftServer = false;
      return { type: "connected", connection: conn };
    }
    if (restarted) throw fail("restarted", "the calculator restarted; start SERVER on it and reconnect");
    if (leftServer) throw fail("leftServer", "the calculator left server mode for the fast transfer; start SERVER on it and reconnect");
    if (!connected) throw fail("notConnected", "not connected");
    switch (req.type) {
      case "list":
        return { type: "listing", path: req.dir, free, entries: [...dirAt(req.dir)].map(([n, e]) => entry(n, e)) };
      case "get": {
        const e = dirAt(req.dir).get(req.name);
        if (!e) throw fail("notFound", `${req.name} is not there`, req.name);
        const text = req.format === "text";
        const bytes = e.dir ? binary("Directory", JSON.stringify(serialize(e.dir))) : text ? enc.encode(`%%HP: T(3)A(R)F(.);\r\n${dec.decode(e.data).replace(/^HPHP4.-.[^:]*:/, "")}\r\n`) : e.data;
        return { type: "file", name: req.name, fileName: `${req.name}.${text ? "txt" : "hp"}`, format: req.format, data: bytes };
      }
      case "put": {
        const d = dirAt(req.dir);
        const old = d.get(req.name);
        if (old?.dir) throw fail("isDirectory", `${req.name} is a directory`, req.name);
        if (old && !req.replace) throw fail("nameClash", `${req.name} is already there`, req.name);
        d.set(req.name, stored(data, req.asString));
        return { type: "stored", name: req.name };
      }
      case "mkdir": {
        const d = dirAt(req.dir);
        if (d.has(req.name)) throw fail("nameClash", `${req.name} is already there`, req.name);
        d.set(req.name, { kind: "Directory", dir: new Map() });
        return { type: "created", name: req.name };
      }
      case "rename": {
        const d = dirAt(req.dir);
        if (!d.has(req.from)) throw fail("notFound", `${req.from} is not there`, req.from);
        if (d.has(req.to)) throw fail("nameClash", `${req.to} is already there`, req.to);
        const e = d.get(req.from);
        d.delete(req.from);
        d.set(req.to, e);
        return { type: "renamed", from: req.from, to: req.to };
      }
      case "purge": {
        const d = dirAt(req.dir);
        const e = d.get(req.name);
        if (!e) throw fail("notFound", `${req.name} is not there`, req.name);
        const n = e.dir ? count(e.dir) : 0;
        if (n > 0 && req.count !== n) throw fail("notEmpty", `${req.name} holds ${n} variables`, req.name, { count: n });
        d.delete(req.name);
        return { type: "purged", name: req.name, count: n };
      }
      case "get-tree":
        return { type: "tree", dir: req.dir, items: treeItems(dirAt(req.dir)), skipped: [] };
      case "put-tree": {
        const base = dirAt(req.dir);
        if (req.name && base.has(req.name)) throw fail("nameClash", `${req.name} is already there`, req.name);
        const top = req.name ? new Map() : base;
        const inUse = [];
        const walk = (path) => {
          let d = top;
          for (const p of path) d = d?.get(p)?.dir;
          return d;
        };
        for (const item of data) {
          const names = item.path.map((p, i) => (i === item.path.length - 1 && item.data ? varName(p) : p));
          const parent = walk(names.slice(0, -1));
          const there = parent?.get(names.at(-1));
          if (there && item.data && !there.dir) inUse.push(names.join("/"));
        }
        if (inUse.length && !req.replace) throw fail("namesInUse", `${inUse.length} names in use`, null, { names: inUse });
        let variables = 0;
        let directories = req.name ? 1 : 0;
        if (req.name) base.set(req.name, { kind: "Directory", dir: top });
        for (const item of data) {
          const names = item.path.map((p, i) => (i === item.path.length - 1 && item.data ? varName(p) : p));
          const parent = walk(names.slice(0, -1));
          if (!item.data) {
            if (!parent.has(names.at(-1))) {
              parent.set(names.at(-1), { kind: "Directory", dir: new Map() });
              directories++;
            }
          } else {
            parent.set(names.at(-1), stored(item.data, req.asString));
            variables++;
          }
        }
        return { type: "treeStored", directories, variables };
      }
      case "memory": {
        const home = homeSize();
        return { type: "memory", free, home, needed: 2 * home + 1024, fits: free >= 2 * home + 1024 };
      }
      case "backup": {
        const home = homeSize();
        if (free < 2 * home + 1024) throw fail("noRoom", "no room", null, { needed: 2 * home + 1024, free });
        return { type: "backup", model, free, home, needed: 2 * home + 1024, fits: true, data: binary("Backup", JSON.stringify(serialize(root))) };
      }
      case "restore": {
        const text = dec.decode(data);
        const list = JSON.parse(text.slice(text.indexOf(":") + 1));
        root.clear();
        for (const [n, e] of deserialize(list)) root.set(n, e);
        connected = false;
        restarted = true;
        return { type: "restored", message: "The calculator restarted and left server mode. Start SERVER on it, then reconnect." };
      }
      case "xmodem-put":
      case "xmodem-get": {
        const put = req.type === "xmodem-put";
        if (model === "48sx") throw fail("notForXmodem", "the HP 48S/SX has no fast transfer; the normal copy works");
        const d = dirAt(req.dir);
        const there = d.get(req.name);
        if (put && !dec.decode(data).startsWith("HPHP4")) throw fail("notForXmodem", "only a binary calculator file (.hp) goes by fast transfer");
        if (put && there) throw fail("nameClash", `${req.name} already exists`, req.name);
        if (!put && !there) throw fail("notFound", `${req.name} does not exist`, req.name);
        if (!put && there.dir) throw fail("isDirectory", `${req.name} is a directory`, req.name);
        connected = false;
        leftServer = true;
        emit({ type: "xmodem", direction: put ? "put" : "get", name: req.name, keys: `'${req.name}' ${put ? "XRECV" : "XSEND"}`, model, switchedToRpn: false, exists: Boolean(there), notes: [] });
        if (holdXmodem) await new Promise((r) => { held = r; });
        if (put) {
          emit({ type: "progress", sent: data.length, total: data.length });
          d.set(req.name, stored(data, false));
          return { type: "stored", name: req.name };
        }
        emit({ type: "progress", done: there.data.length });
        return { type: "file", name: req.name, fileName: `${req.name}.hp`, format: "binary", data: there.data };
      }
      case "disconnect":
        connected = false;
        return { type: "disconnected" };
      default:
        throw fail("calculator", `no request ${req.type}`);
    }
  }

  const worker = () => {
    const w = {
      onmessage: null,
      terminated: false,
      postMessage(m) {
        setTimeout(async () => {
          if (w.terminated) return;
          if (m.cmd === "init") {
            w.onmessage({ data: { type: "ready" } });
            return;
          }
          if (m.cmd === "abort") {
            // As the driver: the connection, then the running request fails as stopped; the held answer is dropped.
            const id = w.running;
            if (id === null || id === undefined) return;
            w.running = null;
            w.onmessage({ data: { type: "connection", connection: connected ? conn : null } });
            w.onmessage({ data: { type: "result", id, ok: false, error: { kind: "aborted", message: "stopped" } } });
            return;
          }
          if (m.cmd !== "start") return;
          w.running = m.id;
          emit = (event) => { if (!w.terminated) w.onmessage({ data: { type: "event", event } }); };
          const out = [];
          try {
            const total = m.data instanceof Uint8Array ? m.data.length : 0;
            if (total) w.onmessage({ data: { type: "event", event: { type: "progress", sent: Math.floor(total / 2), total } } });
            const { data, ...outcome } = await answer(m.request, m.data);
            out.push({ type: "connection", connection: connected ? conn : null }, { type: "result", id: m.id, ok: true, outcome, data });
          } catch (err) {
            if (!err.tx) throw err;
            out.push({ type: "connection", connection: connected ? conn : null }, { type: "result", id: m.id, ok: false, error: err.tx });
          }
          // A request stopped meanwhile has had its answer.
          if (w.running !== m.id) return;
          w.running = null;
          for (const o of out) if (!w.terminated) w.onmessage({ data: o });
        }, 0);
      },
      terminate() { w.terminated = true; },
    };
    return w;
  };
  return { worker, requests, root, answer, release };
}

/** A link that takes everything (the fake calculator answers without it). */
export function stubLink(label = "stub link") {
  return Object.assign(new EventTarget(), {
    label,
    closed: false,
    open: async () => {},
    write: async () => {},
    async close() { this.closed = true; },
  });
}
