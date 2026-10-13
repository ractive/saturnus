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
//! the Pioneers (42S, 27S, 17BII, 17B) have one shift key, whose labels use
//! `left`.
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
//!
//! **Clamshell.** The 28S and the 19BII open like a book ([`Fold`]): one drawing of
//! both halves side by side, the letter half on the left and the display
//! half on the right, the hinge between them. The page shows it so on a
//! wide stage and stacked on a tall one (the display half on top, the
//! letter half's keyboard below), moving each half as a whole.

mod hp17bii;
mod hp19bii;
mod hp27s;
mod hp28s;
mod hp38g;
mod hp39g;
mod hp42s;
mod hp48gx;
mod hp48sx;
mod hp49g;
mod pioneer;

use saturnus_core::Model;
use serde::ser::SerializeMap as _;

/// LCD pixel columns of every model but the 28S and 19BII (137,
/// [`lcd_columns`]).
pub const LCD_COLUMNS: i16 = 131;
/// LCD rows (64) plus the 8-pixel annunciator strip.
pub const LCD_ROWS: i16 = 72;
/// Rows of the annunciator strip the page draws above the pixels.
pub const ANNUNCIATOR_ROWS: i16 = 8;

/// LCD rows plus the annunciator strip of `model`: [`LCD_ROWS`], or 24 on
/// the Pioneers (42S, 27S, 17BII, 17B: 16 rows and the strip), 40 on the
/// 28S and 19BII (32 rows and the strip).
pub fn lcd_rows(model: Model) -> i16 {
    let rows = i16::try_from(model.lcd_size().1).unwrap_or(i16::MAX);
    rows + ANNUNCIATOR_ROWS
}
/// LCD pixel columns of `model`: [`LCD_COLUMNS`], 137 on the 28S and
/// 19BII.
pub fn lcd_columns(model: Model) -> i16 {
    i16::try_from(model.lcd_size().0).unwrap_or(LCD_COLUMNS)
}

/// Pixel pitch of the six menu labels the ROMs draw along the bottom of
/// the display: 21-pixel boxes with a 1-pixel gap, from column 0 (seen on
/// the 48SX, 48GX, 38G, 49G, 39G and 40G ROMs; wiki: hardware/display
/// "Menu labels").
pub const SOFTKEY_LABEL_PITCH: f64 = 22.0;
/// The 28S's menu labels: 22-pixel boxes with a 1-pixel gap from column 0,
/// filling its 137 columns (seen on the 28S ROM; wiki: hardware/hp28s
/// "Display"), and the 19BII's the same (seen on its ROM; wiki:
/// hardware/hp19bii).
pub const SOFTKEY_LABEL_PITCH_28S: f64 = 23.0;
/// Centre of the first menu label, in pixels from the display's left edge
/// (the middle of a 21-pixel box from column 0; the 28S's 22-pixel box
/// centres at 11, [`softkey_label_centre`]).
pub const SOFTKEY_LABEL_CENTRE: f64 = 10.5;

/// Centre x of menu label `i` (0-5) on `model`'s skin `s`, in skin units.
pub fn softkey_label_centre(model: Model, s: &Skin, i: usize) -> f64 {
    let pitch = if lcd_columns(model) == 137 {
        SOFTKEY_LABEL_PITCH_28S
    } else {
        SOFTKEY_LABEL_PITCH
    };
    // A box one pixel narrower than the pitch, from column 0.
    let px = (pitch - 1.0) / 2.0 + pitch * i as f64;
    f64::from(s.lcd.x) + px * f64::from(s.lcd.w) / f64::from(lcd_columns(model))
}

/// How a model types letters from the computer keyboard (the ROM's alpha
/// rules; wiki: hardware/keyboard, hardware/hp38g, hardware/hp39g-40g):
/// a letter is its key after `alpha`, unless the alpha annunciator is
/// already on (the 48 and 49G lock alpha with a second press; the 38G,
/// 39G and 40G cancel it); a lowercase letter adds `lower_shift`, pressed
/// after `alpha` on the 48 and 49G and before it on the aplet models. On
/// the 28S (`direct`) every letter has a key of its own: no alpha press,
/// and a lowercase letter is its key between two presses of LC, which
/// switches lowercase on and off (wiki: hardware/hp28s "Left keyboard").
/// The 19BII's letters are direct too, without alpha or lowercase keys:
/// `alpha` and `lower_shift` are empty, and a lowercase letter types the
/// capital (wiki: hardware/hp19bii "Keyboard").
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
    /// Letter keys of their own (the 28S, 19BII): no `alpha` press; a lowercase
    /// letter is `lower_shift`, the key, `lower_shift` again.
    pub direct: bool,
}

