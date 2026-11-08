// The calculators' tools through WebMCP (iteration 71): where the browser
// has WebMCP (`document.modelContext`, W3C Web Machine Learning CG draft;
// Chrome 149 and Edge 150 origin trials, the tokens in index.html), the
// page registers the tools AI access gives AI apps (web/ai-tools.js, the
// same objects web/ai.js gives tab-mcp, so their calls queue per
// calculator together) with the browser, for its own AI agent: built into
// the browser or added to it as an extension. No relay, no address, no
// switch (owner, 2026-11-08): registered whenever the browser has WebMCP.
// Only that agent and same-origin script reach them: the permissions
// policy feature `tools` (default allowlist 'self') keeps cross-origin
// frames out, and no tool names another origin in `exposedTo`.
//
// The tools of the calculator Calculator shows: its family's
// (`familyOf`), `list_calculators` and `lookup_command` with none shown;
// registered again when its model changes the family (`update`),
// withdrawn when the page is hidden for good or put in the back/forward
// cache (`pagehide`), registered again on `pageshow`. A tool is withdrawn
// by aborting the `AbortSignal` it was registered with (the draft has no
// `unregisterTool`).
//
// Around each call, as tab-mcp's client does for tab-mcp (and
// web/ai-local.js for the desktop app): the arguments checked against
// the tool's schema (WebMCP does not check them), a consequential tool
// (`consequentialHint`: store_variable, purge_variable) confirmed in
// tab-mcp's own dialog (its vendored `confirmInPage`, so it looks the
// same and "The AI's calls" sees it), no "Always allow" (the agent does
// not say who it is), and the `toolcall`/`toolcallend` events on the AI
// access that feed "The AI's calls" (web/ai-log.js). WebMCP turns a
// tool's result into JSON for the agent and a rejected `execute` into an
// error without its message, so a failure comes back as `{error}`; a
// display image as the `image` data URL.

import { FAMILY_TOOLS, familyOf, INSTRUCTIONS } from "./ai-tools.js";
import { CONFIRM_MODULE, ConfirmHost, invalidArguments, toCallToolResult } from "./ai-local.js";

/**
 * The page's WebMCP, or null where the browser has none.
 * `navigator.modelContext` is the name Chrome's early preview builds
 * used (before the draft moved it to the document, and before the 149
 * origin trial; Chrome 156 has only `document.modelContext`): the
 * fallback can go once no Chrome before 149 is in use.
 */
export function modelContextOf(doc = globalThis.document, nav = globalThis.navigator) {
  const context = doc?.modelContext ?? nav?.modelContext ?? null;
  return context && typeof context.registerTool === "function" ? context : null;
}

/** Who calls through WebMCP, for the confirmation ("Your browser's AI (WebMCP) wants to run …") and the log. */
export const WEBMCP_CLIENT = Object.freeze({ grantId: "webmcp", id: "webmcp", name: "Your browser's AI", host: "WebMCP" });

/** The tools registered with no calculator shown. */
const NONE = ["list_calculators", "lookup_command"];

/** Tool call ids of WebMCP's calls, apart from tab-mcp's (its own) and the desktop app's. */
let lastCallId = 0;

const copy = (v) => {
  try {
    return structuredClone(v);
  } catch {
    return v;
  }
};

/**
 * What WebMCP gives the agent of a tool's value: its fields, the display
 * image (an MCP `content` image block) as the `image` data URL.
 */
export function webMcpResult(value) {
  if (!value || typeof value !== "object" || !Array.isArray(value.content)) return value;
  const out = { ...(value.structuredContent ?? {}) };
  const image = value.content.find((c) => c?.type === "image");
  if (image) out.image = `data:${image.mimeType ?? "image/png"};base64,${image.data}`;
  if (!value.structuredContent) {
    const text = value.content.filter((c) => c?.type === "text").map((c) => c.text).join("\n");
    if (text) out.text = text;
  }
  return out;
}

