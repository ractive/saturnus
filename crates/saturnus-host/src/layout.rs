//! Physical key layouts for drawing a generic keyboard.
//!
//! The core knows each key's matrix position, not where it sits on the
//! case. These tables place every key of a model on a grid of
//! [`GRID_COLUMNS`] units per row, top row first, so a front end can draw
//! plain labelled buttons in the real arrangement. The labels are generic
//! text (key names, digits, arrows), not artwork from any calculator.
//!
//! Sources: the 48 arrangement is the one of the key codes counted from the
//! top-left, six keys in the first four rows, ENTER two keys wide, five keys
//! in the rows below (wiki: hardware/keyboard "ROM data structures"). The
//! 38G is the 48 case with VAR and NXT removed, each 38G key at the place of
//! a 48 key (wiki: hardware/hp38g "Keyboard", an inference there); the
//! 39G/40G keys sit at the places of the 49G's the same way (wiki:
//! hardware/hp39g-40g "Keyboard"). The 49G
//! has six softkeys, APPS MODE TOOL / VAR STO NXT beside the arrow keys, then
//! seven rows of five; the wiki has its matrix (wiki: hardware/keyboard
//! "HP49G matrix") but not its case, so this arrangement is unverified, and
//! the round arrow pad is drawn as a plain up / left down right block.
//! The 42S has six keys in its first two rows, ENTER two keys wide, then
//! five keys per row with ▲ ▼ shift EXIT on the left (wiki: hardware/hp42s
//! "Keyboard", and the owner's calculator).

use saturnus::Model;
use saturnus::io::Key;

/// Grid units per keyboard row: six keys of 5 units or five keys of 6.
pub const GRID_COLUMNS: u8 = 30;

/// One key on the drawn keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeySpec {
    /// Script name accepted by `key_down` / `key_up` (see `Key::name`).
    pub name: &'static str,
    /// Text to print on the button.
    pub label: &'static str,
    /// Row, 0 at the top.
    pub row: u8,
    /// Left edge in grid units (0..[`GRID_COLUMNS`]).
    pub x: u8,
    /// Width in grid units.
    pub w: u8,
}

const fn k(name: &'static str, label: &'static str, row: u8, x: u8, w: u8) -> KeySpec {
    KeySpec {
        name,
        label,
        row,
        x,
        w,
    }
}

/// The 48SX and 48GX keyboard.
const HP48: [KeySpec; 49] = [
    k("a", "F1", 0, 0, 5),
    k("b", "F2", 0, 5, 5),
    k("c", "F3", 0, 10, 5),
    k("d", "F4", 0, 15, 5),
    k("e", "F5", 0, 20, 5),
    k("f", "F6", 0, 25, 5),
    k("mth", "MTH", 1, 0, 5),
    k("prg", "PRG", 1, 5, 5),
    k("cst", "CST", 1, 10, 5),
    k("var", "VAR", 1, 15, 5),
    k("up", "▲", 1, 20, 5),
    k("nxt", "NXT", 1, 25, 5),
    k("quote", "'", 2, 0, 5),
    k("sto", "STO", 2, 5, 5),
    k("eval", "EVAL", 2, 10, 5),
    k("left", "◀", 2, 15, 5),
    k("down", "▼", 2, 20, 5),
    k("right", "▶", 2, 25, 5),
    k("sin", "SIN", 3, 0, 5),
    k("cos", "COS", 3, 5, 5),
    k("tan", "TAN", 3, 10, 5),
    k("sqrt", "√x", 3, 15, 5),
    k("power", "yˣ", 3, 20, 5),
    k("inv", "1/x", 3, 25, 5),
    k("enter", "ENTER", 4, 0, 10),
    k("neg", "+/-", 4, 10, 5),
    k("eex", "EEX", 4, 15, 5),
    k("del", "DEL", 4, 20, 5),
    k("backspace", "⌫", 4, 25, 5),
    k("alpha", "α", 5, 0, 6),
    k("7", "7", 5, 6, 6),
    k("8", "8", 5, 12, 6),
    k("9", "9", 5, 18, 6),
    k("divide", "÷", 5, 24, 6),
    k("leftshift", "↰", 6, 0, 6),
    k("4", "4", 6, 6, 6),
    k("5", "5", 6, 12, 6),
    k("6", "6", 6, 18, 6),
    k("multiply", "×", 6, 24, 6),
    k("rightshift", "↱", 7, 0, 6),
    k("1", "1", 7, 6, 6),
    k("2", "2", 7, 12, 6),
    k("3", "3", 7, 18, 6),
    k("minus", "−", 7, 24, 6),
    k("on", "ON", 8, 0, 6),
    k("0", "0", 8, 6, 6),
    k("point", ".", 8, 12, 6),
    k("space", "SPC", 8, 18, 6),
    k("plus", "+", 8, 24, 6),
];

