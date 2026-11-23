// The AI app's tools (web/ai-tools.js, iteration 55) over fake
// calculators: what each tool sends to the protocol, what it returns, its
// refusals, and which tools the page confirms.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { aiTools, familyOf, GUIDE_TOPICS, guideLoader, INSTRUCTIONS, pixelRows } from "../ai-tools.js";
import { buildIndex } from "../reference.js";
import { validate } from "../ai-local.js";

const data = JSON.parse(readFileSync(new URL("../commands.json", import.meta.url), "utf8"));
const reference = { index: async (m) => buildIndex(data, m) };
const PACE = { first: 0, poll: 0, limit: 200 };
/** read_guide's text, web/guide.json (scripts/guide-json.py). */
const GUIDE = JSON.parse(readFileSync(new URL("../guide.json", import.meta.url), "utf8"));

/** A 131×64 frame, all light. */
const FRAME = { width: 131, height: 64, pixels: Buffer.alloc(17 * 64).toString("base64"), annunciators: { leftshift: false, rightshift: false, alpha: false } };

/** A fake calculator of `model`: records the commands, answers from `answers`. */
function fake(model, answers = {}) {
  const calls = [];
  const backend = new Proxy({}, {
    get(_, cmd) {
      return (...args) => {
        calls.push([cmd, ...args]);
        if (cmd === "skin") return Promise.resolve({ keys: [{ name: "1", label: "1" }, { name: "enter", label: "ENTER" }, { name: "plus", label: "+" }, { name: "leftshift", label: "" }, { name: "sin", label: "SIN", left: "ASIN" }, { name: "on", label: "EXIT" }] });
        if (cmd === "stats") return Promise.resolve(typeof answers.stats === "function" ? answers.stats() : answers.stats ?? { loop: "sleep" });
        if (cmd === "typeKeys") return undefined;
        const a = answers[cmd];
        if (a instanceof Error) return Promise.reject(a);
        return Promise.resolve(typeof a === "function" ? a(...args) : a ?? null);
      };
    },
  });
  return { id: model, model, backend, store: { state: { frame: FRAME, keysDown: [], busy: false } }, calls };
}

function tools(targets, { shown = Object.keys(targets)[0], png = async () => "PNG", existing } = {}) {
  const list = Object.values(targets).map((t) => ({ id: t.id, model: t.model, title: t.model, rom: "rom", shown: t.id === shown }));
  const all = aiTools({
    calculators: async () => list,
    target: async (id) => {
      const t = targets[id ?? shown];
      if (!t) throw new Error(`No calculator ${id}`);
      return t;
    },
    existing: existing ?? (async (id) => targets[id] ?? null),
    reference,
    png,
    guide: async () => GUIDE,
    pace: PACE,
  });
  return Object.fromEntries(all.map((t) => [t.name, t]));
}

const call = (tool, input = {}) => tool.execute(input, { signal: new AbortController().signal, client: { grantId: "g", id: "x", host: "x" } });

test("the tools, their schemas and which the page confirms", () => {
  const t = tools({ "48gx": fake("48gx") });
  for (const tool of Object.values(t)) {
    assert.equal(tool.inputSchema.type, "object", tool.name);
    assert.ok(tool.description.length > 20, tool.name);
    assert.match(tool.name, /^[a-z_]+$/);
  }
  const consequential = Object.values(t).filter((x) => x.annotations?.consequentialHint).map((x) => x.name);
  assert.deepEqual(consequential, ["store_variable", "purge_variable"]);
  assert.equal(t.purge_variable.annotations.destructiveHint, true);
  for (const name of ["list_calculators", "list_keys", "read_display", "read_stack", "list_variables", "read_variable", "lookup_command", "read_guide"]) {
    assert.equal(t[name].annotations?.readOnlyHint, true, name);
  }
  for (const name of ["press_keys", "run_rpl", "run_function"]) assert.ok(!t[name].annotations?.consequentialHint, name);
  assert.match(INSTRUCTIONS, /read the display/);
  assert.deepEqual(familyOf("42s").family, "keys");
  assert.deepEqual(familyOf("28s").family, "rpl-keys");
});

