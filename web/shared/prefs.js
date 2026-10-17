// The app's preferences in localStorage, by their names in
// storage-names.js `KEYS`: read, written and removed quietly where the
// browser blocks storage (the home page's and Calculator's).

import { KEYS } from "./storage-names.js";

export const prefs = {
  get(key) {
    try { return localStorage.getItem(KEYS[key]); } catch { return null; }
  },
  set(key, value) {
    try { localStorage.setItem(KEYS[key], value); } catch { /* storage blocked */ }
  },
  remove(key) {
    try { localStorage.removeItem(KEYS[key]); } catch { /* storage blocked */ }
  },
};
