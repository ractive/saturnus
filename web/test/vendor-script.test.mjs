// scripts/vendor-tab-mcp-client.sh (`just vendor-tab-mcp`): a run that
// fails on a line it rewrites after the copy (web/ai-local.js's
// CONFIRM_MODULE, with --pin the justfile's version and integrity) changes
// nothing, since it checks those lines before it writes anything. The
// script runs in a temporary copy of the repository's layout, with fake
// `bun`, `git` and `npm` on PATH (a local build and a registry package,
// no network).
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(new URL("../../scripts/vendor-tab-mcp-client.sh", import.meta.url));
const skip = process.platform === "win32" ? "a bash script" : false;

const CONFIRM_LINE = 'export const CONFIRM_MODULE = "./vendor/tab-mcp-client/confirm-old.min.js";\n';
const PIN_LINES = 'tab_mcp_client_version := "0.0.1"\ntab_mcp_client_integrity := "sha512-old"\n';

function put(path, text, mode) {
  mkdirSync(join(path, ".."), { recursive: true });
  writeFileSync(path, text);
  if (mode) chmodSync(path, mode);
}

/** The new build's files: the entry and its confirmation chunk. */
function build(dir) {
  put(join(dir, "tab-mcp-client.min.js"), 'import("./confirm-new1.min.js");\n');
  put(join(dir, "confirm-new1.min.js"), "export function confirmInPage() {}\n");
}

/** A repository copy (web/ai-local.js and the justfile as given), a local tab-mcp checkout, a package tarball, the fakes. */
function tree({ aiLocal = CONFIRM_LINE, justfile = PIN_LINES } = {}) {
  const root = mkdtempSync(join(tmpdir(), "vendor-script-"));
  const repo = join(root, "repo");
  mkdirSync(join(repo, "scripts"), { recursive: true });
  copyFileSync(SCRIPT, join(repo, "scripts", "vendor-tab-mcp-client.sh"));
  chmodSync(join(repo, "scripts", "vendor-tab-mcp-client.sh"), 0o755);
  put(join(repo, "web", "ai-local.js"), aiLocal);
  put(join(repo, "justfile"), justfile);
  put(join(repo, "web", "vendor", "tab-mcp-client", "vendor.json"), '{"version": "0.0.1"}\n');
  put(join(repo, "web", "vendor", "tab-mcp-client", "confirm-old.min.js"), "old\n");
  // A local checkout whose build is already there (the fake bun builds nothing).
  const client = join(root, "tab-mcp", "packages", "tab-mcp-client");
  put(join(client, "package.json"), "{}\n");
  put(join(client, "dist", "package.json"), '{"version": "9.9.9"}\n');
  build(join(client, "dist", "cdn"));
  // The registry's package.
  build(join(root, "pkg", "package", "cdn"));
  const tarball = join(root, "pkg.tgz");
  const tar = spawnSync("tar", ["-czf", tarball, "-C", join(root, "pkg"), "package"]);
  assert.equal(tar.status, 0, String(tar.stderr));
  const integrity = `sha512-${createHash("sha512").update(readFileSync(tarball)).digest("base64")}`;
  const bin = join(root, "bin");
  put(join(bin, "bun"), "#!/bin/sh\nexit 0\n", 0o755);
  put(join(bin, "git"), '#!/bin/sh\ncase "$*" in *rev-parse*) echo 0123abc;; esac\nexit 0\n', 0o755);
  put(join(bin, "npm"), '#!/bin/sh\ncase "$1" in view) echo "$FAKE_INTEGRITY";; pack) cp "$FAKE_TARBALL" .;; esac\n', 0o755);
  const run = (...args) =>
    spawnSync(join(repo, "scripts", "vendor-tab-mcp-client.sh"), args, {
      encoding: "utf8",
      env: { ...process.env, PATH: `${bin}:${process.env.PATH}`, FAKE_INTEGRITY: integrity, FAKE_TARBALL: tarball },
    });
  return { root, repo, client: join(root, "tab-mcp"), integrity, run };
}

/** Every file the script may write, with its contents. */
function snapshot(repo) {
  const vendor = join(repo, "web", "vendor", "tab-mcp-client");
  const files = { "web/ai-local.js": readFileSync(join(repo, "web", "ai-local.js"), "utf8"), justfile: readFileSync(join(repo, "justfile"), "utf8") };
  for (const n of readdirSync(vendor).sort()) files[`vendor/${n}`] = readFileSync(join(vendor, n), "utf8");
  return files;
}

test("a run takes the build: the files, vendor.json, CONFIRM_MODULE; with --pin the justfile", { skip }, () => {
  const t = tree();
  try {
    const local = t.run("--from-local", t.client);
    assert.equal(local.status, 0, local.stderr);
    let after = snapshot(t.repo);
    assert.deepEqual(Object.keys(after).filter((k) => k.startsWith("vendor/")), ["vendor/confirm-new1.min.js", "vendor/tab-mcp-client.min.js", "vendor/vendor.json"]);
    assert.equal(after["web/ai-local.js"], 'export const CONFIRM_MODULE = "./vendor/tab-mcp-client/confirm-new1.min.js";\n');
    const npm = t.run("--from-npm", "9.9.9", "--pin");
    assert.equal(npm.status, 0, npm.stderr);
    after = snapshot(t.repo);
    assert.equal(after.justfile, `tab_mcp_client_version := "9.9.9"\ntab_mcp_client_integrity := "${t.integrity}"\n`);
  } finally {
    rmSync(t.root, { recursive: true, force: true });
  }
});

test("no CONFIRM_MODULE line: the run fails and changes nothing", { skip }, () => {
  const t = tree({ aiLocal: "// no such line\n" });
  try {
    const before = snapshot(t.repo);
    for (const args of [["--from-local", t.client], ["--from-npm", "9.9.9"], ["--from-npm", "9.9.9", "--pin"]]) {
      const r = t.run(...args);
      assert.equal(r.status, 1, `${args.join(" ")}: ${r.stdout}`);
      assert.match(r.stderr, /no CONFIRM_MODULE line/);
      assert.deepEqual(snapshot(t.repo), before, args.join(" "));
    }
  } finally {
    rmSync(t.root, { recursive: true, force: true });
  }
});

test("--pin without the justfile's version and integrity: the run fails and changes nothing", { skip }, () => {
  for (const justfile of ["other := 1\n", 'tab_mcp_client_version := "0.0.1"\n', 'tab_mcp_client_version := "0.0.1" # note\ntab_mcp_client_integrity := "sha512-old"\n']) {
    const t = tree({ justfile });
    try {
      const before = snapshot(t.repo);
      const r = t.run("--from-npm", "9.9.9", "--pin");
      assert.equal(r.status, 1, r.stdout);
      assert.match(r.stderr, /no tab_mcp_client_version\/integrity/);
      assert.deepEqual(snapshot(t.repo), before, justfile);
    } finally {
      rmSync(t.root, { recursive: true, force: true });
    }
  }
});
