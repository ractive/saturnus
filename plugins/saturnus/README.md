# The saturnus plugin

For [saturnus](https://ractive.ch/saturnus/), which runs classic HP calculators
(48SX, 48GX, 49G, 28S, 42S, 27S, 17BII, 17B, 19BII, 38G, 39G, 40G) with their
real ROMs. The plugin gives an AI app two things:

- **The saturnus skill** (`skills/saturnus/`): how to use the calculators well
  through saturnus's MCP tools. RPL programming, the models' differences, the
  screens that catch key presses, the documented ROM bugs, and worked tasks.
  The tools' own descriptions stay the core; the skill adds to them.
- **The desktop app's calculators as tools** (`.mcp.json`, Claude Code only):
  the MCP server the saturnus desktop app runs on this computer.

## Claude Code

```
/plugin marketplace add ractive/saturnus
/plugin install saturnus@saturnus
```

Then, in the saturnus desktop app, open **Connect your AI app…** and turn on
"Let AI apps on this computer use the calculators". Start Claude Code (or run
`/mcp` and reconnect `saturnus`) and ask it to work on a calculator.

### How the plugin signs in

The desktop app's server is `http://127.0.0.1:4842/mcp`. It accepts a request
only with the per-user key from the token file the desktop app writes when AI
access is turned on:

| System | Token file |
|---|---|
| macOS, Linux | `$XDG_CONFIG_HOME/saturnus/control-token`, by default `~/.config/saturnus/control-token` |
| Windows | `%LOCALAPPDATA%\saturnus\control-token` |

The plugin holds no key. Its server entry has a `headersHelper`
(`scripts/token-header.sh`) that Claude Code runs at each connection; it reads
the token file and hands Claude Code the `Authorization` header. The key never
goes into the plugin, your settings or the project.

- Another port: if you moved the desktop app's AI access to another port, set
  `SATURNUS_MCP_PORT` before starting Claude Code.
- `SATURNUS_TOKEN_FILE` does not reach the helper: Claude Code removes
  variables with TOKEN in their name before it runs a plugin's helper. Keep
  the token file in its default place.
- Windows: the helper is a POSIX shell script, so it needs Claude Code's
  shell to be one, such as Git Bash. It has not been tried on Windows yet.
- The desktop app not running, or AI access off: `/mcp` shows the server as
  failed. Turn AI access on and reconnect.
- If you also added saturnus with the dialog's `claude mcp add` command, you
  have its tools twice; remove one with `claude mcp remove saturnus`.

### The calculators in the browser

The plugin's server is the desktop app's. For the calculators in the browser,
open **Connect your AI app…** in the app and run the `claude mcp add` command it
gives. The plugin's skill works with either.

## Other AI apps

The skill is a plain Agent Skills folder:

- **Codex**: copy `skills/saturnus` to `.agents/skills/saturnus` in your
  project, and add the server with the command from **Connect your AI app…**.
- **pi**: `pi install git:github.com/ractive/saturnus` (the repository's
  `package.json` names the skill).
- **Claude (desktop, claude.ai)**: zip `skills/saturnus` and upload it as a
  skill in Claude's settings. Claude and ChatGPT use the address from the
  app in the browser, added as a custom connector.

More in the docs: <https://ractive.ch/saturnus/docs/ai-apps/>.
