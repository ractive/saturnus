//! Drawn calculator skins: the case, the display window and every key of a
//! model, with its labels and colours, for the SVG keyboard in `web/`.
//!
//! A skin is our own vector drawing. Its geometry was measured from the
//! keyboard line drawings in HP's manuals and its colours were read off
//! photographs; no artwork or pixels from either is reproduced. Each model
//! file names the figures and photos it was measured from and marks what is
//! inferred.
//!
//! **Unit.** Every coordinate is in skin units: 100 units are the pitch of
//! the six menu keys (softkeys) on that model's figure, the one distance
//! every figure shows in full. The origin is the top-left corner of the
//! case. Measured pixel positions were converted with
//! `unit = (pixel - case edge) * 100 / softkey pitch in pixels`.
//!
//! **Labels.** `label` is printed on the key; `left` and `right` are the
//! shifted functions printed above it (left-shift colour on the left,
//! right-shift colour on the right; a single label is centred); `alpha` is
//! the letter ALPHA types with the key, drawn where the model prints it;
//! `below` is printed under the key (CANCEL under ON). The 38G and 39G/40G
//! have one shift key, whose labels use `left`.
//!
//! The HP logo and wordmark are left off: the saturnus logo sits where the
//! logo was ([`Skin::logo`]) and the model name is plain text.

mod hp38g;
mod hp39g;
mod hp48gx;
mod hp48sx;
mod hp49g;

use crate::layout::json_string;
use saturnus::Model;

/// A rectangle in skin units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: i16,
    /// Top edge.
    pub y: i16,
    /// Width.
    pub w: i16,
    /// Height.
    pub h: i16,
}

/// Shorthand for a [`Rect`].
pub const fn r(x: i16, y: i16, w: i16, h: i16) -> Rect {
    Rect { x, y, w, h }
}

impl Rect {
    fn right(self) -> i16 {
        self.x + self.w
    }

    fn bottom(self) -> i16 {
        self.y + self.h
    }

    /// Whether `self` lies inside `outer` (edges may touch).
    pub fn inside(self, outer: Rect) -> bool {
        self.x >= outer.x
            && self.y >= outer.y
            && self.right() <= outer.right()
            && self.bottom() <= outer.bottom()
    }

    /// Whether the two rectangles share any area.
    pub fn overlaps(self, other: Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    fn json(self) -> String {
        format!("[{},{},{},{}]", self.x, self.y, self.w, self.h)
    }
}

/// The outline of a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A rounded rectangle.
    Key,
    /// An arrow key of the 49G/39G cursor pad, pointing up (wider at the
    /// bottom).
    Up,
    /// Cursor pad, pointing down.
    Down,
    /// Cursor pad, pointing left.
    Left,
    /// Cursor pad, pointing right.
    Right,
}

impl Shape {
    fn name(self) -> &'static str {
        match self {
            Shape::Key => "key",
            Shape::Up => "up",
            Shape::Down => "down",
            Shape::Left => "left",
            Shape::Right => "right",
        }
    }
}

/// A key cap's colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cap {
    /// Cap colour, `#rrggbb`.
    pub fill: &'static str,
    /// Colour of the label printed on the cap.
    pub ink: &'static str,
}

/// One key of a skin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkinKey {
    /// Script name of the key (`saturnus::io::Key::name`).
    pub name: &'static str,
    /// Outline, in skin units.
    pub rect: Rect,
    /// Outline shape.
    pub shape: Shape,
    /// Cap colours.
    pub cap: Cap,
    /// Text on the cap ("" for a blank menu key).
    pub label: &'static str,
    /// Left-shift (or the only shift) label above the key, or "".
    pub left: &'static str,
    /// Right-shift label above the key, or "".
    pub right: &'static str,
    /// Alpha letter, or "".
    pub alpha: &'static str,
    /// Text printed below the key, or "".
    pub below: &'static str,
}

/// Shorthand for a [`SkinKey`] with a plain outline.
#[allow(clippy::too_many_arguments)]
pub const fn k(
    name: &'static str,
    rect: Rect,
    cap: Cap,
    label: &'static str,
    left: &'static str,
    right: &'static str,
    alpha: &'static str,
    below: &'static str,
) -> SkinKey {
    SkinKey {
        name,
        rect,
        shape: Shape::Key,
        cap,
        label,
        left,
        right,
        alpha,
        below,
    }
}

/// Where a skin draws the alpha letters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaStyle {
    /// Printed on the case, to the right of the key's lower edge.
    Outside,
    /// Printed on a coloured disc on the right half of the cap (49G).
    Badge,
    /// Printed under the key's right corner, in the gap above the next
    /// row (39G/40G).
    Below,
}

