//! Drawn calculator skins: the case, the display window and every key of a
//! model, with its labels and colours, for the SVG keyboard in `web/`.
//!
//! A skin is our own vector drawing. Its geometry was measured from the
//! owner's photographs of his calculators (48SX, 38G, 49G) and from the
//! keyboard line drawings in HP's manuals, and its colours were read off
//! photographs; no artwork or pixels from either is reproduced. Each model
//! file names the figures and photos it was measured from and marks what is
//! inferred.
//!
//! **Unit.** Every coordinate is in skin units: 100 units are the pitch of
//! the six menu keys (softkeys) on that model's photograph or figure, the
//! one distance every one of them shows in full. The origin is the top-left corner of the
//! case. Measured pixel positions were converted with
//! `unit = (pixel - case edge) * 100 / softkey pitch in pixels`.
//!
//! **Labels.** `label` is printed on the key; `left` and `right` are the
//! shifted functions printed above it (left-shift colour on the left,
//! right-shift colour on the right; a single label is centred); `alpha` is
//! the letter ALPHA types with the key, drawn where the model prints it;
//! `below` is printed under the key (CANCEL under ON). The 38G, 39G/40G and
//! 42S have one shift key, whose labels use `left`.
//!
//! **Wells.** A key's `rect` is its cap. On the 48 and the 38G every cap
//! sits in a dark recessed well, and the 49G's keys have a black outline:
//! [`Cap::well`] is how far that surround reaches beyond the cap on every
//! side (most visible around the 48's light menu keys), drawn in
//! [`Skin::well_fill`].
//!
//! **Display window.** [`Skin::lcd`] is the LCD's active area, 131 x 72
//! pixels (64 rows and the annunciator strip) at square pixels, so its
//! aspect is fixed and its width sets the scale. Every ROM draws the six
//! menu labels in 21-pixel boxes at a pitch of [`SOFTKEY_LABEL_PITCH`]
//! pixels from column 0 (wiki: hardware/display "Menu labels"), so the
//! window is placed where label `i` is centred over menu key `i`:
//! `lcd.x + (22 i + 10.5) * lcd.w / 131 = key centre`. The alignment is
//! checked by a test. The 42S's window is 131 x 24 (16 rows and the
//! strip, [`lcd_rows`]); its display is narrower than its key row, so its
//! labels sit within half a key pitch of their keys, not over them.
//!
//! The HP logo and wordmark are left off: the saturnus logo sits where the
//! logo was ([`Skin::logo`]) and the model name is plain text.

mod hp38g;
mod hp39g;
mod hp42s;
mod hp48gx;
mod hp48sx;
mod hp49g;

use saturnus::Model;
use serde::ser::SerializeMap as _;

/// LCD pixels: columns and rows plus the annunciator strip the page draws
/// above them.
pub const LCD_COLUMNS: i16 = 131;
/// LCD rows (64) plus the 8-pixel annunciator strip.
pub const LCD_ROWS: i16 = 72;
/// Rows of the annunciator strip the page draws above the pixels.
pub const ANNUNCIATOR_ROWS: i16 = 8;

/// LCD rows plus the annunciator strip of `model`: [`LCD_ROWS`], or 24 on
/// the 42S (16 rows and the strip).
pub fn lcd_rows(model: Model) -> i16 {
    if model == Model::Hp42s { 24 } else { LCD_ROWS }
}
/// Pixel pitch of the six menu labels the ROMs draw along the bottom of
/// the display: 21-pixel boxes with a 1-pixel gap, from column 0 (seen on
/// the 48SX, 48GX, 38G, 49G, 39G and 40G ROMs; wiki: hardware/display
/// "Menu labels").
pub const SOFTKEY_LABEL_PITCH: f64 = 22.0;
/// Centre of the first menu label, in pixels from the display's left edge.
pub const SOFTKEY_LABEL_CENTRE: f64 = 10.5;

/// Centre x of menu label `i` (0-5) on skin `s`, in skin units.
pub fn softkey_label_centre(s: &Skin, i: usize) -> f64 {
    let px = SOFTKEY_LABEL_CENTRE + SOFTKEY_LABEL_PITCH * i as f64;
    f64::from(s.lcd.x) + px * f64::from(s.lcd.w) / f64::from(LCD_COLUMNS)
}

