//! Key input for the tools: `press_keys` scripts (the CLI's key script
//! format, plus several keys on one line and symbol names) and the
//! `type_text` character map per model.

use anyhow::{Context, Result, bail};
use saturnus::Model;
use saturnus::io::Key;
use saturnus_drive::script::{self, Action, DEFAULT_HOLD_MS, Line};

/// The script commands of the CLI's format; any other first word is a key.
const COMMANDS: [&str; 5] = ["press", "down", "up", "wait", "wait-idle"];

/// Symbols accepted as key names in `press_keys` scripts.
const SYMBOLS: [(&str, Key); 5] = [
    ("+", Key::Plus),
    ("-", Key::Minus),
    ("*", Key::Multiply),
    ("/", Key::Divide),
    (".", Key::Point),
];

/// A key by script name or symbol.
fn key_named(word: &str) -> Option<Key> {
    SYMBOLS
        .iter()
        .find(|(s, _)| *s == word)
        .map(|&(_, k)| k)
        .or_else(|| Key::from_name(word))
}

/// The script name of a word that may be a symbol.
fn canonical(word: &str) -> &str {
    SYMBOLS
        .iter()
        .find(|(s, _)| *s == word)
        .map_or(word, |(_, k)| k.name())
}

/// Parse a `press_keys` script: the CLI's key script format (see the
/// README, "Key scripts"), where in addition a line may hold several key
/// names separated by spaces (each a `press` with the default hold, in
/// order) and `+ - * / .` name the plus, minus, multiply, divide and point
/// keys. Errors name the offending line.
pub fn parse_script(text: &str) -> Result<Vec<Line>> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let number = i + 1;
        let body = raw.split('#').next().unwrap_or("");
        let words: Vec<&str> = body.split_whitespace().collect();
        let Some(first) = words.first() else {
            continue;
        };
        let is_command = COMMANDS.iter().any(|c| c.eq_ignore_ascii_case(first));
        if !is_command && words.len() > 1 && words.iter().all(|w| key_named(w).is_some()) {
            for w in &words {
                let key = key_named(w).with_context(|| format!("line {number}: {w:?}"))?;
                out.push(Line {
                    number,
                    action: Action::Press {
                        key,
                        hold_ms: DEFAULT_HOLD_MS,
                    },
                });
            }
            continue;
        }
        // One action: hand it to the CLI's parser, padded with empty lines
        // so its messages carry this line's number.
        let line: Vec<&str> = words.iter().map(|w| canonical(w)).collect();
        let padded = format!("{}{}", "\n".repeat(i), line.join(" "));
        out.append(&mut script::parse(&padded)?);
    }
    Ok(out)
}

/// The 48SX/48GX keys that type A-Z in alpha mode (wiki: hardware/keyboard
/// "HP48 matrix", rows of six from the softkeys down; Y and Z are +/- and
/// EEX).
const LETTERS_48: [Key; 26] = [
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::F,
    Key::Mth,
    Key::Prg,
    Key::Cst,
    Key::Var,
    Key::Up,
    Key::Nxt,
    Key::Quote,
    Key::Sto,
    Key::Eval,
    Key::Left,
    Key::Down,
    Key::Right,
    Key::Sin,
    Key::Cos,
    Key::Tan,
    Key::Sqrt,
    Key::Power,
    Key::Inv,
    Key::Neg,
    Key::Eex,
];

/// The 49G keys that type A-Z in alpha mode (wiki: hardware/keyboard
/// "HP49G alpha letters").
const LETTERS_49: [Key; 26] = [
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::F,
    Key::Apps,
    Key::Mode,
    Key::Tool,
    Key::Var,
    Key::Sto,
    Key::Nxt,
    Key::Hist,
    Key::Cat,
    Key::Eqw,
    Key::Symb,
    Key::Power,
    Key::Sqrt,
    Key::Sin,
    Key::Cos,
    Key::Tan,
    Key::Eex,
    Key::Neg,
    Key::X,
    Key::Inv,
    Key::Divide,
];

