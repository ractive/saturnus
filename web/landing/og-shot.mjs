// The Open Graph image of the landing page and the docs (img/og.png,
// 1200x630): the card a link to the site shows on Hacker News, Reddit,
// Mastodon and in chat apps. Laid out as HTML on the landing page's
// tokens (the wordmark, a headline, the twelve models, the address) beside
// the 48GX drawing of the model cards (img/48gx-640.webp), rendered in
// headless Chrome (the page tests' chrome.mjs) at 1200x630, device scale 1.
// Writes the HTML to raw/og.html (gitignored) and the image to img/og.png.
// Usage: node og-shot.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import { serve, chrome, findChrome } from "../test/chrome.mjs";

const RAW = new URL("raw/", import.meta.url).pathname;
const IMG = new URL("img/", import.meta.url).pathname;
mkdirSync(RAW, { recursive: true });

const HTML = `<!doctype html>
<meta charset="utf-8">
<style>
  html, body { margin: 0; width: 1200px; height: 630px; overflow: hidden; }
  body {
    background: #f1efe9; color: #23221e;
    font-family: "Helvetica Neue", Helvetica, Arial, sans-serif;
    display: flex; align-items: center; box-sizing: border-box; padding: 0 76px 0 84px;
  }
  main { flex: 1; }
  .mark { display: flex; align-items: center; gap: 18px; font-size: 38px; font-weight: 700; letter-spacing: -0.01em; }
  .mark img { width: 58px; height: 58px; }
  h1 { font-size: 66px; line-height: 1.08; letter-spacing: -0.02em; margin: 44px 0 26px; font-weight: 700; }
  p { margin: 0; font-size: 25px; line-height: 1.5; color: #6c6962; max-width: 640px; }
  .url { margin-top: 40px; font-size: 25px; color: #6c6962; }
  .calc { height: 560px; margin-left: 36px; }
</style>
<main>
  <div class="mark"><img src="../../logo.svg" alt="">Saturnus</div>
  <h1>Classic HP calculators, in your browser.</h1>
  <p>The HP 48SX, 48GX, 49G, 38G, 39G, 40G, 42S, 27S, 17BII, 17B, 28S and 19BII, emulated on their own ROMs. Free, in the browser and on Mac, Windows and Linux.</p>
  <div class="url">ractive.ch/saturnus</div>
</main>
<img class="calc" src="../img/48gx-640.webp" alt="">
`;
writeFileSync(`${RAW}og.html`, HTML);

const { server, port } = await serve();
const binary = findChrome();
if (!binary) throw new Error("no Chrome");
const c = await chrome(binary);
try {
  const { send, ev } = c;
  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1200, height: 630, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/landing/raw/og.html` });
  const end = Date.now() + 10_000;
  while (!(await ev("document.readyState === 'complete' && [...document.images].every((i) => i.complete && i.naturalWidth > 0)").catch(() => false))) {
    if (Date.now() > end) throw new Error("the card's images did not load");
    await new Promise((r) => setTimeout(r, 100));
  }
  const shot = await send("Page.captureScreenshot", { format: "png", clip: { x: 0, y: 0, width: 1200, height: 630, scale: 1 } });
  writeFileSync(`${IMG}og.png`, Buffer.from(shot.data, "base64"));
  console.log("saved img/og.png");
} finally {
  await c.close();
  server.close();
}
