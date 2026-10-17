// Files' protocol Worker: the device (saturnus-tx through its wasm
// bindings, ../pkg) runs here, not in the page, so its timers keep their
// time when the tab is hidden (a page's timers are throttled to one
// wake-up a second, and after five minutes hidden to one a minute; a
// dedicated Worker's are not: kb/docs/saturnus-tx.md, "Timing in the
// browser"). The page holds the link and relays its bytes (protocol.js);
// this file only starts the driver (driver.js) on the first message.

import init, { TxDevice } from "../pkg/saturnus_web.js";
import { DeviceDriver } from "./driver.js";

let driver = null;
const ready = init();
/** Messages run one after the other, in the order they came. */
let queue = ready.catch((err) => {
  self.postMessage({ type: "failed", message: String(err?.message ?? err) });
});

self.onmessage = (e) => {
  const m = e.data ?? {};
  queue = queue.then(() => {
    if (m.cmd === "init") {
      driver = new DeviceDriver({
        TxDevice,
        now: () => performance.now(),
        setTimer: (fn, ms) => setTimeout(fn, ms),
        clearTimer: (t) => clearTimeout(t),
        post: (msg) => self.postMessage(msg),
        seed: m.seed ?? null,
        listenMs: m.listenMs,
      });
      self.postMessage({ type: "ready" });
      return;
    }
    driver?.handle(m);
  }).catch((err) => {
    self.postMessage({ type: "failed", message: String(err?.message ?? err) });
  });
};
