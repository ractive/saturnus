// The aplet server's Worker: the folder served to a 38G, 39G or 40G
// (saturnus-tx's ApletServer through its wasm bindings, ../pkg) runs here,
// not in the page, so its timers keep their time in a hidden tab, as the
// protocol Worker's (protocol-worker.js). The page holds the link and the
// folder (serve.js); this file starts the driver (aplet-driver.js) on the
// first message, `{cmd: "init", files}`.

import init, { TxApletServer } from "../pkg/saturnus_web.js";
import { ApletDriver } from "./aplet-driver.js";

let driver = null;
const ready = init();
let queue = ready.catch((err) => {
  self.postMessage({ type: "workerFailed", message: String(err?.message ?? err) });
});

self.onmessage = (e) => {
  const m = e.data ?? {};
  queue = queue.then(() => {
    if (m.cmd === "init") {
      driver = new ApletDriver({
        TxApletServer,
        now: () => performance.now(),
        setTimer: (fn, ms) => setTimeout(fn, ms),
        clearTimer: (t) => clearTimeout(t),
        post: (msg) => self.postMessage(msg),
        files: m.files ?? [],
      });
      self.postMessage({ type: "ready" });
      return;
    }
    driver?.handle(m);
  }).catch((err) => {
    self.postMessage({ type: "workerFailed", message: String(err?.message ?? err) });
  });
};