/**
 * The tools of AI access `ai` (web/ai.js `AiAccess`: its `tools`, its
 * `view()`, its events) registered with `context` (a WebMCP
 * `ModelContext`). `confirm` loads tab-mcp's confirmation (tests give a
 * fake); `allowDelayMs` its delay before Allow works.
 */
export class WebMcp {
  constructor({ ai, context = modelContextOf(), doc = globalThis.document, win = globalThis.window, confirm = () => import(CONFIRM_MODULE), now = () => Date.now(), allowDelayMs = undefined } = {}) {
    this.ai = ai;
    this.context = context;
    this.doc = doc;
    this.win = win;
    this.confirmModule = confirm;
    this.now = now;
    this.allowDelayMs = allowDelayMs;
    /** No "Always allow", so it never holds a grant; confirmInPage wants a host. */
    this.confirmHost = new ConfirmHost();
    /** The registrations' controller: aborting it withdraws them. */
    this.controller = null;
    /** The tool names registered now (null: none). */
    this.names = null;
    /** The family registered, or "none" with no calculator shown; undefined before `start`. */
    this.family = undefined;
    /** Whether an agent has called a tool in this page (the sidebar offers "Calls" then). */
    this.called = false;
    /** While the page is hidden (`pagehide`), and the family to register when it shows again. */
    this.hidden = false;
    this.later = undefined;
  }

  /** Whether the tools are registered now. */
  get active() {
    return this.names !== null;
  }

  /** Register for the calculator Calculator shows now; withdraw on page hide, again on show. */
  async start() {
    if (!this.context) return;
    this.win?.addEventListener?.("pagehide", () => {
      this.later = this.family;
      this.hidden = true;
      this.withdraw();
    });
    this.win?.addEventListener?.("pageshow", () => {
      if (!this.hidden) return;
      this.hidden = false;
      if (this.later !== undefined) this.register(this.later);
    });
    const v = await this.ai.view().catch(() => null);
    await this.update(v?.store?.state?.booted ?? null);
  }

  /** The calculator shown is now `model` (null: none): registered again if its tool family differs. */
  update(model) {
    if (!this.context) return Promise.resolve();
    const family = model ? familyOf(model).family : "none";
    // Hidden: registered on show.
    if (this.hidden) {
      this.later = family;
      return Promise.resolve();
    }
    return this.register(family);
  }

  /** The tools of `family` ("rpl", "rpl-keys", "keys", "none") registered, those before withdrawn. */
  async register(family) {
    if (family === this.family) return;
    this.withdraw();
    this.family = family;
    const names = family === "none" ? NONE : ["list_calculators", ...FAMILY_TOOLS[family]];
    const controller = new AbortController();
    this.controller = controller;
    this.names = names;
    const tools = this.ai.tools.filter((t) => names.includes(t.name));
    const done = await Promise.allSettled(tools.map((t) => this.registerOne(this.wrap(t), controller.signal)));
    for (const [i, r] of done.entries()) {
      // Withdrawn meanwhile (the abort's reason) is no failure.
      if (r.status === "rejected" && !controller.signal.aborted) console.warn(`saturnus: WebMCP did not take the tool ${tools[i].name}`, r.reason);
    }
    this.ai.changed?.();
  }

  /** One registration, withdrawn when `signal` aborts. */
  registerOne(tool, signal) {
    const context = this.context;
    // The early `navigator.modelContext` took no signal: it had `unregisterTool(name)`.
    if (typeof context.unregisterTool === "function") {
      signal.addEventListener("abort", () => {
        try {
          context.unregisterTool(tool.name);
        } catch {
          // Gone already.
        }
      }, { once: true });
    }
    return Promise.resolve(context.registerTool(tool, { signal }));
  }

