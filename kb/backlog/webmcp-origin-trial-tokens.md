---
type: backlog
title: Renew the WebMCP origin trial tokens before they expire
date: 2026-11-08
status: planned
priority: high
due: 2026-12-23
tags:
  - backlog
  - web
  - ai
---

# Renew the WebMCP origin trial tokens before they expire

`web/index.html` carries two WebMCP origin trial tokens for
`https://ractive.ch` (iteration 71, [[iterations/iteration-71-webmcp]]).
When a token expires, that browser stops offering `document.modelContext`
to the page and its agent no longer sees saturnus's tools; tab-mcp is not
affected.

- [ ] **Edge, by 2026-12-23** (the token's expiry, 20:52 UTC): renew it in
  Edge's origin trials console
  (<https://developer.microsoft.com/en-us/microsoft-edge/origin-trials/trials/0b76fe60-b266-458e-a285-04e375c0c31a>),
  replace the Edge `<meta http-equiv="origin-trial">` in `web/index.html`
  and its comment, deploy the web page.
- [ ] **Chrome, by 2027-03-30**: renew in the Chrome Origin Trials console
  (<https://developer.chrome.com/origintrials/#/trials/active>, WebMCP), or
  drop the token if WebMCP has shipped in Chrome by then.

The owner registers the origin (a Google and a Microsoft account); Claude
replaces the tokens. Check a new token's payload (base64: origin, feature,
expiry) before it goes in.
