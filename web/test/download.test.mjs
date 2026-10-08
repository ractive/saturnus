// The page's hpcalc.org links come from the one table of known images
// (crates/saturnus-host/src/romid.rs, `KNOWN`) through the real wasm
// (web/pkg, `just web`): `rom_download` per model, as the Worker puts it
// on each ROM slot. `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const pkg = await import("../pkg/saturnus_web.js");
pkg.initSync({ module: readFileSync(new URL("../pkg/saturnus_web_bg.wasm", import.meta.url)) });

test("every model but the 42S links to its image on hpcalc.org", () => {
  const files = {};
  for (const model of pkg.model_names()) {
    const d = pkg.rom_download(model);
    if (model === "42s") {
      assert.equal(d, null);
      continue;
    }
    assert.match(d.url, /^https:\/\/www\.hpcalc\.org\/.+\.zip$/);
    assert.match(d.page, /^https:\/\/www\.hpcalc\.org\/details\/\d+$/);
    assert.ok(d.size > 0 && d.revision);
    files[model] = d.file;
  }
  assert.deepEqual(files, {
    "48sx": "sxrom-j", "48gx": "gxrom-r", "38g": "38G_A167.ROM", "49g": "rom.49g", "39g": "rom.39g", "40g": "rom.39g",
  });
  assert.equal(pkg.rom_download("48sx").page, "https://www.hpcalc.org/details/4371");
  assert.throws(() => pkg.rom_download("99x"));
});
