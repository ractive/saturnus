// Recorded serial traffic in saturnus-tx's trace format
// (crates/saturnus-tx/src/trace.rs): one line per step, the time in ms
// since the start, a direction and the bytes with `\xNN`, `\r`, `\n` and
// `\\` escapes:
//
//        0 = 9600 none        the port (re)opened at these settings
//        0 > \x01, I~* ...    bytes written
//        1 < \x01+ Y~& ...    bytes read
//   # request connect        a note
//
// `parseTrace` and `formatTrace` read and write it; `FakeLink`
// (links.js) plays a trace's calculator and `RecordingLink` writes one.
// Pure: web/test/tx-trace.test.mjs.

/** Bytes as the trace's escaped text. */
export function escapeBytes(bytes) {
  let s = "";
  for (const [i, b] of bytes.entries()) {
    if (b === 0x5c) s += "\\\\";
    // A space that ends the line is escaped: the Rust reader trims line ends (kermit-proto's trace::escape does the same).
    else if (b === 0x20 && i === bytes.length - 1) s += "\\x20";
    else if (b === 0x0d) s += "\\r";
    else if (b === 0x0a) s += "\\n";
    else if (b >= 0x20 && b < 0x7f) s += String.fromCharCode(b);
    else s += `\\x${b.toString(16).padStart(2, "0")}`;
  }
  return s;
}

/** The bytes of escaped text; throws on a bad escape. */
export function unescapeBytes(text) {
  const out = [];
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    if (c !== 0x5c) {
      if (c > 0x7f) throw new Error(`not ASCII at ${i}`);
      out.push(c);
      continue;
    }
    const n = text[i + 1];
    if (n === "\\") out.push(0x5c);
    else if (n === "r") out.push(0x0d);
    else if (n === "n") out.push(0x0a);
    else if (n === "x" && /^[0-9a-fA-F]{2}$/.test(text.slice(i + 2, i + 4))) {
      out.push(parseInt(text.slice(i + 2, i + 4), 16));
      i += 2;
    } else throw new Error(`bad escape at ${i}`);
    i += 1;
  }
  return new Uint8Array(out);
}

/**
 * The steps of trace text: `{at, kind: "open", baud, parity}`, `{at,
 * kind: "write" | "read", bytes}` and `{kind: "note", text}`.
 */
export function parseTrace(text) {
  const steps = [];
  text.split("\n").forEach((raw, i) => {
    const line = raw.replace(/\r$/, "");
    if (!line.trim()) return;
    if (line.startsWith("#")) {
      steps.push({ kind: "note", text: line.slice(1).trim() });
      return;
    }
    const m = /^\s*(\d+) ([=<>]) (.*)$/.exec(line);
    if (!m) throw new Error(`line ${i + 1}: not a trace step`);
    const at = Number(m[1]);
    if (m[2] === "=") {
      const [baud, parity] = m[3].trim().split(/\s+/);
      steps.push({ at, kind: "open", baud: Number(baud), parity });
    } else {
      steps.push({ at, kind: m[2] === ">" ? "write" : "read", bytes: unescapeBytes(m[3]) });
    }
  });
  return steps;
}

/** Steps as trace text (the times rounded to whole ms). */
export function formatTrace(steps) {
  return steps
    .map((s) => {
      if (s.kind === "note") return `# ${s.text}`;
      const at = String(Math.round(s.at)).padStart(8);
      if (s.kind === "open") return `${at} = ${s.baud} ${s.parity}`;
      return `${at} ${s.kind === "write" ? ">" : "<"} ${escapeBytes(s.bytes)}`;
    })
    .join("\n") + "\n";
}

/** Whether two byte arrays hold the same bytes. */
export function sameBytes(a, b) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}