/// The typing rules of `model`; `None` on the Pioneers (42S, 27S, 17BII,
/// 17B), which type letters from their ALPHA menus, not from letter keys,
/// so the page maps no computer-keyboard letters there.
pub fn typing(model: Model) -> Option<Typing> {
    Some(match model {
        Model::Hp42s | Model::Hp27s | Model::Hp17bii | Model::Hp17b => {
            return None;
        }
        // No alpha or lowercase key: a letter is its key, and a lowercase
        // letter types the capital (the empty names press nothing).
        Model::Hp19bii => Typing {
            alpha: "",
            lower_shift: "",
            shift_first: false,
            alpha_locks: false,
            space: &[],
            direct: true,
        },
        Model::Hp28s => Typing {
            alpha: "alpha",
            lower_shift: "lc",
            shift_first: true,
            alpha_locks: false,
            space: &[],
            direct: true,
        },
        Model::Hp48sx | Model::Hp48gx | Model::Hp49g => Typing {
            alpha: "alpha",
            lower_shift: "leftshift",
            shift_first: false,
            alpha_locks: true,
            space: &[],
            direct: false,
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
            direct: false,
        },
    })
}

/// The key that types each letter (and the space, on models whose alpha
/// space is a letter key) on `model`, from the letters printed on the
/// skin: `(letter, key name)` in key order.
pub fn letters(model: Model) -> Vec<(char, &'static str)> {
    let Some(skin) = skin(model) else {
        return Vec::new();
    };
    skin.keys
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
    /// Script name of the key (`saturnus_core::io::Key::name`).
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
    /// Not drawn: the letter is the key's own label (the 28S's and 19BII's letter
    /// keys).
    Label,
}

/// How a panel stands on the case, for the page's depth rule (the
/// decision log, "Skin depth"): one light from the top left, as on the
/// keys. The case itself (the first panel) is always drawn as the body:
/// a rounded outer edge lit on the top left and shaded on the bottom
/// right, and a light falling across it; every other panel is drawn by
/// its relief.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Relief {
    /// Printed or flush: a plain fill.
    Flat,
    /// A plate standing on the surface: a thin edge lit on the top left,
    /// shaded on the bottom right.
    Raised,
    /// A recess in the surface: its upper and left walls in shade, a
    /// faint light on the lower and right lips.
    Sunk,
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
    /// How the panel stands on the case (ignored on the case itself).
    pub relief: Relief,
}

/// Shorthand for a flat [`Panel`] with a straight bottom edge.
pub const fn panel(rect: Rect, radius: i16, bottom_radius: i16, fill: &'static str) -> Panel {
    Panel {
        rect,
        radius,
        bottom_radius,
        fill,
        bow: 0,
        relief: Relief::Flat,
    }
}

/// A [`panel`] standing on the surface ([`Relief::Raised`]).
pub const fn raised(rect: Rect, radius: i16, bottom_radius: i16, fill: &'static str) -> Panel {
    Panel {
        relief: Relief::Raised,
        ..panel(rect, radius, bottom_radius, fill)
    }
}

/// A [`panel`] let into the surface ([`Relief::Sunk`]).
pub const fn sunk(rect: Rect, radius: i16, bottom_radius: i16, fill: &'static str) -> Panel {
    Panel {
        relief: Relief::Sunk,
        ..panel(rect, radius, bottom_radius, fill)
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
    /// Whether the text is heavy (the Pioneers' model numbers); left out
    /// of the JSON when not.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
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

/// A skin of two halves that fold like a book (the 28S, 19BII): where each half
/// lies in the side-by-side drawing, and the hinge between them. Every
/// panel, key, mark and line of the skin lies in one half (the case, the
/// first panel, is the whole drawing and is drawn per half); the page
/// moves each half as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fold {
    /// The letter half: on the left side by side, below when stacked.
    pub letters: Rect,
    /// The display half: on the right side by side, on top when stacked.
    pub display: Rect,
    /// The hinge's barrel side by side, upright between the halves;
    /// stacked, the page lays it across between them.
    pub hinge: Rect,
    /// The part of the letter half shown stacked: its keyboard plate, the
    /// reference card above it left off, so a phone scrolls less.
    pub stacked_letters: Rect,
    /// The part of the display half shown stacked: its face, the spine
    /// beside it left off, so the keys get the phone's width.
    pub stacked_display: Rect,
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
    /// The display's glass, where it reaches further beyond the canvas
    /// than the page's narrow margin (the Pioneers' glass is taller than
    /// their square-pixel canvas); the page draws the window there. Left
    /// out of the JSON when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glass: Option<Rect>,
    /// The two halves of a clamshell (the 28S, 19BII); left out of the JSON when
    /// `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fold: Option<Fold>,
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
    /// Grain of the case's plastic, 0 (smooth, the 49G) to 100: the
    /// opacity of the page's noise over the case, in percent.
    pub texture: i16,
    /// The keys.
    pub keys: &'static [SkinKey],
}

/// The skin of `model`. The 40G uses the 39G's drawing with its own name,
/// the 17B the 17BII's with its own labels, colours and name. Every model
/// has one; the `Option` lets a new model join the hosts only with its
/// skin ([`hosted_models`]).
pub fn skin(model: Model) -> Option<&'static Skin> {
    Some(match model {
        Model::Hp48sx => &hp48sx::SKIN,
        Model::Hp48gx => &hp48gx::SKIN,
        Model::Hp38g => &hp38g::SKIN,
        Model::Hp49g => &hp49g::SKIN,
        Model::Hp39g => &hp39g::SKIN_39G,
        Model::Hp40g => &hp39g::SKIN_40G,
        Model::Hp42s => &hp42s::SKIN,
        Model::Hp27s => &hp27s::SKIN,
        Model::Hp17bii => &hp17bii::SKIN_17BII,
        Model::Hp17b => &hp17bii::SKIN_17B,
        Model::Hp28s => &hp28s::SKIN,
        Model::Hp19bii => &hp19bii::SKIN,
    })
}

