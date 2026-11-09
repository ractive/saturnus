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
// The tools offered: `list_calculators` and `lookup_command`, those of
// the shown calculator's family (`familyOf`; `update` when its model
// changes) and those of the families the last `list_calculators` call
// named (a 48GX the app runs but does not show), so a tool the listing
// names is there when the agent reads it. Each tool is registered once
// and withdrawn alone when no family wants it any more, so a change
// registers only what is new. A tool is withdrawn by aborting the
// `AbortSignal` it was registered with (the draft has no
// `unregisterTool`); a registration the browser refuses is tried again
// at the next change. All withdrawn when the page is hidden
// for good or put in the back/forward cache (`pagehide`), registered
// again on `pageshow`.
//
// Around each call: the arguments checked against the tool's schema
// (WebMCP does not check them), as web/ai-local.js does for the desktop
// app, and the `toolcall`/`toolcallend` events on the AI access that
// feed "The AI's calls" (web/ai-log.js). No confirmation in the page
// (owner, 2026-11-09): the browser or its agent asks before a call (Brave's
// Leo before every one), so a consequential tool (`consequentialHint`:
// store_variable, purge_variable) keeps the hint for the browser and runs
// without the in-app question tab-mcp's calls and the desktop app's get.
// WebMCP turns a tool's result into JSON for the agent and a rejected
// `execute` into an error without its message, so a failure comes back
// as `{error}`. The
// display comes as text unless the call asks for an image (`display` or
// `format` "image" or "both"; then the `image` data URL): a PNG in base64
// costs a text-only agent its context.

import { APPROVAL, APPROVAL_NOTE, FAMILY_TOOLS, familyOf, INSTRUCTIONS } from "./ai-tools.js";
import { invalidArguments, toCallToolResult } from "./ai-local.js";

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

/** Who calls through WebMCP, for the log ("Your browser's AI") and the tools. */
export const WEBMCP_CLIENT = Object.freeze({ grantId: "webmcp", id: "webmcp", name: "Your browser's AI", host: "WebMCP" });

/** The tools registered with no calculator shown. */
const NONE = ["list_calculators", "lookup_command"];

/** What `list_calculators` tells the agent of WebMCP's own ways, before the instructions. */
export const WEBMCP_NOTE = "Here (WebMCP) the tools of a calculator's family are offered while it is shown or once list_calculators has named it: call list_calculators first, and again when the user opens another calculator. The display comes as text unless you ask for an image (display or format \"image\" or \"both\": a PNG as a data: URL).";

/** What WebMCP's agent reads in place of APPROVAL_NOTE: the app asks nothing here, so the agent confirms. */
export const WEBMCP_APPROVAL_NOTE = "store_variable and purge_variable change the calculator's memory, and the app does not ask the user about them here: confirm with the user before calling them. press_keys and run_rpl can store and purge too, so take the same care as at the user's keyboard.";

/** What WebMCP's agent reads in place of APPROVAL in store_variable's and purge_variable's descriptions. */
export const WEBMCP_APPROVAL = "It changes the calculator's memory, and the app does not ask the user: confirm with the user first.";

/** `text` (a description or the instructions) as WebMCP's agent reads it: no promise that the app asks. */
export function webMcpText(text) {
  return String(text ?? "").replace(APPROVAL_NOTE, WEBMCP_APPROVAL_NOTE).replace(APPROVAL, WEBMCP_APPROVAL);
}

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
 * `ModelContext`).
 */
export class WebMcp {
  constructor({ ai, context = modelContextOf(), win = globalThis.window, now = () => Date.now() } = {}) {
    this.ai = ai;
    this.context = context;
    this.win = win;
    this.now = now;
    /** The tools registered (or being registered) now, by name, each with the controller that withdraws it. */
    this.registered = new Map();
    /** The shown calculator's family, "none" with none shown; undefined before `start`. */
    this.family = undefined;
    /** The families the last `list_calculators` call named. */
    this.listed = new Set();
    /** The registration changes, one after the other. */
    this.syncing = Promise.resolve();
    /** Whether an agent has called a tool in this page (the sidebar offers "Calls" then). */
    this.called = false;
    /** While the page is hidden (`pagehide`). */
    this.hidden = false;
  }

