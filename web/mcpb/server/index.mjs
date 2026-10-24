// The saturnus extension for Claude Desktop (iteration 59): a forwarder
// that Claude Desktop starts with its own Node (MCPB server type "node").
// It reads MCP's JSON-RPC messages from stdin, one per line, sends each
// request to the saturnus desktop app's MCP endpoint at
// http://127.0.0.1:PORT/mcp (crates/saturnus-tauri/src/mcp.rs) and writes
// the answers to stdout. Node built-ins only; the same file for every user.
//
// The token: read at run time from the per-user control-token file, by
// the rules of crates/saturnus-drive/src/loopback/token.rs (default_path,
// resolve, read): $SATURNUS_TOKEN_FILE, else %LOCALAPPDATA%\saturnus\
// control-token on Windows, else $XDG_CONFIG_HOME/saturnus/control-token
// (an absolute XDG_CONFIG_HOME), else ~/.config/saturnus/control-token; on
// Unix a file other users may read is refused. It is never printed,
// logged or put into a message, and it goes only to 127.0.0.1 at the
// port, after the server proved it holds it: a token-free
// `GET /hello?nonce=N` answered with HMAC-SHA-256 under the token of
// "saturnus control hello\nN\nPORT" (token.rs `proof`), checked before
// every request (kb: docs/control-api-security).
//
// The app closed: initialize, ping and tools/list are answered from
// offline.json (the app's instructions and tool list, written by
// web/mcpb/build.mjs), so opening Claude Desktop does not open saturnus.
// A tools/call starts the app (macOS `open -b ch.ractive.saturnus`;
// Windows and Linux the installed program, best effort) and waits for the
// endpoint up to 20 s; then, or with AI access off, the call's answer is
// START. A call the just-started app answers "not ready" (its window is
// still loading) is asked again within that time (5 s when the user
// started it). The forwarder never turns AI access on. Something else on
// the port: the same offline answers, and a call gets otherMessage().
// Windows and Linux: starting the app is best effort.
//
// stdout carries MCP only; diagnostics go to stderr.
//
// Environment: SATURNUS_MCP_PORT (the endpoint's port, from the
// extension's settings; default 4842), SATURNUS_TOKEN_FILE. For the tests
// (web/test/mcpb.test.mjs) only: SATURNUS_MCPB_LAUNCH_FILE (a file a
// launch is recorded in instead of starting the app) and
// SATURNUS_MCPB_WAIT_MS (the wait for the app).

import { createHmac, randomBytes, timingSafeEqual } from "node:crypto";
import { appendFileSync, existsSync, readFileSync, realpathSync, statSync } from "node:fs";
import { request } from "node:http";
import { homedir } from "node:os";
import { join, isAbsolute } from "node:path";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { pathToFileURL } from "node:url";

export const DEFAULT_PORT = 4842;
export const BUNDLE_ID = "ch.ractive.saturnus";
/** The answer when the app does not answer: closed, starting too slowly, or AI access off. */
export const START = "Start saturnus and turn on AI access in its settings.";
/** The legacy handshake's revisions the endpoint speaks, oldest first (mcp.rs LEGACY). */
export const LEGACY = ["2025-03-26", "2025-06-18", "2025-11-25"];
export const MODERN = "2026-07-28";
const META_VERSION = "io.modelcontextprotocol/protocolVersion";
const META_SERVER_INFO = "io.modelcontextprotocol/serverInfo";
/** The JSON-RPC error of a call the app could not answer (mcp.rs NOT_ANSWERED). */
const NOT_ANSWERED = -32001;
const HELLO_TIMEOUT_MS = 2000;
/** The longest a call may take: the app's own limit is 600 s. */
const CALL_TIMEOUT_MS = 660_000;
const MAX_ANSWER = 64 * 1024 * 1024;
/** A launch is not repeated within this time; a call then looks only briefly. */
const LAUNCH_AGAIN_MS = 60_000;
const SHORT_WAIT_MS = 2000;
const POLL_MS = 250;
/** A call answered "not ready" when this forwarder did not start the app is asked again this long. */
const NOT_READY_RETRY_MS = 5000;