/// The models the hosts' pages offer: those with a drawn skin, in
/// [`Model::ALL`] order.
pub fn hosted_models() -> impl Iterator<Item = Model> {
    Model::ALL.into_iter().filter(|&m| skin(m).is_some())
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
/// "fill","bow","relief":"flat"|"raised"|"sunk"}],"lcd":[x,y,w,h],"lcdFill",
/// "glass"?:[x,y,w,h],"fold"?:{"letters":[x,y,w,h],"display":[x,y,w,h],
/// "hinge":[x,y,w,h],"stackedLetters":[x,y,w,h],"stackedDisplay":[x,y,w,h]},
/// "logo":[x,y,w,h],
/// "marks":[{"x","y","size","fill","text","italic","bold"?}],
/// "lines":[{"x1","y1","x2","y2","stroke"}],"leftInk","rightInk","alphaInk",
/// "alphaBadge","alphaStyle":"outside"|"corner"|"badge"|"below"|"label","belowInk",
/// "small","wellFill","round","texture",
/// "keys":[{"name","rect","shape","fill","ink","well","label","left"?,
/// "right"?,"alpha"?,"below"?}],"letters":{"A":"a",...},
/// "typing":{"alpha","lowerShift","shiftFirst","alphaLocks","space":[..],"direct"},
/// "lcdRows","lcdColumns"}`; `"typing":null` and no letters on the
/// Pioneers.
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
    /// The model's pixel columns (131, 137 on the 28S and 19BII), likewise.
    pub lcd_columns: i16,
}