/// The key for a character that has its own key on `model`: digits, the
/// point, the four operators, space (not on the 38G, where that key is
/// the comma) and newline for ENTER.
fn plain_key(model: Model, c: char) -> Option<Key> {
    Some(match c {
        '0' => Key::Zero,
        '1' => Key::One,
        '2' => Key::Two,
        '3' => Key::Three,
        '4' => Key::Four,
        '5' => Key::Five,
        '6' => Key::Six,
        '7' => Key::Seven,
        '8' => Key::Eight,
        '9' => Key::Nine,
        '.' => Key::Point,
        '+' => Key::Plus,
        '-' => Key::Minus,
        '*' => Key::Multiply,
        '/' => Key::Divide,
        '\n' => Key::Enter,
        ' ' if model != Model::Hp38g => Key::Space,
        _ => return None,
    })
}

/// The alpha key of an ASCII letter on `model`; `None` on the 38G.
fn letter_key(model: Model, c: char) -> Option<Key> {
    let table = match model {
        Model::Hp48sx | Model::Hp48gx => &LETTERS_48,
        Model::Hp49g => &LETTERS_49,
        Model::Hp38g => return None,
    };
    let i = (c.to_ascii_uppercase() as usize).checked_sub('A' as usize)?;
    table.get(i).copied()
}

