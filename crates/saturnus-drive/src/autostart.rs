//! Scripted boot and Kermit server start-up, shared by the CLI's
//! `--autostart` and the Kermit test host (`saturnus-kermit`).

use anyhow::{Result, bail};
use saturnus::Model;
use saturnus::io::Key;

use crate::script::{Action, DEFAULT_HOLD_MS, Line};

/// Longest wait for the boot prompt, in emulated milliseconds.
pub const BOOT_CAP_MS: u64 = 60_000;

/// The keys that take a cold-booted `model` from its first screen to the
/// stack: NO at "Try To Recover Memory?" (softkey F) on the 48SX and 48GX;
/// NO, then OK at "Memory Clear" on the 49G; OK at the "Memory Clear" box
/// of the 38G, 39G and 40G (menu key 6; wiki: hardware/hp38g,
/// hardware/hp39g-40g); nothing on the 42S, whose cold start shows
/// "Memory Clear" above a ready stack (wiki: hardware/hp42s).
pub fn boot_keys(model: Model) -> &'static [Key] {
    match model {
        Model::Hp48sx | Model::Hp48gx | Model::Hp38g | Model::Hp39g | Model::Hp40g => &[Key::F],
        Model::Hp49g => &[Key::F, Key::F],
        Model::Hp42s => &[],
    }
}

/// Wait for the boot prompt (at most [`BOOT_CAP_MS`]), then press
/// [`boot_keys`].
pub fn boot_script(model: Model) -> Vec<Line> {
    numbered(boot_actions(model))
}

fn press(key: Key) -> Action {
    Action::Press {
        key,
        hold_ms: DEFAULT_HOLD_MS,
    }
}

fn boot_actions(model: Model) -> Vec<Action> {
    let mut actions = vec![Action::WaitIdle {
        cap_ms: BOOT_CAP_MS,
    }];
    actions.extend(boot_keys(model).iter().map(|&k| press(k)));
    actions
}

fn numbered(actions: Vec<Action>) -> Vec<Line> {
    actions
        .into_iter()
        .enumerate()
        .map(|(i, action)| Line {
            number: i + 1,
            action,
        })
        .collect()
}

/// The ROM's `SERVER` command typed after boot, as the saturnng
/// container's entrypoint does: with no `--load`, first answer "Try To
/// Recover Memory?" with NO (softkey F); the 49G then shows "Memory Clear"
/// with an OK softkey, also F, while the 48SX and 48GX go straight to the
/// stack. Then ALPHA ALPHA S E R V E R ENTER.
///
/// The 42S is refused (no serial port). The 38G, 39G and 40G are refused: a cold boot shows a "Memory Clear"
/// box rather than the recover prompt, and they have no RPL command line
/// and no `SERVER` command; their PC link is driven from the calculator
/// (wiki: hardware/hp38g, hardware/hp39g-40g). The 48 models share the key
/// matrix and the alpha letters' positions (wiki: hardware/keyboard "HP48
/// matrix"); the GX's right-shift right-arrow SERVER key is not used, to
/// match the container. The 49G's letters sit on other keys: S = SIN,
/// E = softkey E, R = square root, V = EEX (typed on saturnus with ALPHA
/// locked: the keys from APPS to the divide key give G to Z).
pub fn autostart_script(model: Model, fresh_boot: bool) -> Result<Vec<Line>> {
    if model == Model::Hp42s {
        bail!("the 42S has no serial port and no Kermit server; --autostart is not supported");
    }
    if matches!(model, Model::Hp38g | Model::Hp39g | Model::Hp40g) {
        bail!(
            "the {} has no Kermit server command; --autostart is not supported",
            model.name().to_uppercase()
        );
    }
    let mut actions = Vec::new();
    if fresh_boot {
        actions.extend(boot_actions(model));
    }
    actions.push(press(Key::Alpha));
    actions.push(press(Key::Alpha));
    for c in "SERVER".chars() {
        let key = match (model, c) {
            (Model::Hp49g, 'S') => Key::Sin,
            (Model::Hp49g, 'E') => Key::E,
            (Model::Hp49g, 'R') => Key::Sqrt,
            (Model::Hp49g, _) => Key::Eex,
            // 48SX and 48GX: S = SIN, E = softkey E, R = right arrow,
            // V = square root.
            (_, 'S') => Key::Sin,
            (_, 'E') => Key::E,
            (_, 'R') => Key::Right,
            _ => Key::Sqrt,
        };
        actions.push(press(key));
    }
    // ENTER starts the server; it then never idles in SHUTDN for long, so
    // only hold the key and give the ROM time to draw its banner.
    actions.push(Action::Down(Key::Enter));
    actions.push(Action::Wait {
        ms: DEFAULT_HOLD_MS,
    });
    actions.push(Action::Up(Key::Enter));
    actions.push(Action::Wait { ms: 1_000 });
    Ok(numbered(actions))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn autostart_types_server_on_the_49g() {
        let keys: Vec<Key> = autostart_script(Model::Hp49g, true)
            .unwrap()
            .iter()
            .filter_map(|l| match l.action {
                Action::Press { key, .. } => Some(key),
                _ => None,
            })
            .collect();
        // NO, OK, then SERVER with the 49G's letters.
        assert_eq!(
            keys,
            [
                Key::F,
                Key::F,
                Key::Alpha,
                Key::Alpha,
                Key::Sin,
                Key::E,
                Key::Sqrt,
                Key::Eex,
                Key::E,
                Key::Sqrt
            ]
        );
    }

    #[test]
    fn autostart_refuses_the_38g() {
        let err = autostart_script(Model::Hp38g, true).unwrap_err();
        assert_eq!(
            err.to_string(),
            "the 38G has no Kermit server command; --autostart is not supported"
        );
        assert!(autostart_script(Model::Hp38g, false).is_err());
        let err = autostart_script(Model::Hp40g, true).unwrap_err();
        assert!(
            err.to_string().starts_with("the 40G has no Kermit"),
            "{err}"
        );
    }

    #[test]
    fn autostart_types_server() {
        let keys: Vec<Key> = autostart_script(Model::Hp48sx, true)
            .unwrap()
            .iter()
            .filter_map(|l| match l.action {
                Action::Press { key, .. } => Some(key),
                _ => None,
            })
            .collect();
        assert_eq!(
            keys,
            [
                Key::F,
                Key::Alpha,
                Key::Alpha,
                Key::Sin,
                Key::E,
                Key::Right,
                Key::Sqrt,
                Key::E,
                Key::Right
            ]
        );
        assert!(
            !autostart_script(Model::Hp48sx, false)
                .unwrap()
                .iter()
                .any(|l| l.action
                    == Action::Press {
                        key: Key::F,
                        hold_ms: DEFAULT_HOLD_MS
                    })
        );
    }

    #[test]
    fn boot_script_answers_the_first_screen() {
        let keys = |m| -> Vec<Key> {
            boot_script(m)
                .iter()
                .filter_map(|l| match l.action {
                    Action::Press { key, .. } => Some(key),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(keys(Model::Hp48sx), [Key::F]);
        assert_eq!(keys(Model::Hp49g), [Key::F, Key::F]);
        assert_eq!(keys(Model::Hp38g), [Key::F]);
        assert_eq!(keys(Model::Hp39g), [Key::F]);
        assert_eq!(
            boot_script(Model::Hp48gx)[0].action,
            Action::WaitIdle {
                cap_ms: BOOT_CAP_MS
            }
        );
    }
}
