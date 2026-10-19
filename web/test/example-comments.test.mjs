// The comments on the reference's examples: scripts/apply-comments.py,
// scripts/check-comments.py and the join in scripts/commands-json.py, on
// a copy of crates/saturnus-cli/data/commands (SATURNUS_COMMANDS_DATA).
// Skipped without python3.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPTS = fileURLToPath(new URL("../../scripts/", import.meta.url));
const DATA = fileURLToPath(new URL("../../crates/saturnus-cli/data/commands/", import.meta.url));

/** Run scripts/`name` with `args` on the data in `dir`. */
function run(dir, name, ...args) {
  const r = spawnSync("python3", [join(SCRIPTS, name), ...args], {
    encoding: "utf8",
    env: { ...process.env, SATURNUS_COMMANDS_DATA: dir },
  });
  return { status: r.status, out: `${r.stdout}${r.stderr}`, error: r.error };
}

/** A copy of the data folder, removed after `fn`. */
function withData(fn) {
  const root = mkdtempSync(join(tmpdir(), "example-comments-"));
  const dir = join(root, "commands");
  cpSync(DATA, dir, { recursive: true });
  try {
    return fn(dir, root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function sidecar(root, name, doc) {
  const file = join(root, name);
  writeFileSync(file, JSON.stringify(doc));
  return file;
}

const read = (dir, file) => readFileSync(join(dir, file), "utf8");

test("apply-comments writes a sidecar into the specs, again changes nothing, and refuses an index that does not exist", (t) => {
  const probe = spawnSync("python3", ["--version"]);
  if (probe.error) {
    t.skip(`no python3: ${probe.error.message}`);
    return;
  }
  withData((dir, root) => {
    const rpl = sidecar(root, "rpl.json", { group: "rpl", comments: { RND: [{ i: 3, comment: "Two decimal places in FIX 2" }] } });
    const k39 = sidecar(root, "39g.json", { group: "39g", comments: { COMB: [{ i: 0, comment: "Ways to pick 2 of 5" }] } });
    let r = run(dir, "apply-comments.py", rpl, k39);
    assert.equal(r.status, 0, r.out);
    assert.match(r.out, /wrote reference\.json, keys-39g\.json, keys-40g\.json/);
    const ref = read(dir, "reference.json");
    const keys39 = read(dir, "keys-39g.json");
    assert.equal(JSON.parse(ref).commands.RND.examples[3].comment, "Two decimal places in FIX 2");
    // One command per line, as the file keeps them; the 40G's the same.
    const line = keys39.split("\n").find((l) => l.includes('"name": "COMB"'));
    assert.match(line, /"comment": "Ways to pick 2 of 5"\}\]\},$/);
    assert.ok(read(dir, "keys-40g.json").includes(line));
    // Idempotent.
    r = run(dir, "apply-comments.py", rpl, k39);
    assert.equal(r.status, 0, r.out);
    assert.match(r.out, /nothing changed/);
    assert.equal(read(dir, "reference.json"), ref);
    assert.equal(read(dir, "keys-39g.json"), keys39);
    // An index or a command that does not exist: nothing written.
    const bad = sidecar(root, "bad.json", { group: "rpl", comments: { RND: [{ i: 0, comment: "Fine" }, { i: 9, comment: "No such example" }], NOPE: [{ i: 0, comment: "x" }] } });
    r = run(dir, "apply-comments.py", bad);
    assert.equal(r.status, 1, r.out);
    assert.match(r.out, /RND \[9\]: no example 9 \(RND has 5\)/);
    assert.match(r.out, /no command 'NOPE'/);
    assert.match(r.out, /nothing written/);
    assert.equal(read(dir, "reference.json"), ref);
    // A 40G sidecar is refused: the 39G's is applied to both.
    r = run(dir, "apply-comments.py", sidecar(root, "40g.json", { group: "40g", comments: {} }));
    assert.equal(r.status, 1, r.out);
    // null removes a comment.
    r = run(dir, "apply-comments.py", sidecar(root, "rm.json", { group: "rpl", comments: { RND: [{ i: 3, comment: null }] } }));
    assert.equal(r.status, 0, r.out);
    assert.equal(JSON.parse(read(dir, "reference.json")).commands.RND.examples[3].comment, undefined);
  });
});