/// How a model types letters from the computer keyboard (the ROM's alpha
/// rules; wiki: hardware/keyboard, hardware/hp38g, hardware/hp39g-40g):
/// a letter is its key after `alpha`, unless the alpha annunciator is
/// already on (the 48 and 49G lock alpha with a second press; the 38G,
/// 39G and 40G cancel it); a lowercase letter adds `lower_shift`, pressed
/// after `alpha` on the 48 and 49G and before it on the aplet models.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Typing {
    /// The alpha key's script name.
    pub alpha: &'static str,
    /// The shift that makes a letter lowercase.
    pub lower_shift: &'static str,
    /// Whether `lower_shift` goes before `alpha` rather than after it.
    pub shift_first: bool,
    /// Whether a second `alpha` locks alpha mode (false: it cancels).
    pub alpha_locks: bool,
    /// The keys that type a space on a model with neither a space key nor
    /// a space in its letter map: SHIFT then 2 on the 38G (SPACE is
    /// printed above 2; seen on ROM A1.67). Empty elsewhere.
    pub space: &'static [&'static str],
}

/// The typing rules of `model`; `None` on the 42S, which types letters
/// from its ALPHA menus, not from letter keys, so the page maps no
/// computer-keyboard letters there.
pub fn typing(model: Model) -> Option<Typing> {
    Some(match model {
        Model::Hp42s => return None,
        Model::Hp48sx | Model::Hp48gx | Model::Hp49g => Typing {
            alpha: "alpha",
            lower_shift: "leftshift",
            shift_first: false,
            alpha_locks: true,
            space: &[],
        },
        Model::Hp38g | Model::Hp39g | Model::Hp40g => Typing {
            alpha: "alpha",
            lower_shift: "shift",
            shift_first: true,
            alpha_locks: false,
            space: if matches!(model, Model::Hp38g) {
                &["shift", "2"]
            } else {
                &[]
            },
        },
    })
}

/// The key that types each letter (and the space, on models whose alpha
/// space is a letter key) on `model`, from the letters printed on the
/// skin: `(letter, key name)` in key order.
pub fn letters(model: Model) -> Vec<(char, &'static str)> {
    skin(model)
        .keys
        .iter()
        .filter_map(|k| match k.alpha {
            "SPACE" => Some((' ', k.name)),
            a => {
                let mut chars = a.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if c.is_ascii_uppercase() => Some((c, k.name)),
                    _ => None,
                }
            }
        })
        .collect()
}

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
}

/// `[x, y, w, h]`.
impl serde::Serialize for Rect {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        [self.x, self.y, self.w, self.h].serialize(s)
    }
}

/// The outline of a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
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

/// A key cap's colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Cap {
    /// Cap colour, `#rrggbb`.
    pub fill: &'static str,
    /// Colour of the label printed on the cap.
    pub ink: &'static str,
    /// Margin of the dark well (or outline) around the cap; 0 for none.
    pub well: i16,
}

/// One key of a skin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SkinKey {
    /// Script name of the key (`saturnus::io::Key::name`).
    pub name: &'static str,
    /// Outline, in skin units.
    pub rect: Rect,
    /// Outline shape.
    pub shape: Shape,
    /// Cap colours.
    #[serde(flatten)]
    pub cap: Cap,
    /// Text on the cap ("" for a blank menu key).
    pub label: &'static str,
    /// Left-shift (or the only shift) label above the key, or "".
    #[serde(skip_serializing_if = "str::is_empty")]
    pub left: &'static str,
    /// Right-shift label above the key, or "".
    #[serde(skip_serializing_if = "str::is_empty")]
    pub right: &'static str,
    /// Alpha letter, or "".
    #[serde(skip_serializing_if = "str::is_empty")]
    pub alpha: &'static str,
    /// Text printed below the key, or "".
    #[serde(skip_serializing_if = "str::is_empty")]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AlphaStyle {
    /// Printed on the case, to the right of the key's well, on its lower
    /// edge (48).
    Outside,
    /// Printed on the case off the key's lower right corner (38G).
    Corner,
    /// Printed on a coloured disc on the right half of the cap (49G).
    Badge,
    /// Printed under the key's right corner, in the gap above the next
    /// row (39G/40G).
    Below,
}

