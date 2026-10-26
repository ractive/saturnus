// The landing page's model cards (web/landing/index.html): each card's
// "More" and its picture open the model's <dialog> by invoker
// commands (commandfor/command), Esc closes it, so does a
// click outside (closedby="any") and the close button. Every dialog is
// labelled by its heading, no id is used twice, and the dialog fits a
// 390px phone without sideways scrolling. ‹ and › (and ← →, dialogs.js)
// step to the neighbouring model, one dialog open at a time. Needs Chrome 135 or later (the
// page, not the wasm package); skipped without Chrome.
import { test } from "node:test";
import assert from "node:assert/strict";
import { chrome, findChrome, serve, sleep } from "./chrome.mjs";

const MODELS = ["48sx", "48gx", "49g", "38g", "39g", "40g", "42s", "27s", "17bii", "17b", "28s", "19bii"];

async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(50);
  }
}

test("each model card opens its dialog; Esc, a click outside and × close it", { timeout: 90_000 }, async (t) => {
  const binary = findChrome();
  if (!binary) return t.skip("no Chrome found (SATURNUS_CHROME=/path/to/chrome)");
  const { server, port } = await serve();
  const c = await chrome(binary, (m) => t.diagnostic(m));
  try {
    const { send, ev } = c;
    await send("Page.enable");
    await send("Runtime.enable");
    await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/landing/` });
    await until(ev, `document.readyState === "complete" && document.querySelectorAll(".model").length === 12`, 15_000, "the landing page loaded");

    // The markup: one dialog per card, labelled, no id twice.
    const markup = await ev(`(() => {
      const ids = [...document.querySelectorAll("[id]")].map((e) => e.id);
      const dlgs = [...document.querySelectorAll("dialog.model-dlg")];
      return {
        dupes: ids.filter((id, i) => ids.indexOf(id) !== i),
        dialogs: dlgs.map((d) => d.id),
        labelled: dlgs.every((d) => (document.getElementById(d.getAttribute("aria-labelledby"))?.textContent ?? "").includes("HP ")),
        closedby: dlgs.every((d) => d.getAttribute("closedby") === "any"),
        scripts: document.scripts.length,
      };
    })()`);
    assert.deepEqual(markup.dupes, []);
    assert.deepEqual(markup.dialogs, MODELS.map((m) => `dlg-${m}`));
    assert.ok(markup.labelled, "every dialog is labelled by its heading");
    assert.ok(markup.closedby, "every dialog closes on Esc and a click outside");
    assert.equal(markup.scripts, 1, "one script: dialogs.js, ← and → between the dialogs");

    const open = (id) => ev(`document.getElementById(${JSON.stringify(id)}).open && document.getElementById(${JSON.stringify(id)}).matches(":modal")`);
    const esc = async () => {
      await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
      await send("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
    };
    for (const [i, m] of MODELS.entries()) {
      const id = `dlg-${m}`;
      const more = await ev(`(() => { const b = document.querySelectorAll(".model .more button")[${i}]; return { label: b.getAttribute("aria-label"), target: b.getAttribute("commandfor") }; })()`);
      assert.equal(more.target, id);
      assert.match(more.label, /^More about the HP /);
      // A real click on "More", scrolled into sight first (the row scrolls).
      const box = await ev(`(() => { const b = document.querySelectorAll(".model .more button")[${i}]; b.scrollIntoView({ block: "center", inline: "center", behavior: "instant" }); const r = b.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
      await send("Input.dispatchMouseEvent", { type: "mousePressed", x: box.x, y: box.y, button: "left", clickCount: 1 });
      await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: box.x, y: box.y, button: "left", clickCount: 1 });
      await until(ev, `document.getElementById("${id}").open`, 3_000, `${m}: More opens its dialog`);
      assert.ok(await open(id), `${m}: modal`);
      assert.match(await ev(`document.activeElement.closest("dialog")?.id ?? ""`), new RegExp(`^${id}$`), `${m}: focus in the dialog`);
      assert.equal(await ev(`document.querySelector("#${id} .dlg-actions a.primary").getAttribute("href")`), `app/#/calculator/${m}`);
      assert.equal(await ev(`document.querySelector('#${id} .dlg-actions a[href^="docs/"]').getAttribute("href")`), `docs/${m}/`, "the model's docs page");
      await esc();
      await until(ev, `!document.getElementById("${id}").open`, 3_000, `${m}: Esc closes it`);
    }

    // The picture opens it too; a click outside and × close it.
    await ev(`document.querySelectorAll(".model .model-pic")[10].click(); true`);
    await until(ev, `document.getElementById("dlg-28s").open`, 3_000, "the picture opens it");
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x: 8, y: 8, button: "left", clickCount: 1 });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: 8, y: 8, button: "left", clickCount: 1 });
    await until(ev, `!document.getElementById("dlg-28s").open`, 3_000, "a click outside closes it");
    await ev(`document.querySelectorAll(".model .model-pic")[2].click(); true`);
    await until(ev, `document.getElementById("dlg-49g").open`, 3_000, "the picture opens it");
    await ev(`document.querySelector("#dlg-49g .dlg-close").click(); true`);
    await until(ev, `!document.getElementById("dlg-49g").open`, 3_000, "× closes it");

    // › and ‹ step to the neighbour, ← and → too; one dialog open at a time, the ends wrap.
    const opened = () => ev(`[...document.querySelectorAll("dialog[open]")].map((d) => d.id).join()`);
    const key = async (k, code) => {
      await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: k, code, windowsVirtualKeyCode: code === "ArrowLeft" ? 37 : 39 });
      await send("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, windowsVirtualKeyCode: code === "ArrowLeft" ? 37 : 39 });
    };
    await ev(`document.getElementById("dlg-49g").showModal(); true`);
    await ev(`document.querySelector("#dlg-49g .dlg-next").click(); true`);
    await until(ev, `document.getElementById("dlg-38g").open`, 3_000, "› opens the next");
    assert.equal(await opened(), "dlg-38g", "› closes the one it sits in");
    await key("ArrowLeft", "ArrowLeft");
    await until(ev, `document.getElementById("dlg-49g").open`, 3_000, "← opens the previous");
    await ev(`document.querySelector("#dlg-49g .dlg-prev").click(); true`);
    await key("ArrowRight", "ArrowRight");
    await key("ArrowRight", "ArrowRight");
    await until(ev, `document.getElementById("dlg-38g").open`, 3_000, "‹ then → →");
    assert.equal(await opened(), "dlg-38g");
    await ev(`document.getElementById("dlg-38g").close(); document.getElementById("dlg-48sx").showModal(); true`);
    await key("ArrowLeft", "ArrowLeft");
    await until(ev, `document.getElementById("dlg-19bii").open`, 3_000, "← on the first wraps to the last");
    assert.equal(await opened(), "dlg-19bii");
    await esc();
    await until(ev, `!document.querySelector("dialog[open]")`, 3_000, "Esc closes it");

    // The screenshots: each link opens its picture in its dialog instead of
    // the image file; × and a click outside close it.
    for (const id of ["dlg-app", "dlg-files", "dlg-ai"]) {
      const before = await ev("location.href");
      await ev(`document.querySelector('a.shot-open[data-dialog="${id}"]').click(); true`);
      await until(ev, `document.getElementById("${id}").open`, 3_000, `${id} opens`);
      assert.equal(await ev("location.href"), before, `${id}: the link does not navigate`);
      await ev(`document.querySelector("#${id} .dlg-close").click(); true`);
      await until(ev, `!document.getElementById("${id}").open`, 3_000, `× closes ${id}`);
    }
    await ev(`document.querySelector('a.shot-open[data-dialog="dlg-files"]').click(); true`);
    await until(ev, `document.getElementById("dlg-files").open`, 3_000, "dlg-files opens again");
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x: 8, y: 8, button: "left", clickCount: 1 });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: 8, y: 8, button: "left", clickCount: 1 });
    await until(ev, `!document.getElementById("dlg-files").open`, 3_000, "a click outside closes it");

    // A phone: a sheet the screen's width, nothing wider than the page.
    await send("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 1, mobile: true });
    await ev(`document.querySelectorAll(".model .more button")[10].click(); true`);
    await until(ev, `document.getElementById("dlg-28s").open`, 3_000, "opens on a phone");
    await sleep(300);
    const phone = await ev(`(() => {
      const d = document.getElementById("dlg-28s"), r = d.getBoundingClientRect(), body = d.querySelector(".dlg-body");
      return { left: Math.round(r.left), width: Math.round(r.width), bottom: Math.round(r.bottom), wide: body.scrollWidth > body.clientWidth, page: document.documentElement.scrollWidth };
    })()`);
    assert.deepEqual(phone, { left: 0, width: 390, bottom: 844, wide: false, page: 390 });
    assert.deepEqual(c.errors, []);
  } finally {
    await c.close();
    server.closeAllConnections();
    server.close();
  }
});
