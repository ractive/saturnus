// AI access in the desktop app (iteration 58): AI apps on this computer
// (Claude Code, Codex, VS Code, Cursor) use the calculators over MCP at
// http://127.0.0.1:4842/mcp, served by the app itself while the setting
// "Let AI apps on this computer use the calculators" is on
// (crates/saturnus-tauri/src/mcp.rs and mcp_app.rs). No relay, no
// internet.
//
// The tools are the web app's (web/ai-tools.js `aiTools`, with the same
// deps as web/ai.js gives tab-mcp) and so are the server's instructions:
// the app's endpoint only forwards `tools/list` and `tools/call` here
// (the Tauri event `mcp-call`; the answer goes back with `mcp_reply`),
// and web/test/ai-local.test.mjs checks that both paths list the same.
//
// What tab-mcp's client does around a call, done here the same way (its
// client exports no local entry, so saturnus does it itself): the tool
// list in MCP's shape (`listTools`, tab-mcp's annotation mapping), the
// arguments checked against the tool's schema (`validate`), the
// confirmation of a consequential tool in tab-mcp's own dialog (its
// vendored `confirmDialog`, so it looks the same and "The AI's calls"
// sees it), "Always allow" per AI app and tool until the app reloads or
// the setting goes off, the result in MCP's shape (`toCallToolResult`),
// and the `toolcall`/`toolcallend` events that feed "The AI's calls"
// (web/ai-log.js). Never the token in an event, the log or a note.
//
// The same face as web/ai.js's `AiAccess` towards the page (app.js
// `watchAi`, ai-log.js, ai-dialog.js): `state` ("connected" while the
// endpoint runs), `mcp` (`address`, `headers`, `grants`: the AI apps seen
// lately), `error`, `log`, `keepsRunning`, `connect()` (turn on),
// `disconnect()` (turn off), `change` events. `local` tells the dialog
// which one it has.

import { aiTools, INSTRUCTIONS, titleOf } from "./ai-tools.js";
import { AiLog } from "./ai-log.js";
import { keepsRunning, screenPng } from "./ai.js";

/** The confirmation of tab-mcp's client (web/vendor/tab-mcp-client/, its UI chunk's export). */
export const CONFIRM_MODULE = "./vendor/tab-mcp-client/ui-xh7shp5z.min.js";
/** The time limit of a tool call (ms), for the app's endpoint; one that asks the user first gets longer. */
export const CALL_LIMIT_MS = 120_000;
export const CONFIRM_LIMIT_MS = 300_000;
/** Where the AI apps are, for the confirmation ("Claude Code (this computer)"). */
const HERE = "this computer";

// --- MCP's shape of the tools and results, as tab-mcp's client makes it
// (tab-mcp-client src/mapping.ts, mcp.ts `listTools`).

/** `_meta` key that keeps the original WebMCP hints. */
const ANNOTATIONS_META_KEY = "ch.ractive.tab-mcp/annotations";
const WEBMCP_HINTS = ["readOnlyHint", "untrustedContentHint", "consequentialHint", "debugging"];
const MCP_ONLY_HINTS = ["destructiveHint", "idempotentHint", "openWorldHint"];

/** MCP annotations of a tool's WebMCP hints, and the originals for `_meta`. */
export function mapAnnotations(hints = {}) {
  const annotations = {};
  if (hints.readOnlyHint === true) {
    annotations.readOnlyHint = true;
    annotations.destructiveHint = false;
  } else if (hints.readOnlyHint === false) {
    annotations.readOnlyHint = false;
  }
  if (hints.consequentialHint === true) annotations.destructiveHint = true;
  if (hints.untrustedContentHint === true) annotations.openWorldHint = true;
  for (const key of MCP_ONLY_HINTS) if (typeof hints[key] === "boolean") annotations[key] = hints[key];
  const meta = {};
  for (const key of WEBMCP_HINTS) if (typeof hints[key] === "boolean") meta[key] = hints[key];
  const out = {};
  if (Object.keys(annotations).length) out.annotations = annotations;
  if (Object.keys(meta).length) out.meta = meta;
  return out;
}