  /** Whether tools are registered now. */
  get active() {
    return this.registered.size > 0;
  }

  /** The names of the tools registered now, in the tools' order. */
  get names() {
    return this.ai.tools.map((t) => t.name).filter((n) => this.registered.has(n));
  }

  /** Register for the calculator Calculator shows now; withdraw on page hide, again on show. */
  async start() {
    if (!this.context) return;
    this.win?.addEventListener?.("pagehide", () => {
      this.hidden = true;
      this.withdraw();
    });
    this.win?.addEventListener?.("pageshow", () => {
      if (!this.hidden) return;
      this.hidden = false;
      this.sync();
    });
    const v = await this.ai.view().catch(() => null);
    await this.update(v?.store?.state?.booted ?? null);
  }

  /** The calculator shown is now `model` (null: none): its family's tools registered, those no family wants withdrawn. */
  update(model) {
    if (!this.context) return Promise.resolve();
    this.family = model ? familyOf(model).family : "none";
    return this.sync();
  }

  /** The families of the calculators `list_calculators` named (its result's `family`): their tools registered. */
  listing(families) {
    this.listed = new Set(families.filter((f) => Object.hasOwn(FAMILY_TOOLS, f)));
    return this.sync();
  }

  /** The tool names wanted now. */
  wanted() {
    const names = new Set(NONE);
    for (const family of [this.family, ...this.listed]) for (const name of FAMILY_TOOLS[family] ?? []) names.add(name);
    return names;
  }

  /** The registrations brought in line with `wanted()`, after the changes before; never rejects. */
  sync() {
    if (!this.context) return Promise.resolve();
    const run = this.syncing.then(() => this.syncNow()).catch((err) => console.warn("saturnus: WebMCP registration failed", err));
    this.syncing = run;
    return run;
  }

  async syncNow() {
    if (this.hidden || this.family === undefined) return;
    const want = this.wanted();
    let changed = false;
    for (const [name, controller] of this.registered) {
      if (want.has(name)) continue;
      controller.abort();
      this.registered.delete(name);
      changed = true;
    }
    const adding = this.ai.tools.filter((t) => want.has(t.name) && !this.registered.has(t.name));
    const controllers = adding.map((t) => {
      const controller = new AbortController();
      this.registered.set(t.name, controller);
      return controller;
    });
    const done = await Promise.allSettled(adding.map((t, i) => this.registerOne(this.wrap(t), controllers[i].signal)));
    for (const [i, r] of done.entries()) {
      if (r.status === "fulfilled") continue;
      // Refused: not registered, tried again at the next change. Withdrawn meanwhile (the abort's reason) is no failure.
      if (this.registered.get(adding[i].name) === controllers[i]) this.registered.delete(adding[i].name);
      if (!controllers[i].signal.aborted) console.warn(`saturnus: WebMCP did not take the tool ${adding[i].name}`, r.reason);
    }
    if (changed || adding.length) this.ai.changed?.();
  }

  /** One registration, withdrawn when `signal` aborts; a `registerTool` that throws rejects. */
  async registerOne(tool, signal) {
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
    await context.registerTool(tool, { signal });
  }

  /** Every tool withdrawn (the calls running go on to their end). */
  withdraw() {
    for (const controller of this.registered.values()) controller.abort();
    const had = this.registered.size > 0;
    this.registered.clear();
    if (had) this.ai.changed?.();
  }

