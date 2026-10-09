//! The cold boot's question, answered for the user. A 48SX, 48GX or 49G
//! booted with an empty memory asks "Try To Recover Memory?" with YES
//! and NO on the first and sixth menu keys; with nothing to recover,
//! NO is the only sensible answer, and until it is given the calculator
//! takes no typing and has no memory to read. A host that keeps the
//! user's calculator ([`super::Engine::set_answer_recover`]) answers NO
//! itself once the question is on the screen and the ROM waits for a
//! key; the 49G then shows "Memory Clear" in a box with OK, which is
//! dismissed the same way. Any key the user presses first, or a
//! question that does not come in time, ends it.
//!
//! The screen is recognised by its layout, not its text (measured on
//! the three ROMs, J, R and 2.10): the question's menu row has a label
//! under the first and the sixth menu keys and none under the four
//! between, the rows between the top lines and the menu are blank, and
//! the top lines hold the question. The 49G's box after NO has an OK
//! label under the sixth menu key and the five others solid.

use saturnus::Model;

/// Rows of the menu labels at the bottom of a 64-row display: a blank
/// row, then 7 rows of label.
const MENU_ROWS: usize = 8;
/// A solid (empty) label: all of its 7 by 21 pixels lit.
const SOLID: usize = 7 * 21;
/// Pitch and width of a menu label, in pixels (wiki: hardware/display
/// "Menu labels").
const LABEL_PITCH: usize = 22;
const LABEL_WIDTH: usize = 21;
/// Emulated ms after the boot in which the question may come.
pub const PROMPT_WAIT_MS: f64 = 30_000.0;
/// Emulated ms after NO in which the 49G's "Memory Clear" box may come.
pub const DISMISS_WAIT_MS: f64 = 5_000.0;

/// What is answered next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// "Try To Recover Memory?": NO.
    Prompt,
    /// The 49G's "Memory Clear": OK.
    Dismiss,
}

/// An answer pending, until emulated time `until_ms`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recover {
    pub step: Step,
    pub until_ms: f64,
}

/// Whether `model` asks the question on a cold boot.
pub fn asks(model: Model) -> bool {
    matches!(model, Model::Hp48sx | Model::Hp48gx | Model::Hp49g)
}

/// Lit pixels of menu label `i` (0 to 5) in the menu row of `rows`.
fn label(rows: &[[bool; saturnus::LCD_WIDTH]], i: usize) -> usize {
    let top = rows.len() - MENU_ROWS;
    rows[top..]
        .iter()
        .map(|r| {
            r[i * LABEL_PITCH..(i * LABEL_PITCH + LABEL_WIDTH).min(r.len())]
                .iter()
                .filter(|&&p| p)
                .count()
        })
        .sum()
}

fn lit(rows: &[[bool; saturnus::LCD_WIDTH]], from: usize, to: usize) -> usize {
    rows[from..to]
        .iter()
        .map(|r| r.iter().filter(|&&p| p).count())
        .sum()
}

/// Whether the screen `rows` (64 of them) is the question.
pub fn is_prompt(rows: &[[bool; saturnus::LCD_WIDTH]]) -> bool {
    if rows.len() != 64 {
        return false;
    }
    let labels: Vec<usize> = (0..6).map(|i| label(rows, i)).collect();
    labels[0] > SOLID / 4
        && labels[5] > SOLID / 4
        && labels[1..5].iter().all(|&n| n == 0)
        && lit(rows, 16, 64 - MENU_ROWS) == 0
        && lit(rows, 0, 16) > 0
}

/// Whether the screen `rows` is the 49G's "Memory Clear" box: five
/// solid menu labels and OK under the sixth.
pub fn is_memory_clear(rows: &[[bool; saturnus::LCD_WIDTH]]) -> bool {
    if rows.len() != 64 {
        return false;
    }
    let labels: Vec<usize> = (0..6).map(|i| label(rows, i)).collect();
    labels[..5].iter().all(|&n| n >= SOLID) && labels[5] > SOLID / 4 && labels[5] < SOLID
}

#[cfg(test)]
mod tests {
    use super::*;
    use saturnus::LCD_WIDTH;

    fn screen() -> Vec<[bool; LCD_WIDTH]> {
        vec![[false; LCD_WIDTH]; 64]
    }

    /// Label `i` drawn as a box with a hole (its text).
    fn draw_label(s: &mut [[bool; LCD_WIDTH]], i: usize, solid: bool) {
        let x0 = i * LABEL_PITCH;
        for (y, row) in s.iter_mut().enumerate().skip(57) {
            for (x, p) in row.iter_mut().enumerate().skip(x0).take(LABEL_WIDTH) {
                *p = solid || !(58..62).contains(&y) || !(x0 + 6..x0 + 14).contains(&x);
            }
        }
    }

    #[test]
    fn the_question_is_two_labels_a_top_line_and_nothing_between() {
        let mut s = screen();
        draw_label(&mut s, 0, false);
        draw_label(&mut s, 5, false);
        s[5][10] = true;
        assert!(is_prompt(&s));
        // The stack's level numbers between: a normal screen.
        let mut stack = s.clone();
        stack[40][1] = true;
        assert!(!is_prompt(&stack));
        // A third label: a menu.
        let mut menu = s.clone();
        draw_label(&mut menu, 2, false);
        assert!(!is_prompt(&menu));
        // No question on top.
        let mut blank_top = s.clone();
        blank_top[5][10] = false;
        assert!(!is_prompt(&blank_top));
        // The 42S's 16 rows never are.
        assert!(!is_prompt(&s[..16]));
    }

    #[test]
    fn memory_clear_is_five_solid_labels_and_ok() {
        let mut s = screen();
        for i in 0..5 {
            draw_label(&mut s, i, true);
        }
        draw_label(&mut s, 5, false);
        assert!(is_memory_clear(&s));
        let mut menu = s.clone();
        draw_label(&mut menu, 2, false);
        assert!(!is_memory_clear(&menu));
        assert!(!is_prompt(&s));
    }

    #[test]
    fn only_the_48_and_49g_ask() {
        assert!(asks(Model::Hp48sx) && asks(Model::Hp48gx) && asks(Model::Hp49g));
        assert!(!asks(Model::Hp38g) && !asks(Model::Hp39g) && !asks(Model::Hp42s));
    }
}