/** The tools as `tools/list` gives them. */
export function listTools(tools) {
  return tools.map((tool) => {
    const entry = { name: tool.name };
    if (tool.title !== undefined) entry.title = tool.title;
    entry.description = tool.description;
    entry.inputSchema = JSON.parse(JSON.stringify(tool.inputSchema ?? { type: "object" }));
    const hints = {};
    for (const [k, v] of Object.entries(tool.annotations ?? {})) if (typeof v === "boolean") hints[k] = v;
    const { annotations, meta } = mapAnnotations(hints);
    if (annotations) entry.annotations = annotations;
    if (meta) entry._meta = { [ANNOTATIONS_META_KEY]: meta };
    return entry;
  });
}

const isPlainObject = (v) => typeof v === "object" && v !== null && !Array.isArray(v);

/** An error result with `message`. */
export function errorResult(message) {
  return { content: [{ type: "text", text: message }], isError: true };
}

/** What a tool returned, as MCP's result: a string one text block, `{content}` as it is, undefined empty, other JSON its text (and `structuredContent` for an object). */
export function toCallToolResult(value) {
  if (typeof value === "string") return { content: [{ type: "text", text: value }] };
  if (value === undefined) return { content: [] };
  if (isPlainObject(value) && Array.isArray(value.content)) return value;
  let text;
  try {
    text = JSON.stringify(value);
  } catch {
    // cyclic or BigInt
  }
  if (text === undefined) return errorResult("The tool returned a value that is not JSON.");
  const result = { content: [{ type: "text", text }] };
  if (isPlainObject(value)) result.structuredContent = value;
  return result;
}

/** A thrown value as an error result. */
function thrownResult(error) {
  const message = error instanceof Error ? error.message : String(error);
  return errorResult(message || "The tool failed.");
}

// --- The arguments checked against the tool's schema: the keywords the
// tools use (type, properties, required, enum, items, the lengths and
// bounds, additionalProperties).

function typeOk(type, v) {
  switch (type) {
    case "object": return isPlainObject(v);
    case "array": return Array.isArray(v);
    case "string": return typeof v === "string";
    case "integer": return Number.isInteger(v);
    case "number": return typeof v === "number" && Number.isFinite(v);
    case "boolean": return typeof v === "boolean";
    case "null": return v === null;
    default: return true;
  }
}

/** The problems of `value` against `schema` at `at` (a JSON pointer), the first few. */
export function validate(schema, value, at = "", out = []) {
  if (!isPlainObject(schema) || out.length >= 5) return out;
  const types = schema.type === undefined ? [] : [schema.type].flat();
  if (types.length && !types.some((t) => typeOk(t, value))) {
    out.push(`${at || "/"} must be ${types.join(" or ")}`);
    return out;
  }
  if (Array.isArray(schema.enum) && !schema.enum.some((e) => JSON.stringify(e) === JSON.stringify(value))) out.push(`${at || "/"} must be one of ${schema.enum.map((e) => JSON.stringify(e)).join(", ")}`);
  if (typeof value === "string") {
    const n = [...value].length;
    if (typeof schema.minLength === "number" && n < schema.minLength) out.push(`${at || "/"} must have at least ${schema.minLength} characters`);
    if (typeof schema.maxLength === "number" && n > schema.maxLength) out.push(`${at || "/"} must have at most ${schema.maxLength} characters`);
  }
  if (typeof value === "number") {
    if (typeof schema.minimum === "number" && value < schema.minimum) out.push(`${at || "/"} must be at least ${schema.minimum}`);
    if (typeof schema.maximum === "number" && value > schema.maximum) out.push(`${at || "/"} must be at most ${schema.maximum}`);
  }
  if (Array.isArray(value)) {
    if (typeof schema.minItems === "number" && value.length < schema.minItems) out.push(`${at || "/"} must have at least ${schema.minItems} items`);
    if (typeof schema.maxItems === "number" && value.length > schema.maxItems) out.push(`${at || "/"} must have at most ${schema.maxItems} items`);
    if (isPlainObject(schema.items)) value.forEach((v, i) => validate(schema.items, v, `${at}/${i}`, out));
  }
  if (isPlainObject(value)) {
    for (const key of schema.required ?? []) if (!Object.hasOwn(value, key)) out.push(`${at || "/"} must have the property ${key}`);
    const props = isPlainObject(schema.properties) ? schema.properties : {};
    for (const [key, v] of Object.entries(value)) {
      if (Object.hasOwn(props, key)) validate(props[key], v, `${at}/${key}`, out);
      else if (schema.additionalProperties === false) out.push(`${at || "/"} must not have the property ${key}`);
      else if (isPlainObject(schema.additionalProperties)) validate(schema.additionalProperties, v, `${at}/${key}`, out);
    }
  }
  return out.slice(0, 5);
}

