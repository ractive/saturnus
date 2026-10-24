// The Claude Desktop extension's forwarder (web/mcpb/server/index.mjs,
// iteration 59) as Claude Desktop runs it: unpacked from the built bundle,
// a child process on stdin and stdout, against a fake saturnus endpoint on 127.0.0.1. The token from a
// temporary file (SATURNUS_TOKEN_FILE), the proof checked before it is
// sent, the headers, notifications, event streams, the answers while the
// app is closed, the start of the app (recorded, not run:
// SATURNUS_MCPB_LAUNCH_FILE), AI access off, and the token never on stdout
// or stderr.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { DEFAULT_PORT, NOT_READY, START, portOf, proofOf, readToken, sseMessages, tokenPath } from "../mcpb/server/index.mjs";
import { buildBundle, unzip } from "../mcpb/build.mjs";

/** The bundle, built and unpacked into a temporary folder, as Claude Desktop installs it. */
const BUNDLE = mkdtempSync(join(tmpdir(), "saturnus-mcpb-bundle-"));
for (const [name, data] of unzip(await buildBundle())) {
  mkdirSync(dirname(join(BUNDLE, name)), { recursive: true });
  writeFileSync(join(BUNDLE, name), data);
}
process.on("exit", () => rmSync(BUNDLE, { recursive: true, force: true }));
const SCRIPT = join(BUNDLE, "server", "index.mjs");
const OFFLINE = JSON.parse(readFileSync(join(BUNDLE, "server", "offline.json"), "utf8"));
const TOKEN = "0123456789abcdef".repeat(4);

/** A temporary folder with a token file (mode 0600). */
function tempToken(token = TOKEN) {
  const dir = mkdtempSync(join(tmpdir(), "saturnus-mcpb-"));
  const file = join(dir, "control-token");
  writeFileSync(file, `${token}\n`, { mode: 0o600 });
  return { dir, file, cleanup: () => rmSync(dir, { recursive: true, force: true }) };
}

/** A free port on 127.0.0.1 (listened on and closed again). */
async function freePort() {
  const s = createServer();
  await new Promise((r) => s.listen(0, "127.0.0.1", r));
  const { port } = s.address();
  await new Promise((r) => s.close(r));
  return port;
}

/**
 * A fake endpoint on `port`: `/hello` proves `token` (or `proof` overrides),
 * `/mcp` records each request and answers with `answer(msg, req)`:
 * `{status, type, body, headers}`.
 */
async function fakeEndpoint(port, { token = TOKEN, proof = null, answer } = {}) {
  const requests = [];
  const hellos = [];
  const server = createServer((req, res) => {
    const url = new URL(req.url, "http://x");
    if (url.pathname === "/hello") {
      hellos.push({ headers: req.headers });
      const nonce = url.searchParams.get("nonce");
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ proof: proof ?? proofOf(token, nonce, port) }));
      return;
    }
    const chunks = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => {
      const msg = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      requests.push({ msg, headers: req.headers });
      const a = answer?.(msg, req) ?? defaultAnswer(msg);
      res.writeHead(a.status ?? 200, { "Content-Type": a.type ?? "application/json", ...(a.headers ?? {}) });
      res.end(a.body === undefined ? "" : typeof a.body === "string" ? a.body : JSON.stringify(a.body));
    });
  });
  await new Promise((r) => server.listen(port, "127.0.0.1", r));
  return { requests, hellos, close: () => new Promise((r) => { server.closeAllConnections(); server.close(r); }) };
}

function defaultAnswer(msg) {
  if (!("id" in msg)) return { status: 202 };
  if (msg.method === "initialize") return { body: { jsonrpc: "2.0", id: msg.id, result: { protocolVersion: "2025-06-18", capabilities: { tools: {} }, serverInfo: { name: "saturnus", version: "live" }, instructions: "live" } }, headers: { "Mcp-Session-Id": "s".repeat(64) } };
  if (msg.method === "tools/list") return { body: { jsonrpc: "2.0", id: msg.id, result: { tools: [{ name: "live_tool", inputSchema: { type: "object" } }] } } };
  return { body: { jsonrpc: "2.0", id: msg.id, result: { content: [{ type: "text", text: `ran ${msg.params?.name}` }] } } };
}

