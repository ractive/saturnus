---
title: AI apps
description: Let an AI app such as Claude Code, Codex, Cursor, Claude Desktop, Claude or ChatGPT use the calculators in saturnus, and add the saturnus skill and the Claude Code plugin.
section: index
---

An AI app can use the calculators you have open in saturnus: read the display, press keys, run RPL, store programs on the 48 and 49 models and look up commands in the reference. It works over MCP, the protocol AI apps use for tools. You watch the calculator while the AI app works, and you allow each store or purge of a variable in the app first.

## Connect an AI app

In the app, choose **Connect your AI app…** (in the panel beside the calculator, or in the command palette). The dialog gives an address and, for Cursor, VS Code, Claude Code and Codex, a link or a command that adds it.

- **In the browser**, the address goes through a relay on the internet, so any AI app can use it, Claude and ChatGPT included (as a custom connector in their settings). The calculator keeps running in a background tab while an AI app is connected.
- **In the desktop app**, turn on "Let AI apps on this computer use the calculators". The address is local (`http://127.0.0.1:4842/mcp`) and nothing goes over the internet. The links and commands include a key from a file only your user account can read. Claude Desktop gets an extension instead (below). Claude on the web and ChatGPT connect from the internet, so they cannot use this address.

The tools tell the AI app what it needs to start: which calculators run, which tools work on each model, and how to check its work on the display.

## Claude Desktop

Claude Desktop uses the calculators of the desktop app through an extension:

1. In the desktop app, open **Connect your AI app…** and turn on "Let AI apps on this computer use the calculators".
2. Choose **Add to Claude Desktop**. Claude Desktop shows the extension; choose **Install**. You can also [download the extension](../app/saturnus.mcpb) and open it with Claude Desktop.

When Claude needs a calculator and saturnus is closed, the extension starts it. If you moved AI access to another port, set the same port in the extension's settings in Claude Desktop. The extension holds no key: it reads the key from the file the desktop app writes for your user account, and sends it only to saturnus on this computer, after saturnus has shown that it holds the same key.

## The saturnus skill

The skill adds what the tools do not say: RPL programming, the differences between the models (the 42S's menus, the 38G's HOME view, the business models' menus), the screens that catch key presses, the known ROM bugs, and worked examples such as a program on the 48GX, a loan on the 17BII and a linear fit on the 38G. It is optional; the tools work without it.

It is in the open Agent Skills format, a folder with `SKILL.md`, in the saturnus repository under [plugins/saturnus/skills/saturnus](https://github.com/ractive/saturnus/tree/main/plugins/saturnus/skills/saturnus).

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