/// The skin of `model` with what the page draws and types from it;
/// `None` for a model without a drawn skin ([`skin`]).
pub fn skin_view(model: Model) -> Option<SkinView> {
    Some(SkinView {
        skin: skin(model)?,
        letters: Letters(letters(model)),
        typing: typing(model),
        lcd_rows: lcd_rows(model) - ANNUNCIATOR_ROWS,
        lcd_columns: lcd_columns(model),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The skin of a model that has one.
    fn skin(model: Model) -> &'static Skin {
        super::skin(model).unwrap()
    }

    /// The models with a drawn skin.
    fn skinned() -> impl Iterator<Item = Model> {
        Model::ALL.into_iter().filter(|&m| super::skin(m).is_some())
    }
    use crate::layout::alpha_letter;
    use saturnus_core::io::Key;
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
        for model in skinned() {
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
        for model in skinned() {
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
    /// strip; 137 columns on the 28S and 19BII) at square pixels, at least 3 units
    /// per pixel.
    #[test]
    fn lcd_window_is_the_display_at_square_pixels() {
        for model in skinned() {
            let lcd = skin(model).lcd;
            let columns = lcd_columns(model);
            assert!(lcd.w >= 3 * columns, "{}", model.name());
            let rows = lcd_rows(model);
            let want_h = f64::from(lcd.w) * f64::from(rows) / f64::from(columns);
            assert!(
                (f64::from(lcd.h) - want_h).abs() <= 1.0,
                "{}: {} x {} is not {columns}:{rows}",
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
        for model in skinned() {
            let s = skin(model);
            // The 42S's menu row keeps its labels; the Pioneers' display is
            // narrower than their key row (photo), so a label sits within
            // half a key pitch (50 units) of its key.
            let (menu, tolerance) = if model == Model::Hp42s {
                (["sigmaplus", "inv", "sqrt", "log", "ln", "xeq"], 50.0)
            } else if model.is_pioneer() {
                (["a", "b", "c", "d", "e", "f"], 50.0)
            } else {
                (["a", "b", "c", "d", "e", "f"], 3.0)
            };
            for (i, name) in menu.iter().enumerate() {
                let key = s.keys.iter().find(|k| k.name == *name).unwrap();
                let key_centre = f64::from(key.rect.x) + f64::from(key.rect.w) / 2.0;
                let label = softkey_label_centre(model, s, i);
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
    /// A clamshell's halves count as cases of their own.
    #[test]
    fn case_ends_shortly_below_the_bottom_row() {
        for model in skinned() {
            let s = skin(model);
            let cases = match s.fold {
                Some(f) => vec![f.letters, f.display],
                None => vec![bounds(s)],
            };
            for case in cases {
                let keys: Vec<Rect> = s
                    .keys
                    .iter()
                    .map(|k| k.rect)
                    .filter(|r| r.inside(case))
                    .collect();
                let lowest = keys.iter().map(|k| k.bottom()).max().unwrap();
                let left = keys.iter().map(|k| k.x).min().unwrap() - case.x;
                let right = keys.iter().map(|k| k.right()).max().unwrap();
                let side = f64::from(left.min(case.right() - right));
                let below = f64::from(case.bottom() - lowest);
                assert!(
                    below <= side * 1.3 && below >= side * 0.8,
                    "{}: {below} units below the keys, {side} beside them",
                    model.name()
                );
            }
        }
    }

    /// A clamshell (the 28S, 19BII): two halves of one size side by side with a
    /// gap, the hinge between the letter half's keys and the display half,
    /// and everything else in one half: every key, panel (but the case),
    /// mark, line, the LCD, its glass and the logo. Stacked, each half is
    /// cut to its keys and their labels (the letter half below its card,
    /// the display half without its spine).
    #[test]
    fn fold_halves_hold_everything() {
        for model in skinned() {
            let s = skin(model);
            let Some(f) = s.fold else {
                continue;
            };
            assert_eq!((f.letters.w, f.letters.h), (f.display.w, f.display.h));
            assert_eq!(f.letters.right().max(f.display.right()), s.width);
            assert!(f.letters.right() < f.display.x, "a gap between the halves");
            assert!(!f.letters.overlaps(f.display));
            assert!(f.hinge.inside(f.letters) && f.hinge.h > f.letters.h / 2);
            let in_one = |r: Rect| r.inside(f.letters) || r.inside(f.display);
            let at = |x: i16, y: i16| in_one(super::r(x, y, 0, 0));
            for k in s.keys {
                assert!(in_one(k.rect), "{}: {}", model.name(), k.name);
                assert!(
                    !k.rect.overlaps(f.hinge),
                    "{}: {} on the hinge",
                    model.name(),
                    k.name
                );
            }
            for p in &s.panels[1..] {
                assert!(in_one(p.rect), "{}: panel {:?}", model.name(), p.rect);
                assert!(
                    !p.rect.overlaps(f.hinge),
                    "{}: panel {:?} on the hinge",
                    model.name(),
                    p.rect
                );
            }
            for m in s.marks {
                assert!(at(m.x, m.y), "{}: {}", model.name(), m.text);
            }
            for l in s.lines {
                assert!(at(l.x1, l.y1) && at(l.x2, l.y2), "{}", model.name());
            }
            let glass = s.glass.unwrap_or(s.lcd);
            for r in [s.lcd, glass, s.logo] {
                assert!(r.inside(f.display), "{}: {r:?}", model.name());
            }
            // Stacked: each half's keys and the labels above them are
            // kept, the card above the letter keys is not.
            for (half, shown) in [
                (f.letters, f.stacked_letters),
                (f.display, f.stacked_display),
            ] {
                assert!(shown.inside(half), "{}", model.name());
                let keys = s.keys.iter().filter(|k| k.rect.inside(half));
                for k in keys.clone() {
                    let well = super::r(
                        k.rect.x - k.cap.well,
                        k.rect.y - k.cap.well,
                        k.rect.w + 2 * k.cap.well,
                        k.rect.h + 2 * k.cap.well,
                    );
                    assert!(
                        well.inside(shown),
                        "{}: {} off the stacked view",
                        model.name(),
                        k.name
                    );
                }
                let top_key = keys.map(|k| k.rect.y - k.cap.well).min().unwrap();
                assert!(
                    shown.y < top_key - 2 * s.small,
                    "the labels over the top row stay"
                );
                for p in s.panels[1..].iter().filter(|p| p.rect.inside(half)) {
                    let cut = p.rect.overlaps(shown) && !p.rect.inside(shown);
                    assert!(
                        !cut || p.rect.y < shown.y,
                        "{}: {:?} cut at the side",
                        model.name(),
                        p.rect
                    );
                }
            }
            let card = s.panels[1..]
                .iter()
                .filter(|p| p.rect.inside(f.letters))
                .map(|p| p.rect)
                .find(|r| r.y < f.stacked_letters.y);
            assert!(
                card.is_some_and(|c| c.bottom() <= f.stacked_letters.y),
                "{}",
                model.name()
            );
        }
        assert!(skin(Model::Hp28s).fold.is_some());
        assert!(
            skinned()
                .filter(|&m| skin(m).fold.is_some())
                .eq([Model::Hp28s, Model::Hp19bii])
        );
    }

    /// Every model but the Pioneers types all 26 letters, each from one key
    /// of its matrix; the 39G/40G also type a space from the plus key, the
    /// 38G with SHIFT then 2, the 19BII from its SPACE key. The Pioneers
    /// (42S, 27S, 17BII, 17B) have no letter keys and no typing data; the
    /// 19BII has no alpha or lowercase key.
    #[test]
    fn letter_map_per_model() {
        for m in skinned().filter(|m| m.is_pioneer()) {
            assert!(letters(m).is_empty() && typing(m).is_none(), "{}", m.name());
        }
        for model in skinned().filter(|m| !m.is_pioneer()) {
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
            assert_eq!(
                space,
                matches!(model, Model::Hp39g | Model::Hp40g | Model::Hp19bii)
            );
            let t = typing(model).unwrap();
            let shifted_space: &[&str] = if model == Model::Hp38g {
                &["shift", "2"]
            } else {
                &[]
            };
            assert_eq!(t.space, shifted_space, "{}", model.name());
            // The 19BII has neither key: its names are empty.
            for name in [t.alpha, t.lower_shift]
                .into_iter()
                .chain(t.space.iter().copied())
                .filter(|n| model != Model::Hp19bii || !n.is_empty())
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
        assert_eq!(at(Model::Hp19bii, 'A'), Some("lettera"));
        assert_eq!(at(Model::Hp19bii, ' '), Some("space"));
        let t19 = typing(Model::Hp19bii).unwrap();
        assert!(t19.direct && t19.alpha.is_empty() && t19.lower_shift.is_empty());
        assert_eq!(at(Model::Hp39g, 'A'), Some("vars"));
        assert_eq!(at(Model::Hp40g, ' '), Some("plus"));
        let t = |m: Model| typing(m).unwrap();
        assert!(t(Model::Hp48sx).alpha_locks && !t(Model::Hp38g).alpha_locks);
        assert!(t(Model::Hp39g).shift_first && !t(Model::Hp49g).shift_first);
    }

    #[test]
    fn colours_are_hex() {
        for model in skinned() {
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
        for model in skinned() {
            for m in skin(model).marks {
                assert!(!m.text.contains("HP"), "{}: {}", model.name(), m.text);
            }
        }
    }

    fn skin_json(model: Model) -> String {
        serde_json::to_string(&skin_view(model).unwrap()).unwrap()
    }

    #[test]
    fn json_shape() {
        for model in skinned() {
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
        assert!(j.contains("\"typing\":{\"alpha\":\"alpha\",\"lowerShift\":\"leftshift\",\"shiftFirst\":false,\"alphaLocks\":true,\"space\":[],\"direct\":false}"));
        assert!(skin_json(Model::Hp38g).contains("\"space\":[\"shift\",\"2\"]"));
        assert!(skin_json(Model::Hp38g).contains("\"alphaStyle\":\"corner\""));
        assert!(j.contains("\"wellFill\":") && j.contains("\"well\":9"));
        assert!(j.contains("\"relief\":\"sunk\"") && j.contains("\"texture\":"));
        assert!(skin_json(Model::Hp49g).contains("\"bow\":30"));
        assert!(skin_json(Model::Hp39g).contains("\" \":\"plus\""));
        let j42 = skin_json(Model::Hp42s);
        assert!(j42.contains("\"name\":\"rdn\""));
        assert!(j42.ends_with("\"letters\":{},\"typing\":null,\"lcdRows\":16,\"lcdColumns\":131}"));
        assert!(j.ends_with(",\"lcdRows\":64,\"lcdColumns\":131}"));
        assert!(!j.contains("\"fold\""));
        let j28 = skin_json(Model::Hp28s);
        assert!(j28.contains("\"fold\":{\"letters\":[0,0,836,1414],\"display\":[850,0,836,1414],"));
        assert!(j28.contains("\"stackedLetters\":[") && j28.contains("\"stackedDisplay\":["));
        assert!(j28.contains("\"alphaStyle\":\"label\""));
        assert!(j28.contains("\"direct\":true"));
        assert!(j28.contains("\"letters\":{\"A\":\"lettera\",\"B\":\"letterb\""));
        assert!(j28.ends_with(",\"lcdRows\":32,\"lcdColumns\":137}"));
        let j19 = skin_json(Model::Hp19bii);
        assert!(j19.contains("\"fold\":{\"letters\":[0,0,832,1386],\"display\":[846,0,832,1386],"));
        assert!(j19.contains("\"typing\":{\"alpha\":\"\",\"lowerShift\":\"\",\"shiftFirst\":false,\"alphaLocks\":false,\"space\":[],\"direct\":true}"));
        assert!(j19.contains("\"letters\":{\"A\":\"lettera\",\"B\":\"letterb\""));
        assert!(j19.contains("\" \":\"space\""));
        assert!(j19.ends_with(",\"lcdRows\":32,\"lcdColumns\":137}"));
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
        // Every cap stands on a black skirt (drawn as its well), the same
        // on every key; the skirts keep the measured key pitch.
        assert!(s.keys.iter().all(|k| k.cap.well == 6));
        assert_eq!(key("inv").rect.x - key("sigmaplus").rect.x, 101);
    }

    /// The depth rule's data: a texture in 0-100, and a raised or sunk
    /// panel is a plate, not a thin band (its rim would swallow it). The
    /// case itself is drawn as the body whatever its relief says.
    #[test]
    fn reliefs_and_textures() {
        for model in skinned() {
            let s = skin(model);
            assert!((0..=100).contains(&s.texture), "{}", model.name());
            assert_eq!(s.panels[0].relief, Relief::Flat, "{}", model.name());
            for p in &s.panels[1..] {
                if p.relief != Relief::Flat {
                    assert!(
                        p.rect.w >= 60 && p.rect.h >= 60,
                        "{}: {:?} panel {:?} too small",
                        model.name(),
                        p.relief,
                        p.rect
                    );
                }
            }
            // Every skin has at least one shaped panel, so the depth rule shows.
            assert!(
                s.panels[1..].iter().any(|p| p.relief != Relief::Flat),
                "{}",
                model.name()
            );
        }
        assert_eq!(skin(Model::Hp49g).texture, 0, "the 49G's blue is smooth");
    }

    /// WCAG contrast ratio of two `#rrggbb` colours, 1 to 21.
    fn contrast(a: &str, b: &str) -> f64 {
        let luminance = |c: &str| {
            let ch = |i: usize| {
                let v = f64::from(u8::from_str_radix(&c[i..i + 2], 16).unwrap()) / 255.0;
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * ch(1) + 0.7152 * ch(3) + 0.0722 * ch(5)
        };
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// The owner's colours (2026-10-08): light blue menu keys, dark blue
    /// digits and operators, a black ALPHA, an orange SHIFT; every label
    /// at least WCAG AA (4.5:1) on its cap or on the face it is printed on.
    #[test]
    fn hp39g_colours() {
        for model in [Model::Hp39g, Model::Hp40g] {
            let s = skin(model);
            let key = |n: &str| s.keys.iter().find(|k| k.name == n).unwrap();
            let menu = key("a").cap;
            assert!(["b", "c", "d", "e", "f"].iter().all(|n| key(n).cap == menu));
            let digits = key("7").cap;
            for n in [
                "0", "1", "2", "3", "4", "5", "6", "8", "9", "divide", "multiply", "minus", "plus",
                "point", "neg", "enter", "on",
            ] {
                assert_eq!(key(n).cap, digits, "{n}");
            }
            assert_eq!(key("sin").cap, key("vars").cap);
            assert_ne!(key("sin").cap, menu);
            assert_eq!(key("alpha").cap.fill, "#1f1e22");
            assert_eq!(key("shift").cap.fill, "#c64d0a");
            for k in s.keys.iter().filter(|k| !k.label.is_empty()) {
                let c = contrast(k.cap.ink, k.cap.fill);
                assert!(c >= 4.5, "{}: {} {c:.2}", model.name(), k.name);
            }
            let face = s.panels[1].fill;
            for ink in [s.left_ink, s.right_ink, s.alpha_ink, s.below_ink] {
                let c = contrast(ink, face);
                assert!(c >= 4.5, "{}: {ink} on {face} {c:.2}", model.name());
            }
        }
    }

    /// The Pioneers: 27S, 17BII and 17B in the case measured on the
    /// photographs (not the 42S's grid), each with its own blank shift key,
    /// CLR on the ON line, the menu keys printed with ^, the model's labels;
    /// every label at least 4.5:1 on its cap or on what it is printed on
    /// (the 39G rule).
    #[test]
    fn pioneer_skins() {
        let rects = |m: Model| skin(m).keys.iter().map(|k| k.rect).collect::<Vec<_>>();
        assert_ne!(
            rects(Model::Hp27s),
            rects(Model::Hp42s),
            "measured, not the 42S's"
        );
        for (model, shift, name) in [
            (Model::Hp27s, "#6098c0", "27S"),
            (Model::Hp17bii, "#d5a356", "17B II"),
            (Model::Hp17b, "#e0a850", "17B"),
        ] {
            let s = skin(model);
            let key = |n: &str| s.keys.iter().find(|k| k.name == n).unwrap();
            assert_eq!(
                rects(model),
                pioneer::GRID,
                "{}: the Pioneer grid",
                model.name()
            );
            assert_eq!(key("shift").cap.fill, shift, "{}", model.name());
            assert!(key("shift").label.is_empty());
            assert_eq!((key("on").label, key("on").left), ("CLR", "OFF"));
            assert_eq!(key("on").below, "ON");
            // INPUT spans the first two columns, gap included.
            assert_eq!(key("input").rect.x, key("sto").rect.x);
            assert_eq!(key("input").rect.right(), key("rcl").rect.right());
            assert!(s.keys[..6].iter().all(|k| k.label == "^"));
            assert!(
                s.marks.iter().any(|m| m.text == name && m.bold),
                "{}",
                model.name()
            );
            assert!(
                s.keys
                    .iter()
                    .all(|k| k.right.is_empty() && k.alpha.is_empty())
            );
            for k in s.keys.iter().filter(|k| !k.label.is_empty()) {
                let c = contrast(k.cap.ink, k.cap.fill);
                assert!(c >= 4.5, "{}: {} {c:.2}", model.name(), k.name);
            }
            // The keyboard plate and the band behind the menu row, then the
            // surround the lettering is on.
            for plate in [pioneer::PLATE, pioneer::BAND] {
                let face = s.panels[plate].fill;
                for ink in [s.left_ink, s.below_ink] {
                    let c = contrast(ink, face);
                    assert!(c >= 4.5, "{}: {ink} on {face} {c:.2}", model.name());
                }
            }
            for m in s.marks {
                let c = contrast(m.fill, s.panels[pioneer::SURROUND].fill);
                assert!(c >= 4.5, "{}: {} {c:.2}", model.name(), m.text);
            }
            // Bars behind shifted labels (the 27S's digit block).
            for p in &s.panels[pioneer::BAND + 1..] {
                let c = contrast(s.left_ink, p.fill);
                assert!(c >= 4.5, "{}: label on {} {c:.2}", model.name(), p.fill);
            }
            // The glass reaches beyond the canvas, inside the bezel.
            let glass = s.glass.expect("the Pioneers' glass");
            assert!(s.lcd.inside(glass) && glass.inside(s.panels[3].rect));
            assert!(glass.h > s.lcd.h + 40, "taller than the canvas");
        }
        let shifted = |m: Model| skin(m).keys.iter().map(|k| k.left).collect::<Vec<_>>();
        assert_eq!(
            shifted(Model::Hp27s)[..12],
            [
                "√x", "x²", "eˣ", "LN", "yˣ", "1/x", "10ˣ", "LOG", "ASIN", "ACOS", "ATAN", "MAIN"
            ]
        );
        assert_eq!(
            skin(Model::Hp27s).panels.len(),
            6 + 12,
            "a grey bar behind each menu name of the digit block"
        );
        // The 17B is the 17BII without RND, R↓, x≷y and ENTER.
        let (bii, b) = (skin(Model::Hp17bii), skin(Model::Hp17b));
        let differs: Vec<&str> = bii
            .keys
            .iter()
            .zip(b.keys)
            .filter(|(x, y)| (x.label, x.left, x.below) != (y.label, y.left, y.below))
            .map(|(x, _)| x.name)
            .collect();
        assert_eq!(differs, ["rcl", "lparen", "rparen", "equals"]);
        let at = |s: &Skin, n: &str| *s.keys.iter().find(|k| k.name == n).unwrap();
        assert_eq!(at(bii, "rcl").left, "RND");
        assert_eq!(at(bii, "equals").below, "ENTER");
        assert!(at(b, "equals").below.is_empty() && at(b, "rcl").left.is_empty());
        assert_ne!(bii.panels, b.panels);
        assert_eq!(
            bii.lines.len(),
            3,
            "the 17BII's gold frame, open at the top"
        );
        assert!(b.lines.is_empty());
    }

    /// The 28S: letter keys typing their own letters, SPACE and ENTER two
    /// keys wide, a red blank shift key, ON with OFF above and ATTN below,
    /// the cursor menu printed over the menu keys, the model name; every
    /// label at least 4.5:1 on its cap or on what it is printed on (the
    /// 39G rule).
    #[test]
    fn hp28s_skin() {
        let s = skin(Model::Hp28s);
        let key = |n: &str| {
            s.keys
                .iter()
                .find(|k| k.name == n)
                .unwrap_or_else(|| panic!("{n}"))
        };
        for k in s.keys.iter().filter(|k| !k.alpha.is_empty()) {
            assert_eq!(k.label, k.alpha, "{}", k.name);
            assert_eq!(k.name, format!("letter{}", k.alpha.to_lowercase()));
        }
        // ENTER spans the menu row's first two columns, SPACE the letter
        // half's, gaps included.
        assert_eq!(key("enter").rect.x, key("a").rect.x);
        assert_eq!(key("enter").rect.right(), key("b").rect.right());
        assert_eq!(key("space").rect.x, key("lettera").rect.x);
        assert_eq!(key("space").rect.right(), key("letterb").rect.right());
        assert!(key("shift").label.is_empty());
        assert_eq!(key("shift").cap.fill, "#7e2a35");
        assert_eq!(
            (key("on").label, key("on").left, key("on").below),
            ("ON", "OFF", "ATTN")
        );
        assert!(
            ["a", "b", "c", "d", "e", "f"]
                .iter()
                .all(|n| key(n).label.is_empty())
        );
        assert_eq!(key("nxt").cap, key("7").cap, "NEXT is a light key");
        let cursor: Vec<&str> = s
            .marks
            .iter()
            .filter(|m| m.y == 638)
            .map(|m| m.text)
            .collect();
        assert_eq!(cursor, ["INS", "DEL", "▲", "▼", "◀", "▶"]);
        assert!(s.marks.iter().any(|m| m.text == "28S" && m.bold));
        assert!(s.keys.iter().all(|k| k.right.is_empty()));
        for k in s.keys.iter().filter(|k| !k.label.is_empty()) {
            let c = contrast(k.cap.ink, k.cap.fill);
            assert!(c >= 4.5, "{} {c:.2}", k.name);
        }
        // Shifted labels and ATTN on both plates and the bands.
        let plates = [hp28s::LETTER_PLATE, hp28s::DISPLAY_PLATE];
        let bands = &s.panels[hp28s::FIRST_BAND..hp28s::FIRST_BAND + 19];
        assert!(bands.iter().all(|p| p.rect.h == 18));
        for face in plates.iter().map(|&i| &s.panels[i]).chain(bands) {
            for ink in [s.left_ink, s.below_ink] {
                let c = contrast(ink, face.fill);
                assert!(c >= 4.5, "{ink} on {} {c:.2}", face.fill);
            }
        }
        // Each mark on the topmost panel under it.
        for m in s.marks {
            let under = s
                .panels
                .iter()
                .rev()
                .find(|p| super::r(m.x, m.y - m.size / 2, 0, 0).inside(p.rect))
                .unwrap();
            let c = contrast(m.fill, under.fill);
            assert!(c >= 4.5, "{} on {} {c:.2}", m.text, under.fill);
        }
        assert!(s.panels[hp28s::CARD].rect.bottom() <= s.fold.unwrap().stacked_letters.y);
        assert_eq!(s.panels[hp28s::NAME_PLATE].relief, Relief::Sunk);
        assert_eq!(s.alpha_style, AlphaStyle::Label);
    }
    /// The 19BII (P1, P2 and the Owner's Manual's Figure 1-2): letter keys
    /// typing their own letters, SPACE and INPUT two keys wide, the blank
    /// orange shift, cream menu keys, digits, operators and EXIT, grey
    /// function keys, ENTER printed under =, every label 4.5:1 on its cap
    /// or on the face under it.
    #[test]
    fn hp19bii_skin() {
        let s = skin(Model::Hp19bii);
        let key = |n: &str| {
            s.keys
                .iter()
                .find(|k| k.name == n)
                .unwrap_or_else(|| panic!("{n}"))
        };
        for k in s.keys.iter().filter(|k| k.alpha.len() == 1) {
            assert_eq!(k.label, k.alpha, "{}", k.name);
            assert_eq!(k.name, format!("letter{}", k.alpha.to_lowercase()));
        }
        assert_eq!(key("space").rect.x + key("space").rect.w, 272);
        assert_eq!(key("input").rect.x, key("a").rect.x);
        assert_eq!(key("input").rect.right(), key("b").rect.right());
        assert!(key("shift").label.is_empty());
        assert_eq!(key("shift").cap.fill, "#eda646");
        assert_eq!(key("exit").cap, key("7").cap, "EXIT is a cream key");
        assert_eq!(key("a").cap, key("7").cap, "the menu keys are cream");
        assert_eq!(key("sto").cap, key("lettera").cap, "STO is grey");
        assert_eq!((key("equals").left, key("equals").below), ("LAST", "ENTER"));
        assert_eq!(
            (key("dsp").left, key("prt").left, key("exit").left),
            ("MODES", "PRINTER", "MAIN")
        );
        assert!(
            ["a", "b", "c", "d", "e", "f"]
                .iter()
                .all(|n| key(n).label.is_empty())
        );
        assert!(s.marks.iter().any(|m| m.text == "19BII" && m.bold));
        assert!(s.keys.iter().all(|k| k.right.is_empty()));
        for k in s.keys.iter().filter(|k| !k.label.is_empty()) {
            let c = contrast(k.cap.ink, k.cap.fill);
            assert!(c >= 4.5, "{} {c:.2}", k.name);
        }
        for face in [hp19bii::LETTER_PLATE, hp19bii::DISPLAY_PLATE].map(|i| &s.panels[i]) {
            for ink in [s.left_ink, s.below_ink] {
                let c = contrast(ink, face.fill);
                assert!(c >= 4.5, "{ink} on {} {c:.2}", face.fill);
            }
        }
        for m in s.marks {
            let under = s
                .panels
                .iter()
                .rev()
                .find(|p| super::r(m.x, m.y - m.size / 2, 0, 0).inside(p.rect))
                .unwrap();
            let c = contrast(m.fill, under.fill);
            assert!(c >= 4.5, "{} on {} {c:.2}", m.text, under.fill);
        }
        let groups = &s.panels[hp19bii::FIRST_GROUP..];
        assert_eq!(groups.len(), 4);
        assert!(
            groups
                .iter()
                .all(|g| g.rect.inside(s.panels[hp19bii::CARD].rect))
        );
        assert!(s.panels[hp19bii::CARD].rect.bottom() <= s.fold.unwrap().stacked_letters.y);
        assert_eq!(s.panels[hp19bii::NAME_PLATE].relief, Relief::Sunk);
        assert_eq!(s.alpha_style, AlphaStyle::Label);
    }
}