/// A filled area of the case: the case itself, a face plate, the display
/// bezel, a key panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Panel {
    /// Bounds.
    pub rect: Rect,
    /// Corner radius of the top corners.
    pub radius: i16,
    /// Corner radius of the bottom corners.
    pub bottom_radius: i16,
    /// Fill colour.
    pub fill: &'static str,
}

/// Shorthand for a [`Panel`].
pub const fn panel(rect: Rect, radius: i16, bottom_radius: i16, fill: &'static str) -> Panel {
    Panel {
        rect,
        radius,
        bottom_radius,
        fill,
    }
}

/// Free text on the case (the model name, SETUP, LAST).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    /// Centre of the text's baseline, x.
    pub x: i16,
    /// Baseline.
    pub y: i16,
    /// Font size.
    pub size: i16,
    /// Colour.
    pub fill: &'static str,
    /// The text.
    pub text: &'static str,
}

/// A straight line on the case (the brackets of SETUP and LAST).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    /// Start x.
    pub x1: i16,
    /// Start y.
    pub y1: i16,
    /// End x.
    pub x2: i16,
    /// End y.
    pub y2: i16,
    /// Colour.
    pub stroke: &'static str,
}

/// A model's drawn skin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skin {
    /// Width of the drawing.
    pub width: i16,
    /// Height of the drawing.
    pub height: i16,
    /// Areas drawn back to front; the first is the case.
    pub panels: &'static [Panel],
    /// The display window: the LCD canvas is fitted into it.
    pub lcd: Rect,
    /// Background of the display window around the canvas.
    pub lcd_fill: &'static str,
    /// Where the saturnus logo goes (the HP logo's place).
    pub logo: Rect,
    /// Text drawn on the case.
    pub marks: &'static [Mark],
    /// Lines drawn on the case.
    pub lines: &'static [Line],
    /// Colour of left-shift labels (or of the only shift).
    pub left_ink: &'static str,
    /// Colour of right-shift labels.
    pub right_ink: &'static str,
    /// Colour of alpha letters.
    pub alpha_ink: &'static str,
    /// Disc colour behind alpha letters ([`AlphaStyle::Badge`]).
    pub alpha_badge: &'static str,
    /// Where alpha letters are drawn.
    pub alpha_style: AlphaStyle,
    /// Colour of text below keys.
    pub below_ink: &'static str,
    /// Font size of shift labels and alpha letters.
    pub small: i16,
    /// The keys.
    pub keys: &'static [SkinKey],
}

/// The skin of `model`. The 40G uses the 39G's drawing with its own name.
pub fn skin(model: Model) -> &'static Skin {
    match model {
        Model::Hp48sx => &hp48sx::SKIN,
        Model::Hp48gx => &hp48gx::SKIN,
        Model::Hp38g => &hp38g::SKIN,
        Model::Hp49g => &hp49g::SKIN,
        Model::Hp39g => &hp39g::SKIN_39G,
        Model::Hp40g => &hp39g::SKIN_40G,
    }
}

fn opt_field(name: &str, value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!(",\"{name}\":{}", json_string(value))
    }
}

fn key_json(k: &SkinKey) -> String {
    format!(
        "{{\"name\":{},\"rect\":{},\"shape\":\"{}\",\"fill\":{},\"ink\":{},\"label\":{}{}{}{}{}}}",
        json_string(k.name),
        k.rect.json(),
        k.shape.name(),
        json_string(k.cap.fill),
        json_string(k.cap.ink),
        json_string(k.label),
        opt_field("left", k.left),
        opt_field("right", k.right),
        opt_field("alpha", k.alpha),
        opt_field("below", k.below),
    )
}