const log = (msg) => process.stderr.write(`saturnus extension: ${msg}\n`);

/** The endpoint's port: SATURNUS_MCP_PORT when it is one (1-65535), else 4842. */
export function portOf(env = process.env) {
  const v = String(env.SATURNUS_MCP_PORT ?? "").trim();
  if (/^\d{1,5}$/.test(v) && Number(v) >= 1 && Number(v) <= 65535) return Number(v);
  return DEFAULT_PORT;
}

/** The token file's path, as token.rs `resolve(None)`. */
export function tokenPath(env = process.env, platform = process.platform) {
  if (env.SATURNUS_TOKEN_FILE) return env.SATURNUS_TOKEN_FILE;
  if (platform === "win32") {
    if (!env.LOCALAPPDATA) return null;
    return join(env.LOCALAPPDATA, "saturnus", "control-token");
  }
  const xdg = env.XDG_CONFIG_HOME;
  const base = xdg && isAbsolute(xdg) ? xdg : (env.HOME || homedir()) ? join(env.HOME || homedir(), ".config") : null;
  return base ? join(base, "saturnus", "control-token") : null;
}

/**
 * The token in `path`, as token.rs `read`: `{token}`, or `{problem}` (a
 * message that names the file, never its content); `missing` when there is
 * no file.
 */
export function readToken(path, platform = process.platform) {
  if (!path) return { problem: "No home folder: the key file of saturnus cannot be found.", missing: true };
  let meta;
  try {
    meta = statSync(path);
  } catch (e) {
    if (e.code === "ENOENT") return { problem: START, missing: true };
    return { problem: `Cannot read the key file ${path} (${e.code ?? "error"}).` };
  }
  if (!meta.isFile()) return { problem: `The key file ${path} is not a regular file.` };
  if (platform !== "win32" && (meta.mode & 0o077) !== 0) {
    return { problem: `The key file ${path} can be read by other users. Run chmod 600 on it, or delete it and turn AI access off and on in saturnus for a new one.` };
  }
  let text;
  try {
    text = readFileSync(path, "utf8").trim();
  } catch (e) {
    return { problem: `Cannot read the key file ${path} (${e.code ?? "error"}).` };
  }
  if (!/^[0-9a-fA-F]{64}$/.test(text)) {
    return { problem: `The key file ${path} is not a saturnus key. Delete it, then turn AI access off and on in saturnus for a new one.` };
  }
  return { token: text.toLowerCase() };
}

/** The proof a server holding `token` gives for `nonce` on `port` (token.rs `proof`). */
export function proofOf(token, nonce, port) {
  return createHmac("sha256", token).update(`saturnus control hello\n${nonce}\n${port}`).digest("hex");
}

/** One HTTP exchange with 127.0.0.1:`port`: `{status, headers, body}`; rejects on a network error. */
function exchange(port, { method, path, headers = {}, body = null, timeoutMs, signal = null }) {
  return new Promise((resolve, reject) => {
    const req = request({ host: "127.0.0.1", port, method, path, headers, agent: false, timeout: timeoutMs, signal });
    req.on("timeout", () => req.destroy(Object.assign(new Error("timed out"), { code: "ETIMEDOUT" })));
    req.on("error", reject);
    req.on("response", (res) => {
      const chunks = [];
      let size = 0;
      res.on("data", (c) => {
        size += c.length;
        if (size > MAX_ANSWER) req.destroy(Object.assign(new Error("answer too large"), { code: "ETOOBIG" }));
        else chunks.push(c);
      });
      res.on("end", () => resolve({ status: res.statusCode, headers: res.headers, body: Buffer.concat(chunks) }));
      res.on("error", reject);
    });
    req.end(body ?? undefined);
  });
}

/**
 * Whether the endpoint on `port` holds `token`: "up" (its proof is
 * right), "down" (nothing answers) or "other" (something answers without
 * the proof: another program, or an older saturnus).
 */