/** The MCP error of arguments that do not match a tool's schema, as tab-mcp words it; undefined when they match. */
export function invalidArguments(tool, args) {
  const problems = validate(tool.inputSchema ?? { type: "object" }, args);
  if (!problems.length) return undefined;
  return `Input validation error: Invalid arguments for tool ${tool.name}: ${problems.join("; ")}`;
}

/** Hide "Always allow" and its note in the newest open tab-mcp confirmation of `doc`. */
function noAlwaysAllow(doc) {
  const dialogs = doc.querySelectorAll?.("dialog.tab-mcp-confirm") ?? [];
  const d = dialogs[dialogs.length - 1];
  if (!d) return;
  for (const b of d.querySelectorAll("button")) if (b.textContent === "Always allow") b.hidden = true;
  const note = d.querySelector(".tab-mcp-always-note");
  if (note) note.hidden = true;
}

/** Tool call ids, unique in the page. */
let lastCallId = 0;

const copy = (v) => {
  try {
    return structuredClone(v);
  } catch {
    return v;
  }
};

/**
 * The desktop app's AI access. `calculatorView()` gives Calculator's
 * mounted handle (null before its first visit). Events: `change`,
 * `toolcall`, `toolcallend`, `note`.
 */
export class LocalAiAccess extends EventTarget {
  constructor({
    calculatorView,
    tauri = globalThis.__TAURI__,
    confirm = () => import(CONFIRM_MODULE),
    doc = globalThis.document,
    now = () => Date.now(),
    allowDelayMs = undefined,
  } = {}) {
    super();
    this.local = true;
    this.calculatorView = calculatorView;
    this.tauri = tauri;
    this.confirmModule = confirm;
    this.doc = doc;
    this.now = now;
    this.allowDelayMs = allowDelayMs;
    /** The endpoint's state (`mcp_status`); the header only from the commands, never from an event. */
    this.status = { enabled: false, running: false, port: 4842, defaultPort: 4842, address: null, error: null, apps: [] };
    /** What went wrong on this side (a command refused). */
    this.failure = null;
    /** The tools that ask the user first. */
    this.consequential = new Set();
    /** "Always allow": tool names per AI app, until the app reloads or AI access goes off. */
    this.allowed = new Map();
    /** The calls running, by the endpoint's id, with their abort controllers. */
    this.running = new Map();
    this.loading = null;
    this.tools = [];
    this.log = new AiLog(this, { now: () => this.now() });
  }

  get available() { return Boolean(this.tauri?.core); }
  get lost() { return false; }
  get state() { return this.status.running ? "connected" : "disconnected"; }
  get error() { return this.failure ?? this.status.error ?? null; }
  set error(v) { this.failure = v; }

  /** The address, its header and the AI apps seen lately, in the shape the dialog and the sidebar read. */
  get mcp() {
    const s = this.status;
    return {
      address: s.address,
      headers: s.authorization ? { Authorization: s.authorization } : {},
      grants: (s.apps ?? []).map((a) => ({ grantId: a.name, client: { name: a.name, host: "" }, secondsAgo: a.secondsAgo })),
    };
  }

  /** Whether the calculators keep running in a hidden window: a call runs, or an AI app is connected. */
  get keepsRunning() {
    return this.running.size > 0 || keepsRunning(this);
  }

  /** The setting has no "keep on": it is kept by the app. */
  get keepOn() { return this.status.enabled; }
  set keepOn(_) {}

  changed() {
    this.dispatchEvent(new Event("change"));
  }

  note(what, detail = {}) {
    this.dispatchEvent(new CustomEvent("note", { detail: { what, ...detail } }));
  }

  invoke(cmd, args) {
    return this.tauri.core.invoke(cmd, args);
  }