test("commands-json joins each comment to the example generated from its spec, RPL and key-driven", (t) => {
  const probe = spawnSync("python3", ["--version"]);
  if (probe.error) {
    t.skip(`no python3: ${probe.error.message}`);
    return;
  }
  withData((dir, root) => {
    // RND: spec 1 runs on the 48s and the 49G (their 2nd example), spec 4
    // on the 28S only (its 2nd). DATE+: both specs everywhere.
    const rpl = sidecar(root, "rpl.json", { group: "rpl", comments: {
      RND: [{ i: 1, comment: "Spec one" }, { i: 4, comment: "Spec four" }],
    } });
    const k = sidecar(root, "k.json", { group: "42s", comments: { "%": [{ i: 0, comment: "Fifteen percent of 200" }] } });
    let r = run(dir, "apply-comments.py", rpl, k);
    assert.equal(r.status, 0, r.out);
    const out = join(root, "commands.json");
    r = run(dir, "commands-json.py", "--out", out);
    assert.equal(r.status, 0, r.out);
    const data = JSON.parse(readFileSync(out, "utf8"));
    const rnd = data.commands.RND.models;
    for (const m of ["48sx", "48gx", "49g"]) {
      assert.deepEqual(rnd[m].examples.map((x) => x.comment), [undefined, "Spec one", undefined], m);
    }
    assert.deepEqual(rnd["28s"].examples.map((x) => x.comment), [undefined, "Spec four"]);
    assert.equal(rnd["28s"].examples[1].input, "12345.678");
    assert.equal(data.keyModels["42s"].commands["%"].examples[0].comment, "Fifteen percent of 200");
    // A comment on an example never generated (the examples are older
    // than the specs) fails rather than vanishing.
    const ref = JSON.parse(read(dir, "reference.json"));
    ref.commands.RND.examples[1].input = "12345.678 -4.";
    writeFileSync(join(dir, "reference.json"), `${JSON.stringify(ref, null, 2)}\n`);
    r = run(dir, "commands-json.py", "--out", out);
    assert.equal(r.status, 1, r.out);
    assert.match(r.out, /never generated .*RND \[1\]/);
  });
});

test("check-comments fails on length, full stop and banned words, and lists numbers not in the example", (t) => {
  const probe = spawnSync("python3", ["--version"]);
  if (probe.error) {
    t.skip(`no python3: ${probe.error.message}`);
    return;
  }
  withData((dir, root) => {
    // The committed comments pass.
    let r = run(dir, "check-comments.py");
    assert.equal(r.status, 0, r.out);
    const bad = sidecar(root, "bad.json", { group: "rpl", comments: {
      "!": [{ i: 0, comment: "x".repeat(91) }],
      "%": [{ i: 0, comment: "Fifteen percent of 200." }],
      "%T": [{ i: 0, comment: "This example shows a share" }, { i: 1, comment: "Here we just take a part!" }],
      ABS: [{ i: 0, comment: "Simply drops the sign 🙂" }],
      "%CH": [{ i: 5, comment: "No such example" }],
    } });
    r = run(dir, "check-comments.py", bad);
    assert.equal(r.status, 1, r.out);
    for (const want of [
      /! \[0\]: 91 characters, over 90/,
      /% \[0\]: ends with a full stop/,
      /%T \[0\]: contains "this example"/,
      /%T \[1\]: contains "here"/,
      /%T \[1\]: contains "we"/,
      /%T \[1\]: contains "just"/,
      /%T \[1\]: contains an exclamation mark/,
      /ABS \[0\]: contains "simply"/,
      /ABS \[0\]: contains an emoji/,
      /%CH \[5\]: no example 5/,
    ]) assert.match(r.out, want);
    // Numbers: from the input or the result on every model, decimal
    // points ignored; others listed, without failing.
    const nums = sidecar(root, "nums.json", { group: "rpl", comments: {
      "HMS+": [{ i: 0, comment: "1.3 is 1h30m, plus 2h45m gives 4h15m" }],
      "%CH": [{ i: 0, comment: "From 50 to 60 is a rise of 20 percent" }],
      RND: [{ i: 1, comment: "Rounds 12345.678 to 12300, not 12350" }],
    } });
    r = run(dir, "check-comments.py", nums);
    assert.equal(r.status, 0, r.out);
    assert.match(r.out, /HMS\+ \[0\]: 30 not in the example/);
    assert.doesNotMatch(r.out, /%CH \[0\]/);
    assert.match(r.out, /RND \[1\]: 12350 not in the example/);
    const keyed = sidecar(root, "k.json", { group: "17bii", comments: { "%T": [{ i: 0, comment: "50 of 200 is 25 percent, not 30" }] } });
    r = run(dir, "check-comments.py", keyed);
    assert.equal(r.status, 0, r.out);
    assert.match(r.out, /%T \[0\]: 30 not in the example/);
  });
});
