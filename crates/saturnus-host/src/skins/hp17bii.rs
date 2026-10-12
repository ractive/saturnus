//! HP 17BII and HP 17B skin.
//!
//! Sources (wiki: hardware/hp17bii "Key labels", "Colours"): the keyboard
//! figures on PDF page 2 of the HP 17BII Owner's Manual (edition 2, 1991;
//! wiki: sources/hp17bii-om) and of the HP-17B Owner's Manual (edition 1,
//! 1987; wiki: sources/hp17b-om), read as text for the labels, with RND
//! above RCL on the 17BII from the manual's text and photograph P1; the
//! Wikimedia Commons photographs P1 (a 1999 17BII) and P2 (a 17B) (wiki:
//! sources/commons-pioneer-photos), whose sampled medians the wiki lists
//! as the colours. Nothing of the figures or the photos is stored here.
//!
//! One drawing serves both models, as the 39G's serves the 40G: the 17B's
//! keyboard is the 17BII's without the RND, R↓ and x≷y labels and without
//! ENTER under =, in its own colours and with its own name ([`keys`]).
//!
//! Inferred: the geometry. Both are the 42S's Pioneer case with the same
//! 37-key grid, so every key, panel, the LCD window and the logo take the
//! 42S skin's measured positions ([`super::hp42s`]); they were not
//! re-measured on the manuals' figures. The 42S's lighter band behind the
//! top row is left off (the wiki names none). The case lettering ("17B II"
//! or "17B", "BUSINESS") sits where the 42S prints its own. The thin gold
//! line framing the 17BII's keyboard (P1) runs 10 units inside the
//! keyboard plate; its colour is inferred from the gold surround. The
//! recessed rim and the black skirts under the caps follow the 42S,
//! darkened for each case. The 17B's shift key is the flash-lit photo's
//! gold-orange toned down (P2 #fbc05a is brightened by the flash).
//!
//! Every label keeps 4.5:1 (the 39G rule) at the wiki's colours, so none
//! is toned: the shifted labels on the faceplate, the key text on the
//! caps, ON and ENTER on the faceplate, the lettering on the surround.
//!
//! The menu keys print a small ^ only; one shift key, a blank coloured cap,
//! whose labels use `left`. CLR is the ON-line key, with OFF above and ON
//! below it.

use super::*;

/// The black skirt every cap stands on (the 42S's).
const SKIRT: i16 = 6;

/// The 17BII's dark brown keys with off-white labels.
const KEY_17BII: Cap = Cap {
    fill: "#2f2820",
    ink: "#d7cfbc",
    well: SKIRT,
};
/// The 17BII's gold-orange shift key, blank.
const SHIFT_17BII: Cap = Cap {
    fill: "#dcad56",
    ink: "#2f2820",
    well: SKIRT,
};
/// The 17B's near-black keys.
const KEY_17B: Cap = Cap {
    fill: "#181616",
    ink: "#c2bea5",
    well: SKIRT,
};
/// The 17B's gold-orange shift key, blank.
const SHIFT_17B: Cap = Cap {
    fill: "#e0a850",
    ink: "#181616",
    well: SKIRT,
};

