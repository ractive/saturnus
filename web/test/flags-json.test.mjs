// scripts/flags-json.py --check on hand-made files: malformed settings
// of a multi-flag field are reported as errors, not a crash. Skipped
// without python3.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(new URL("../../scripts/flags-json.py", import.meta.url));
const FLAGS = fileURLToPath(new URL("../flags.json", import.meta.url));

/** `--check` of `doc`: its exit status and what it printed. */
function check(doc) {
  const dir = mkdtempSync(join(tmpdir(), "flags-json-"));
  try {
    const file = join(dir, "flags.json");
    writeFileSync(file, JSON.stringify(doc));
    const r = spawnSync("python3", [SCRIPT, "--check", file], { encoding: "utf8" });
    return { status: r.status, out: `${r.stdout}${r.stderr}`, error: r.error };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** The committed flags.json with the 48SX's coordinate field's settings replaced by `values`. */
function withValues(values) {
  const doc = JSON.parse(readFileSync(FLAGS, "utf8"));
  doc.models["48sx"].system.find((e) => e.first === -15).values = values;
  return doc;
}

test("flags-json --check: the committed file passes; malformed settings are errors, not a crash", (t) => {
  const ok = check(JSON.parse(readFileSync(FLAGS, "utf8")));
  if (ok.error) {
    t.skip(`no python3: ${ok.error.message}`);
    return;
  }
  assert.equal(ok.status, 0, ok.out);
  const cases = [
    ["not a dict", ["Rectangular"]],
    ["no name", [{ set: [], clear: [-15, -16] }]],
    ["set not a list", [{ name: "R", set: -16, clear: [] }]],
    ["clear missing", [{ name: "R", set: [-16] }]],
    ["a flag not a number", [{ name: "R", set: ["-16"], clear: [] }]],
  ];
  for (const [what, values] of cases) {
    const r = check(withValues(values));
    assert.equal(r.status, 1, `${what}: ${r.out}`);
    assert.match(r.out, /48sx: -15: setting 0: needs a name and `set` and `clear` lists/, what);
    assert.doesNotMatch(r.out, /Traceback/, what);
  }
  // Well formed, but two settings match the same flags.
  const r = check(withValues([{ name: "A", set: [], clear: [-16] }, { name: "B", set: [], clear: [-15] }]));
  assert.equal(r.status, 1);
  assert.match(r.out, /matches \['A', 'B'\]/);
});