  /** The tools built, the app's events heard, the window attached (once). */
  load() {
    this.loading ??= (async () => {
      if (!this.available) throw new Error("AI access needs the desktop app.");
      this.tools = aiTools({
        calculators: () => this.calculators(),
        target: (id) => this.target(id),
        reference: { index: (m) => this.reference().then((r) => r.index(m)) },
        png: (frame, model) => this.png(frame, model),
      });
      for (const t of this.tools) if (t.annotations?.consequentialHint) this.consequential.add(t.name);
      const listen = this.tauri.event.listen;
      await listen("mcp-call", (e) => { this.onCall(e.payload); });
      await listen("mcp-cancel", (e) => { this.running.get(e.payload?.id)?.abort(); });
      await listen("mcp-status", () => { this.refresh(); });
      const limits = Object.fromEntries(this.tools.map((t) => [t.name, t.annotations?.consequentialHint ? CONFIRM_LIMIT_MS : CALL_LIMIT_MS]));
      await this.invoke("mcp_attach", { instructions: INSTRUCTIONS, limits });
      await this.refresh();
      // The AI apps seen lately go stale after a while: asked again now and then while it runs.
      const timer = setInterval(() => { if (this.status.running) this.refresh(); }, 60_000);
      timer.unref?.();
    })();
    this.loading.catch(() => { this.loading = null; });
    return this.loading;
  }

  /** On load: attach, so AI apps can call as soon as the setting is on. */
  resume() {
    this.load().catch((err) => console.error("saturnus: AI access did not start", err));
  }

  /** The state from the app; notes what changed. */
  async refresh() {
    const before = this.status;
    try {
      this.status = await this.invoke("mcp_status");
    } catch (err) {
      this.failure = `AI access is not available: ${err?.message ?? err}`;
    }
    this.noteChange(before, this.status);
    this.changed();
  }

