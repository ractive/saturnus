---
type: iteration
title: "Iteration 77: read_guide, and WebMCP instructions past the 1000-character cut"
date: 2026-11-16
status: in-progress
tags: [iteration, web, ai]
branch: iter-77/read-guide
---

# Iteration 77: read_guide, and WebMCP instructions past the 1000-character cut

## Why

WebMCP has no server instructions, so `web/ai-webmcp.js` appends
`WEBMCP_NOTE` and `INSTRUCTIONS` to `list_calculators`'s description
(5879 characters). The tab-mcp extension cuts every page tool's
description at 1000 characters (`MAX_DESCRIPTION` in tab-mcp's
`src/worker/bridge.ts`), so an AI app reaching Saturnus through the
extension sees the tool's own text, the WebMCP note and about 220
characters of the instructions. Every other tool's description is
under 900 characters. Tool *results* pass up to 100 KB.

Separately, the plugin's skill (`plugins/saturnus/skills/saturnus/`:
`SKILL.md` and `references/rpl.md`, `models.md`, `workflows.md`, about
21 KB) reaches only Claude Code with the plugin. AI apps that reach
Saturnus through tab-mcp (client library, web/ai.js), the desktop app's
endpoint (web/ai-local.js) or WebMCP get only tool descriptions and,
where the transport has them, the server instructions.

## Design

1. **WebMCP: instructions in the result.** `list_calculators`'s WebMCP
   description stays under 1000 characters: its own description, the
   WebMCP note, and one line saying its result carries how to use the
   tools and that `read_guide` has more. Under WebMCP its result gains
   an `instructions` field holding `webMcpText(INSTRUCTIONS)` (every
   call: an AI app's chat and the page's lifetime don't line up, so
   "first call only" cannot be told apart). Add it to `OUT.calculators`.
   The tab-mcp and desktop paths keep real server instructions and do
   not get the field.
2. **`read_guide({ topic })`**, a read-only tool on every path (tab-mcp,
   desktop endpoint, WebMCP; registered with no calculator shown, like
   `lookup_command`). Topics: `skill` (SKILL.md without its frontmatter),
   `rpl`, `models`, `workflows`; unknown topic → error naming the topics.
   The plugin's files stay the one source: the web build makes them
   available to the page (bundled or copied into the build like the
   other generated data; follow how `web/build.sh` and the deploy ship
   files), no second hand-kept copy. Result: `{ topic, text }`. The
   description says when to read which topic, and its annotations mark
   it read-only (not untrusted content: the text is ours).
3. `INSTRUCTIONS` gains one sentence pointing at `read_guide` (rpl
   before writing RPL programs, models before moving work between
   models). SKILL.md's text says nothing Claude-Code-specific that is
   wrong when served this way (check its references to "this skill" and
   the references/ paths; reword if needed, keep it a valid skill).
4. Docs that list the tools (web/README.md, web/docs/, the plugin
   README, the skill's own tool list in its description) name read_guide.

## Tasks

- [x] A test asserts every WebMCP tool description is at most 1000
  characters (after `webMcpText` and the list_calculators additions).
- [x] `list_calculators` under WebMCP: short description, `instructions`
  in the result, `OUT.calculators` updated; tests in
  web/test/ai-webmcp.test.mjs.
- [x] `read_guide` in web/ai-tools.js with its topics, served from the
  plugin's files through the build; tests in web/test/ai-tools.test.mjs
  (each topic returns the file's text, unknown topic errors).
- [x] read_guide works on the desktop endpoint (web/ai-local.js) and
  through tab-mcp (web/ai.js); existing tool-list tests updated.
- [x] INSTRUCTIONS points at read_guide; docs and the skill's tool list
  updated.
- [x] `just web-test` and the touched page tests pass; CHANGELOG
  Unreleased entry.

## Out of scope

- Raising tab-mcp's 1000-character limit (tab-mcp's own decision).
- Serving the skill as MCP resources or prompts.