/// Key presses that type `text` on `model`.
///
/// Digits, `.`, `+ - * /`, space and newline (ENTER) press their keys; on
/// the 38G space is not typeable. The operators behave like their keys: in
/// RPN entry they act at once. Letters A-Z and a-z go through alpha mode:
/// a single capital is ALPHA then the letter, a longer run locks alpha
/// (ALPHA ALPHA), types the run, a lowercase letter with left shift
/// first, and unlocks with ALPHA. The 38G types no letters (its alpha
/// keys differ; use `press_keys`). Every other character is refused.
pub fn type_keys(model: Model, text: &str) -> Result<Vec<Key>> {
    let chars: Vec<char> = text.chars().filter(|&c| c != '\r').collect();
    let mut keys = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() {
            let end = chars[i..]
                .iter()
                .position(|c| !c.is_ascii_alphabetic())
                .map_or(chars.len(), |n| i + n);
            let run = &chars[i..end];
            let key = |c: char| {
                letter_key(model, c).with_context(|| {
                    format!(
                        "letters cannot be typed on the {}; use press_keys",
                        model.name()
                    )
                })
            };
            if let [single] = run
                && single.is_ascii_uppercase()
            {
                keys.extend([Key::Alpha, key(*single)?]);
            } else {
                keys.extend([Key::Alpha, Key::Alpha]);
                for &c in run {
                    if c.is_ascii_lowercase() {
                        keys.push(Key::LeftShift);
                    }
                    keys.push(key(c)?);
                }
                keys.push(Key::Alpha);
            }
            i = end;
            continue;
        }
        match plain_key(model, c) {
            Some(k) => keys.push(k),
            None => bail!(
                "character {c:?} at position {} cannot be typed on the {}: type_text \
                 takes letters, digits, '.', '+', '-', '*', '/', space and newline \
                 (ENTER); use press_keys for anything else",
                i + 1,
                model.name()
            ),
        }
        i += 1;
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn pressed(lines: &[Line]) -> Vec<Key> {
        lines
            .iter()
            .filter_map(|l| match l.action {
                Action::Press { key, .. } => Some(key),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn one_line_of_keys_and_symbols() {
        let lines = parse_script("6 ENTER 7 * ENTER").unwrap();
        assert_eq!(
            pressed(&lines),
            [Key::Six, Key::Enter, Key::Seven, Key::Multiply, Key::Enter]
        );
        assert!(lines.iter().all(|l| l.number == 1));
    }

    #[test]
    fn cli_format_still_parses() {
        let lines =
            parse_script("wait-idle 60_000\n\nf\npress + 100\ndown on # x\nup on\nwait 5").unwrap();
        let actions: Vec<Action> = lines.iter().map(|l| l.action).collect();
        assert_eq!(
            actions,
            [
                Action::WaitIdle { cap_ms: 60_000 },
                Action::Press {
                    key: Key::F,
                    hold_ms: DEFAULT_HOLD_MS
                },
                Action::Press {
                    key: Key::Plus,
                    hold_ms: 100
                },
                Action::Down(Key::On),
                Action::Up(Key::On),
                Action::Wait { ms: 5 },
            ]
        );
        assert_eq!(lines[1].number, 3);
        assert_eq!(lines[4].number, 6);
    }

    #[test]
    fn errors_name_the_original_line() {
        let e = parse_script("1 2\n3 frobnicate\n").unwrap_err();
        let msg = format!("{e:#}");
        assert!(msg.contains("line 2"), "{msg}");
        assert!(msg.contains("frobnicate"), "{msg}");
        assert!(parse_script("wait").is_err());
        assert!(parse_script("press f 10 20").is_err());
    }

    #[test]
    fn digits_and_operators() {
        assert_eq!(
            type_keys(Model::Hp48sx, "6 7*\n").unwrap(),
            [Key::Six, Key::Space, Key::Seven, Key::Multiply, Key::Enter]
        );
        assert_eq!(
            type_keys(Model::Hp38g, "1.5+2\r\n").unwrap(),
            [
                Key::One,
                Key::Point,
                Key::Five,
                Key::Plus,
                Key::Two,
                Key::Enter
            ]
        );
    }

    #[test]
    fn letters_on_the_48() {
        assert_eq!(
            type_keys(Model::Hp48gx, "S").unwrap(),
            [Key::Alpha, Key::Sin]
        );
        assert_eq!(
            type_keys(Model::Hp48sx, "SERVER").unwrap(),
            [
                Key::Alpha,
                Key::Alpha,
                Key::Sin,
                Key::E,
                Key::Right,
                Key::Sqrt,
                Key::E,
                Key::Right,
                Key::Alpha
            ]
        );
        assert_eq!(
            type_keys(Model::Hp48sx, "aZ").unwrap(),
            [
                Key::Alpha,
                Key::Alpha,
                Key::LeftShift,
                Key::A,
                Key::Eex,
                Key::Alpha
            ]
        );
    }

    #[test]
    fn letters_on_the_49g() {
        // The 49G autostart's SERVER letters.
        assert_eq!(
            type_keys(Model::Hp49g, "SERVER").unwrap(),
            [
                Key::Alpha,
                Key::Alpha,
                Key::Sin,
                Key::E,
                Key::Sqrt,
                Key::Eex,
                Key::E,
                Key::Sqrt,
                Key::Alpha
            ]
        );
        assert_eq!(
            type_keys(Model::Hp49g, "Z").unwrap(),
            [Key::Alpha, Key::Divide]
        );
        assert_eq!(type_keys(Model::Hp49g, "X").unwrap(), [Key::Alpha, Key::X]);
        // Every table entry is a key of its model.
        for c in 'A'..='Z' {
            let k49 = letter_key(Model::Hp49g, c).unwrap();
            assert!(
                k49.position(Model::Hp49g.keyboard_layout()).is_some(),
                "{c}"
            );
            let k48 = letter_key(Model::Hp48sx, c).unwrap();
            assert!(
                k48.position(Model::Hp48sx.keyboard_layout()).is_some(),
                "{c}"
            );
        }
    }

    #[test]
    fn untypeable_characters_are_refused() {
        let e = type_keys(Model::Hp48sx, "1 'X'").unwrap_err();
        assert!(e.to_string().contains("position 3"), "{e}");
        assert!(type_keys(Model::Hp48sx, "é").is_err());
        assert!(type_keys(Model::Hp38g, "A").is_err());
        assert!(type_keys(Model::Hp38g, "1 2").is_err());
    }
}