/// The 38G keyboard: the 48 case without VAR and NXT, each 38G key at the
/// place of a 48 key, under its own name (wiki: hardware/hp38g "Keyboard").
const HP38: [KeySpec; 47] = [
    k("a", "F1", 0, 0, 5),
    k("b", "F2", 0, 5, 5),
    k("c", "F3", 0, 10, 5),
    k("d", "F4", 0, 15, 5),
    k("e", "F5", 0, 20, 5),
    k("f", "F6", 0, 25, 5),
    k("plot", "PLOT", 1, 0, 5),
    k("symb", "SYMB", 1, 5, 5),
    k("num", "NUM", 1, 10, 5),
    k("up", "▲", 1, 20, 5),
    k("lib", "LIB", 2, 0, 5),
    k("var", "VAR", 2, 5, 5),
    k("math", "MATH", 2, 10, 5),
    k("left", "◀", 2, 15, 5),
    k("down", "▼", 2, 20, 5),
    k("right", "▶", 2, 25, 5),
    k("home", "HOME", 3, 0, 5),
    k("sin", "SIN", 3, 5, 5),
    k("cos", "COS", 3, 10, 5),
    k("tan", "TAN", 3, 15, 5),
    k("xt", "X,T,θ", 3, 20, 5),
    k("sqrt", "√x", 3, 25, 5),
    k("enter", "ENTER", 4, 0, 10),
    k("lparen", "(", 4, 10, 5),
    k("rparen", ")", 4, 15, 5),
    k("neg", "(-)", 4, 20, 5),
    k("power", "xʸ", 4, 25, 5),
    k("alpha", "A…Z", 5, 0, 6),
    k("7", "7", 5, 6, 6),
    k("8", "8", 5, 12, 6),
    k("9", "9", 5, 18, 6),
    k("divide", "÷", 5, 24, 6),
    k("shift", "SHIFT", 6, 0, 6),
    k("4", "4", 6, 6, 6),
    k("5", "5", 6, 12, 6),
    k("6", "6", 6, 18, 6),
    k("multiply", "×", 6, 24, 6),
    k("del", "DEL", 7, 0, 6),
    k("1", "1", 7, 6, 6),
    k("2", "2", 7, 12, 6),
    k("3", "3", 7, 18, 6),
    k("minus", "−", 7, 24, 6),
    k("on", "ON", 8, 0, 6),
    k("0", "0", 8, 6, 6),
    k("point", ".", 8, 12, 6),
    k("comma", ",", 8, 18, 6),
    k("plus", "+", 8, 24, 6),
];

/// The 39G and 40G keyboard: the 49G arrangement below, each 39G key at
/// the place of a 49G key (wiki: hardware/hp39g-40g "Keyboard", an
/// inference there; the 40G is assumed to match).
const HP39: [KeySpec; 51] = [
    k("a", "F1", 0, 0, 5),
    k("b", "F2", 0, 5, 5),
    k("c", "F3", 0, 10, 5),
    k("d", "F4", 0, 15, 5),
    k("e", "F5", 0, 20, 5),
    k("f", "F6", 0, 25, 5),
    k("symb", "SYMB", 1, 0, 5),
    k("plot", "PLOT", 1, 5, 5),
    k("num", "NUM", 1, 10, 5),
    k("up", "▲", 1, 20, 5),
    k("home", "HOME", 2, 0, 5),
    k("aplet", "APLET", 2, 5, 5),
    k("views", "VIEWS", 2, 10, 5),
    k("left", "◀", 2, 15, 5),
    k("down", "▼", 2, 20, 5),
    k("right", "▶", 2, 25, 5),
    k("vars", "VARS", 3, 0, 6),
    k("math", "MATH", 3, 6, 6),
    k("ddx", "d/dx", 3, 12, 6),
    k("xt", "X,T,θ", 3, 18, 6),
    k("del", "DEL", 3, 24, 6),
    k("sin", "SIN", 4, 0, 6),
    k("cos", "COS", 4, 6, 6),
    k("tan", "TAN", 4, 12, 6),
    k("ln", "LN", 4, 18, 6),
    k("log", "LOG", 4, 24, 6),
    k("square", "x²", 5, 0, 6),
    k("power", "xʸ", 5, 6, 6),
    k("lparen", "(", 5, 12, 6),
    k("rparen", ")", 5, 18, 6),
    k("divide", "÷", 5, 24, 6),
    k("comma", ",", 6, 0, 6),
    k("7", "7", 6, 6, 6),
    k("8", "8", 6, 12, 6),
    k("9", "9", 6, 18, 6),
    k("multiply", "×", 6, 24, 6),
    k("alpha", "A…Z", 7, 0, 6),
    k("4", "4", 7, 6, 6),
    k("5", "5", 7, 12, 6),
    k("6", "6", 7, 18, 6),
    k("minus", "−", 7, 24, 6),
    k("shift", "SHIFT", 8, 0, 6),
    k("1", "1", 8, 6, 6),
    k("2", "2", 8, 12, 6),
    k("3", "3", 8, 18, 6),
    k("plus", "+", 8, 24, 6),
    k("on", "ON", 9, 0, 6),
    k("0", "0", 9, 6, 6),
    k("point", ".", 9, 12, 6),
    k("neg", "(-)", 9, 18, 6),
    k("enter", "ENTER", 9, 24, 6),
];

