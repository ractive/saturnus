// The vendored tab-mcp client (web/vendor/tab-mcp-client/, `just
// vendor-tab-mcp`, iteration 55) is exactly what vendor.json records: each
// file's SHA-256, no file more or less, and every chunk the entry loads.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const DIR = fileURLToPath(new URL("../vendor/tab-mcp-client/", import.meta.url));

/** The check, over a directory's listing and reader (tests give a tampered one). */
export function vendorProblems(names, read) {
  const manifest = JSON.parse(read("vendor.json"));
  const problems = [];
  const listed = Object.keys(manifest.files).sort();
  const present = names.filter((n) => n.endsWith(".min.js")).sort();
  for (const n of present) if (!listed.includes(n)) problems.push(`${n} is not in vendor.json`);
  for (const n of listed) {
    if (!present.includes(n)) {
      problems.push(`${n} is missing`);
      continue;
    }
    const sha = createHash("sha256").update(read(n, null)).digest("hex");
    if (sha !== manifest.files[n]) problems.push(`${n} does not match its SHA-256`);
  }
  if (!present.includes(manifest.entry)) problems.push(`the entry ${manifest.entry} is missing`);
  else {
    for (const m of String(read(manifest.entry)).matchAll(/import\("\.\/([^"]+)"\)/g)) {
      if (!present.includes(m[1])) problems.push(`the entry loads ${m[1]}, which is missing`);
    }
  }
  if (!manifest.version || !manifest.source?.kind) problems.push("vendor.json names no version or source");
  return problems;
}

const reader = (dir) => (name, enc = "utf8") => readFileSync(join(dir, name), enc ?? undefined);

test("the vendored files are the ones vendor.json records", () => {
  assert.deepEqual(vendorProblems(readdirSync(DIR), reader(DIR)), []);
});

test("a changed, an extra and a missing file are each reported", () => {
  const read = reader(DIR);
  const names = readdirSync(DIR);
  const changed = (name, enc) => (name === "tab-mcp-client.min.js" && enc === null ? Buffer.from("tampered") : read(name, enc));
  assert.match(vendorProblems(names, changed).join("\n"), /tab-mcp-client\.min\.js does not match/);
  assert.match(vendorProblems([...names, "extra-1234.min.js"], read).join("\n"), /extra-1234\.min\.js is not in vendor\.json/);
  const chunk = names.find((n) => n.startsWith("connection-"));
  const without = vendorProblems(names.filter((n) => n !== chunk), read).join("\n");
  assert.match(without, new RegExp(`${chunk.replace(/\./g, "\\.")} is missing`));
  assert.match(without, /the entry loads connection-/);
});