/** The forwarder as a child process: `send(msg)`, `next(id)`, `out` (stdout lines), `err` (stderr). */
function forwarder(env) {
  const child = spawn(process.execPath, [SCRIPT], {
    env: { PATH: process.env.PATH, HOME: process.env.HOME, SATURNUS_MCPB_WAIT_MS: "1500", ...env },
    stdio: ["pipe", "pipe", "pipe"],
  });
  const out = [];
  let err = "";
  const waiters = [];
  createInterface({ input: child.stdout }).on("line", (line) => {
    out.push(line);
    const msg = JSON.parse(line);
    for (const w of [...waiters]) {
      if (w.id === msg.id) {
        waiters.splice(waiters.indexOf(w), 1);
        w.resolve(msg);
      }
    }
  });
  child.stderr.on("data", (c) => { err += c; });
  return {
    child,
    out,
    get err() { return err; },
    send(msg) { child.stdin.write(`${typeof msg === "string" ? msg : JSON.stringify(msg)}\n`); },
    next(id, ms = 10_000) {
      const seen = out.map((l) => JSON.parse(l)).find((m) => m.id === id);
      if (seen) return Promise.resolve(seen);
      return new Promise((resolve, reject) => {
        const w = { id, resolve };
        waiters.push(w);
        setTimeout(() => reject(new Error(`no answer for id ${id}; stderr: ${err}`)), ms).unref();
      });
    },
    async stop() {
      child.stdin.end();
      await new Promise((r) => (child.exitCode !== null ? r() : child.on("exit", r)));
    },
  };
}

const INIT = { jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "claude-ai", version: "1" } } };

/** No line of stdout or stderr carries the token, and every stdout line is a JSON-RPC message. */
function assertClean(f) {
  assert.ok(!f.err.includes(TOKEN), "token on stderr");
  for (const line of f.out) {
    assert.ok(!line.includes(TOKEN), "token on stdout");
    assert.equal(JSON.parse(line).jsonrpc, "2.0", line);
  }
}

test("the token file's path and the port follow token.rs and the manifest", () => {
  assert.equal(tokenPath({ SATURNUS_TOKEN_FILE: "/x/t" }, "darwin"), "/x/t");
  assert.equal(tokenPath({ HOME: "/home/u" }, "linux"), join("/home/u", ".config", "saturnus", "control-token"));
  assert.equal(tokenPath({ HOME: "/home/u", XDG_CONFIG_HOME: "/cfg" }, "linux"), join("/cfg", "saturnus", "control-token"));
  assert.equal(tokenPath({ HOME: "/home/u", XDG_CONFIG_HOME: "rel" }, "linux"), join("/home/u", ".config", "saturnus", "control-token"), "a relative XDG_CONFIG_HOME is ignored");
  assert.equal(tokenPath({ LOCALAPPDATA: "C:\\Users\\u\\AppData\\Local" }, "win32"), join("C:\\Users\\u\\AppData\\Local", "saturnus", "control-token"));
  assert.equal(tokenPath({}, "win32"), null);
  assert.equal(portOf({}), DEFAULT_PORT);
  assert.equal(portOf({ SATURNUS_MCP_PORT: "4900" }), 4900);
  for (const bad of ["", "0", "70000", "${user_config.port}", "12ab"]) assert.equal(portOf({ SATURNUS_MCP_PORT: bad }), DEFAULT_PORT, bad);
});

test("the token is read as token.rs reads it, and problems never show it", () => {
  const t = tempToken(TOKEN.toUpperCase());
  try {
    assert.deepEqual(readToken(t.file), { token: TOKEN });
    assert.equal(readToken(join(t.dir, "none")).missing, true);
    writeFileSync(t.file, "not a token secret-ish\n", { mode: 0o600 });
    const bad = readToken(t.file);
    assert.match(bad.problem, /not a saturnus key/);
    assert.ok(!bad.problem.includes("secret-ish"));
    if (process.platform !== "win32") {
      writeFileSync(t.file, `${TOKEN}\n`);
      chmodSync(t.file, 0o644);
      const exposed = readToken(t.file);
      assert.match(exposed.problem, /other users/);
      assert.ok(!exposed.problem.includes(TOKEN));
    }
  } finally {
    t.cleanup();
  }
});

test("the proof is token.rs's (the same fixed value as its test)", () => {
  assert.equal(proofOf("ab".repeat(32), "cd".repeat(32), 4842), "cb5b940c81864eae6e01e3d7dfe7916eefa6e60c1425f4fea5abd58633074b73");
});

test("the endpoint's not-ready answer starts as the forwarder expects", () => {
  assert.ok(readFileSync(new URL("../../crates/saturnus-tauri/src/mcp.rs", import.meta.url), "utf8").includes(`"${NOT_READY}:`));
});

test("event streams give their messages", () => {
  const a = { jsonrpc: "2.0", method: "notifications/x" };
  const b = { jsonrpc: "2.0", id: 3, result: {} };
  assert.deepEqual(sseMessages(`event: message\ndata: ${JSON.stringify(a)}\n\nevent: message\ndata: ${JSON.stringify(b)}\n\n`), [a, b]);
  assert.deepEqual(sseMessages(": comment\n\n"), []);
});