export async function hello(port, token) {
  const nonce = randomBytes(32).toString("hex");
  let r;
  try {
    r = await exchange(port, { method: "GET", path: `/hello?nonce=${nonce}`, timeoutMs: HELLO_TIMEOUT_MS });
  } catch {
    return "down";
  }
  if (r.status !== 200) return "other";
  let proof = "";
  try {
    proof = String(JSON.parse(r.body.toString("utf8")).proof ?? "");
  } catch {
    return "other";
  }
  const expected = Buffer.from(proofOf(token, nonce, port));
  const got = Buffer.from(proof);
  return got.length === expected.length && timingSafeEqual(got, expected) ? "up" : "other";
}

/** The installed app's program on Windows, the first that exists. */
function windowsApp(env) {
  const roots = [env.LOCALAPPDATA, env.LOCALAPPDATA && join(env.LOCALAPPDATA, "Programs"), env.ProgramFiles, env["ProgramFiles(x86)"]].filter(Boolean);
  for (const root of roots) {
    for (const exe of ["saturnus.exe", "saturnus-app.exe"]) {
      const p = join(root, "saturnus", exe);
      if (existsSync(p)) return p;
    }
  }
  return null;
}

/**
 * Start the desktop app, detached; resolves to whether it started (the
 * program ran: `spawn`, not `error`). Best effort on Windows and Linux,
 * where the installed program is looked for in its usual places.
 */
