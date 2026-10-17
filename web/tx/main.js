// tx.html's host: Files alone in the page (web/tx/app.js mounts nothing
// by itself). A file, not an inline script: the site's CSP allows only
// its own scripts (web/site.htaccess, `script-src 'self'`).

import { mountFiles } from "./app.js";

mountFiles(document.getElementById("files-view"), { shown: () => true });
