// The saved states in this browser: IndexedDB database `saturnus`, store
// `states`, two slots per model. The user's own (Save state, Load state)
// under the model's name, written by the page (backend.js); the
// auto-saved one (iteration 27) under `auto:<model>`, written by the
// Worker whenever the calculator changed and settled (worker.js), and
// restored when the model boots. One never overwrites the other. The
// auto-saved state is the calculator Files runs too (iteration 41):
// the database code is shared with it (web/shared/calc-states.js).

export { autoKey, dbDelete, dbGet, dbPut } from "./shared/calc-states.js";

/**
 * The models whose saved state holds the ROM: the 49G's state carries its
 * 2 MB flash, which is the ROM. Remove ROMs deletes those states too.
 */
export const ROM_HOLDING_STATES = ["49g"];