/// The 49G keyboard.
const HP49: [KeySpec; 51] = [
    k("a", "F1", 0, 0, 5),
    k("b", "F2", 0, 5, 5),
    k("c", "F3", 0, 10, 5),
    k("d", "F4", 0, 15, 5),
    k("e", "F5", 0, 20, 5),
    k("f", "F6", 0, 25, 5),
    k("apps", "APPS", 1, 0, 5),
    k("mode", "MODE", 1, 5, 5),
    k("tool", "TOOL", 1, 10, 5),
    k("up", "▲", 1, 20, 5),
    k("var", "VAR", 2, 0, 5),
    k("sto", "STO", 2, 5, 5),
    k("nxt", "NXT", 2, 10, 5),
    k("left", "◀", 2, 15, 5),
    k("down", "▼", 2, 20, 5),
    k("right", "▶", 2, 25, 5),
    k("hist", "HIST", 3, 0, 6),
    k("cat", "CAT", 3, 6, 6),
    k("eqw", "EQW", 3, 12, 6),
    k("symb", "SYMB", 3, 18, 6),
    k("backspace", "⌫", 3, 24, 6),
    k("power", "yˣ", 4, 0, 6),
    k("sqrt", "√x", 4, 6, 6),
    k("sin", "SIN", 4, 12, 6),
    k("cos", "COS", 4, 18, 6),
    k("tan", "TAN", 4, 24, 6),
    k("eex", "EEX", 5, 0, 6),
    k("neg", "+/-", 5, 6, 6),
    k("x", "X", 5, 12, 6),
    k("inv", "1/x", 5, 18, 6),
    k("divide", "÷", 5, 24, 6),
    k("alpha", "α", 6, 0, 6),
    k("7", "7", 6, 6, 6),
    k("8", "8", 6, 12, 6),
    k("9", "9", 6, 18, 6),
    k("multiply", "×", 6, 24, 6),
    k("leftshift", "↰", 7, 0, 6),
    k("4", "4", 7, 6, 6),
    k("5", "5", 7, 12, 6),
    k("6", "6", 7, 18, 6),
    k("minus", "−", 7, 24, 6),
    k("rightshift", "↱", 8, 0, 6),
    k("1", "1", 8, 6, 6),
    k("2", "2", 8, 12, 6),
    k("3", "3", 8, 18, 6),
    k("plus", "+", 8, 24, 6),
    k("on", "ON", 9, 0, 6),
    k("0", "0", 9, 6, 6),
    k("point", ".", 9, 12, 6),
    k("space", "SPC", 9, 18, 6),
    k("enter", "ENTER", 9, 24, 6),
];