  /** `tool` as WebMCP takes it: its own fields, and an `execute` that checks, runs and logs. */
  wrap(tool) {
    const out = { name: tool.name, title: tool.title, description: webMcpText(tool.description), inputSchema: textByDefaultSchema(tool.inputSchema), annotations: webMcpAnnotations(tool.annotations), execute: (input, options) => this.call(tool, input, options?.signal) };
    // WebMCP has no server instructions: the agent reads them with the first tool it should call.
    if (tool.name === "list_calculators") out.description = `${out.description}\n\n${WEBMCP_NOTE}\n\nHow to use these tools:\n${webMcpText(INSTRUCTIONS)}`;
    if (tool.outputSchema) out.outputSchema = tool.outputSchema;
    return out;
  }

  /** One call from the agent: logged, checked, run with no question in the page; resolves to what WebMCP gives the agent. */
  async call(tool, input, signal = new AbortController().signal) {
    const raw = input && typeof input === "object" ? input : {};
    const call = { id: `webmcp-${++lastCallId}`, tool: tool.name, title: tool.title, arguments: copy(raw), client: { ...WEBMCP_CLIENT }, asks: false };
    const started = this.now();
    if (!this.called) {
      this.called = true;
      this.ai.changed?.();
    }
    this.ai.dispatchEvent(new CustomEvent("toolcall", { detail: call }));
    const [outcome, result] = await this.run(tool, raw, signal);
    this.ai.dispatchEvent(new CustomEvent("toolcallend", { detail: { id: call.id, tool: tool.name, client: { ...WEBMCP_CLIENT }, outcome, result: copy(result), durationMs: this.now() - started } }));
    if (result.isError) return { error: result.content?.find((c) => c?.type === "text")?.text ?? "The tool failed." };
    const out = webMcpResult(result);
    // The tools of every calculator listed, registered before the agent reads the list.
    if (tool.name === "list_calculators" && Array.isArray(out?.calculators)) await this.listing(out.calculators.map((c) => c?.family));
    return out;
  }

  /** `[outcome, MCP result]` of a call (the log's form; `call` makes WebMCP's of it). */
  async run(tool, raw, signal) {
    const failed = (message) => ({ content: [{ type: "text", text: message }], isError: true });
    const invalid = invalidArguments(tool, raw);
    if (invalid !== undefined) return ["error", failed(invalid)];
    if (signal.aborted) return ["cancelled", failed("Cancelled")];
    let res;
    try {
      res = toCallToolResult(await tool.execute(textByDefault(tool, raw), { signal, client: { ...WEBMCP_CLIENT } }));
    } catch (err) {
      res = failed((err instanceof Error ? err.message : String(err)) || "The tool failed.");
    }
    if (signal.aborted) return ["cancelled", res];
    return [res.isError === true ? "error" : "ok", res];
  }
}

/** A tool's annotations as WebMCP's `ToolAnnotations` has them (MCP-only hints such as `destructiveHint` left out). */
export function webMcpAnnotations(a = {}) {
  const out = {};
  for (const key of ["readOnlyHint", "untrustedContentHint", "consequentialHint", "debugging"]) if (typeof a?.[key] === "boolean") out[key] = a[key];
  return out;
}

/** The argument of input schema `schema` that says whether the result carries the display as an image ("display", "format"), or null. */
function imageArgument(schema) {
  const props = schema?.properties ?? {};
  return ["display", "format"].find((k) => props[k]?.enum?.includes("image")) ?? null;
}

/**
 * `raw` with the display as text where the call names nothing: a PNG in
 * base64 is no use to a text-only agent and costs its context (tab-mcp's
 * clients get an MCP image block and keep the tools' own defaults).
 */
export function textByDefault(tool, raw) {
  const key = imageArgument(tool.inputSchema);
  return key && raw[key] === undefined ? { ...raw, [key]: "text" } : raw;
}

/** `schema` as WebMCP registers it: the display argument says its default is text. */
export function textByDefaultSchema(schema) {
  const key = imageArgument(schema);
  if (!key) return schema;
  const prop = schema.properties[key];
  const description = `${String(prop.description ?? "").replace(/\s*Default: [^.]*\.$/, "")} Default: text; "image" or "both" adds the display as a PNG (a data: URL).`.trim();
  return { ...schema, properties: { ...schema.properties, [key]: { ...prop, description } } };
}
