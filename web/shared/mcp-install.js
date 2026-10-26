// One-click ways to add an MCP address to the common AI apps, for
// "Connect your AI app" (web/ai-dialog.js): the same code in the browser
// (the relay's address) and in the desktop app (a local address with a
// bearer token header). Pure functions, no DOM.
//
// The formats, as their docs give them (checked 2026-10-23):
// - Cursor: cursor://anysphere.cursor-deeplink/mcp/install?name=NAME&config=BASE64,
//   the config the bare server object of mcp.json ({url, headers}) as
//   base64 of its JSON.
//   https://cursor.com/docs/context/mcp/install-links, https://cursor.com/docs/context/mcp
// - VS Code: vscode:mcp/install?<encodeURIComponent(JSON)> (Insiders:
//   vscode-insiders:), the JSON {name, type: "http", url, headers}.
//   https://code.visualstudio.com/docs/copilot/guides/mcp-developer-guide
//   ("Create an MCP installation URL"),
//   https://code.visualstudio.com/docs/agents/reference/mcp-configuration
// - Claude Code: claude mcp add --transport http NAME URL --header "Name: value".
//   https://code.claude.com/docs/en/mcp
// - Codex: codex mcp add NAME --url URL; it has no flag for headers, so
//   with headers a config.toml table ([mcp_servers.NAME] url, http_headers).
//   https://learn.chatgpt.com/docs/extend/mcp?surface=cli (was developers.openai.com/codex/mcp)
//   Codex 0.163 speaks the older MCP revision unless its feature
//   mcp_2026_07_28 is on, and the relay speaks only 2026-07-28: without
//   it Codex signs in but lists no tools (seen 2026-10-23). So for the
//   relay the command turns the feature on first (`codex features
//   enable`, kept in config.toml). The desktop app's server speaks both.
// - Claude (desktop, claude.ai): no link; the address goes in Settings as
//   a custom connector. ChatGPT: under Plugins as an MCP server; the Mac
//   app's MCPs are Codex's (tab-mcp client 0.9's dialog, 2026-10-25).

/** Base64 of a string's UTF-8 bytes (btoa alone takes Latin-1 only). */
function base64(text) {
  let bin = "";
  for (const b of new TextEncoder().encode(text)) bin += String.fromCharCode(b);
  return btoa(bin);
}

/**
 * `s` as one shell word. As it is when every character means itself in
 * POSIX shells, PowerShell and cmd.exe (letters, digits, _ . / : -).
 * Else in double quotes when it has none of " $ ` \ ! %: the same word in
 * all three (a token, an http(s) address with a query). Else, POSIX only,
 * in single quotes, a single quote inside as '\''.
 */
export function shellQuote(s) {
  if (/^[A-Za-z0-9_./:-]+$/.test(s)) return s;
  if (!/["$`\\!%]/.test(s)) return `"${s}"`;
  return `'${s.replaceAll("'", `'\\''`)}'`;
}

/** Codex's feature for MCP 2026-07-28, off by default in 0.163. */
export const CODEX_FEATURE = "mcp_2026_07_28";

/** The server object the apps' JSON configs share: the address and its headers, if any. */
function server(url, headers) {
  return Object.keys(headers).length ? { url, headers } : { url };
}

/** Cursor's install link. */
export function cursorLink({ name, url, headers = {} }) {
  return `cursor://anysphere.cursor-deeplink/mcp/install?name=${encodeURIComponent(name)}&config=${encodeURIComponent(base64(JSON.stringify(server(url, headers))))}`;
}

/** VS Code's install link; `insiders` for VS Code Insiders. */
export function vscodeLink({ name, url, headers = {} }, insiders = false) {
  const config = { name, type: "http", ...server(url, headers) };
  return `${insiders ? "vscode-insiders" : "vscode"}:mcp/install?${encodeURIComponent(JSON.stringify(config))}`;
}

/** The Claude Code command that adds the server. */
export function claudeCodeCommand({ name, url, headers = {} }) {
  const parts = ["claude", "mcp", "add", "--transport", "http", shellQuote(name), shellQuote(url)];
  for (const [k, v] of Object.entries(headers)) parts.push("--header", shellQuote(`${k}: ${v}`));
  return parts.join(" ");
}

/**
 * Codex: the command without headers; with headers (no flag takes them)
 * the table for ~/.codex/config.toml. TOML's basic strings take JSON's
 * escapes, so JSON.stringify writes them. `newOnly`: the server speaks
 * only MCP 2026-07-28 (the relay), so the command first turns on Codex's
 * feature for it, one command per line (every shell runs them in turn).
 */
export function codexSetup({ name, url, headers = {}, newOnly = false }) {
  const entries = Object.entries(headers);
  if (!entries.length) {
    const add = `codex mcp add ${shellQuote(name)} --url ${shellQuote(url)}`;
    return { kind: "command", text: newOnly ? `codex features enable ${CODEX_FEATURE}\n${add}` : add };
  }
  const q = JSON.stringify;
  const key = /^[A-Za-z0-9_-]+$/.test(name) ? name : q(name);
  const lines = [`[mcp_servers.${key}]`, `url = ${q(url)}`];
  lines.push(`http_headers = { ${entries.map(([k, v]) => `${q(k)} = ${q(v)}`).join(", ")} }`);
  return { kind: "config", text: lines.join("\n") + "\n" };
}

/**
 * What to show per AI app for the server `name` at `url` with `headers`
 * (an object of header names to values): a link that opens the app, a
 * command or config to copy, or a line of text. `newOnly` as for
 * codexSetup.
 */
export function installTargets({ name, url, headers = {}, newOnly = false }) {
  const s = { name, url, headers };
  const codex = codexSetup({ ...s, newOnly });
  return [
    { id: "cursor", app: "Cursor", kind: "link", href: cursorLink(s) },
    { id: "vscode", app: "VS Code", kind: "link", href: vscodeLink(s), insidersHref: vscodeLink(s, true) },
    { id: "claude-code", app: "Claude Code", kind: "command", text: claudeCodeCommand(s) },
    { id: "codex", app: "Codex", kind: codex.kind, text: codex.text },
    { id: "connector", app: "Claude, ChatGPT", kind: "note", text: "In Claude, add the address in Settings as a custom connector. In ChatGPT, add it under Plugins as an MCP server (the Mac app: Settings → Plugins → MCPs → Add, Streamable HTTP; it needs Codex's feature above)." },
  ];
}
