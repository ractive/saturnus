// The Claude Desktop extension's bundle (web/mcpb/build.mjs, iteration
// 59): the same bytes from the same sources, its four files, a manifest
// valid by the MCPB 0.3 spec (github.com/modelcontextprotocol/mcpb,
// MANIFEST.md: the required fields, server type node with its entry point
// in the bundle, the port as user_config passed to the forwarder), and the
// tool list and instructions the desktop app's endpoint gives.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { buildBundle, crc32, unzip, workspaceVersion, zip } from "../mcpb/build.mjs";
import { listTools } from "../ai-local.js";
import { aiTools, INSTRUCTIONS } from "../ai-tools.js";

const bytes = await buildBundle();
const files = Object.fromEntries(unzip(bytes));
const manifest = JSON.parse(files["manifest.json"].toString("utf8"));

test("the same sources give the same bytes, and the zip reads back", async () => {
  assert.deepEqual(await buildBundle(), bytes);
  assert.equal(crc32(Buffer.from("123456789")), 0xcbf43926, "CRC-32's check value");
  const round = [["a.txt", Buffer.from("x")], ["d/b.bin", Buffer.from([0, 1, 2])]];
  assert.deepEqual(unzip(zip(round)), round);
  assert.deepEqual(Object.keys(files), ["manifest.json", "icon.png", "server/index.mjs", "server/offline.json"]);
});

test("the manifest has what the MCPB 0.3 spec requires", () => {
  assert.equal(manifest.manifest_version, "0.3");
  assert.equal(manifest.name, "saturnus");
  assert.match(manifest.version, /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/);
  assert.equal(manifest.version, workspaceVersion());
  assert.ok(manifest.description.length > 20);
  assert.equal(typeof manifest.author.name, "string");
  assert.equal(manifest.server.type, "node");
  assert.ok(files[manifest.server.entry_point], "the entry point is in the bundle");
  assert.equal(manifest.server.mcp_config.command, "node");
  assert.deepEqual(manifest.server.mcp_config.args, [`\${__dirname}/${manifest.server.entry_point}`]);
  assert.equal(manifest.server.mcp_config.env.SATURNUS_MCP_PORT, "${user_config.port}");
  assert.deepEqual(Object.keys(manifest.server.mcp_config.env), ["SATURNUS_MCP_PORT"], "nothing secret in the manifest");
  const port = manifest.user_config.port;
  assert.equal(port.type, "number");
  assert.equal(port.default, 4842);
  assert.ok(files[manifest.icon], "the icon is in the bundle");
  assert.deepEqual(files["icon.png"].subarray(1, 4).toString(), "PNG");
  assert.deepEqual(manifest.compatibility.platforms, ["darwin", "win32", "linux"]);
});

test("the forwarder is the source, and the offline answers are the endpoint's tools and instructions", () => {
  assert.deepEqual(files["server/index.mjs"], readFileSync(new URL("../mcpb/server/index.mjs", import.meta.url)));
  assert.doesNotMatch(files["server/index.mjs"].toString(), /from "(?!node:)/, "Node built-ins only");
  const offline = JSON.parse(files["server/offline.json"].toString("utf8"));
  const none = async () => {};
  const tools = listTools(aiTools({ calculators: none, target: none, reference: { index: none }, png: none }));
  assert.deepEqual(offline.tools, tools);
  assert.equal(offline.instructions, INSTRUCTIONS);
  assert.equal(offline.version, manifest.version);
  assert.deepEqual(manifest.tools, tools.map((t) => ({ name: t.name, description: t.description })));
});