  /** The log's notes for a change of state. */
  noteChange(before, now) {
    if (!before.running && now.running) this.note("listening", { address: now.address?.replace(/^http:\/\//, "") ?? "" });
    if (before.running && !now.running) {
      this.allowed.clear();
      this.note("off");
    }
    const had = new Set((before.running ? before.apps : []).map((a) => a.name));
    for (const a of now.running ? now.apps : []) if (!had.has(a.name)) this.note("paired", { app: a.name });
  }

  /** Turn AI access on, or off, or move it to `port`. */
  async set(enabled, port = undefined) {
    this.failure = null;
    const before = this.status;
    try {
      this.status = await this.invoke("mcp_set", port === undefined ? { enabled } : { enabled, port });
    } catch (err) {
      this.failure = String(err?.message ?? err);
    }
    this.noteChange(before, this.status);
    this.changed();
    return this.status;
  }

  /**
   * "Add to Claude Desktop": the app opens the extension it carries
   * (saturnus.mcpb, iteration 59), so Claude Desktop shows its install
   * dialog. Resolves to whether it opened; else `error` says why.
   */
  async addToClaudeDesktop() {
    this.failure = null;
    try {
      await this.invoke("mcpb_open");
      return true;
    } catch (err) {
      this.failure = String(err?.message ?? err);
      return false;
    } finally {
      this.changed();
    }
  }

  connect() {
    return this.set(true);
  }

  /** Turn AI access off: the app stops serving at once, the calls running end. */
  disconnect() {
    for (const c of this.running.values()) c.abort();
    this.set(false);
  }

  newAddress() {
    return null;
  }

  revoke() {
    return false;
  }

  startPairing() {}
  stopPairing() {}

  /** Calculator's parts for the tools, or null before it is mounted. */
  async view() {
    const handle = await this.calculatorView?.();
    return handle ? handle.ai() : null;
  }

  async reference() {
    const v = await this.view();
    if (v?.reference) return v.reference;
    this.ownReference ??= import("./palette.js").then(({ ReferenceLoader }) => new ReferenceLoader(new URL("./commands.json", import.meta.url)));
    return this.ownReference;
  }

  /** The desktop app runs one calculator: Calculator's. */
  async calculators() {
    const v = await this.view();
    const shown = v?.store.state.booted ?? null;
    return shown ? [{ id: shown, model: shown, title: titleOf(shown), rom: v.store.state.romName || null, shown: true }] : [];
  }

  async target(id) {
    const v = await this.view();
    const shown = v?.store.state.booted ?? null;
    if (!shown) throw new Error(id ? `No calculator ${id} is running. list_calculators names them.` : "The app's Calculator view shows no calculator. list_calculators names the running ones; or ask the user to open one.");
    if (id && id !== shown) throw new Error(`No calculator ${id} is running. list_calculators names them.`);
    return { id: shown, model: shown, backend: v.backend, store: v.store };
  }

  /** The display as a PNG (base64), as web/ai.js makes it. */
  png(frame, model) {
    return screenPng(frame, model);
  }

  /** A call from the app's endpoint: answered with `mcp_reply`. */
  async onCall(msg) {
    const id = msg?.id;
    let reply;
    try {
      if (msg.method === "tools/list") reply = { result: { tools: listTools(this.tools) } };
      else if (msg.method === "tools/call") reply = { result: await this.call(id, msg.params ?? {}, msg.client, msg.clientStable === true) };
      else reply = { error: `Method ${msg.method} is not served by the app.` };
    } catch (err) {
      reply = { error: String(err?.message ?? err) };
    }
    try {
      await this.invoke("mcp_reply", { id, result: reply.result ?? null, error: reply.error ?? null });
    } catch {
      // The call ended meanwhile (time limit, turned off): nobody waits.
    }
  }

  /** One tool call: checked, confirmed where the tool asks, run, logged. */
  async call(id, params, clientName, stable = false) {
    const tool = this.tools.find((t) => t.name === params.name);
    if (typeof params.name !== "string" || !tool) return errorResult(`Tool ${params.name} not found`);
    const raw = params.arguments ?? {};
    if (!isPlainObject(raw)) return errorResult("Invalid params: arguments must be an object");
    const name = clientName || "AI app";
    // A name only guessed (User-Agent, the default) is no key for "Always allow": its grant is its own.
    const client = { grantId: stable ? name : `?${name}`, id: "local", name, host: HERE, stable };
    const controller = new AbortController();
    this.running.set(id, controller);
    this.changed();
    const call = { id: ++lastCallId, tool: tool.name, arguments: copy(raw), client: { ...client } };
    if (tool.title !== undefined) call.title = tool.title;
    const started = this.now();
    this.dispatchEvent(new CustomEvent("toolcall", { detail: call }));
    let outcome;
    let result;
    try {
      [outcome, result] = await this.decideAndRun(tool, raw, controller.signal, client);
    } finally {
      this.running.delete(id);
      this.changed();
    }
    this.dispatchEvent(new CustomEvent("toolcallend", { detail: { id: call.id, tool: tool.name, client: { ...client }, outcome, result: copy(result), durationMs: this.now() - started } }));
    return result;
  }

  async decideAndRun(tool, raw, signal, client) {
    const invalid = invalidArguments(tool, raw);
    if (invalid !== undefined) return ["error", errorResult(invalid)];
    if (tool.annotations?.consequentialHint === true) {
      let allowed = false;
      try {
        allowed = await this.confirm(tool, raw, signal, client);
      } catch {
        allowed = false;
      }
      if (!allowed) return [signal.aborted ? "cancelled" : "declined", errorResult("The user declined")];
    }
    if (signal.aborted) return ["cancelled", errorResult("Cancelled")];
    let res;
    try {
      res = toCallToolResult(await tool.execute(raw, { signal, client }));
    } catch (err) {
      res = thrownResult(err);
    }
    if (signal.aborted) return ["cancelled", res];
    return [res.isError === true ? "error" : "ok", res];
  }

  /**
   * Ask the user in tab-mcp's confirmation; "Always allow" remembered for
   * this AI app and tool. Only for an AI app that named itself (`stable`):
   * a name guessed from its User-Agent may be another app's, so it is
   * asked every time and the dialog offers no "Always allow".
   */
  async confirm(tool, args, signal, client) {
    if (client.stable && this.allowed.get(client.grantId)?.has(tool.name)) return true;
    if (!this.doc) return false;
    const { confirmDialog } = await this.confirmModule();
    const question = { tool: { name: tool.name, description: tool.description }, arguments: args, client, signal };
    if (tool.title !== undefined) question.tool.title = tool.title;
    const asked = confirmDialog(question, this.doc, this.allowDelayMs === undefined ? {} : { allowDelayMs: this.allowDelayMs });
    // The dialog is in the document once it is asked: without a stable name, its Always allow goes.
    if (!client.stable) noAlwaysAllow(this.doc);
    const answer = await asked;
    if (answer === "always" && client.stable && this.status.running) {
      const set = this.allowed.get(client.grantId) ?? new Set();
      set.add(tool.name);
      this.allowed.set(client.grantId, set);
    }
    return answer !== "deny";
  }
}