/// The skin of `model` as JSON for the page:
///
/// `{"width","height","panels":[{"rect":[x,y,w,h],"radius","bottomRadius",
/// "fill"}],"lcd":[x,y,w,h],"lcdFill","logo":[x,y,w,h],"marks":[{"x","y",
/// "size","fill","text"}],"lines":[{"x1","y1","x2","y2","stroke"}],
/// "leftInk","rightInk","alphaInk","alphaBadge","alphaStyle":"outside"|
/// "badge"|"below","belowInk","small","keys":[{"name","rect","shape","fill","ink",
/// "label","left"?,"right"?,"alpha"?,"below"?}]}`.
pub fn skin_json(model: Model) -> String {
    let s = skin(model);
    let panels: Vec<String> = s
        .panels
        .iter()
        .map(|p| {
            format!(
                "{{\"rect\":{},\"radius\":{},\"bottomRadius\":{},\"fill\":{}}}",
                p.rect.json(),
                p.radius,
                p.bottom_radius,
                json_string(p.fill)
            )
        })
        .collect();
    let marks: Vec<String> = s
        .marks
        .iter()
        .map(|m| {
            format!(
                "{{\"x\":{},\"y\":{},\"size\":{},\"fill\":{},\"text\":{}}}",
                m.x,
                m.y,
                m.size,
                json_string(m.fill),
                json_string(m.text)
            )
        })
        .collect();
    let lines: Vec<String> = s
        .lines
        .iter()
        .map(|l| {
            format!(
                "{{\"x1\":{},\"y1\":{},\"x2\":{},\"y2\":{},\"stroke\":{}}}",
                l.x1,
                l.y1,
                l.x2,
                l.y2,
                json_string(l.stroke)
            )
        })
        .collect();
    let keys: Vec<String> = s.keys.iter().map(key_json).collect();
    let style = match s.alpha_style {
        AlphaStyle::Outside => "outside",
        AlphaStyle::Badge => "badge",
        AlphaStyle::Below => "below",
    };
    format!(
        "{{\"width\":{},\"height\":{},\"panels\":[{}],\"lcd\":{},\"lcdFill\":{},\"logo\":{},\
         \"marks\":[{}],\"lines\":[{}],\"leftInk\":{},\"rightInk\":{},\"alphaInk\":{},\
         \"alphaBadge\":{},\"alphaStyle\":\"{style}\",\"belowInk\":{},\"small\":{},\"keys\":[{}]}}",
        s.width,
        s.height,
        panels.join(","),
        s.lcd.json(),
        json_string(s.lcd_fill),
        s.logo.json(),
        marks.join(","),
        lines.join(","),
        json_string(s.left_ink),
        json_string(s.right_ink),
        json_string(s.alpha_ink),
        json_string(s.alpha_badge),
        json_string(s.below_ink),
        s.small,
        keys.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::alpha_letter;
    use saturnus::io::Key;
    use std::collections::HashSet;

    fn bounds(s: &Skin) -> Rect {
        r(0, 0, s.width, s.height)
    }

    fn is_colour(c: &str) -> bool {
        c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|d| d.is_ascii_hexdigit())
    }

    /// Every key of the model's matrix is on its skin exactly once, and
    /// nothing else is.
    #[test]
    fn skins_cover_the_matrix_exactly_once() {
        for model in Model::ALL {
            let s = skin(model);
            let mut seen = HashSet::new();
            for key in s.keys {
                let k = Key::from_name(key.name)
                    .unwrap_or_else(|| panic!("{}: unknown key {}", model.name(), key.name));
                assert!(
                    k.position(model.keyboard_layout()).is_some(),
                    "{}: {} is not on its matrix",
                    model.name(),
                    key.name
                );
                assert!(seen.insert(k), "{}: {} twice", model.name(), key.name);
            }
            let matrix: HashSet<Key> = Key::on_layout(model.keyboard_layout()).collect();
            assert_eq!(seen, matrix, "{}", model.name());
        }
    }

    #[test]
    fn keys_fit_the_case_and_do_not_overlap() {
        for model in Model::ALL {
            let s = skin(model);
            let case = s.panels[0].rect;
            assert_eq!(case, bounds(s), "{}: the case is the drawing", model.name());
            for (i, a) in s.keys.iter().enumerate() {
                assert!(a.rect.w > 0 && a.rect.h > 0, "{}", a.name);
                assert!(
                    a.rect.inside(case),
                    "{}: {} off the case",
                    model.name(),
                    a.name
                );
                for b in &s.keys[i + 1..] {
                    assert!(
                        !a.rect.overlaps(b.rect),
                        "{}: {} overlaps {}",
                        model.name(),
                        a.name,
                        b.name
                    );
                }
                assert!(
                    !a.rect.overlaps(s.lcd),
                    "{}: {} on the LCD",
                    model.name(),
                    a.name
                );
            }
            assert!(
                s.lcd.inside(case) && s.logo.inside(case),
                "{}",
                model.name()
            );
            assert!(!s.logo.overlaps(s.lcd), "{}", model.name());
            for p in s.panels {
                assert!(p.rect.inside(case), "{}: panel off the case", model.name());
            }
        }
    }

    /// The LCD window holds the 131 x 72 canvas (64 rows and the
    /// annunciator strip) at a scale of at least 3 units per pixel.
    #[test]
    fn lcd_window_holds_the_display() {
        for model in Model::ALL {
            let lcd = skin(model).lcd;
            assert!(lcd.w >= 3 * 131 && lcd.h >= 3 * 72, "{}", model.name());
        }
    }

    #[test]
    fn colours_are_hex() {
        for model in Model::ALL {
            let s = skin(model);
            let mut all = vec![
                s.lcd_fill,
                s.left_ink,
                s.right_ink,
                s.alpha_ink,
                s.alpha_badge,
                s.below_ink,
            ];
            all.extend(s.panels.iter().map(|p| p.fill));
            all.extend(s.marks.iter().map(|m| m.fill));
            all.extend(s.lines.iter().map(|l| l.stroke));
            all.extend(s.keys.iter().flat_map(|k| [k.cap.fill, k.cap.ink]));
            for c in all {
                assert!(is_colour(c), "{}: {c:?}", model.name());
            }
        }
    }

    /// The alpha letters drawn on the 39G/40G are the ones the ROM types
    /// (wiki: hardware/hp39g-40g "Alpha letters"), as on the plain grid.
    #[test]
    fn aplet_49_alpha_letters_match_the_rom() {
        for model in [Model::Hp39g, Model::Hp40g] {
            for key in skin(model).keys {
                let rom = alpha_letter(model, key.name).unwrap_or("");
                let drawn = if key.alpha == "SPACE" {
                    "␣"
                } else {
                    key.alpha
                };
                // θ : ; are printed under 0 . (−) too; the grid omits them.
                if matches!(key.name, "0" | "point" | "neg") {
                    assert!(!drawn.is_empty(), "{}", key.name);
                    continue;
                }
                assert_eq!(drawn, rom, "{}: {}", model.name(), key.name);
            }
        }
    }

    /// 48: A-F on the softkeys, then G-Z in reading order up to +/- and EEX
    /// (wiki: hardware/keyboard; the 48SX and 48G figures).
    #[test]
    fn hp48_alpha_letters_in_reading_order() {
        for model in [Model::Hp48sx, Model::Hp48gx] {
            let letters: String = skin(model).keys.iter().map(|k| k.alpha).collect();
            assert_eq!(letters, "ABCDEFGHIJKLMNOPQRSTUVWXYZ", "{}", model.name());
        }
    }

    /// 49G: F1-F6 carry A-F; APPS ... divide carry G-Z in key order (wiki:
    /// hardware/hp49g "Alpha letters").
    #[test]
    fn hp49_alpha_letters() {
        let letters: Vec<(&str, &str)> = skin(Model::Hp49g)
            .keys
            .iter()
            .filter(|k| !k.alpha.is_empty())
            .map(|k| (k.name, k.alpha))
            .collect();
        let names: Vec<&str> = letters.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            [
                "a", "b", "c", "d", "e", "f", "apps", "mode", "tool", "var", "sto", "nxt", "hist",
                "cat", "eqw", "symb", "power", "sqrt", "sin", "cos", "tan", "eex", "neg", "x",
                "inv", "divide"
            ]
        );
        let abc: String = letters.iter().map(|(_, l)| *l).collect();
        assert_eq!(abc, "ABCDEFGHIJKLMNOPQRSTUVWXYZ");
    }

    /// 38G letters (wiki: hardware/hp38g "Keyboard").
    #[test]
    fn hp38_alpha_letters() {
        let s = skin(Model::Hp38g);
        let letter = |n: &str| s.keys.iter().find(|k| k.name == n).map(|k| k.alpha);
        for (n, l) in [
            ("home", "A"),
            ("sqrt", "F"),
            ("lparen", "G"),
            ("power", "J"),
            ("7", "K"),
            ("divide", "N"),
            ("multiply", "R"),
            ("minus", "V"),
            ("0", "W"),
            ("comma", "Y"),
            ("plus", "Z"),
        ] {
            assert_eq!(letter(n), Some(l), "{n}");
        }
        let count = s.keys.iter().filter(|k| !k.alpha.is_empty()).count();
        assert_eq!(count, 26);
    }

    #[test]
    fn forty_g_differs_only_in_its_name() {
        let a = skin(Model::Hp39g);
        let b = skin(Model::Hp40g);
        assert_eq!(a.keys, b.keys);
        assert_eq!(a.panels, b.panels);
        assert_ne!(a.marks, b.marks);
        assert!(b.marks.iter().any(|m| m.text == "40G"));
        for model in Model::ALL {
            for m in skin(model).marks {
                assert!(!m.text.contains("HP"), "{}: {}", model.name(), m.text);
            }
        }
    }

    #[test]
    fn json_shape() {
        for model in Model::ALL {
            let j = skin_json(model);
            assert!(j.starts_with("{\"width\":"), "{j}");
            assert_eq!(
                j.matches("\"shape\":").count(),
                skin(model).keys.len(),
                "{}",
                model.name()
            );
        }
        let j = skin_json(Model::Hp48sx);
        assert!(j.contains("\"name\":\"enter\""));
        assert!(j.contains("\"alphaStyle\":\"outside\""));
        assert!(skin_json(Model::Hp49g).contains("\"alphaStyle\":\"badge\""));
        assert!(skin_json(Model::Hp40g).contains("\"alphaStyle\":\"below\""));
    }
}
