// saturnus's plain Web Worker: the calculators' hub (hub.js) with one page,
// where the browser has no SharedWorker (Samsung Internet) and in the
// tests. The page is its only connection; the protocol is the same
// (web/protocol.md).

import { Hub } from "./hub.js";

const hub = new Hub();
const page = hub.connect(self);
self.onmessage = (e) => hub.receive(page, e.data ?? {});