test("with the app closed: initialize, ping and tools/list from the bundle, a call starts the app, then the message", async () => {
  const t = tempToken();
  const port = await freePort();
  const launches = join(t.dir, "launches");
  const f = forwarder({ SATURNUS_TOKEN_FILE: t.file, SATURNUS_MCP_PORT: String(port), SATURNUS_MCPB_LAUNCH_FILE: launches });
  try {
    const offline = OFFLINE;
    f.send(INIT);
    const init = await f.next(1);
    assert.equal(init.result.protocolVersion, "2025-06-18");
    assert.equal(init.result.serverInfo.name, "saturnus");
    assert.equal(init.result.instructions, offline.instructions);
    f.send({ jsonrpc: "2.0", method: "notifications/initialized" });
    f.send({ jsonrpc: "2.0", id: 2, method: "tools/list" });
    assert.deepEqual((await f.next(2)).result.tools, offline.tools);
    f.send({ jsonrpc: "2.0", id: 3, method: "ping" });
    assert.deepEqual((await f.next(3)).result, {});
    assert.ok(!existsSync(launches), "nothing so far starts the app");
    f.send({ jsonrpc: "2.0", id: 4, method: "tools/call", params: { name: "read_display", arguments: {} } });
    const call = await f.next(4);
    assert.equal(call.result.isError, true);
    assert.equal(call.result.content[0].text, START);
    assert.equal(readFileSync(launches, "utf8"), "launch\n");
    f.send({ jsonrpc: "2.0", id: 5, method: "resources/list" });
    assert.equal((await f.next(5)).error.code, -32601);
    f.send("{not json");
    assert.equal((await f.next(null)).error.code, -32700);
    assert.equal(f.out.length, 6, "the notification got no answer");
    await f.stop();
    assertClean(f);
  } finally {
    t.cleanup();
  }
});

test("with the app on: the proof first, then the token, the session and the headers; events and notifications", async () => {
  const t = tempToken();
  const port = await freePort();
  const ep = await fakeEndpoint(port, {
    answer: (msg) => (msg.method === "subscriptions/listen"
      ? { type: "text/event-stream", body: `event: message\ndata: {"jsonrpc":"2.0","method":"notifications/subscriptions/acknowledged"}\n\nevent: message\ndata: {"jsonrpc":"2.0","id":${msg.id},"result":{}}\n\n` }
      : null),
  });
  const f = forwarder({ SATURNUS_TOKEN_FILE: t.file, SATURNUS_MCP_PORT: String(port) });
  try {
    f.send(INIT);
    assert.equal((await f.next(1)).result.instructions, "live");
    f.send({ jsonrpc: "2.0", method: "notifications/initialized" });
    f.send({ jsonrpc: "2.0", id: 2, method: "tools/list" });
    assert.equal((await f.next(2)).result.tools[0].name, "live_tool");
    f.send({ jsonrpc: "2.0", id: 3, method: "tools/call", params: { name: "run_rpl", arguments: { text: "1 2 +" } } });
    assert.equal((await f.next(3)).result.content[0].text, "ran run_rpl");
    // The stateless revision: its headers from the body.
    const meta = { "io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientCapabilities": {} };
    f.send({ jsonrpc: "2.0", id: 4, method: "tools/call", params: { name: "pürge", arguments: {}, _meta: meta } });
    await f.next(4);
    f.send({ jsonrpc: "2.0", id: 5, method: "subscriptions/listen", params: { _meta: meta } });
    await f.next(5);
    await f.stop();
    const methods = ep.requests.map((r) => r.msg.method);
    assert.deepEqual(methods.filter((m) => m !== "notifications/initialized"), ["initialize", "tools/list", "tools/call", "tools/call", "subscriptions/listen"]);
    assert.ok(methods.includes("notifications/initialized"), "the notification went to the app");
    assert.ok(ep.hellos.length >= ep.requests.length, "a proof before every request");
    for (const h of ep.hellos) assert.equal(h.headers.authorization, undefined, "never the token with the hello");
    for (const r of ep.requests) {
      assert.equal(r.headers.authorization, `Bearer ${TOKEN}`);
      assert.equal(r.headers.host, `127.0.0.1:${port}`);
      assert.match(r.headers["content-type"], /^application\/json/);
    }
    const list = ep.requests[methods.indexOf("tools/list")];
    assert.equal(list.headers["mcp-session-id"], "s".repeat(64));
    assert.equal(list.headers["mcp-protocol-version"], "2025-06-18");
    const modern = ep.requests.filter((r) => r.msg.params?._meta);
    assert.equal(modern[0].headers["mcp-protocol-version"], "2026-07-28");
    assert.equal(modern[0].headers["mcp-method"], "tools/call");
    assert.equal(modern[0].headers["mcp-name"], `=?base64?${Buffer.from("pürge").toString("base64")}?=`);
    assert.equal(modern[1].headers["mcp-method"], "subscriptions/listen");
    const lines = f.out.map((l) => JSON.parse(l));
    assert.ok(lines.some((m) => m.method === "notifications/subscriptions/acknowledged"), "the stream's events");
    assert.equal(lines.filter((m) => !("id" in m)).length, 1, "nothing for the notification");
    assertClean(f);
  } finally {
    await ep.close();
    t.cleanup();
  }
});

