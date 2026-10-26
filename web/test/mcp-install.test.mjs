// The AI apps' install links and commands (web/shared/mcp-install.js):
// exact output for an address, with and without headers, the shell
// quoting, and the encodings decoding back to the config.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { CODEX_FEATURE, claudeCodeCommand, codexSetup, cursorLink, installTargets, shellQuote, vscodeLink } from "../shared/mcp-install.js";

const URL_ = "https://tab-mcp.ractive.ch/a/AbC123/mcp";
const plain = { name: "saturnus", url: URL_ };
const local = { name: "saturnus", url: "http://127.0.0.1:4870/mcp", headers: { Authorization: "Bearer t0k'en x" } };

function cursorConfig(href) {
  const u = new URL(href);
  assert.equal(u.protocol, "cursor:");
  assert.equal(u.host, "anysphere.cursor-deeplink");
  assert.equal(u.pathname, "/mcp/install");
  return { name: u.searchParams.get("name"), config: JSON.parse(Buffer.from(u.searchParams.get("config"), "base64").toString("utf8")) };
}

test("Cursor: name and base64 of the server object", () => {
  const href = cursorLink(plain);
  const b64 = Buffer.from(JSON.stringify({ url: URL_ })).toString("base64");
  assert.equal(href, `cursor://anysphere.cursor-deeplink/mcp/install?name=saturnus&config=${encodeURIComponent(b64)}`);
  assert.deepEqual(cursorConfig(href), { name: "saturnus", config: { url: URL_ } });
  assert.deepEqual(cursorConfig(cursorLink(local)), { name: "saturnus", config: { url: local.url, headers: local.headers } });
  // UTF-8 survives the base64.
  assert.deepEqual(cursorConfig(cursorLink({ name: "s", url: "https://x.example/ü" })).config, { url: "https://x.example/ü" });
});

test("VS Code: URL-encoded JSON with type http", () => {
  const href = vscodeLink(plain);
  assert.equal(href, `vscode:mcp/install?${encodeURIComponent(`{"name":"saturnus","type":"http","url":"${URL_}"}`)}`);
  const decode = (h, scheme) => {
    assert.ok(h.startsWith(`${scheme}:mcp/install?`), h);
    return JSON.parse(decodeURIComponent(h.slice(`${scheme}:mcp/install?`.length)));
  };
  assert.deepEqual(decode(vscodeLink(local), "vscode"), { name: "saturnus", type: "http", url: local.url, headers: local.headers });
  assert.deepEqual(decode(vscodeLink(plain, true), "vscode-insiders"), { name: "saturnus", type: "http", url: URL_ });
});

test("Claude Code: the command, headers after the address", () => {
  assert.equal(claudeCodeCommand(plain), `claude mcp add --transport http saturnus ${URL_}`);
  assert.equal(claudeCodeCommand(local), `claude mcp add --transport http saturnus http://127.0.0.1:4870/mcp --header "Authorization: Bearer t0k'en x"`);
  // A value with $ falls back to POSIX single quotes.
  assert.equal(claudeCodeCommand({ ...plain, headers: { "X-Key": "a$b'c" } }), `claude mcp add --transport http saturnus ${URL_} --header 'X-Key: a$b'\\''c'`);
});

test("Codex: the command, or config.toml with headers", () => {
  assert.deepEqual(codexSetup(plain), { kind: "command", text: `codex mcp add saturnus --url ${URL_}` });
  assert.deepEqual(codexSetup(local), {
    kind: "config",
    text: `[mcp_servers.saturnus]\nurl = "http://127.0.0.1:4870/mcp"\nhttp_headers = { "Authorization" = "Bearer t0k'en x" }\n`,
  });
  // Only 2026-07-28 (the relay): Codex's feature turned on first, one command per line.
  assert.deepEqual(codexSetup({ ...plain, newOnly: true }), { kind: "command", text: `codex features enable ${CODEX_FEATURE}\ncodex mcp add saturnus --url ${URL_}` });
  // With headers (the desktop app, which speaks both) the config alone.
  assert.equal(codexSetup({ ...local, newOnly: true }).text, codexSetup(local).text);
  assert.match(codexSetup({ name: "a b", url: URL_, headers: { X: 'q"\\' } }).text, /^\[mcp_servers\."a b"\]\n.*\nhttp_headers = \{ "X" = "q\\"\\\\" \}\n$/);
});

test("shellQuote: plain words as they are, double quotes where every shell agrees, else single quotes", () => {
  assert.equal(shellQuote(URL_), URL_);
  assert.equal(shellQuote("http://127.0.0.1:4870/mcp"), "http://127.0.0.1:4870/mcp");
  // Double quotes: the same word in POSIX shells, PowerShell and cmd.exe.
  assert.equal(shellQuote("a b"), '"a b"');
  assert.equal(shellQuote("it's"), `"it's"`);
  assert.equal(shellQuote("https://x.example/?a=1&b=2"), '"https://x.example/?a=1&b=2"');
  assert.equal(shellQuote("Authorization: Bearer abc.DEF-123_~+/="), '"Authorization: Bearer abc.DEF-123_~+/="');
  // Characters that mean something inside double quotes somewhere: POSIX single quotes.
  assert.equal(shellQuote("$(rm -rf ~)"), "'$(rm -rf ~)'");
  assert.equal(shellQuote("it's $x"), `'it'\\''s $x'`);
  for (const c of ['"', "$", "`", "\\", "!", "%"]) assert.equal(shellQuote(`a${c}b`), `'a${c}b'`, c);
  // The shell reads each back as the one word it was.
  for (const s of ["a b", "it's", "x?a=1&b=2;c|d", "$(x) `y` \"z\" \\ ;|&", "'", "''", "100%", "hi!"]) {
    assert.equal(execFileSync("sh", ["-c", `printf %s ${shellQuote(s)}`]).toString(), s);
  }
});

test("installTargets: every app, in order", () => {
  const t = installTargets(plain);
  assert.deepEqual(t.map((x) => [x.id, x.kind]), [["cursor", "link"], ["vscode", "link"], ["claude-code", "command"], ["codex", "command"], ["connector", "note"]]);
  assert.equal(t[3].text, codexSetup(plain).text);
  assert.equal(installTargets({ ...plain, newOnly: true })[3].text, codexSetup({ ...plain, newOnly: true }).text);
  assert.equal(t[0].href, cursorLink(plain));
  assert.equal(t[1].href, vscodeLink(plain));
  assert.equal(t[1].insidersHref, vscodeLink(plain, true));
  assert.equal(t[2].text, claudeCodeCommand(plain));
  assert.equal(installTargets(local)[3].kind, "config");
  // The note points at Codex's feature only when that command shows it.
  assert.doesNotMatch(t[4].text, /Codex's feature/);
  assert.match(installTargets({ ...plain, newOnly: true })[4].text, /it needs Codex's feature above/);
});
