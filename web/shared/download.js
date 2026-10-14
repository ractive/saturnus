// Saving bytes as a download, the browser's way, for both pages: an
// `<a download>` on an object URL, clicked, the URL let go a minute later
// (long enough for the slowest browser to have started the download).

/** How long a download's object URL is kept, in ms. */
export const REVOKE_MS = 60_000;

/**
 * Save `data` (a Blob, or bytes; `type` for bytes) as a download named
 * `name`. `doc`, `urls` and `later` for the tests.
 */
export function saveDownload(name, data, { type = "application/octet-stream", doc = globalThis.document, urls = globalThis.URL, later = setTimeout } = {}) {
  const blob = data instanceof Blob ? data : new Blob([data], { type });
  const url = urls.createObjectURL(blob);
  const a = doc.createElement("a");
  a.href = url;
  a.download = name;
  a.hidden = true;
  doc.body.append(a);
  a.click();
  a.remove();
  later(() => urls.revokeObjectURL(url), REVOKE_MS);
}