/// The 42S keyboard.
const HP42: [KeySpec; 37] = [
    k("sigmaplus", "Σ+", 0, 0, 5),
    k("inv", "1/x", 0, 5, 5),
    k("sqrt", "√x", 0, 10, 5),
    k("log", "LOG", 0, 15, 5),
    k("ln", "LN", 0, 20, 5),
    k("xeq", "XEQ", 0, 25, 5),
    k("sto", "STO", 1, 0, 5),
    k("rcl", "RCL", 1, 5, 5),
    k("rdn", "R↓", 1, 10, 5),
    k("sin", "SIN", 1, 15, 5),
    k("cos", "COS", 1, 20, 5),
    k("tan", "TAN", 1, 25, 5),
    k("enter", "ENTER", 2, 0, 10),
    k("swap", "x≷y", 2, 10, 5),
    k("neg", "+/-", 2, 15, 5),
    k("eex", "E", 2, 20, 5),
    k("backspace", "←", 2, 25, 5),
    k("up", "▲", 3, 0, 6),
    k("7", "7", 3, 6, 6),
    k("8", "8", 3, 12, 6),
    k("9", "9", 3, 18, 6),
    k("divide", "÷", 3, 24, 6),
    k("down", "▼", 4, 0, 6),
    k("4", "4", 4, 6, 6),
    k("5", "5", 4, 12, 6),
    k("6", "6", 4, 18, 6),
    k("multiply", "×", 4, 24, 6),
    k("shift", "⇧", 5, 0, 6),
    k("1", "1", 5, 6, 6),
    k("2", "2", 5, 12, 6),
    k("3", "3", 5, 18, 6),
    k("minus", "−", 5, 24, 6),
    k("on", "EXIT", 6, 0, 6),
    k("0", "0", 6, 6, 6),
    k("point", ".", 6, 12, 6),
    k("rs", "R/S", 6, 18, 6),
    k("plus", "+", 6, 24, 6),
];

/// The drawn keyboard of `model`.
pub fn layout(model: Model) -> &'static [KeySpec] {
    match model {
        Model::Hp48sx | Model::Hp48gx => &HP48,
        Model::Hp38g => &HP38,
        Model::Hp49g => &HP49,
        Model::Hp39g | Model::Hp40g => &HP39,
        Model::Hp42s => &HP42,
    }
}

/// The key a [`KeySpec`] presses, if its name is known.
pub fn key_of(spec: &KeySpec) -> Option<Key> {
    Key::from_name(spec.name)
}

/// The letter ALPHA types with key `name` on `model`, where the drawn
/// keyboard shows it: the 39G and 40G, whose keys carry no letters in their
/// labels. Observed by booting the 39G ROM (A-D on VARS MATH d/dx X,T,θ,
/// then rows of five down to X-Z on 1 2 3, space on plus; wiki:
/// hardware/hp39g-40g "Alpha letters"). `None` for other keys and models.
pub fn alpha_letter(model: Model, name: &str) -> Option<&'static str> {
    if !matches!(model, Model::Hp39g | Model::Hp40g) {
        return None;
    }
    const LETTERS: [(&str, &str); 27] = [
        ("vars", "A"),
        ("math", "B"),
        ("ddx", "C"),
        ("xt", "D"),
        ("sin", "E"),
        ("cos", "F"),
        ("tan", "G"),
        ("ln", "H"),
        ("log", "I"),
        ("square", "J"),
        ("power", "K"),
        ("lparen", "L"),
        ("rparen", "M"),
        ("divide", "N"),
        ("comma", "O"),
        ("7", "P"),
        ("8", "Q"),
        ("9", "R"),
        ("multiply", "S"),
        ("4", "T"),
        ("5", "U"),
        ("6", "V"),
        ("minus", "W"),
        ("1", "X"),
        ("2", "Y"),
        ("3", "Z"),
        ("plus", "␣"),
    ];
    LETTERS.iter().find(|(n, _)| *n == name).map(|&(_, l)| l)
}

/// One key of [`Grid`]: a [`KeySpec`] and the alpha letter drawn on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct GridKey {
    /// Script name.
    pub name: &'static str,
    /// Text on the button.
    pub label: &'static str,
    /// Row, 0 at the top.
    pub row: u8,
    /// Left edge in grid units.
    pub x: u8,
    /// Width in grid units.
    pub w: u8,
    /// The letter ALPHA types with it, where the drawn keyboard shows it
    /// ([`alpha_letter`]); absent from the JSON otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha: Option<&'static str>,
}

/// The drawn keyboard of a model for the plain button grid. Serializes as
/// `{"columns":30,"rows":N,"keys":[{"name","label","row","x","w",
/// "alpha"?},...]}`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Grid {
    /// Columns per row: [`GRID_COLUMNS`].
    pub columns: u8,
    /// Rows.
    pub rows: u8,
    /// The keys.
    pub keys: Vec<GridKey>,
}

