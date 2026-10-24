#!/usr/bin/env node
// Builds saturnus.mcpb, the extension for Claude Desktop (iteration 59):
// a zip with
//
//   manifest.json        web/mcpb/manifest.json with the version (the
//                        workspace's, Cargo.toml) and the tools' names and
//                        descriptions filled in (MCPB manifest 0.3)
//   icon.png             web/pwa/icons/icon-512.png
//   server/index.mjs     web/mcpb/server/index.mjs, the forwarder
//   server/offline.json  the version, the server's instructions and the
//                        tool list as tools/list gives them (web/ai-tools.js
//                        through web/ai-local.js `listTools`), for the
//                        answers while the app is closed
//
// The same sources give the same bytes: the entries stored uncompressed
// (no zlib version in the output), in a fixed order, with a fixed date
// and mode. Built at build time, never committed: web/site.sh puts it in
// the web app (app/saturnus.mcpb), the desktop app's build.rs into its
// embedded page, `just mcpb` anywhere.
//
// Usage: node web/mcpb/build.mjs OUT.mcpb

import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = new URL("./", import.meta.url);
const web = new URL("../", here);
const root = new URL("../", web);

/**
 * The workspace's version from Cargo.toml's [workspace.package]; for a
 * copy of web/ alone (web/test/site.test.mjs builds the site from one),
 * the wasm package's, which wasm-pack writes from the same version.
 */
export function workspaceVersion() {
  const cargo = new URL("Cargo.toml", root);
  if (!existsSync(cargo)) {
    const pkg = new URL("pkg/package.json", web);
    if (existsSync(pkg)) return JSON.parse(readFileSync(pkg, "utf8")).version;
    throw new Error("no Cargo.toml above web/ and no web/pkg/package.json: cannot tell the version");
  }
  const toml = readFileSync(cargo, "utf8");
  const section = toml.split(/^\[workspace\.package\]\s*$/m)[1]?.split(/^\[/m)[0] ?? "";
  const m = section.match(/^version\s*=\s*"([^"]+)"/m);
  if (!m) throw new Error("no version in Cargo.toml's [workspace.package]");
  return m[1];
}

/** The tool list and the instructions, as the desktop app's endpoint gives them. */
export async function toolsAndInstructions() {
  const { aiTools, INSTRUCTIONS } = await import(new URL("ai-tools.js", web).href);
  const { listTools } = await import(new URL("ai-local.js", web).href);
  const none = async () => {
    throw new Error("not called");
  };
  const tools = listTools(aiTools({ calculators: none, target: none, reference: { index: none }, png: none }));
  return { tools, instructions: INSTRUCTIONS };
}

/** The bundle's files, in order: [name, bytes]. */
export async function bundleFiles() {
  const version = workspaceVersion();
  const { tools, instructions } = await toolsAndInstructions();
  const manifest = JSON.parse(readFileSync(new URL("manifest.json", here), "utf8"));
  manifest.version = version;
  manifest.tools = tools.map((t) => ({ name: t.name, description: t.description }));
  const json = (v) => Buffer.from(`${JSON.stringify(v, null, 2)}\n`, "utf8");
  return [
    ["manifest.json", json(manifest)],
    ["icon.png", readFileSync(new URL("pwa/icons/icon-512.png", web))],
    ["server/index.mjs", readFileSync(new URL("server/index.mjs", here))],
    ["server/offline.json", json({ version, instructions, tools })],
  ];
}

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

/** CRC-32 (the zip's) of `buf`. */
export function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** A zip of `files` ([name, bytes]), stored, dated 1980-01-01 00:00, mode 0644. */
export function zip(files) {
  const DOS_TIME = 0;
  const DOS_DATE = (0 << 9) | (1 << 5) | 1; // 1980-01-01
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const [name, data] of files) {
    const n = Buffer.from(name, "utf8");
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4); // version needed: 2.0
    local.writeUInt16LE(0x0800, 6); // UTF-8 names
    local.writeUInt16LE(0, 8); // stored
    local.writeUInt16LE(DOS_TIME, 10);
    local.writeUInt16LE(DOS_DATE, 12);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(n.length, 26);
    local.writeUInt16LE(0, 28);
    locals.push(local, n, data);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE((3 << 8) | 20, 4); // made by: Unix, 2.0
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt16LE(0, 10);
    central.writeUInt16LE(DOS_TIME, 12);
    central.writeUInt16LE(DOS_DATE, 14);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(n.length, 28);
    central.writeUInt32LE(((0o100644 << 16) >>> 0), 38); // a regular file, rw-r--r--
    central.writeUInt32LE(offset, 42);
    centrals.push(central, n);
    offset += local.length + n.length + data.length;
  }
  const cd = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(files.length, 8);
  end.writeUInt16LE(files.length, 10);
  end.writeUInt32LE(cd.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, cd, end]);
}

/** The entries of a stored zip made by `zip`: [name, bytes]. */
export function unzip(buf) {
  const out = [];
  const endAt = buf.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  if (endAt < 0) throw new Error("not a zip");
  let p = buf.readUInt32LE(endAt + 16);
  const count = buf.readUInt16LE(endAt + 10);
  for (let i = 0; i < count; i++) {
    if (buf.readUInt32LE(p) !== 0x02014b50) throw new Error("bad central directory");
    const method = buf.readUInt16LE(p + 10);
    const size = buf.readUInt32LE(p + 20);
    const nameLen = buf.readUInt16LE(p + 28);
    const extra = buf.readUInt16LE(p + 30);
    const comment = buf.readUInt16LE(p + 32);
    const at = buf.readUInt32LE(p + 42);
    const name = buf.subarray(p + 46, p + 46 + nameLen).toString("utf8");
    if (method !== 0) throw new Error(`${name} is compressed`);
    const start = at + 30 + buf.readUInt16LE(at + 26) + buf.readUInt16LE(at + 28);
    const data = buf.subarray(start, start + size);
    if (crc32(data) !== buf.readUInt32LE(p + 16)) throw new Error(`${name}: CRC mismatch`);
    out.push([name, data]);
    p += 46 + nameLen + extra + comment;
  }
  return out;
}

/** The bundle's bytes. */
export async function buildBundle() {
  return zip(await bundleFiles());
}

async function cli() {
  const out = process.argv[2];
  if (!out) {
    process.stderr.write("usage: node web/mcpb/build.mjs OUT.mcpb\n");
    process.exit(2);
  }
  const bytes = await buildBundle();
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, bytes);
  process.stdout.write(`web/mcpb/build.mjs: ${out}, ${bytes.length} bytes\n`);
}

let started = false;
try {
  started = Boolean(process.argv[1]) && pathToFileURL(realpathSync(process.argv[1])).href === pathToFileURL(realpathSync(fileURLToPath(import.meta.url))).href;
} catch {
  started = false;
}
if (started) await cli();
