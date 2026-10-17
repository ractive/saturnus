// The app's routes (iteration 50d), in the address's hash so the site
// needs no rewrite rules: `#/` the home page, `#/calculator` and
// `#/calculator/<model>` the Calculator view (on that model), `#/files`
// and `#/files/<model>` Files (its left pane on that model's calculator).
// Pure functions; the shell (web/main.js) follows `hashchange`.

/** The views a route shows. */
export const VIEWS = ["home", "calculator", "files"];

/** A model's name in a route: lower-case letters and digits ("48gx", "17bii"). */
const MODEL = /^[a-z0-9]{2,8}$/;

/**
 * The route of `hash` (`location.hash`): `{view, model}`, `model` null
 * when none is given. An empty hash is null (the shell picks the start);
 * anything it does not know is the home page.
 */
export function parseRoute(hash) {
  const path = String(hash ?? "").replace(/^#/, "");
  if (path === "") return null;
  const [, view, model, ...rest] = path.split("/");
  if (!path.startsWith("/") || rest.length || !["calculator", "files"].includes(view)) return { view: "home", model: null };
  if (model === undefined || model === "") return { view, model: null };
  return MODEL.test(model) ? { view, model } : { view, model: null };
}

/** The hash of route `{view, model}`. */
export function routeHash({ view, model = null }) {
  if (view !== "calculator" && view !== "files") return "#/";
  return model ? `#/${view}/${model}` : `#/${view}`;
}

/**
 * Where the app starts with no route: the installed app on a phone or a
 * tablet (`standalone` and `coarse`: the home screen's icon) opens the
 * last calculator; the desktop app (`desktop`) the calculator; otherwise
 * the home page.
 */
export function startRoute({ standalone = false, coarse = false, desktop = false } = {}) {
  if (desktop || (standalone && coarse)) return { view: "calculator", model: null };
  return { view: "home", model: null };
}
