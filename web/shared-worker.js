// saturnus's SharedWorker: one live calculator per model for every page of
// this site that shows one, the emulator and saturnus tx at once
// (iteration 46). Each page is a connection to the hub (hub.js), which
// speaks the protocol of web/protocol.md. Both pages open this script by
// the same URL, the emulator's folder's (web/shared/calc-hub.js).

import { Hub } from "./hub.js";

const hub = new Hub({ locks: self.navigator?.locks ?? null });

self.onconnect = (e) => {
  const port = e.ports[0];
  const page = hub.connect(port);
  port.onmessage = (m) => hub.receive(page, m.data ?? {});
};