/// The layout of `model` as a [`Grid`].
pub fn grid(model: Model) -> Grid {
    let keys = layout(model);
    Grid {
        columns: GRID_COLUMNS,
        rows: keys.iter().map(|k| k.row).max().map_or(0, |r| r + 1),
        keys: keys
            .iter()
            .map(|k| GridKey {
                name: k.name,
                label: k.label,
                row: k.row,
                x: k.x,
                w: k.w,
                alpha: alpha_letter(model, k.name),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn layout_json(model: Model) -> String {
        serde_json::to_string(&grid(model)).unwrap()
    }

    #[test]
    fn aplet_49_keys_show_their_alpha_letters() {
        for model in [Model::Hp39g, Model::Hp40g] {
            let j = layout_json(model);
            assert!(
                j.contains(
                    "\"name\":\"vars\",\"label\":\"VARS\",\"row\":3,\"x\":0,\"w\":6,\"alpha\":\"A\""
                ),
                "{j}"
            );
            assert_eq!(j.matches("\"alpha\":").count(), 27);
            let names: HashSet<&str> = layout(model).iter().map(|k| k.name).collect();
            for c in ["vars", "3", "plus", "comma"] {
                assert!(names.contains(c) && alpha_letter(model, c).is_some(), "{c}");
            }
        }
        assert!(!layout_json(Model::Hp48sx).contains("\"alpha\":"));
    }

    #[test]
    fn every_key_exists_on_its_model_once() {
        for model in Model::ALL {
            let mut seen = HashSet::new();
            for spec in layout(model) {
                let key = key_of(spec)
                    .unwrap_or_else(|| panic!("{}: unknown {}", model.name(), spec.name));
                assert!(
                    model.has_key(key),
                    "{}: {} not on its matrix",
                    model.name(),
                    spec.name
                );
                assert!(seen.insert(key), "{}: {} twice", model.name(), spec.name);
            }
        }
    }

    #[test]
    fn layouts_cover_the_matrix() {
        // Every matrix key of every model is drawn; the 38G's matrix has
        // no key at the 48's VAR and NXT places.
        for model in Model::ALL {
            let drawn: HashSet<Key> = layout(model).iter().filter_map(key_of).collect();
            let matrix: HashSet<Key> = model.keys().collect();
            assert_eq!(drawn, matrix, "{}", model.name());
        }
    }

    #[test]
    fn keys_do_not_overlap_and_fit_the_grid() {
        for model in Model::ALL {
            let keys = layout(model);
            for (i, a) in keys.iter().enumerate() {
                assert!(a.w > 0 && a.x + a.w <= GRID_COLUMNS, "{}", a.name);
                for b in &keys[i + 1..] {
                    let overlap = a.row == b.row && a.x < b.x + b.w && b.x < a.x + a.w;
                    assert!(!overlap, "{}: {} overlaps {}", model.name(), a.name, b.name);
                }
            }
        }
    }

    #[test]
    fn known_places() {
        let at = |model: Model, name: &str| {
            layout(model)
                .iter()
                .find(|k| k.name == name)
                .map(|k| (k.row, k.x, k.w))
        };
        // 48: ENTER is two keys wide at the left of row 4; ON bottom left.
        assert_eq!(at(Model::Hp48sx, "enter"), Some((4, 0, 10)));
        assert_eq!(at(Model::Hp48sx, "on"), Some((8, 0, 6)));
        assert_eq!(at(Model::Hp48sx, "plus"), Some((8, 24, 6)));
        // 49G: ENTER bottom right.
        assert_eq!(at(Model::Hp49g, "enter"), Some((9, 24, 6)));
        // 38G: its VAR key sits at the 48's STO place; no NXT key.
        assert_eq!(at(Model::Hp38g, "var"), Some((2, 5, 5)));
        assert_eq!(at(Model::Hp38g, "nxt"), None);
        // 39G and 40G: one layout, ENTER bottom right as on the 49G.
        assert_eq!(layout(Model::Hp39g), layout(Model::Hp40g));
        assert_eq!(at(Model::Hp39g, "enter"), Some((9, 24, 6)));
        // 42S: ENTER two keys wide in row 2; EXIT bottom left.
        assert_eq!(at(Model::Hp42s, "enter"), Some((2, 0, 10)));
        assert_eq!(at(Model::Hp42s, "on"), Some((6, 0, 6)));
    }

    #[test]
    fn json_shape() {
        let j = layout_json(Model::Hp48sx);
        assert!(j.starts_with("{\"columns\":30,\"rows\":9,\"keys\":[{\"name\":\"a\""));
        assert!(j.contains("{\"name\":\"enter\",\"label\":\"ENTER\",\"row\":4,\"x\":0,\"w\":10}"));
        assert_eq!(j.matches("\"name\"").count(), 49);
    }
}