test("a server without the proof never gets the token", async () => {
  const t = tempToken();
  const port = await freePort();
  const ep = await fakeEndpoint(port, { proof: "0".repeat(64) });
  const launches = join(t.dir, "launches");
  const f = forwarder({ SATURNUS_TOKEN_FILE: t.file, SATURNUS_MCP_PORT: String(port), SATURNUS_MCPB_LAUNCH_FILE: launches });
  try {
    f.send(INIT);
    assert.match((await f.next(1)).error.message, /Something else answers on port/);
    f.send({ jsonrpc: "2.0", method: "notifications/initialized" });
    f.send({ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "read_display", arguments: {} } });
    assert.match((await f.next(2)).error.message, /the key was not sent/);
    await f.stop();
    assert.equal(ep.requests.length, 0);
    assert.ok(!existsSync(launches));
    assertClean(f);
  } finally {
    await ep.close();
    t.cleanup();
  }
});

test("a call starts the app and goes through once it answers, after the client's initialize", async () => {
  const t = tempToken();
  const port = await freePort();
  const launches = join(t.dir, "launches");
  const f = forwarder({ SATURNUS_TOKEN_FILE: t.file, SATURNUS_MCP_PORT: String(port), SATURNUS_MCPB_LAUNCH_FILE: launches, SATURNUS_MCPB_WAIT_MS: "8000" });
  let ep = null;
  try {
    f.send(INIT);
    await f.next(1);
    f.send({ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "read_display", arguments: {} } });
    const until = Date.now() + 5000;
    while (!existsSync(launches) && Date.now() < until) await new Promise((r) => setTimeout(r, 50));
    assert.ok(existsSync(launches), "the app was started");
    // The window loads after the endpoint is up: "not ready" twice, then the answer.
    let loading = 2;
    ep = await fakeEndpoint(port, {
      answer: (msg) => (msg.method === "tools/call" && loading-- > 0
        ? { body: { jsonrpc: "2.0", id: msg.id, result: { content: [{ type: "text", text: `${NOT_READY}: its window is loading or closed. Try again in a moment.` }], isError: true } } }
        : null),
    });
    assert.equal((await f.next(2)).result.content[0].text, "ran read_display");
    assert.deepEqual(ep.requests.map((r) => r.msg.method), ["initialize", "tools/call", "tools/call", "tools/call"]);
    assert.equal(ep.requests[0].msg.params.clientInfo.name, "claude-ai");
    assert.equal(ep.requests[1].headers["mcp-session-id"], "s".repeat(64));
    assert.equal(f.out.filter((l) => JSON.parse(l).id === 2).length, 1, "one answer for the call");
    await f.stop();
    assertClean(f);
  } finally {
    await ep?.close();
    t.cleanup();
  }
});

test("AI access off, a refused key and a missing key file give clear errors with the request's id", async () => {
  const t = tempToken();
  const port = await freePort();
  let mode = "off";
  const ep = await fakeEndpoint(port, {
    answer: (msg) => (mode === "off"
      ? { status: 503, body: { jsonrpc: "2.0", id: msg.id, error: { code: -32001, message: "AI access is off in the saturnus app." } } }
      : { status: 401, body: {} }),
  });
  const f = forwarder({ SATURNUS_TOKEN_FILE: t.file, SATURNUS_MCP_PORT: String(port) });
  try {
    f.send({ jsonrpc: "2.0", id: 7, method: "tools/call", params: { name: "read_display", arguments: {} } });
    assert.equal((await f.next(7)).error.message, START);
    mode = "401";
    f.send({ jsonrpc: "2.0", id: 8, method: "tools/call", params: { name: "read_display", arguments: {} } });
    const refused = await f.next(8);
    assert.equal(refused.id, 8);
    assert.match(refused.error.message, /did not accept the key/);
    await f.stop();
    assertClean(f);
  } finally {
    await ep.close();
  }
  const g = forwarder({ SATURNUS_TOKEN_FILE: join(t.dir, "none"), SATURNUS_MCP_PORT: String(port), SATURNUS_MCPB_LAUNCH_FILE: join(t.dir, "launches") });
  try {
    g.send(INIT);
    assert.equal((await g.next(1)).result.serverInfo.name, "saturnus", "initialize works without the key file");
    g.send({ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "read_display", arguments: {} } });
    assert.equal((await g.next(2)).error.message, START);
    await g.stop();
    assert.ok(!existsSync(join(t.dir, "launches")), "no key file: AI access was never on, nothing to start for");
  } finally {
    t.cleanup();
  }
});