/// `label` on the 17BII, nothing on the 17B.
const fn pick(bii: bool, label: &'static str) -> &'static str {
    if bii { label } else { "" }
}

/// The keyboard, in `key` and `shift` caps; `bii` adds the 17BII's labels
/// (RND, R↓, x≷y, ENTER under =).
const fn keys(key: Cap, shift: Cap, bii: bool) -> [SkinKey; 37] {
    [
        // Row 1: the six menu keys, each printed with a small ^.
        k("a", r(59, 531, 61, 45), key, "^", "", "", "", ""),
        k("b", r(160, 531, 61, 45), key, "^", "", "", "", ""),
        k("c", r(260, 531, 61, 45), key, "^", "", "", "", ""),
        k("d", r(360, 531, 61, 45), key, "^", "", "", "", ""),
        k("e", r(459, 531, 61, 45), key, "^", "", "", "", ""),
        k("f", r(559, 531, 61, 45), key, "^", "", "", "", ""),
        // Row 2.
        k("sto", r(59, 629, 61, 45), key, "STO", "", "", "", ""),
        k(
            "rcl",
            r(160, 629, 61, 45),
            key,
            "RCL",
            pick(bii, "RND"),
            "",
            "",
            "",
        ),
        k("percent", r(260, 629, 61, 45), key, "%", "MATH", "", "", ""),
        k("dsp", r(360, 629, 61, 45), key, "DSP", "MODES", "", "", ""),
        k(
            "prt",
            r(459, 629, 61, 45),
            key,
            "PRT",
            "PRINTER",
            "",
            "",
            "",
        ),
        k("exit", r(559, 629, 61, 45), key, "EXIT", "MAIN", "", "", ""),
        // Row 3: INPUT spans two keys.
        k(
            "input",
            r(59, 728, 162, 45),
            key,
            "INPUT",
            "CLEAR DATA",
            "",
            "",
            "",
        ),
        k("neg", r(260, 728, 61, 45), key, "+/-", "E", "", "", ""),
        k(
            "lparen",
            r(360, 728, 61, 45),
            key,
            "(",
            pick(bii, "R↓"),
            "",
            "",
            "",
        ),
        k(
            "rparen",
            r(459, 728, 61, 45),
            key,
            ")",
            pick(bii, "x≷y"),
            "",
            "",
            "",
        ),
        k("backspace", r(559, 728, 61, 45), key, "⬅", "", "", "", ""),
        // Rows 4-7: a narrow key on the left, four wide keys.
        k("up", r(59, 827, 62, 45), key, "▲", "", "", "", ""),
        k("7", r(174, 827, 83, 45), key, "7", "", "", "", ""),
        k("8", r(295, 827, 84, 45), key, "8", "", "", "", ""),
        k("9", r(417, 827, 83, 45), key, "9", "", "", "", ""),
        k("divide", r(538, 827, 84, 45), key, "÷", "1/x", "", "", ""),
        k("down", r(59, 926, 62, 45), key, "▼", "", "", "", ""),
        k("4", r(174, 926, 83, 45), key, "4", "", "", "", ""),
        k("5", r(295, 926, 84, 45), key, "5", "", "", "", ""),
        k("6", r(417, 926, 83, 45), key, "6", "", "", "", ""),
        k("multiply", r(538, 926, 84, 45), key, "×", "yˣ", "", "", ""),
        k("shift", r(59, 1025, 62, 45), shift, "", "", "", "", ""),
        k("1", r(174, 1025, 83, 45), key, "1", "", "", "", ""),
        k("2", r(295, 1025, 84, 45), key, "2", "", "", "", ""),
        k("3", r(417, 1025, 83, 45), key, "3", "", "", "", ""),
        k("minus", r(538, 1025, 84, 45), key, "−", "√x", "", "", ""),
        k("on", r(59, 1125, 62, 45), key, "CLR", "OFF", "", "", "ON"),
        k("0", r(174, 1125, 83, 45), key, "0", "MEM", "", "", ""),
        k("point", r(295, 1125, 84, 45), key, "·", "SHOW", "", "", ""),
        k(
            "equals",
            r(417, 1125, 83, 45),
            key,
            "=",
            "LAST",
            "",
            "",
            pick(bii, "ENTER"),
        ),
        k("plus", r(538, 1125, 84, 45), key, "+", "x²", "", "", ""),
    ]
}

const KEYS_17BII: [SkinKey; 37] = keys(KEY_17BII, SHIFT_17BII, true);
const KEYS_17B: [SkinKey; 37] = keys(KEY_17B, SHIFT_17B, false);

/// The case, its recessed rim, the display surround, the keyboard
/// faceplate and the LCD frame, in the 42S's places.
const fn panels(
    case: &'static str,
    rim: &'static str,
    surround: &'static str,
    face: &'static str,
    frame: &'static str,
) -> [Panel; 5] {
    [
        panel(r(0, 0, 677, 1236), 24, 34, case),
        sunk(r(30, 126, 617, 1097), 8, 27, rim),
        raised(r(34, 129, 609, 345), 6, 0, surround),
        raised(r(34, 484, 609, 735), 0, 24, face),
        raised(r(47, 236, 583, 148), 3, 3, frame),
    ]
}

const PANELS_17BII: [Panel; 5] = panels("#383228", "#1d1914", "#efdcb0", "#433b32", "#8a7a5c");
const PANELS_17B: [Panel; 5] = panels("#1c1614", "#0e0b0a", "#2a1c16", "#1a1616", "#735b37");

/// The thin gold line around the 17BII's keyboard, 10 units inside the
/// faceplate.
const GOLD: &str = "#c9a964";
const LINES_17BII: [Line; 4] = [
    Line {
        x1: 44,
        y1: 494,
        x2: 633,
        y2: 494,
        stroke: GOLD,
    },
    Line {
        x1: 633,
        y1: 494,
        x2: 633,
        y2: 1209,
        stroke: GOLD,
    },
    Line {
        x1: 633,
        y1: 1209,
        x2: 44,
        y2: 1209,
        stroke: GOLD,
    },
    Line {
        x1: 44,
        y1: 1209,
        x2: 44,
        y2: 494,
        stroke: GOLD,
    },
];

const MARKS_17BII: [Mark; 2] = [
    Mark {
        x: 113,
        y: 187,
        size: 20,
        fill: "#111111",
        text: "17B II",
        italic: false,
    },
    Mark {
        x: 490,
        y: 187,
        size: 20,
        fill: "#4a4640",
        text: "BUSINESS",
        italic: false,
    },
];

const MARKS_17B: [Mark; 2] = [
    Mark {
        x: 113,
        y: 187,
        size: 20,
        fill: "#c9a25e",
        text: "17B",
        italic: false,
    },
    Mark {
        x: 490,
        y: 187,
        size: 20,
        fill: "#c9a25e",
        text: "BUSINESS",
        italic: false,
    },
];

/// The HP 17BII skin.
pub const SKIN_17BII: Skin = Skin {
    width: 677,
    height: 1236,
    panels: &PANELS_17BII,
    // The 42S's window: 131 x 24 (16 rows and the annunciator strip).
    lcd: r(65, 260, 547, 100),
    lcd_fill: "#b7c2a2",
    // Where the 42S's logo sits (inferred: the same case).
    logo: r(82, 48, 48, 48),
    marks: &MARKS_17BII,
    lines: &LINES_17BII,
    left_ink: "#deae64",
    right_ink: "#deae64",
    alpha_ink: "#d7cfbc",
    alpha_badge: "#d7cfbc",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#d7cfbc",
    small: 17,
    well_fill: "#17120d",
    round: 18,
    // Inferred: the 42S's matte plastic.
    texture: 9,
    keys: &KEYS_17BII,
};

/// The HP 17B skin: the 17BII's drawing with its own labels, colours and
/// name.
pub const SKIN_17B: Skin = Skin {
    width: 677,
    height: 1236,
    panels: &PANELS_17B,
    lcd: r(65, 260, 547, 100),
    lcd_fill: "#b7c2a2",
    logo: r(82, 48, 48, 48),
    marks: &MARKS_17B,
    lines: &[],
    left_ink: "#c78852",
    right_ink: "#c78852",
    alpha_ink: "#c2bea5",
    alpha_badge: "#c2bea5",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#c2bea5",
    small: 17,
    well_fill: "#0a0808",
    round: 18,
    texture: 9,
    keys: &KEYS_17B,
};
