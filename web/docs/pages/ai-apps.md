---
title: AI apps
description: Let an AI app such as Claude Code, Codex, Cursor, Claude Desktop, Claude or ChatGPT use the calculators in saturnus, and add the saturnus skill and the Claude Code plugin.
section: index
---

An AI app can use the calculators you have open in saturnus: read the display, press keys, run RPL, store programs on the 48 and 49 models and look up commands in the reference. It works over [MCP](https://modelcontextprotocol.io/), the protocol AI apps use for tools. You watch the calculator while the AI app works, and you allow each store or purge of a variable in the app first.

## Connect an AI app

In the app, choose **Connect your AI app…** (in the panel beside the calculator, or in the command palette). The dialog gives an address and, for [Cursor](https://cursor.com/), [VS Code](https://code.visualstudio.com/), [Claude Code](https://claude.com/product/claude-code) and [Codex](https://en.wikipedia.org/wiki/OpenAI_Codex), a link or a command that adds it.

- **In the browser**, the address goes through a relay on the internet, so any AI app can use it, [Claude](https://claude.com/product/overview) and [ChatGPT](https://en.wikipedia.org/wiki/ChatGPT) included (Claude as a custom connector in its settings, ChatGPT as an MCP server under Plugins). The calculator keeps running in a background tab while an AI app is connected.
- **In the desktop app**, turn on "Let AI apps on this computer use the calculators". The address is local (`http://127.0.0.1:4842/mcp`) and nothing goes over the internet. The links and commands include a key from a file only your user account can read. [Claude Desktop](https://claude.com/download) gets an extension instead (below). Claude on the web and ChatGPT connect from the internet, so they cannot use this address.

The tools tell the AI app what it needs to start: which calculators run, which tools work on each model, and how to check its work on the display.

## Your browser's own AI

Some browsers have an AI of their own that can use a page's tools through [WebMCP](https://github.com/webmachinelearning/webmcp): [Chrome](https://www.google.com/chrome/) from version 149 and [Edge](https://www.microsoft.com/en-us/edge) from version 150 try it out on saturnus's site, and extensions for Chrome can use it too. In such a browser the app offers the same tools to the browser's AI as well, with no address or code: the tools for the calculator Calculator shows, and for every calculator the AI has listed. The display comes as text unless the AI asks for a picture. Only the browser's AI and the app itself can call them, not other sites.

Storing or purging a variable through the store and purge tools asks you in the app first, every time. The AI can still press keys and run commands, as you can, and those can store or purge too without asking. The question protects against a mistaken call; an AI that can click in the page could also click Allow, so give it the page only if you trust it. The calls show in **The AI's calls** under "Your browser's AI"; choose **Calls** in the panel beside the calculator. Connecting an AI app through the dialog works as before, beside it. The desktop app has no WebMCP.

## Claude Desktop

Claude Desktop uses the calculators of the desktop app through an extension:

1. In the desktop app, open **Connect your AI app…** and turn on "Let AI apps on this computer use the calculators".
2. Choose **Add to Claude Desktop**. Claude Desktop shows the extension; choose **Install**. You can also [download the extension](../app/saturnus.mcpb) and open it with Claude Desktop.

When Claude needs a calculator and saturnus is closed, the extension starts it. On Windows and Linux it finds saturnus only where its installers put it; otherwise start saturnus yourself. If you moved AI access to another port, set the same port in the extension's settings in Claude Desktop. The extension holds no key: it reads the key from the file the desktop app writes for your user account, and sends it only to saturnus on this computer, after saturnus has shown that it holds the same key.

## The saturnus skill

The skill adds what the tools do not say: RPL programming, the differences between the models (the 42S's menus, the 38G's HOME view, the business models' menus), the screens that catch key presses, the known ROM bugs, and worked examples such as a program on the 48GX, a loan on the 17BII and a linear fit on the 38G. It is optional; the tools work without it.

It is in the open [Agent Skills](https://agentskills.io/) format, a folder with `SKILL.md`, in the saturnus repository under [plugins/saturnus/skills/saturnus](https://github.com/ractive/saturnus/tree/main/plugins/saturnus/skills/saturnus).

| AI app | How to add it |
|---|---|
| Claude Code | Install the plugin (below), or copy the folder to `~/.claude/skills/saturnus` |
| Codex | Copy the folder to `.agents/skills/saturnus` in your project |
| pi | `pi install git:github.com/ractive/saturnus` |
| Claude (desktop and claude.ai) | Zip the folder and upload it as a skill in Claude's settings |

## The Claude Code plugin

The plugin brings the skill and the desktop app's calculators as tools in one install:

```
/plugin marketplace add ractive/saturnus
/plugin install saturnus@saturnus
```

Its tools connect to the desktop app on this computer, so turn on AI access in the desktop app first. The plugin reads the key from the same file the desktop app writes; it holds no key itself. If you moved the desktop app's AI access to another port, set `SATURNUS_MCP_PORT` to it before you start Claude Code. Run `/mcp` in Claude Code to see whether it is connected.

For the calculators in the browser, add the address from **Connect your AI app…** with the command the dialog gives; the plugin's skill works with either.

The plugin's [README](https://github.com/ractive/saturnus/blob/main/plugins/saturnus/README.md) has the details.