  /** Every tool withdrawn (the calls running go on to their end). */
  withdraw() {
    this.controller?.abort();
    this.controller = null;
    this.names = null;
    this.family = undefined;
  }

  /** `tool` as WebMCP takes it: its own fields, and an `execute` that checks, confirms, runs and logs. */
  wrap(tool) {
    const out = { name: tool.name, title: tool.title, description: tool.description, inputSchema: tool.inputSchema, annotations: webMcpAnnotations(tool.annotations), execute: (input, options) => this.call(tool, input, options?.signal) };
    // WebMCP has no server instructions: the agent reads them with the first tool it should call.
    if (tool.name === "list_calculators") out.description = `${tool.description}\n\nHow to use these tools:\n${INSTRUCTIONS}`;
    if (tool.outputSchema) out.outputSchema = tool.outputSchema;
    return out;
  }

  /** One call from the agent: logged, checked, confirmed where the tool asks, run; resolves to what WebMCP gives the agent. */
  async call(tool, input, signal = new AbortController().signal) {
    const raw = input && typeof input === "object" ? input : {};
    const call = { id: `webmcp-${++lastCallId}`, tool: tool.name, title: tool.title, arguments: copy(raw), client: { ...WEBMCP_CLIENT } };
    const started = this.now();
    if (!this.called) {
      this.called = true;
      this.ai.changed?.();
    }
    this.ai.dispatchEvent(new CustomEvent("toolcall", { detail: call }));
    const [outcome, result] = await this.decideAndRun(tool, raw, signal);
    this.ai.dispatchEvent(new CustomEvent("toolcallend", { detail: { id: call.id, tool: tool.name, client: { ...WEBMCP_CLIENT }, outcome, result: copy(result), durationMs: this.now() - started } }));
    if (result.isError) return { error: result.content?.find((c) => c?.type === "text")?.text ?? "The tool failed." };
    return webMcpResult(result);
  }

  /** `[outcome, MCP result]` of a call (the log's form; `call` makes WebMCP's of it). */
  async decideAndRun(tool, raw, signal) {
    const failed = (message) => ({ content: [{ type: "text", text: message }], isError: true });
    const invalid = invalidArguments(tool, raw);
    if (invalid !== undefined) return ["error", failed(invalid)];
    if (tool.annotations?.consequentialHint === true) {
      let allowed = false;
      try {
        allowed = await this.confirm(tool, raw, signal);
      } catch {
        allowed = false;
      }
      if (!allowed) return [signal.aborted ? "cancelled" : "declined", failed("The user declined.")];
    }
    if (signal.aborted) return ["cancelled", failed("Cancelled")];
    let res;
    try {
      res = toCallToolResult(await tool.execute(raw, { signal, client: { ...WEBMCP_CLIENT } }));
    } catch (err) {
      res = failed((err instanceof Error ? err.message : String(err)) || "The tool failed.");
    }
    if (signal.aborted) return ["cancelled", res];
    return [res.isError === true ? "error" : "ok", res];
  }

  /** Ask the user in tab-mcp's confirmation, every time (no "Always allow"). */
  async confirm(tool, args, signal) {
    if (!this.doc) return false;
    const { confirmInPage } = await this.confirmModule();
    const question = { tool: { name: tool.name, title: tool.title, description: tool.description }, arguments: args, client: { ...WEBMCP_CLIENT }, signal };
    const options = { alwaysAllow: false };
    if (this.allowDelayMs !== undefined) options.allowDelayMs = this.allowDelayMs;
    return confirmInPage(this.confirmHost, question, this.doc, options);
  }
}

/** A tool's annotations as WebMCP's `ToolAnnotations` has them (MCP-only hints such as `destructiveHint` left out). */
export function webMcpAnnotations(a = {}) {
  const out = {};
  for (const key of ["readOnlyHint", "untrustedContentHint", "consequentialHint", "debugging"]) if (typeof a?.[key] === "boolean") out[key] = a[key];
  return out;
}