export function launch(env = process.env, platform = process.platform) {
  if (env.SATURNUS_MCPB_LAUNCH_FILE) {
    appendFileSync(env.SATURNUS_MCPB_LAUNCH_FILE, "launch\n");
    return Promise.resolve(true);
  }
  let cmd;
  let args = [];
  if (platform === "darwin") {
    cmd = "open";
    args = ["-b", BUNDLE_ID];
  } else if (platform === "win32") {
    cmd = windowsApp(env);
  } else {
    cmd = "saturnus-app";
  }
  if (!cmd) {
    log("the saturnus app was not found in its usual places; start it yourself");
    return Promise.resolve(false);
  }
  return new Promise((resolve) => {
    try {
      const child = spawn(cmd, args, { detached: true, stdio: "ignore", windowsHide: false });
      child.once("spawn", () => {
        child.unref();
        resolve(true);
      });
      child.once("error", (e) => {
        log(`cannot start the saturnus app (${e.code ?? e.message})`);
        resolve(false);
      });
    } catch (e) {
      log(`cannot start the saturnus app (${e.code ?? e.message})`);
      resolve(false);
    }
  });
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** The SEP-2243 header value of `s`: as it is when plain printable ASCII, else the base64 sentinel. */
function headerValue(s) {
  return /^[\x21-\x7e](?:[\x20-\x7e]*[\x21-\x7e])?$/.test(s) ? s : `=?base64?${Buffer.from(s, "utf8").toString("base64")}?=`;
}

/** The events' messages of an event stream body. */
export function sseMessages(text) {
  const out = [];
  for (const event of text.split(/\r?\n\r?\n/)) {
    const data = event.split(/\r?\n/).filter((l) => l.startsWith("data:")).map((l) => l.slice(5).replace(/^ /, "")).join("\n");
    if (!data) continue;
    try {
      out.push(JSON.parse(data));
    } catch {
      log("the app sent an event that is not JSON; it was dropped");
    }
  }
  return out;
}

/** The start of the endpoint's answer while its window loads (mcp.rs `CallError::NotReady`). */
export const NOT_READY = "The saturnus app is not ready";

/** Whether `m` is the endpoint's "not ready" answer: a tool result with isError, or an error. */
function notReady(m) {
  const text = m?.result?.isError ? m.result.content?.[0]?.text : m?.error?.message;
  return typeof text === "string" && text.startsWith(NOT_READY);
}

const isModern = (msg) => typeof msg?.params?._meta?.[META_VERSION] === "string";

/** The forwarder: `handle(line)` per stdin line, answers through `write(message)`. */
export class Forwarder {
  constructor({ env = process.env, platform = process.platform, write, offline }) {
    this.env = env;
    this.platform = platform;
    this.write = write;
    this.offline = offline;
    this.port = portOf(env);
    this.waitMs = Number(env.SATURNUS_MCPB_WAIT_MS) > 0 ? Number(env.SATURNUS_MCPB_WAIT_MS) : 20_000;
    /** The client's initialize, replayed to the app for its session. */
    this.initialize = null;
    /** The app's legacy session and the revision agreed, once it answered an initialize. */
    this.session = null;
    this.version = null;
    this.replaying = null;
    this.lastLaunch = -Infinity;
    this.starting = null;
    /** Calls in flight by JSON-RPC id, for notifications/cancelled. */
    this.inFlight = new Map();
  }

  /** One line from stdin. */
  async handle(line) {
    if (!line.trim()) return;
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      this.write({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "Parse error: the message is not valid JSON" } });
      return;
    }
    if (Array.isArray(msg)) {
      this.write({ jsonrpc: "2.0", id: null, error: { code: -32600, message: "Batches are not supported" } });
      return;
    }
    if (typeof msg !== "object" || msg === null || typeof msg.method !== "string") return; // a response: the app asks nothing
    if (!("id" in msg)) return this.notification(msg);
    try {
      await this.request(msg);
    } catch (e) {
      log(`a call failed (${e.code ?? e.message})`);
      this.write(this.error(msg.id, "The call to saturnus failed. Try again."));
    }
  }

  error(id, message, code = NOT_ANSWERED) {
    return { jsonrpc: "2.0", id, error: { code, message } };
  }

  async notification(msg) {
    if (msg.method === "notifications/cancelled") {
      this.inFlight.get(msg.params?.requestId)?.abort();
      return;
    }
    // Others go to the app when it runs (it takes them, 202), never start
    // it; post() sends nothing without the proof.
    const t = readToken(tokenPath(this.env, this.platform), this.platform);
    if (!t.token) return;
    try {
      await this.post(msg, t.token);
    } catch {
      // Nothing to answer.
    }
  }

  /** Abort every call in flight (stdin closed: nobody reads the answers). */
  abortAll() {
    for (const c of this.inFlight.values()) c.abort();
  }

  async request(msg) {
    const { id, method } = msg;
    // Registered before anything waits, so a cancellation at any point
    // finds it; an aborted call never gets an answer.
    const controller = new AbortController();
    const { signal } = controller;
    this.inFlight.set(id, controller);
    const write = (m) => {
      if (!signal.aborted) this.write(m);
    };
    const toolError = (text) => (method === "tools/call" ? { jsonrpc: "2.0", id, result: { content: [{ type: "text", text }], isError: true } } : this.error(id, text));
    const began = Date.now();
    try {
      if (method === "initialize") this.initialize = msg;
      const t = readToken(tokenPath(this.env, this.platform), this.platform);
      if (!t.token) {
        if (!t.missing) log(t.problem);
        return write(this.offlineAnswer(msg) ?? this.error(id, t.problem));
      }
      let state = await hello(this.port, t.token);
      if (signal.aborted) return;
      if (state !== "up") {
        // Closed, or something else on the port (an older saturnus, another
        // program): what needs no calculator is answered from the bundle.
        const answer = this.offlineAnswer(msg);
        if (answer) return write(answer);
        if (state === "down") state = await this.start(t.token);
        if (signal.aborted) return;
        if (state === "other") return write(toolError(this.otherMessage()));
        if (state !== "up") return write(toolError(START));
      }
      if (method !== "initialize" && !isModern(msg)) await this.replayInitialize(t.token);
      if (signal.aborted) return;
      let answers = this.answers(msg, await this.post(msg, t.token, signal));
      // Just started: the endpoint answers before the window has loaded
      // its tools. Asked again until it has, within the wait for the app
      // (or a few seconds when the user started it).
      const until = Math.max(this.lastLaunch + this.waitMs, began + NOT_READY_RETRY_MS);
      while (answers.length === 1 && notReady(answers[0]) && Date.now() < until && !signal.aborted) {
        await sleep(POLL_MS);
        if (signal.aborted) return;
        answers = this.answers(msg, await this.post(msg, t.token, signal));
      }
      for (const m of answers) write(m);
    } catch (e) {
      if (signal.aborted) return; // cancelled: no answer
      if (e.code === "ENOPROOF") return write(toolError(e.state === "other" ? this.otherMessage() : START));
      log(`the app closed the connection (${e.code ?? e.message})`);
      write(this.error(id, "saturnus closed during the call. Start it again, then try again."));
    } finally {
      this.inFlight.delete(id);
    }
  }

  otherMessage() {
    return `Something else answers on port ${this.port} instead of saturnus, so the key was not sent. If saturnus runs, update it; otherwise check the port in its AI settings and in this extension's settings.`;
  }

  /** Start the app (not again within a minute) and wait for its endpoint; "up", "down" or "other". */
  start(token) {
    this.starting ??= (async () => {
      const now = Date.now();
      let limit = SHORT_WAIT_MS;
      if (now - this.lastLaunch > LAUNCH_AGAIN_MS) {
        this.lastLaunch = now;
        if (await launch(this.env, this.platform)) limit = this.waitMs;
      }
      const end = Date.now() + Math.min(limit, this.waitMs);
      for (;;) {
        const s = await hello(this.port, token);
        if (s !== "down" || Date.now() >= end) {
          if (s === "down") log(`no answer from saturnus on port ${this.port}: is AI access on in its settings?`);
          return s;
        }
        await sleep(POLL_MS);
      }
    })().finally(() => {
      this.starting = null;
    });
    return this.starting;
  }

  /** Give the app the client's initialize once, for its session (the AI app's name in saturnus). */
  async replayInitialize(token) {
    if (this.session || !this.initialize || isModern(this.initialize)) return;
    this.replaying ??= this.post(this.initialize, token).then(() => {}, () => {}).finally(() => {
      this.replaying = null;
    });
    await this.replaying;
  }

  /**
   * POST `msg` to the endpoint with its headers, after a fresh proof that
   * the endpoint holds the token (every POST: a replay, each retry); the
   * HTTP answer. Without the proof nothing is sent: an error with code
   * ENOPROOF and the hello's `state`.
   */
  async post(msg, token, signal = null) {
    const state = await hello(this.port, token);
    if (state !== "up") throw Object.assign(new Error(`no proof (${state})`), { code: "ENOPROOF", state });
    if (signal?.aborted) throw Object.assign(new Error("cancelled"), { code: "ABORT_ERR" });
    const body = Buffer.from(JSON.stringify(msg), "utf8");
    const headers = {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
      Accept: "application/json, text/event-stream",
      "Content-Length": String(body.length),
      "User-Agent": "saturnus-mcpb",
    };
    if (isModern(msg)) {
      headers["MCP-Protocol-Version"] = msg.params._meta[META_VERSION];
      headers["Mcp-Method"] = msg.method;
      if (msg.method === "tools/call" && typeof msg.params.name === "string") headers["Mcp-Name"] = headerValue(msg.params.name);
    } else if (msg.method !== "initialize") {
      if (this.version) headers["MCP-Protocol-Version"] = this.version;
      if (this.session) headers["Mcp-Session-Id"] = this.session;
    }
    const r = await exchange(this.port, { method: "POST", path: "/mcp", headers, body, timeoutMs: CALL_TIMEOUT_MS, signal });
    if (msg.method === "initialize" && r.status === 200) {
      const session = r.headers["mcp-session-id"];
      if (typeof session === "string" && /^[\x21-\x7e]{1,128}$/.test(session)) this.session = session;
      try {
        const v = JSON.parse(r.body.toString("utf8"))?.result?.protocolVersion;
        if (LEGACY.includes(v)) this.version = v;
      } catch {
        // No version: the requests go without the header.
      }
    }
    return r;
  }

  /** The messages for the client from the app's answer `r` to `msg`. */
  answers(msg, r) {
    const type = String(r.headers["content-type"] ?? "").split(";")[0].trim().toLowerCase();
    if (type === "text/event-stream") {
      const events = sseMessages(r.body.toString("utf8"));
      // The stream must end with the request's answer, or the client would wait for ever.
      if (!events.some((m) => m && m.id === msg.id && ("result" in m || "error" in m))) events.push(this.error(msg.id, "saturnus ended the call without an answer. Try again."));
      return events;
    }
    if (r.status === 202 || r.body.length === 0) return [this.error(msg.id, r.status < 300 ? "saturnus gave no answer. Try again." : START)];
    let v;
    try {
      v = JSON.parse(r.body.toString("utf8"));
    } catch {
      return [this.error(msg.id, `saturnus answered with HTTP ${r.status} and no message.`)];
    }
    if (r.status === 401) return [this.error(msg.id, "saturnus did not accept the key. Turn AI access off and on in saturnus, then try again.")];
    if (r.status === 503 && /AI access is off/.test(v?.error?.message ?? "")) return [this.error(msg.id, START)];
    // Refusals before the body was read carry no id: give them the request's.
    if (v && typeof v === "object" && v.error && (v.id === null || v.id === undefined)) v.id = msg.id;
    return [v];
  }

  /** The answer while the app is closed, for the methods that need no calculator; else null. */
  offlineAnswer(msg) {
    const { id, method } = msg;
    const o = this.offline;
    const serverInfo = { name: "saturnus", title: "saturnus calculators", version: o.version };
    const modern = isModern(msg);
    const result = (value) => {
      if (modern) {
        value.resultType = "complete";
        value._meta = { [META_SERVER_INFO]: serverInfo };
      }
      return { jsonrpc: "2.0", id, result: value };
    };
    if (method === "initialize" && !modern) {
      const asked = msg.params?.protocolVersion;
      return result({ protocolVersion: LEGACY.includes(asked) ? asked : LEGACY[LEGACY.length - 1], capabilities: { tools: { listChanged: false } }, serverInfo, instructions: o.instructions });
    }
    if (method === "server/discover" && modern) return result({ supportedVersions: [MODERN], capabilities: { tools: { listChanged: false } }, ttlMs: 0, cacheScope: "private", instructions: o.instructions });
    if (method === "ping") return result({});
    if (method === "tools/list") return result(modern ? { tools: o.tools, ttlMs: 0, cacheScope: "private" } : { tools: o.tools });
    if (method !== "tools/call") return { jsonrpc: "2.0", id, error: { code: -32601, message: "Method not found" } };
    return null;
  }
}

/** Run on stdin and stdout. */
export function main() {
  let offline;
  try {
    offline = JSON.parse(readFileSync(new URL("./offline.json", import.meta.url), "utf8"));
  } catch (e) {
    log(`offline.json is missing or broken (${e.message}); reinstall the extension`);
    process.exit(1);
  }
  const fwd = new Forwarder({ write: (m) => process.stdout.write(`${JSON.stringify(m)}\n`), offline });
  const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });
  rl.on("line", (line) => {
    fwd.handle(line).catch((e) => log(`unexpected: ${e.message}`));
  });
  // stdin closed: Claude Desktop is gone or stopped the server. The calls
  // in flight are aborted (nobody reads their answers) and the process
  // ends once what was written has reached stdout.
  rl.on("close", () => {
    fwd.abortAll();
    process.stdout.write("", () => process.exit(0));
  });
}

// Run when started as the program (not when a test imports it).
let started = false;
try {
  started = Boolean(process.argv[1]) && pathToFileURL(realpathSync(process.argv[1])).href === import.meta.url;
} catch {
  started = false;
}
if (started) main();