/// A filled area of the case: the case itself, a face plate, the display
/// bezel, a key panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Panel {
    /// Bounds.
    pub rect: Rect,
    /// Corner radius of the top corners.
    pub radius: i16,
    /// Corner radius of the bottom corners.
    pub bottom_radius: i16,
    /// Fill colour.
    pub fill: &'static str,
    /// How far the middle of the bottom edge bows below its corners (the
    /// 49G's display surround); `rect` includes the bow. 0 for straight.
    pub bow: i16,
}

/// Shorthand for a [`Panel`] with a straight bottom edge.
pub const fn panel(rect: Rect, radius: i16, bottom_radius: i16, fill: &'static str) -> Panel {
    Panel {
        rect,
        radius,
        bottom_radius,
        fill,
        bow: 0,
    }
}

/// Free text on the case (the model name, SETUP, LAST).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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
    /// Whether the text is slanted (the model names on the 48 and 49G).
    pub italic: bool,
}

/// A straight line on the case (the brackets of SETUP and LAST).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
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
    /// Colour of the wells and outlines around the caps ([`Cap::well`]).
    pub well_fill: &'static str,
    /// Corner radius of the caps, in percent of the cap's shorter side.
    pub round: i16,
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
        Model::Hp42s => &hp42s::SKIN,
    }
}

/// The letters a model types, serialized as an object `{"A":"a",...}`
/// (letter to key name) in key order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Letters(pub Vec<(char, &'static str)>);

impl serde::Serialize for Letters {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (c, name) in &self.0 {
            map.serialize_entry(&c.to_string(), name)?;
        }
        map.end()
    }
}

/// The skin of `model` for the page, as [`skin_view`] gives it. Serializes
/// as
///
/// `{"width","height","panels":[{"rect":[x,y,w,h],"radius","bottomRadius",
/// "fill","bow"}],"lcd":[x,y,w,h],"lcdFill","logo":[x,y,w,h],"marks":[{"x",
/// "y","size","fill","text","italic"}],"lines":[{"x1","y1","x2","y2",
/// "stroke"}],"leftInk","rightInk","alphaInk","alphaBadge","alphaStyle":
/// "outside"|"corner"|"badge"|"below","belowInk","small","wellFill","round",
/// "keys":[{"name","rect","shape","fill","ink","well","label","left"?,
/// "right"?,"alpha"?,"below"?}],"letters":{"A":"a",...},
/// "typing":{"alpha","lowerShift","shiftFirst","alphaLocks","space":[..]},
/// "lcdRows"}`; `"typing":null` and no letters on the 42S.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinView {
    /// The drawing.
    #[serde(flatten)]
    pub skin: &'static Skin,
    /// The key that types each letter ([`letters`]).
    pub letters: Letters,
    /// The typing rules ([`typing`]).
    pub typing: Option<Typing>,
    /// The model's pixel rows without the annunciator strip (64, 16 on
    /// the 42S), so the page sizes the canvas before a ROM runs.
    pub lcd_rows: i16,
}

