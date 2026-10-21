---
type: iteration
title: "Iteration 57: the docs site and the command reference page"
date: 2026-10-21
status: in-progress
tags:
  - iteration
  - web
  - docs
branch: iter-57/docs-site
---

# Iteration 57: the docs site and the command reference page

The first two steps of the encyclopedia ([[backlog/encyclopedia]], owner decisions of
2026-10-21): the build for `/saturnus/docs/` and the shared command reference page. No
written content yet beyond a docs index and placeholders; the 48GX sample page comes
next, after this is merged.

## Design

### Build

- Sources: Markdown files with a small frontmatter (title, description, section,
  optional model) in `docs/site/` of this repository (or the place the site build
  already reads from, if that fits better; say which).
- A small script, run by the site build (`web/site.sh` and `pages.yml`), renders each
  page to HTML with the app's look (tokens, fonts, light and dark mode, the logo) and
  writes the Markdown beside it: `/saturnus/docs/48gx` (an `index.html` or `.html`
  that the server serves at that path) and `/saturnus/docs/48gx.md`. One Markdown
  library, pinned, or a small renderer if that is simpler; no static site generator.
- A model page takes that calculator's colours (as the logo and the model cards do);
  section pages keep the neutral colours.
- `/saturnus/llms.txt` in the llmstxt.org format (H1, summary blockquote, sections with
  link lists to the `.md` files; one entry per model's command file, not per command).
- A docs index page `/saturnus/docs/` with the four sections: Models, Command
  reference, How the Saturn works, Moving files to a real calculator.
- Each page ends with a "Sources" list of the online sources it used (links); never a
  link into the wiki.
- `.htaccess`/CSP: what the new paths need on ractive.ch (the site already has
  `web/site.htaccess`); `.md` served as `text/markdown; charset=utf-8`.

### Command reference page

- `/saturnus/docs/commands`: one dynamic page that loads `web/commands.json` and uses
  the app's `web/reference.js` (lookup and ranking) and
  `web/components/entry-view.js` (one command), read-only (no run action). A model
  picker, a search field, and a link per command through the address
  (`/saturnus/docs/commands#48gx/hms-`; readable slugs for names like `→LIST`, `%CH`,
  `∫`). Works without a ROM, and without the app's worker or emulator.
- The app's home page links to it (the reference without a ROM), and the command
  palette offers "Command reference".
- For LLMs the build writes one Markdown file per model with all its commands and
  examples (`/saturnus/docs/commands/48gx.md`, 12 files); `llms.txt` lists them.

## Tasks

- [x] Build script, sources folder, docs index, `.md` beside each page, `llms.txt`, in the site build and pages.yml
- [x] Command reference page sharing `reference.js`, `entry-view.js` and `commands.json`; model picker, search, links per command
- [x] The 12 generated command `.md` files
- [x] Links from the home page and the command palette
- [x] Tests in `web/test/` (build output, `llms.txt` format, the reference page in Chrome)
- [x] Screenshots: the docs index and the command reference (2 max)
- [ ] `just gates`, PR, review, merge, deploy

## As built

- Sources in `web/docs/pages/` (the site build reads `web/`), the build `web/docs/build.mjs`
  (Node, no dependencies) with its own small Markdown renderer (`web/docs/markdown.mjs`):
  the project has no JavaScript package manager to pin a library with.
- A page is `docs/<page>/index.html` beside `docs/<page>.md`; Apache's DirectorySlash
  sends `/saturnus/docs/48gx` to `/saturnus/docs/48gx/` (no rewrite rules), and the
  browser keeps the `#48gx/hms-` of an address across that redirect. The site links
  the slash form.
- The reference page imports the app's modules from `app/` beside `docs/` (one copy of
  each on the site); the entry's rules in `style.css` are scoped for `.docs-ref` too.
- Placeholders: the 12 model pages and the two unwritten sections; `llms.txt` leaves
  them out until written.