test("read_guide: each topic is the plugin's file (SKILL.md without its frontmatter); an unknown topic names the topics", async () => {
  const t = tools({ "48gx": fake("48gx") });
  const skill = new URL("../../plugins/saturnus/skills/saturnus/", import.meta.url);
  // Links between the files become read_guide calls (scripts/guide-json.py topic_links).
  const file = (path) => readFileSync(new URL(path, skill), "utf8").replace(/\[[^\]]*\]\((?:references\/)?([a-z]+)\.md(?:#[^)]*)?\)/g, 'read_guide("$1")');
  const want = {
    skill: file("SKILL.md").replace(/^---\n[\s\S]*?\n---\n+/, ""),
    rpl: file("references/rpl.md"),
    models: file("references/models.md"),
    workflows: file("references/workflows.md"),
  };
  assert.deepEqual([...GUIDE_TOPICS], Object.keys(want));
  for (const topic of GUIDE_TOPICS) {
    const r = await call(t.read_guide, { topic });
    // A difference here: web/guide.json is stale, run `just guide`.
    assert.deepEqual(r, { topic, text: want[topic] }, topic);
  }
  for (const topic of GUIDE_TOPICS) assert.doesNotMatch(want[topic], /\]\((?!https?:)[^)]*\.md/, `${topic}: a link read_guide cannot follow`);
  assert.match(want.rpl, /in read_guide\("workflows"\)\.\n$/);
  assert.match(want.skill, /^# Working on Saturnus's calculators\n/);
  assert.doesNotMatch(want.skill, /^---|^name: saturnus/m);
  assert.deepEqual(t.read_guide.inputSchema.properties.topic.enum, [...GUIDE_TOPICS]);
  assert.deepEqual(t.read_guide.inputSchema.required, ["topic"]);
  for (const topic of [undefined, "", "nope", "SKILL.md"]) {
    await assert.rejects(call(t.read_guide, { topic }), /^Error: There is no guide topic .*\. The topics: skill, rpl, models, workflows\.$/, String(topic));
  }
  // The description says when to read which topic; the instructions point at it.
  for (const topic of GUIDE_TOPICS) assert.match(t.read_guide.description, new RegExp(`"${topic}"`));
  assert.match(INSTRUCTIONS, /read_guide .*"rpl" topic before you write an RPL program, and "models" before you move work/);
  // Without the guide (no `guide` given): an error, not a crash.
  const bare = aiTools({ calculators: async () => [], target: async () => null, reference });
  await assert.rejects(call(bare.find((x) => x.name === "read_guide"), { topic: "rpl" }), /The guide is not available here/);
});

test("no read_guide topic says the app asks the user: read_guide serves the same text through WebMCP, where it does not", () => {
  for (const topic of GUIDE_TOPICS) assert.doesNotMatch(GUIDE.topics[topic], /approves|in the app first|allows? (it|them) in the app|wait until the user allows/i, topic);
});

test("no tool's description says who asks (one list for every caller); the display's default says both callers'", () => {
  const t = tools({ "48gx": fake("48gx") });
  for (const tool of Object.values(t)) assert.doesNotMatch(tool.description, /approve|in the app first|may ask/i, tool.name);
  for (const [name, key, fallback] of [["read_display", "format", "both"], ["press_keys", "display", "both"], ["run_rpl", "display", "text"], ["to_stack", "display", "text"], ["run_function", "display", "both"]]) {
    assert.match(t[name].inputSchema.properties[key].description, new RegExp(`Default: ${fallback}; through WebMCP \\(an agent in the browser\\) text, where "image" or "both" adds the PNG as a data: URL\\.$`), name);
  }
});

test("guideLoader: web/guide.json fetched once, again after a failure", async () => {
  const fetchWas = globalThis.fetch;
  const urls = [];
  let fail = true;
  globalThis.fetch = async (url) => {
    urls.push(String(url));
    if (fail) return { ok: false, status: 404 };
    return { ok: true, json: async () => GUIDE };
  };
  try {
    const load = guideLoader();
    await assert.rejects(load(), /HTTP 404/);
    fail = false;
    assert.equal(await load(), GUIDE);
    assert.equal(await load(), GUIDE);
    assert.equal(urls.length, 2);
    assert.match(urls[0], /\/web\/guide\.json$/);
  } finally {
    globalThis.fetch = fetchWas;
  }
});

test("list_calculators: each with its family's tools; none running says what to do", async () => {
  const t = tools({ "48gx": fake("48gx"), "42s": fake("42s") });
  const r = await call(t.list_calculators);
  assert.deepEqual(r.calculators.map((c) => [c.id, c.shown, c.family]), [["48gx", true, "rpl"], ["42s", false, "keys"]]);
  assert.ok(r.calculators[1].tools.includes("run_function"));
  const none = aiTools({ calculators: async () => [], target: async () => null, reference });
  assert.match((await call(none[0])).note, /No calculator is running/);
});

test("the 49G's mode: list_calculators, read_stack and read_display; left out where it cannot be read and on other models", async () => {
  const g = fake("49g", { flags: { set: [-95] }, commandLine: { active: false }, stack: [{ type: "real", value: 1, text: "1" }] });
  const gx = fake("48gx", { flags: { set: [] }, commandLine: { active: false }, stack: [] });
  const t = tools({ "49g": g, "48gx": gx });
  const list = (await call(t.list_calculators)).calculators;
  assert.equal(list[0].mode, "algebraic");
  assert.ok(!("mode" in list[1]), "no mode on the 48GX");
  assert.ok(!gx.calls.some(([cmd]) => cmd === "flags"), "the 48GX's flags not read");
  assert.equal((await call(t.read_stack)).mode, "algebraic");
  assert.equal((await call(t.read_display, { format: "text" })).mode, "algebraic");
  assert.ok(!("mode" in (await call(t.read_stack, { calculator: "48gx" }))));
  assert.ok(!("mode" in (await call(t.read_display, { calculator: "48gx", format: "text" }))));
  assert.ok(!g.calls.some(([cmd]) => cmd === "typeKeys"), "no key pressed");
  // RPN mode.
  const rpn = fake("49g", { flags: { set: [-3] }, commandLine: { active: false }, stack: [] });
  assert.equal((await call(tools({ "49g": rpn }).list_calculators)).calculators[0].mode, "rpn");
  // Still starting, or the flags not readable: no mode rather than a guess.
  const starting = fake("49g", { flags: { set: [-95] }, stack: [] });
  starting.store.state.starting = true;
  assert.ok(!("mode" in (await call(tools({ "49g": starting }).list_calculators)).calculators[0]));
  assert.ok(!("mode" in (await call(tools({ "49g": starting }).read_stack))));
  const unreadable = fake("49g", { flags: new Error("not ready"), commandLine: { active: false }, stack: [] });
  assert.ok(!("mode" in (await call(tools({ "49g": unreadable }).list_calculators)).calculators[0]));
  assert.ok(!("mode" in (await call(tools({ "49g": unreadable }).read_stack))));
});

test("list_calculators never attaches: no mode for a 49G without a target yet", async () => {
  let attaches = 0;
  const bg = fake("49g", { flags: { set: [-95] }, commandLine: { active: false }, stack: [] });
  const all = aiTools({
    calculators: async () => [{ id: "49g", model: "49g", title: "49g", rom: "rom", shown: false }],
    target: async () => { attaches++; return bg; },
    existing: async () => null,
    reference,
    pace: PACE,
  });
  const t = Object.fromEntries(all.map((x) => [x.name, x]));
  const one = (await call(t.list_calculators)).calculators[0];
  assert.equal(one.family, "rpl");
  assert.ok(!("mode" in one));
  assert.equal(attaches, 0, "the listing attached nothing");
  assert.ok(!bg.calls.length, "nothing read");
  // read_stack attaches and gives it.
  assert.equal((await call(t.read_stack, { calculator: "49g" })).mode, "algebraic");
  assert.equal(attaches, 1);
  // Without an `existing` dep, no mode either.
  const bare = aiTools({ calculators: async () => [{ id: "49g", model: "49g", title: "49g", rom: "rom", shown: true }], target: async () => { attaches++; return bg; }, reference });
  assert.ok(!("mode" in (await call(bare[0])).calculators[0]));
  assert.equal(attaches, 1);
});

test("list_calculators reads the mode after the calls queued for that calculator", async () => {
  let algebraic = true;
  let release;
  const gate = new Promise((r) => { release = r; });
  const g = fake("49g", {
    flags: () => ({ set: algebraic ? [-95] : [] }),
    run: async () => { await gate; algebraic = false; return { error: null, closed: true }; },
    commandLine: { active: false },
    stack: [],
  });
  const t = tools({ "49g": g });
  const run = call(t.run_rpl, { text: "CF(-95)" });
  const listed = call(t.list_calculators);
  await new Promise((r) => setTimeout(r, 5));
  release();
  assert.equal((await run).mode, "rpn");
  assert.equal((await listed).calculators[0].mode, "rpn", "read after the queued CF(-95)");
});

test("press_keys: checks the names first, presses, waits, returns the display", async () => {
  const c = fake("48gx", { commandLine: { active: false }, stack: [{ type: "real", value: 3, text: "3" }] });
  const t = tools({ "48gx": c });
  await assert.rejects(call(t.press_keys, { keys: ["1", "nope"] }), /no key "nope"/);
  assert.ok(!c.calls.some(([cmd]) => cmd === "typeKeys"), "nothing pressed");
  const r = await call(t.press_keys, { keys: ["1", "enter"] });
  assert.deepEqual(c.calls.find(([cmd]) => cmd === "typeKeys"), ["typeKeys", ["1", "enter"]]);
  assert.equal(r.content[0].type, "image");
  assert.equal(r.content[0].data, "PNG");
  assert.equal(r.structuredContent.pressed, 2);
  assert.deepEqual(r.structuredContent.stack, [{ level: 1, type: "real", text: "3" }]);
  const text = await call(t.press_keys, { keys: ["1"], display: "none" });
  assert.deepEqual(text, { calculator: "48gx", model: "48gx", pressed: 1 });
});

test("press_keys refuses while the calculator types or transfers", async () => {
  const c = fake("48gx");
  c.store.state.busy = true;
  await assert.rejects(call(tools({ "48gx": c }).press_keys, { keys: ["1"] }), /busy/);
});

test("run_rpl: the 48GX's run, its error and the 49G's mode; refused on the 42S", async () => {
  const gx = fake("48gx", { run: { error: "+ Error: Too Few Arguments", closed: true, running: false }, commandLine: { active: false }, stack: [] });
  const r = await call(tools({ "48gx": gx }).run_rpl, { text: "+" });
  // The host reads the 48's character codes (\GS, \v/, ...) in the text.
  assert.deepEqual(gx.calls.find(([cmd]) => cmd === "run"), ["run", "+", { codes: true }]);
  assert.equal(r.error, "+ Error: Too Few Arguments");
  assert.equal(r.ran, "+");
  assert.deepEqual(r.stack, []);
  const g = fake("49g", { run: { error: null, closed: true }, flags: { set: [-95] }, commandLine: { active: false }, stack: [] });
  assert.equal((await call(tools({ "49g": g }).run_rpl, { text: "1" })).mode, "algebraic");
  await assert.rejects(call(tools({ "42s": fake("42s") }).run_rpl, { text: "1" }), /no RPL command line/);
});

test("run_rpl on the 28S types through its keys", async () => {
  const c = fake("28s", { entryModes: { lowercase: false, menuLock: false }, commandLine: { active: false, text: "", cursor: 0 } });
  const r = await call(tools({ "28s": c }).run_rpl, { text: "1 2 +", display: "text" });
  const typed = c.calls.find(([cmd]) => cmd === "typeKeys")[1];
  assert.equal(typed.at(-1), "enter");
  assert.equal(r.ran, "1 2 +");
  assert.equal(r.stack, undefined, "no stack read on the 28S");
  assert.equal(r.commandLine, undefined, "no line open");
});

test("run_rpl on the 28S types the 48's codes it has keys for, and refuses the others", async () => {
  const c = fake("28s", { entryModes: { lowercase: false, menuLock: false }, commandLine: { active: false, text: "", cursor: 0 } });
  const t = tools({ "28s": c });
  await call(t.run_rpl, { text: "\\<< 1 \\>>", display: "text" });
  const typed = c.calls.find(([cmd]) => cmd === "typeKeys")[1];
  assert.deepEqual(typed, ["space", "program", "space", "1", "space", "shift", "program", "enter"]);
  await assert.rejects(call(t.run_rpl, { text: "1 \\oo" }), /28S cannot type the code \\oo: it has no backslash key/);
  assert.equal(c.calls.filter(([cmd]) => cmd === "typeKeys").length, 1, "nothing typed for \\oo");
});

test("run_rpl on the 28S refuses while a command line is open; read_display shows the line", async () => {
  const c = fake("28s", { entryModes: { lowercase: false, menuLock: false }, commandLine: { active: true, text: "«1", cursor: 2 } });
  const t = tools({ "28s": c });
  await assert.rejects(call(t.run_rpl, { text: "1 2 +" }), /command line open \("«1"\).*"enter".*"on"/s);
  assert.ok(!c.calls.some(([cmd]) => cmd === "typeKeys"), "nothing typed");
  const r = await call(t.read_display, { format: "text" });
  assert.deepEqual(r.commandLine, { text: "«1", cursor: 2 });
  assert.equal(r.stack, undefined);
  assert.doesNotMatch(t.run_rpl.description, /cannot be read/);
});

test("the memory tools: reads, a store with its compile error, a purge; the dir; refused off the 48/49", async () => {
  const c = fake("48gx", {
    stack: [{ type: "program", source: "« 1 + »", text: "« 1 + »" }, { type: "real", value: 2, text: "2" }],
    memoryTree: { path: ["HOME", "A"], variables: [{ name: "A", type: "Directory", size: 30.5, variables: [{ name: "P", type: "Program", size: 12.5 }] }] },
    editText: { text: "« 1 + »", was: "1:2" },
    storeText: (m) => (m.text.includes("«") && !m.text.includes("»") ? { error: "Invalid Syntax" } : {}),
    purge: {},
  });
  const t = tools({ "48gx": c });
  assert.deepEqual((await call(t.read_stack, { levels: 1 })).levels, [{ level: 1, type: "program", text: "« 1 + »" }]);
  const vars = await call(t.list_variables);
  assert.deepEqual(vars.current, ["HOME", "A"]);
  assert.deepEqual(vars.home[0].variables[0], { name: "P", type: "Program", size: 12.5 });
  // What the tool returns fits its outputSchema, which tab-mcp 0.16 lists to AI apps, whose MCP clients check it.
  assert.deepEqual(validate(t.list_variables.outputSchema, vars), []);
  assert.equal((await call(t.read_variable, { name: "P" })).text, "« 1 + »");
  assert.deepEqual(c.calls.find(([cmd]) => cmd === "editText")[1], { dir: ["HOME", "A"], name: "P" });
  assert.deepEqual(await call(t.store_variable, { name: "Q", text: "« 2 *", dir: ["B"] }), { calculator: "48gx", stored: false, error: "Invalid Syntax" });
  assert.equal((await call(t.store_variable, { name: "Q", text: "« 2 * »", dir: ["HOME", "B"] })).stored, true);
  assert.deepEqual(c.calls.filter(([cmd]) => cmd === "storeText").map(([, m]) => m.dir), [["HOME", "B"], ["HOME", "B"]]);
  assert.ok(c.calls.filter(([cmd]) => cmd === "storeText").every(([, m]) => m.codes === true), "the host reads the codes");
  await call(t.purge_variable, { name: "Q" });
  assert.deepEqual(c.calls.find(([cmd]) => cmd === "purge").slice(1), [["HOME", "A"], "Q"]);
  const k = tools({ "42s": fake("42s") });
  for (const name of ["read_stack", "list_variables"]) await assert.rejects(call(k[name]), /48SX, 48GX and 49G/);
  await assert.rejects(call(k.store_variable, { name: "X", text: "1" }), /48SX, 48GX and 49G/);
});

test("run_function on the 42S presses the reference's keys; an unknown name says so", async () => {
  const c = fake("42s");
  const t = tools({ "42s": c });
  const r = await call(t.run_function, { name: "SIN", display: "text" });
  assert.equal(r.ran, "SIN");
  const pressed = c.calls.filter(([cmd]) => cmd === "typeKeys").map(([, k]) => k);
  assert.deepEqual(pressed.at(-1), ["sin"]);
  await assert.rejects(call(t.run_function, { name: "NOSUCH" }), /no function "NOSUCH"/);
  await assert.rejects(call(tools({ "48gx": fake("48gx") }).run_function, { name: "SIN" }), /typed: use run_rpl/);
});

test("run_function cancels a pending shift first", async () => {
  const c = fake("42s");
  let shifted = true;
  c.store.state.frame = { ...FRAME, get annunciators() { return { leftshift: shifted }; } };
  const backend = c.backend;
  c.backend = new Proxy({}, { get: (_, cmd) => (cmd === "typeKeys" ? (k) => { if (k[0] === "shift") shifted = false; return backend.typeKeys(k); } : backend[cmd]) });
  await call(tools({ "42s": c }).run_function, { name: "SIN", display: "none" });
  const pressed = c.calls.filter(([cmd]) => cmd === "typeKeys").map(([, k]) => k);
  assert.deepEqual(pressed[0], ["shift"]);
  assert.deepEqual(pressed.at(-1), ["sin"]);
});

test("lookup_command: a name by its ASCII spelling, a search, a key-driven entry, the shown model by default", async () => {
  const t = tools({ "48gx": fake("48gx") });
  const list = await call(t.lookup_command, { name: "->LIST" });
  assert.equal(list.model, "48gx");
  assert.equal(list.name, "→LIST");
  assert.ok(list.description);
  assert.ok(list.examples.length > 0 && list.examples[0].typed);
  const found = await call(t.lookup_command, { query: "sort" });
  assert.ok(found.matches.some((m) => m.name === "SORT") || found.matches.length > 0);
  const sin = await call(t.lookup_command, { name: "SIN", model: "42s" });
  assert.deepEqual(sin.keyNames, ["sin"]);
  assert.match(sin.howToRun, /run_function/);
  await assert.rejects(call(t.lookup_command, {}), /name or query/);
  await assert.rejects(call(t.lookup_command, { name: "NOSUCHCMD" }), /no command/);
});

test("read_display: text, pixels, and no image where none can be made", async () => {
  const c = fake("42s");
  c.store.state.frame = { ...FRAME, width: 131, height: 16, pixels: Buffer.alloc(17 * 16, 0xff).toString("base64"), annunciators: { leftshift: true } };
  const t = tools({ "42s": c }, { png: async () => null });
  const r = await call(t.read_display, { pixels: true });
  assert.deepEqual(r.annunciators, ["leftshift"]);
  assert.equal(r.pixels.length, 16);
  assert.equal(r.pixels[0], "#".repeat(131));
  assert.equal(pixelRows(FRAME)[0], ".".repeat(131));
});

test("an image that fails to be made leaves the display's text", async () => {
  for (const png of [async () => { throw new Error("no deflate"); }, () => { throw new Error("sync"); }]) {
    const c = fake("48gx", { commandLine: { active: false }, stack: [{ type: "real", value: 3, text: "3" }] });
    const t = tools({ "48gx": c }, { png });
    const r = await call(t.press_keys, { keys: ["1"] });
    assert.equal(r.content, undefined, "no image");
    assert.equal(r.pressed, 1);
    assert.deepEqual(r.stack, [{ level: 1, type: "real", text: "3" }]);
  }
});

test("the image's look: the call's look, else undefined for the app's setting; every tool with an image takes look", async () => {
  const looks = [];
  const png = async (frame, model, look) => { looks.push(look); return "PNG"; };
  const c = fake("48gx", { commandLine: { active: false }, stack: [] });
  const t = tools({ "48gx": c }, { png });
  for (const name of ["read_display", "press_keys", "run_rpl", "to_stack", "run_function"]) {
    assert.deepEqual(t[name].inputSchema.properties.look.enum, ["lcd", "bw"], name);
  }
  for (const name of ["read_stack", "list_keys", "lookup_command"]) assert.equal(t[name].inputSchema.properties.look, undefined, name);
  await call(t.read_display);
  await call(t.read_display, { look: "bw" });
  await call(t.press_keys, { keys: ["1"], look: "lcd" });
  await call(t.read_display, { look: "sepia" });
  assert.deepEqual(looks, [undefined, "bw", "lcd", undefined]);
  const r = await call(t.read_display, { look: "bw" });
  assert.equal(r.content[0].data, "PNG");
  assert.equal(r.structuredContent.look, undefined, "the text part is unchanged");
});

test("calls on one calculator run one after the other", async () => {
  const order = [];
  let release;
  const gate = new Promise((r) => { release = r; });
  const c = fake("48gx", { run: async (text) => { order.push(`start ${text}`); if (text === "A") await gate; order.push(`end ${text}`); return { closed: true }; }, commandLine: { active: false }, stack: [] });
  const t = tools({ "48gx": c });
  const a = call(t.run_rpl, { text: "A", display: "none" });
  const b = call(t.run_rpl, { text: "B", display: "none" });
  await new Promise((r) => setTimeout(r, 20));
  assert.deepEqual(order, ["start A"]);
  release();
  await Promise.all([a, b]);
  assert.deepEqual(order, ["start A", "end A", "start B", "end B"]);
});

test("run_rpl refuses while the user's command line is open, and types nothing", async () => {
  const c = fake("48gx", { commandLine: { active: true, text: "12", cursor: 2 }, run: { closed: true } });
  await assert.rejects(call(tools({ "48gx": c }).run_rpl, { text: "1" }), /command line open \("12"\).*"enter".*"on"/s);
  assert.ok(!c.calls.some(([cmd]) => cmd === "run"), "nothing typed");
});

/** `uiState` answers, as the protocol gives them. */
const AT_STACK = { kind: "stack", from: "memory", stackInControl: true, halted: 0, commandLine: false };
const IN_FORM = { kind: "inputForm", from: "memory", stackInControl: false, halted: 0, commandLine: false };

test("run_rpl refuses while a form has the keyboard, with the screen and how to leave it", async () => {
  // The owner's report: the 48GX in the PLOT form took UPDIR into a field.
  const c = fake("48gx", { uiState: IN_FORM, commandLine: { active: false }, run: { closed: true } });
  await assert.rejects(call(tools({ "48gx": c }).run_rpl, { text: "UPDIR" }), /an input form, not the stack.*screen: "input form".*CANCEL.*to_stack/s);
  assert.ok(!c.calls.some(([cmd]) => cmd === "run"), "nothing typed");
  // A halted program leaves the stack usable: run_rpl runs, and says so.
  const halted = { ...AT_STACK, kind: "halted", halted: 1 };
  const h = fake("48gx", { uiState: halted, commandLine: { active: false }, run: { closed: true }, stack: [] });
  const r = await call(tools({ "48gx": h }).run_rpl, { text: "CONT" });
  assert.equal(r.screen, "program halted");
  assert.equal(r.haltedPrograms, 1);
  assert.match(r.next, /CONT.*KILL/);
});

test("while a program runs: run_rpl says so, results carry screen running, RAM is not read", async () => {
  const c = fake("48gx", { stats: { loop: "frame" }, uiState: IN_FORM, commandLine: { active: false }, run: { closed: true }, stack: [] });
  await assert.rejects(call(tools({ "48gx": c }).run_rpl, { text: "1" }), /still running a program.*screen: "running".*wait and read_display/s);
  assert.ok(!c.calls.some(([cmd]) => cmd === "run" || cmd === "uiState"));
  const d = await call(tools({ "48gx": c }).read_display, { format: "text" });
  assert.equal(d.screen, "running");
  assert.match(d.next, /ATTN/);
  // run_rpl that returns still running: its screen is "running" too.
  let n = 0;
  const r = fake("48gx", { stats: () => ({ loop: n++ ? "frame" : "sleep" }), uiState: AT_STACK, commandLine: { active: false }, run: { closed: true, running: true }, stack: [] });
  const ran = await call(tools({ "48gx": r }).run_rpl, { text: "« 1 1000 START NEXT » EVAL" });
  assert.equal(ran.running, true);
  assert.equal(ran.screen, "running");
  // The stack then holds the program's working objects: not shown as a result.
  assert.match(ran.stack, /not read while a program runs/);
  assert.ok(!r.calls.some(([cmd]) => cmd === "stack"));
  const s = await call(tools({ "48gx": c }).read_stack);
  assert.equal(s.running, true);
  assert.match(s.note, /working objects/);
});

test("right after a cold boot the tools wait until the boot's question is answered", async () => {
  const c = fake("48gx", { run: { error: null, closed: true, running: false }, uiState: AT_STACK, commandLine: { active: false }, stack: [] });
  c.store.state.starting = true;
  setTimeout(() => { c.store.state.starting = false; }, 30);
  const r = await call(tools({ "48gx": c }).run_rpl, { text: "1" });
  assert.equal(r.ran, "1");
  assert.equal(c.store.state.starting, false, "it waited");
  assert.ok(c.calls.some(([cmd]) => cmd === "run"));
});

test("pure reads do not wait for the boot's question", async () => {
  const c = fake("48gx", { uiState: AT_STACK, commandLine: { active: false }, stack: [{ type: "real", text: "1" }] });
  c.store.state.starting = true;
  const s = await call(tools({ "48gx": c }).read_stack);
  assert.equal(s.depth, 1);
  assert.equal((await call(tools({ "48gx": c }).read_display, { format: "text" })).stack.length, 1);
});

test("a call cancelled while it waits for the boot's question types nothing", async () => {
  const c = fake("48gx", { run: { error: null, closed: true }, uiState: AT_STACK, commandLine: { active: false }, stack: [] });
  c.store.state.starting = true;
  const abort = new AbortController();
  const ran = tools({ "48gx": c }).run_rpl.execute({ text: "1" }, { signal: abort.signal, client: { grantId: "g", id: "x", host: "x" } });
  setTimeout(() => abort.abort(), 20);
  await assert.rejects(ran, /Cancelled/);
  c.store.state.starting = false;
  assert.ok(!c.calls.some(([cmd]) => cmd === "run"));
});

test("press_keys and read_display say what has the keyboard; nothing at the stack", async () => {
  const c = fake("49g", { uiState: { ...IN_FORM, kind: "message", from: "display" }, commandLine: { active: false }, stack: [] });
  const r = await call(tools({ "49g": c }).press_keys, { keys: ["1"], display: "text" });
  assert.equal(r.screen, "message");
  assert.match(r.next, /"enter".*closes it/);
  const s = fake("48sx", { uiState: AT_STACK, commandLine: { active: false }, stack: [] });
  const d = await call(tools({ "48sx": s }).read_display, { format: "text" });
  assert.equal(d.screen, "stack");
  assert.equal(d.next, undefined);
  // Not on the key-driven models.
  const k = await call(tools({ "42s": fake("42s") }).read_display, { format: "text" });
  assert.equal(k.screen, undefined);
});

test("to_stack presses CANCEL until the stack has the keyboard, and says what it closed", async () => {
  const states = [{ ...IN_FORM, kind: "chooseBox", from: "display" }, IN_FORM, AT_STACK];
  const c = fake("48gx", { uiState: () => states[c.calls.filter(([cmd]) => cmd === "typeKeys").length], commandLine: { active: false }, stack: [] });
  const r = await call(tools({ "48gx": c }).to_stack);
  assert.deepEqual(r.closed, ["choose box", "input form"]);
  assert.equal(r.pressed, 2);
  assert.equal(r.screen, "stack");
  assert.deepEqual(c.calls.filter(([cmd]) => cmd === "typeKeys").map(([, k]) => k), [["on"], ["on"]]);
  // At the stack: nothing pressed.
  const s = fake("48gx", { uiState: AT_STACK, commandLine: { active: false }, stack: [] });
  assert.equal((await call(tools({ "48gx": s }).to_stack)).pressed, 0);
  assert.ok(!s.calls.some(([cmd]) => cmd === "typeKeys"));
  // Never where CANCEL could drop typing or stop a program: refused, nothing pressed.
  const refusals = [
    [{ ...AT_STACK, kind: "commandLine", commandLine: true }, /command line.*"enter"/s],
    [{ ...IN_FORM, commandLine: true }, /input form with a field being edited.*keeps what is typed/s],
    [{ ...IN_FORM, kind: "input", commandLine: true }, /program input.*decided/s],
    [{ ...AT_STACK, kind: "halted", halted: 1 }, /halted program.*CONT.*KILL/s],
    [{ ...IN_FORM, kind: "unknown" }, /another screen.*read_display/s],
  ];
  for (const [state, why] of refusals) {
    const x = fake("48gx", { uiState: state, commandLine: { active: state.commandLine, text: "12" }, stack: [] });
    await assert.rejects(call(tools({ "48gx": x }).to_stack), why, state.kind);
    assert.ok(!x.calls.some(([cmd]) => cmd === "typeKeys"), state.kind);
  }
  // A running program: not read from RAM, refused, never ATTN.
  const busy = fake("48gx", { stats: { loop: "frame" }, uiState: IN_FORM });
  await assert.rejects(call(tools({ "48gx": busy }).to_stack), /running program.*wait and read_display/s);
  assert.ok(!busy.calls.some(([cmd]) => cmd === "typeKeys" || cmd === "uiState"));
  // Reaching such a screen after closing one: stops and reports.
  const into = [{ ...IN_FORM, kind: "message", from: "display" }, { ...IN_FORM, kind: "input", commandLine: true }];
  const m = fake("48gx", { uiState: () => into[Math.min(m.calls.filter(([cmd]) => cmd === "typeKeys").length, 1)], commandLine: { active: true }, stack: [] });
  const stopped = (await call(tools({ "48gx": m }).to_stack)).structuredContent;
  assert.deepEqual(stopped.closed, ["message"]);
  assert.equal(stopped.screen, "program input");
  assert.match(stopped.note, /^Stopped: /);
  // A box CANCEL does not close: five presses, then the note.
  const u = fake("48gx", { uiState: { ...IN_FORM, kind: "message" }, commandLine: { active: false }, stack: [] });
  const stuck = (await call(tools({ "48gx": u }).to_stack)).structuredContent;
  assert.equal(stuck.pressed, 5);
  assert.equal(stuck.screen, "message");
  assert.match(stuck.note, /Still not at the stack/);
  // Only on the 48SX, 48GX and 49G.
  await assert.rejects(call(tools({ "28s": fake("28s") }).to_stack), /48SX, 48GX and 49G/);
  assert.ok(familyOf("49g").tools.includes("to_stack"));
  assert.ok(!familyOf("28s").tools.includes("to_stack"));
  assert.match(INSTRUCTIONS, /to_stack/);
});

test("the 48's EquationWriter: named, left by to_stack only from its scrolling mode, its stack not listed", async () => {
  const WRITER = { ...IN_FORM, kind: "equationWriter" };
  const VIEW = { ...IN_FORM, kind: "equationView" };
  /** The EquationWriter's working objects on the stack, as the owner's report had them. */
  const working = Array.from({ length: 109 }, (_, i) => (i ? { type: "unknown" } : { type: "list", text: "{ }" }));
  // The names and what to do.
  const v = fake("48gx", { uiState: VIEW, commandLine: { active: false }, stack: working });
  const view = await call(tools({ "48gx": v }).read_display, { format: "text" });
  assert.equal(view.screen, "equation view");
  assert.match(view.next, /ENTER does nothing.*arrow keys.*\["on"\].*returns to the EquationWriter, where the formula can be edited, and keeps it.*to_stack/s);
  const w = fake("48gx", { uiState: WRITER, commandLine: { active: false }, stack: working });
  const writer = await call(tools({ "48gx": w }).read_display, { format: "text" });
  assert.equal(writer.screen, "equation writer");
  assert.match(writer.next, /decide first.*\["enter"\].*on the stack.*replaces that level.*form's field.*\["on"\].*drops the formula.*stays on the stack/s);
  // Its working objects are not listed as the user's stack: not in the display, not by read_stack.
  for (const d of [view, writer]) {
    assert.match(d.stack, /EquationWriter is open.*working objects.*not listed/s);
    assert.equal(d.stackDepth, undefined);
  }
  const s = await call(tools({ "48gx": w }).read_stack);
  assert.equal(s.levels, undefined);
  assert.equal(s.depth, undefined);
  assert.equal(s.screen, "equation writer");
  assert.match(s.note, /working objects.*Leave it first.*109 objects now, the EquationWriter's included/s);
  // run_rpl refuses in both.
  for (const state of [WRITER, VIEW]) {
    const r = fake("48gx", { uiState: state, commandLine: { active: false }, run: { closed: true } });
    await assert.rejects(call(tools({ "48gx": r }).run_rpl, { text: "1" }), (err) => {
      assert.match(err.message, /the EquationWriter.*not the stack/s);
      assert.doesNotMatch(err.message, /presses CANCEL until the stack is back/);
      assert.equal(err.message.match(/to_stack/g)?.length ?? 0, state === VIEW ? 1 : 0, "to_stack named at most once");
      return true;
    });
    assert.ok(!r.calls.some(([cmd]) => cmd === "run"));
  }
  // to_stack: one CANCEL in the scrolling mode, then stops at the writer with its next.
  const states = [VIEW, WRITER];
  const c = fake("48gx", { uiState: () => states[Math.min(c.calls.filter(([cmd]) => cmd === "typeKeys").length, 1)], commandLine: { active: false }, stack: working });
  const r = (await call(tools({ "48gx": c }).to_stack)).structuredContent;
  assert.deepEqual(r.closed, ["equation view"]);
  assert.equal(r.pressed, 1);
  assert.equal(r.screen, "equation writer");
  assert.match(r.note, /^Stopped: .*the EquationWriter: it would drop the formula/);
  assert.match(r.next, /decide first/);
  assert.deepEqual(c.calls.filter(([cmd]) => cmd === "typeKeys").map(([, k]) => k), [["on"]]);
  // In the writer: refused, nothing pressed.
  const x = fake("48gx", { uiState: WRITER, commandLine: { active: false }, stack: working });
  await assert.rejects(call(tools({ "48gx": x }).to_stack), /does not press CANCEL in the EquationWriter.*screen: "equation writer".*decide first/s);
  assert.ok(!x.calls.some(([cmd]) => cmd === "typeKeys"));
});

test("an unknown screen's next does not promise what to_stack will not do", async () => {
  const u = fake("48gx", { uiState: { ...IN_FORM, kind: "unknown" }, commandLine: { active: false }, stack: [] });
  const d = await call(tools({ "48gx": u }).read_display, { format: "text" });
  assert.equal(d.screen, "unknown");
  assert.doesNotMatch(d.next, /to_stack presses it/);
  assert.match(d.next, /\["on"\].*yourself.*stops a program waiting in WAIT.*to_stack presses nothing/s);
  assert.doesNotMatch(d.next, /interactive stack|matrix/i);
});

test("a command line in another screen: INPUT, told apart from an edited object", async () => {
  const e = fake("48gx", { uiState: { ...IN_FORM, kind: "input", commandLine: true }, commandLine: { active: true, text: "5" }, stack: [{ type: "real", text: "1" }] });
  const d = await call(tools({ "48gx": e }).read_display, { format: "text" });
  assert.equal(d.screen, "program input");
  assert.match(d.next, /program waits for input \(INPUT\).*\["enter"\].*gives the text to the program.*\["on"\].*yourself/s);
  assert.doesNotMatch(d.next, /VISIT|object is being edited/);
  const s = await call(tools({ "48gx": e }).read_stack);
  assert.equal(s.depth, 1);
  // The equation writer's note is the 49G's: not on the 48.
  assert.equal(s.note, undefined);
  assert.equal(d.stackNote, undefined);
  const g = fake("49g", { uiState: { ...IN_FORM, kind: "input", commandLine: true }, commandLine: { active: true, text: "5" }, stack: [{ type: "real", text: "1" }], flags: { set: [] } });
  assert.match((await call(tools({ "49g": g }).read_stack)).note, /49G's equation writer/);
});

test("the interactive stack: named, its next from the manuals, closed by to_stack with one CANCEL", async () => {
  const ISTACK = { ...IN_FORM, kind: "interactiveStack" };
  const levels = [{ type: "real", text: "1" }, { type: "real", text: "3" }, { type: "real", text: "2" }];
  const i = fake("48gx", { uiState: ISTACK, commandLine: { active: false }, stack: levels });
  const d = await call(tools({ "48gx": i }).read_display, { format: "text" });
  assert.equal(d.screen, "interactive stack");
  assert.match(d.next, /▲ and ▼.*move the pointer.*menu key.*acts on the level the pointer is at, at once.*\["on"\].*\["enter"\].*leaves.*stays.*to_stack/s);
  // Its stack is the user's: listed, no note.
  assert.equal(d.stack.length, 3);
  assert.equal(d.stackNote, undefined);
  const s = await call(tools({ "48gx": i }).read_stack);
  assert.equal(s.depth, 3);
  assert.equal(s.note, undefined);
  // run_rpl refuses, pointing at to_stack.
  const r = fake("48gx", { uiState: ISTACK, commandLine: { active: false }, run: { closed: true } });
  await assert.rejects(call(tools({ "48gx": r }).run_rpl, { text: "1" }), /the interactive stack, not the stack.*screen: "interactive stack".*to_stack presses CANCEL for you\.$/s);
  assert.ok(!r.calls.some(([cmd]) => cmd === "run"));
  // to_stack: one CANCEL, then the stack.
  const states = [ISTACK, AT_STACK];
  const c = fake("48gx", { uiState: () => states[Math.min(c.calls.filter(([cmd]) => cmd === "typeKeys").length, 1)], commandLine: { active: false }, stack: levels });
  const t = await call(tools({ "48gx": c }).to_stack);
  assert.deepEqual(t.closed, ["interactive stack"]);
  assert.equal(t.pressed, 1);
  assert.deepEqual(c.calls.filter(([cmd]) => cmd === "typeKeys").map(([, k]) => k), [["on"]]);
});

test("an object edited on the command line: named, not closed by to_stack, off the stack meanwhile", async () => {
  const EDIT = { ...IN_FORM, kind: "objectEdit", commandLine: true };
  const e = fake("49g", { uiState: EDIT, commandLine: { active: true, text: "3.5" }, stack: [{ type: "real", text: "2" }, { type: "real", text: "1" }], flags: { set: [] } });
  const d = await call(tools({ "49g": e }).read_display, { format: "text" });
  assert.equal(d.screen, "object edit");
  assert.match(d.next, /▼ on a stack level.*VISIT.*TOOL EDIT.*decide first.*\["enter"\].*puts the edited object back.*VISIT: stores it in the variable.*\["on"\].*puts the original back.*off the stack/s);
  assert.match(d.stackNote, /being edited.*off the stack.*VISIT edits a variable/s);
  const s = await call(tools({ "49g": e }).read_stack);
  assert.equal(s.depth, 2);
  assert.match(s.note, /until ENTER or CANCEL it is off the stack.*VISIT edits a variable/s);
  // to_stack refuses and presses nothing.
  await assert.rejects(call(tools({ "49g": e }).to_stack), /does not press CANCEL in an object being edited.*screen: "object edit".*decide first/s);
  assert.ok(!e.calls.some(([cmd]) => cmd === "typeKeys"));
  // run_rpl refuses without offering to_stack.
  await assert.rejects(call(tools({ "49g": e }).run_rpl, { text: "1" }), (err) => {
    assert.match(err.message, /an object being edited, not the stack/);
    assert.doesNotMatch(err.message, /to_stack/);
    return true;
  });
});

test("the MatrixWriter: named, with a cell being typed, not closed by to_stack", async () => {
  const MW = { ...IN_FORM, kind: "matrixWriter" };
  const m = fake("48sx", { uiState: MW, commandLine: { active: false }, stack: [] });
  const d = await call(tools({ "48sx": m }).read_display, { format: "text" });
  assert.equal(d.screen, "matrix writer");
  assert.match(d.next, /decide first.*Type a number.*\["enter"\].*cell.*\["enter"\] with nothing typed.*puts the matrix on the stack.*\["on"\].*new matrix is dropped.*comes back unchanged/s);
  assert.match(d.next, /form's field.*goes into the field.*returns to the form unchanged/s);
  assert.match(d.stackNote, /MatrixWriter is open.*opened from the stack with ▼ is off the stack/s);
  const s = await call(tools({ "48sx": m }).read_stack);
  assert.match(s.note, /opened from the stack with ▼ is off the stack/);
  // A cell being typed: its own next.
  const typed = fake("48sx", { uiState: { ...MW, commandLine: true }, commandLine: { active: true, text: "2" }, stack: [] });
  const t = await call(tools({ "48sx": typed }).read_display, { format: "text" });
  assert.equal(t.screen, "matrix writer");
  assert.match(t.next, /being typed into a cell.*\["enter"\] puts it in the cell.*\["on"\].*clears the typing and stays/s);
  assert.match(t.next, /form's field.*returning to the form unchanged/s);
  // to_stack refuses in both and presses nothing.
  for (const x of [m, typed]) {
    await assert.rejects(call(tools({ "48sx": x }).to_stack), /does not press CANCEL in the MatrixWriter.*screen: "matrix writer"/s);
    assert.ok(!x.calls.some(([cmd]) => cmd === "typeKeys"));
  }
});

test("the tools whose results carry calculator text say it is untrusted", () => {
  const t = tools({ "48gx": fake("48gx") });
  for (const name of ["read_display", "read_stack", "list_variables", "read_variable", "press_keys", "run_rpl", "to_stack", "run_function"]) {
    assert.equal(t[name].annotations?.untrustedContentHint, true, name);
  }
});

test("a queued call cancelled before its turn presses nothing", async () => {
  let release;
  const gate = new Promise((r) => { release = r; });
  const c = fake("48gx", { run: async () => { await gate; return { closed: true }; }, commandLine: { active: false }, stack: [] });
  const t = tools({ "48gx": c });
  const first = call(t.run_rpl, { text: "A", display: "none" });
  const abort = new AbortController();
  const second = t.press_keys.execute({ keys: ["1"] }, { signal: abort.signal, client: { grantId: "g", id: "x", host: "x" } });
  abort.abort();
  release();
  await first;
  await assert.rejects(second, /Cancelled/);
  assert.ok(!c.calls.some(([cmd]) => cmd === "typeKeys"), "no key pressed");
});

test("a call cancelled while its keys play drops those not played: the next call's keys come alone", async () => {
  // The calculator plays one queued key every 15 ms, as the engine's queue does.
  const played = [];
  let queue = [];
  const c = fake("48gx", { uiState: AT_STACK, commandLine: { active: false }, stack: [], stats: () => ({ loop: queue.length ? "frame" : "sleep" }) });
  const backend = c.backend;
  const timer = setInterval(() => { if (queue.length) played.push(queue.shift()); }, 15);
  c.backend = new Proxy({}, {
    get: (_, cmd) => {
      if (cmd === "typeKeys") return (k) => { queue.push(...k); return backend.typeKeys(k); };
      if (cmd === "releaseAll") return () => { queue = []; return backend.releaseAll(); };
      return backend[cmd];
    },
  });
  try {
    const t = tools({ "48gx": c }, { png: async () => null });
    const abort = new AbortController();
    const first = t.press_keys.execute({ keys: Array(20).fill("1"), display: "none" }, { signal: abort.signal, client: { grantId: "g", id: "x", host: "x" } });
    await new Promise((r) => setTimeout(r, 70));
    abort.abort();
    await assert.rejects(first, /Cancelled/);
    const second = await call(t.press_keys, { keys: ["plus"], display: "none" });
    assert.equal(second.pressed, 1);
    assert.ok(c.calls.some(([cmd]) => cmd === "releaseAll"), "the queue dropped");
    assert.ok(played.filter((k) => k === "1").length < 20, "not every key of the cancelled call played");
    assert.equal(played.at(-1), "plus");
    assert.equal(played.indexOf("plus"), played.length - 1, "no key of the cancelled call after the next call's");
    // A call cancelled before it pressed anything drops nothing (the user's own keys stay).
    const quiet = fake("48gx", { uiState: AT_STACK, commandLine: { active: false }, stack: [] });
    const stop = new AbortController();
    stop.abort();
    await assert.rejects(tools({ "48gx": quiet }).press_keys.execute({ keys: ["1"] }, { signal: stop.signal }), /Cancelled/);
    assert.ok(!quiet.calls.some(([cmd]) => cmd === "releaseAll" || cmd === "typeKeys"));
  } finally {
    clearInterval(timer);
  }
});

test("a cursor blinking on an open line is not a program running", async () => {
  // Awake at one look in four (the blink), asleep otherwise: not running.
  let n = 0;
  const b = fake("48gx", { stats: () => ({ loop: n++ % 4 === 3 ? "frame" : "sleep" }), uiState: { ...IN_FORM, kind: "matrixWriter", commandLine: true }, commandLine: { active: true, text: "5" }, stack: [] });
  for (let i = 0; i < 4; i++) {
    const d = await call(tools({ "48gx": b }).read_display, { format: "text" });
    assert.equal(d.screen, "matrix writer");
    assert.equal(d.running, undefined);
  }
  // Awake at every look: running.
  const r = fake("48gx", { stats: { loop: "frame" }, uiState: AT_STACK, commandLine: { active: false }, stack: [] });
  assert.equal((await call(tools({ "48gx": r }).read_display, { format: "text" })).screen, "running");
});