/// The skin of `model` with what the page draws and types from it.
pub fn skin_view(model: Model) -> SkinView {
    SkinView {
        skin: skin(model),
        letters: Letters(letters(model)),
        typing: typing(model),
        lcd_rows: lcd_rows(model) - ANNUNCIATOR_ROWS,
    }
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
                    model.has_key(k),
                    "{}: {} is not on its matrix",
                    model.name(),
                    key.name
                );
                assert!(seen.insert(k), "{}: {} twice", model.name(), key.name);
            }
            let matrix: HashSet<Key> = model.keys().collect();
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

    /// The LCD window is the 131 x 72 canvas (64 rows and the annunciator
    /// strip) at square pixels, at least 3 units per pixel.
    #[test]
    fn lcd_window_is_the_display_at_square_pixels() {
        for model in Model::ALL {
            let lcd = skin(model).lcd;
            assert!(lcd.w >= 3 * LCD_COLUMNS, "{}", model.name());
            let rows = lcd_rows(model);
            let want_h = f64::from(lcd.w) * f64::from(rows) / f64::from(LCD_COLUMNS);
            assert!(
                (f64::from(lcd.h) - want_h).abs() <= 1.0,
                "{}: {} x {} is not 131:{rows}",
                model.name(),
                lcd.w,
                lcd.h
            );
        }
    }

    /// The owner's review of iteration 8: the six menu labels the ROM
    /// draws must sit directly above the six menu keys. Each label's
    /// centre is within 3 units (under one LCD pixel) of its key's centre.
    #[test]
    fn softkey_labels_sit_above_the_menu_keys() {
        for model in Model::ALL {
            let s = skin(model);
            // The 42S's menu row keeps its labels; its display is narrower
            // than its key row (photo), so a label sits within half a key
            // pitch (50 units) of its key.
            let (menu, tolerance) = if model == Model::Hp42s {
                (["sigmaplus", "inv", "sqrt", "log", "ln", "xeq"], 50.0)
            } else {
                (["a", "b", "c", "d", "e", "f"], 3.0)
            };
            for (i, name) in menu.iter().enumerate() {
                let key = s.keys.iter().find(|k| k.name == *name).unwrap();
                let key_centre = f64::from(key.rect.x) + f64::from(key.rect.w) / 2.0;
                let label = softkey_label_centre(s, i);
                assert!(
                    (label - key_centre).abs() <= tolerance,
                    "{}: label {} at {label:.1}, key {name} at {key_centre:.1}",
                    model.name(),
                    i + 1
                );
            }
            // And the labels stay inside the display's bezel panel.
            let bezel = s
                .panels
                .iter()
                .find(|p| s.lcd.inside(p.rect) && p.rect != s.panels[0].rect)
                .unwrap_or_else(|| panic!("{}: no bezel around the LCD", model.name()));
            assert!(
                bezel.rect.x < s.lcd.x && bezel.rect.right() > s.lcd.right(),
                "{}: bezel narrower than the display",
                model.name()
            );
        }
    }

    /// The owner's review of iteration 8: no excess case below the bottom
    /// key row. The margin under the lowest key is at most 1.3 times the
    /// margin beside the outermost keys.
    #[test]
    fn case_ends_shortly_below_the_bottom_row() {
        for model in Model::ALL {
            let s = skin(model);
            let lowest = s.keys.iter().map(|k| k.rect.bottom()).max().unwrap();
            let left = s.keys.iter().map(|k| k.rect.x).min().unwrap();
            let right = s.keys.iter().map(|k| k.rect.right()).max().unwrap();
            let side = f64::from(left.min(s.width - right));
            let below = f64::from(s.height - lowest);
            assert!(
                below <= side * 1.3 && below >= side * 0.8,
                "{}: {below} units below the keys, {side} beside them",
                model.name()
            );
        }
    }

    /// Every model but the 42S types all 26 letters, each from one key of
    /// its matrix; the 39G/40G also type a space from the plus key, the
    /// 38G with SHIFT then 2. The 42S has no letter keys and no typing
    /// data.
    #[test]
    fn letter_map_per_model() {
        assert!(letters(Model::Hp42s).is_empty() && typing(Model::Hp42s).is_none());
        for model in Model::ALL.into_iter().filter(|&m| m != Model::Hp42s) {
            let map = letters(model);
            let abc: String = map.iter().map(|(c, _)| *c).filter(|c| *c != ' ').collect();
            assert_eq!(abc, "ABCDEFGHIJKLMNOPQRSTUVWXYZ", "{}", model.name());
            let mut keys = HashSet::new();
            for (_, name) in &map {
                let k = Key::from_name(name).unwrap();
                assert!(model.has_key(k), "{name}");
                assert!(keys.insert(k), "{}: {name} types two letters", model.name());
            }
            let space = map.iter().any(|(c, _)| *c == ' ');
            assert_eq!(space, matches!(model, Model::Hp39g | Model::Hp40g));
            let t = typing(model).unwrap();
            let shifted_space: &[&str] = if model == Model::Hp38g {
                &["shift", "2"]
            } else {
                &[]
            };
            assert_eq!(t.space, shifted_space, "{}", model.name());
            for name in [t.alpha, t.lower_shift]
                .into_iter()
                .chain(t.space.iter().copied())
            {
                assert!(
                    Key::from_name(name).is_some_and(|k| model.has_key(k)),
                    "{}: {name}",
                    model.name()
                );
            }
        }
        let at = |m: Model, c: char| {
            letters(m)
                .into_iter()
                .find(|(l, _)| *l == c)
                .map(|(_, n)| n)
        };
        assert_eq!(at(Model::Hp48sx, 'A'), Some("a"));
        assert_eq!(at(Model::Hp48gx, 'Z'), Some("eex"));
        assert_eq!(at(Model::Hp49g, 'Z'), Some("divide"));
        assert_eq!(at(Model::Hp38g, 'A'), Some("home"));
        assert_eq!(at(Model::Hp39g, 'A'), Some("vars"));
        assert_eq!(at(Model::Hp40g, ' '), Some("plus"));
        let t = |m: Model| typing(m).unwrap();
        assert!(t(Model::Hp48sx).alpha_locks && !t(Model::Hp38g).alpha_locks);
        assert!(t(Model::Hp39g).shift_first && !t(Model::Hp49g).shift_first);
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

    fn skin_json(model: Model) -> String {
        serde_json::to_string(&skin_view(model)).unwrap()
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
        assert!(j.contains("\"letters\":{\"A\":\"a\",\"B\":\"b\""));
        assert!(j.contains("\"typing\":{\"alpha\":\"alpha\",\"lowerShift\":\"leftshift\",\"shiftFirst\":false,\"alphaLocks\":true,\"space\":[]}"));
        assert!(skin_json(Model::Hp38g).contains("\"space\":[\"shift\",\"2\"]"));
        assert!(skin_json(Model::Hp38g).contains("\"alphaStyle\":\"corner\""));
        assert!(j.contains("\"wellFill\":") && j.contains("\"well\":9"));
        assert!(skin_json(Model::Hp49g).contains("\"bow\":30"));
        assert!(skin_json(Model::Hp39g).contains("\" \":\"plus\""));
        let j42 = skin_json(Model::Hp42s);
        assert!(j42.contains("\"name\":\"rdn\""));
        assert!(j42.ends_with("\"letters\":{},\"typing\":null,\"lcdRows\":16}"));
        assert!(j.ends_with(",\"lcdRows\":64}"));
    }

    /// 42S: ENTER is two keys wide, the shift key is orange and blank, the
    /// case names the model, and the top row carries its shifted labels.
    #[test]
    fn hp42s_skin() {
        let s = skin(Model::Hp42s);
        let key = |n: &str| {
            s.keys
                .iter()
                .find(|k| k.name == n)
                .unwrap_or_else(|| panic!("{n}"))
        };
        let enter = key("enter");
        let swap = key("swap");
        let sto = key("sto");
        let rcl = key("rcl");
        // ENTER spans STO and RCL, gap included.
        assert_eq!(enter.rect.x, sto.rect.x);
        assert_eq!(enter.rect.right(), rcl.rect.right());
        assert!(enter.rect.w > 2 * sto.rect.w);
        assert!(enter.rect.right() < swap.rect.x);
        let shift = key("shift");
        assert_eq!(shift.cap.fill, "#ef8b1d");
        assert!(shift.label.is_empty());
        assert_eq!(key("on").label, "EXIT");
        assert_eq!(key("on").below, "ON");
        assert!(s.marks.iter().any(|m| m.text == "42S"));
        let shifted: Vec<&str> = s.keys[..6].iter().map(|k| k.left).collect();
        assert_eq!(shifted, ["Σ−", "yˣ", "x²", "10ˣ", "eˣ", "GTO"]);
        // One shift: nothing in the right-shift or alpha slots.
        assert!(
            s.keys
                .iter()
                .all(|k| k.right.is_empty() && k.alpha.is_empty())
        );
        // No wells: the 42S's caps stand on the plate.
        assert!(s.keys.iter().all(|k| k.cap.well == 0));
    }
}
