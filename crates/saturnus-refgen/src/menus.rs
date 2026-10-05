//! The ROM's built-in menus, crawled on the keyboard: which commands each
//! menu page offers, which keys open which submenus, and the submenus'
//! label text.
//!
//! - A menu page is shown with `n.pp MENU`; its six labels are read off
//!   the LCD. A label with a tab on its top-left corner opens a submenu.
//! - Command keys are pressed in program entry mode (after `«`), where a
//!   menu key types its command's name instead of running it; ENTER puts
//!   the program on the stack and `RCLMENU` the menu shown afterwards (if a
//!   key navigated after all, that page is pressed again key by key).
//! - A submenu key is pressed normally and `RCLMENU` names its target.
//! - Each run starts with ON and pushes a marker number first; the stack
//!   is fetched once per menu over Kermit, so a run that went astray only
//!   loses its own results.
//!
//! Submenu labels are read with [`Font`], built from the ROM's own label
//! drawing: `TMENU` on a list of one-character strings.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use hptx_core::TransferMode;
use saturnus::Model;
use saturnus_mcp::emulator::Emulator;

/// LCD rows and columns of the menu labels.
const LCD_ROWS: usize = 64;
const LCD_COLS: usize = 131;
const TAB_ROW: usize = 56;
const BOX_TOP: usize = 57;
const TEXT_ROWS: std::ops::RangeInclusive<usize> = 58..=62;
const BOX_BOTTOM: usize = 63;
const LABEL_W: usize = 21;
const LABEL_PITCH: usize = 22;
const SOFTKEYS: [&str; 6] = ["a", "b", "c", "d", "e", "f"];
/// ON twice: the first may only clear an error message and leave the
/// command line it was about.
const CANCEL: &str = "on on";
/// Most pages read per menu.
const MAX_PAGES: u32 = 12;
/// Status area rows above the stack (the TIME menus' clock is there).
const STATUS_ROWS: usize = 16;
/// Presses of a key that should change the screen, or of the keys that
/// lead back to the VAR menu.
const PRESS_TRIES: usize = 4;
/// Most presses that bring a cycling label back (PARIT has five states).
const MAX_TOGGLE_STATES: usize = 8;
/// Run markers are this plus the run index.
const MARKER_BASE: u64 = 100_000;
/// Temporary variable for the fetched stack.
const TEMP: &str = "SATRNREF";

/// What a menu key is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelKind {
    /// No label.
    Empty,
    /// A plain label: a command, or an action.
    Plain,
    /// A label with a tab: opens a submenu.
    Submenu,
}

/// One menu label off the LCD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    /// What the key is.
    pub kind: LabelKind,
    /// Text pixels (light on the dark label) per column, bit `r` for text
    /// row `r`.
    pub columns: [u8; LABEL_W],
}

/// The six labels of the screen `text` (64 lines of 131 `#`/`.`).
pub fn read_labels(text: &str) -> Result<[Label; 6]> {
    let rows: Vec<&[u8]> = text.lines().map(str::as_bytes).collect();
    if rows.len() != LCD_ROWS || rows.iter().any(|r| r.len() != LCD_COLS) {
        bail!("the screen is not {LCD_ROWS} lines of {LCD_COLS} pixels");
    }
    let dark = |r: usize, c: usize| rows[r][c] == b'#';
    Ok(std::array::from_fn(|i| {
        let x0 = i * LABEL_PITCH;
        let boxed = (BOX_TOP..=BOX_BOTTOM).any(|r| (x0..x0 + LABEL_W).any(|c| dark(r, c)));
        let mut columns = [0u8; LABEL_W];
        if boxed {
            for (c, col) in columns.iter_mut().enumerate() {
                for (bit, r) in TEXT_ROWS.enumerate() {
                    if !dark(r, x0 + c) {
                        *col |= 1 << bit;
                    }
                }
            }
        }
        let kind = if !boxed || columns.iter().all(|&c| c == 0) {
            LabelKind::Empty
        } else if (x0 + 1..=x0 + 5).all(|c| dark(TAB_ROW, c)) {
            LabelKind::Submenu
        } else {
            LabelKind::Plain
        };
        Label { kind, columns }
    }))
}

/// The label glyphs: each character's text columns, without the blank
/// columns around it.
#[derive(Clone, Debug, Default)]
pub struct Font {
    glyphs: Vec<(char, Vec<u8>)>,
}

/// Preference among glyphs that draw alike: letters, digits, then the rest.
fn rank(c: char) -> u8 {
    if c.is_ascii_uppercase() {
        0
    } else if c.is_ascii_digit() {
        1
    } else if c.is_ascii_lowercase() {
        2
    } else {
        3
    }
}

fn trimmed(columns: &[u8]) -> &[u8] {
    let start = columns
        .iter()
        .position(|&c| c != 0)
        .unwrap_or(columns.len());
    let end = columns
        .iter()
        .rposition(|&c| c != 0)
        .map_or(start, |e| e + 1);
    &columns[start..end]
}

impl Font {
    /// A font from `(character, label)` pairs.
    pub fn from_labels(pairs: impl IntoIterator<Item = (char, Label)>) -> Self {
        let mut glyphs: Vec<(char, Vec<u8>)> = pairs
            .into_iter()
            .filter(|(_, l)| l.kind != LabelKind::Empty)
            .map(|(c, l)| (c, trimmed(&l.columns).to_vec()))
            .collect();
        glyphs.sort_by_key(|(c, _)| (rank(*c), *c));
        Font { glyphs }
    }

    /// Number of glyphs.
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Whether the font has no glyphs.
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// The text of `label`: at each position the widest glyph that matches
    /// and is followed by a blank column or the end; `?` where none does.
    pub fn read(&self, label: &Label) -> String {
        let cols = trimmed(&label.columns);
        let mut out = String::new();
        let mut i = 0;
        while i < cols.len() {
            if cols[i] == 0 {
                i += 1;
                continue;
            }
            let found = self
                .glyphs
                .iter()
                .filter(|(_, g)| {
                    let end = i + g.len();
                    !g.is_empty()
                        && end <= cols.len()
                        && cols[i..end] == g[..]
                        && (end == cols.len() || cols[end] == 0)
                })
                .max_by_key(|(c, g)| (g.len(), std::cmp::Reverse(rank(*c))));
            match found {
                Some((c, g)) => {
                    out.push(*c);
                    i += g.len();
                }
                None => {
                    out.push('?');
                    while i < cols.len() && cols[i] != 0 {
                        i += 1;
                    }
                }
            }
        }
        out
    }
}

/// The key script that opens a program on the command line.
fn program_keys(model: Model) -> &'static str {
    if model == Model::Hp49g {
        "rightshift plus"
    } else {
        "leftshift minus"
    }
}

/// The key script that runs EVAL.
fn eval_keys(model: Model) -> &'static str {
    if model == Model::Hp49g {
        "rightshift symb"
    } else {
        "eval"
    }
}

/// HP character codes drawn into the font (`n CHR`).
const FIRST_CHAR: u32 = 33;
const LAST_CHAR: u32 = 255;

/// Build the label font: a temporary menu of the characters 33-255, one
/// per label, paged through with NXT. The Kermit server must be stopped.
pub fn build_font(emu: &mut Emulator) -> Result<Font> {
    let count = LAST_CHAR - FIRST_CHAR + 1;
    emu.start_server()?;
    let reply = emu.run_command(&format!(
        "{FIRST_CHAR}. {LAST_CHAR}. FOR c c CHR NEXT {count}. \u{2192}LIST TMENU"
    ))?;
    emu.stop_server()?;
    if let Some(e) = reply.error {
        bail!("cannot build the character menu: {e}");
    }
    let mut pairs = Vec::new();
    let mut code = FIRST_CHAR;
    while code <= LAST_CHAR {
        let (screen, _) = emu.screen_text()?;
        for label in read_labels(&screen)? {
            if code <= LAST_CHAR {
                let c = hptx_core::charset::decode(&[code as u8])
                    .chars()
                    .next()
                    .context("undecodable character")?;
                pairs.push((c, label));
            }
            code += 1;
        }
        emu.press_keys("nxt")?;
    }
    Ok(Font::from_labels(pairs))
}

/// What one submenu key opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submenu {
    /// Key index 0-5 (A-F).
    pub key: usize,
    /// The label text.
    pub label: String,
    /// The menu it shows, `None` when the key showed none.
    pub target: Option<u32>,
}

/// One page of a menu.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    /// The labels' text, `""` for an empty key.
    pub labels: Vec<String>,
    /// The words typed by the plain keys in program entry mode.
    pub words: Vec<String>,
    /// The submenu keys.
    pub submenus: Vec<Submenu>,
}

/// A crawled menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    /// The `MENU` number.
    pub number: u32,
    /// Its pages in order.
    pub pages: Vec<Page>,
    /// Why its keys could not be crawled (only the labels were read).
    pub error: Option<String>,
}

/// What a run does after showing its menu page.
#[derive(Clone, Debug)]
enum RunKind {
    /// Type these keys in program entry mode, each after left-shift if
    /// `shifted`.
    Program {
        page: usize,
        keys: Vec<usize>,
        shifted: bool,
    },
    /// Press one submenu key.
    Submenu { page: usize, key: usize },
}

impl RunKind {
    fn page(&self) -> usize {
        match self {
            RunKind::Program { page, .. } | RunKind::Submenu { page, .. } => *page,
        }
    }
}

/// One top-level item of a fetched stack.
#[derive(Clone, Debug, PartialEq)]
enum Item {
    Number(f64),
    /// A program's words.
    Program(Vec<String>),
    Other,
}

/// The top-level items of an ASCII-transferred list, in order.
fn list_items(text: &str) -> Result<Vec<Item>> {
    let body = text
        .find('{')
        .and_then(|a| text.rfind('}').map(|b| &text[a + 1..b]))
        .context("the fetched stack holds no list")?;
    let mut items = Vec::new();
    let mut words = Vec::new();
    let mut depth = 0usize;
    let mut kind_program = false;
    let mut in_string = false;
    let mut string_word = String::new();
    for w in body.split_whitespace() {
        if in_string {
            string_word.push(' ');
            string_word.push_str(w);
            if ends_string(w) {
                in_string = false;
                words.push(std::mem::take(&mut string_word));
            }
            continue;
        }
        if w.starts_with('"') && !(w.len() > 1 && ends_string(w)) {
            in_string = true;
            string_word = w.to_string();
            continue;
        }
        match w {
            "«" | "{" | "[" | "[[" => {
                if depth == 0 {
                    kind_program = w == "«";
                    words.clear();
                }
                depth += 1;
            }
            "»" | "}" | "]" | "]]" if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    items.push(if kind_program {
                        Item::Program(std::mem::take(&mut words))
                    } else {
                        Item::Other
                    });
                }
            }
            _ if depth > 0 => words.push(w.to_string()),
            _ => items.push(
                w.trim_end_matches('.')
                    .parse::<f64>()
                    .map_or(Item::Other, Item::Number),
            ),
        }
    }
    Ok(items)
}

fn ends_string(w: &str) -> bool {
    w.ends_with('"') && !w.ends_with("\\\"")
}

/// The menu number and page of a `RCLMENU` result.
fn menu_of(x: f64) -> (u32, u32) {
    let n = x.trunc();
    (n as u32, ((x - n) * 100.0).round() as u32)
}

/// Crawls the menus of one calculator. Each menu starts from the state
/// the crawler was made in, so a key that changed a setting (IR/W in the
/// I/O setup menu switches the port) cannot affect the next.
#[derive(Debug)]
pub struct Crawler<'a> {
    emu: &'a mut Emulator,
    font: &'a Font,
    state: PathBuf,
    /// The labels of the VAR menu in that state.
    var_labels: [Label; 6],
}

impl Drop for Crawler<'_> {
    fn drop(&mut self) {
        // Best effort: a leftover file in the temporary directory.
        let _ = std::fs::remove_file(&self.state);
    }
}

impl<'a> Crawler<'a> {
    /// A crawler over `emu` (Kermit server stopped) reading labels with
    /// `font`; saves the machine state to a temporary file.
    pub fn new(emu: &'a mut Emulator, font: &'a Font) -> Result<Self> {
        let state = std::env::temp_dir().join(format!(
            "saturnus-refgen-{}-{}.state",
            std::process::id(),
            emu.model().name()
        ));
        emu.press_keys(CANCEL)?;
        emu.press_keys("var")?;
        let (screen, _) = emu.screen_text()?;
        let var_labels = read_labels(&screen)?;
        emu.save_state(&state, false)?;
        Ok(Crawler {
            emu,
            font,
            state,
            var_labels,
        })
    }

    /// Type `text` as single key presses, each waiting for the calculator
    /// to settle. (`type_text` loses keys on the 48SX while the TIME
    /// menu's clock is shown.)
    fn type_keys(&mut self, text: &str) -> Result<()> {
        let keys = saturnus_mcp::keys::type_keys(self.emu.model(), text)?;
        let script: Vec<&str> = keys.iter().map(|k| k.name()).collect();
        self.emu.press_keys(&script.join(" "))?;
        Ok(())
    }

    /// Copy the state each menu starts from to `path`.
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        std::fs::copy(&self.state, path)
            .with_context(|| format!("cannot copy the crawl state to {}", path.display()))?;
        Ok(())
    }

    /// Show page `page` (1-based) of menu `n` on a cleared command line.
    fn show(&mut self, n: u32, page: u32) -> Result<()> {
        self.emu.press_keys(CANCEL)?;
        self.type_keys(&format!("{n}.{page:02} MENU\n"))?;
        Ok(())
    }

    /// Fetch [`TEMP`] as ASCII and purge it (server running).
    fn fetch_var(&mut self) -> Result<String> {
        let text = self.emu.receive_object(TEMP, TransferMode::Ascii)?;
        let reply = self.emu.run_command(&format!("'{TEMP}' PURGE"))?;
        if let Some(e) = reply.error {
            bail!("cannot purge {TEMP}: {e}");
        }
        Ok(hptx_core::charset::decode(&text))
    }

    /// Crawl menu `n`; `None` when its first page has no labels.
    pub fn menu(&mut self, n: u32) -> Result<Option<Menu>> {
        self.emu.load_state(&self.state)?;
        let mut screens: Vec<[Label; 6]> = Vec::new();
        for page in 1..=MAX_PAGES {
            self.show(n, page)?;
            let (screen, _) = self.emu.screen_text()?;
            let labels = read_labels(&screen)?;
            let empty = labels.iter().all(|l| l.kind == LabelKind::Empty);
            // A number MENU does not know shows the VAR menu.
            if page == 1 && (empty || labels == self.var_labels) {
                return Ok(None);
            }
            if empty || screens.first() == Some(&labels) {
                break;
            }
            screens.push(labels);
        }
        let mut pages: Vec<Page> = screens
            .iter()
            .map(|labels| Page {
                labels: labels.iter().map(|l| self.font.read(l)).collect(),
                ..Page::default()
            })
            .collect();
        let error = self
            .crawl_keys(n, &screens, &mut pages)
            .err()
            .map(|e| format!("{e:#}"));
        if error.is_some() {
            for page in &mut pages {
                page.words.clear();
                page.submenus.clear();
            }
        }
        Ok(Some(Menu {
            number: n,
            pages,
            error,
        }))
    }

    /// Find the words and submenus behind the keys of `screens`.
    fn crawl_keys(&mut self, n: u32, screens: &[[Label; 6]], pages: &mut [Page]) -> Result<()> {
        let mut runs = Vec::new();
        for (p, labels) in screens.iter().enumerate() {
            let plain: Vec<usize> = (0..6)
                .filter(|&k| labels[k].kind == LabelKind::Plain)
                .collect();
            if !plain.is_empty() {
                runs.push(RunKind::Program {
                    page: p,
                    keys: plain,
                    shifted: false,
                });
            }
            for k in (0..6).filter(|&k| labels[k].kind == LabelKind::Submenu) {
                runs.push(RunKind::Submenu { page: p, key: k });
            }
        }
        // Batches of plain keys first; a batch that navigated or did not
        // compile is pressed again key by key, and a key whose word alone
        // does not compile (IF, FOR) once more with left-shift, which types
        // the whole structure.
        while !runs.is_empty() {
            let results = self.run_each(n, &runs, screens)?;
            let mut again = Vec::new();
            for (run, result) in runs.iter().zip(&results) {
                let stayed = result.stayed.unwrap_or(false);
                match run {
                    RunKind::Program {
                        page,
                        keys,
                        shifted,
                    } => {
                        if stayed && result.program {
                            pages[*page].words.extend(result.words.iter().cloned());
                        } else if keys.len() > 1 {
                            again.extend(keys.iter().map(|&k| RunKind::Program {
                                page: *page,
                                keys: vec![k],
                                shifted: false,
                            }));
                        } else if stayed && !shifted {
                            again.push(RunKind::Program {
                                page: *page,
                                keys: keys.clone(),
                                shifted: true,
                            });
                        }
                    }
                    RunKind::Submenu { page, key } => {
                        let label = pages[*page].labels[*key].clone();
                        pages[*page].submenus.push(Submenu {
                            key: *key,
                            label,
                            target: result.menu.map(menu_of).map(|(m, _)| m).filter(|&m| m != n),
                        });
                    }
                }
            }
            runs = again;
        }
        for page in pages.iter_mut() {
            page.submenus.sort_by_key(|s| s.key);
        }
        Ok(())
    }

    /// The words each of `scripts` (keyboard keys, maybe shifted) types in
    /// program entry mode, in order.
    pub fn keyboard(&mut self, scripts: &[String]) -> Result<Vec<Vec<String>>> {
        self.emu.load_state(&self.state)?;
        self.escape()?;
        self.type_keys("CLEAR\n")?;
        let prog = program_keys(self.emu.model());
        for (i, script) in scripts.iter().enumerate() {
            self.escape()?;
            self.type_keys(&format!("{}\n", MARKER_BASE + i as u64))?;
            self.press_changing(prog)?;
            self.press_changing(script)?;
            self.press_changing("enter")?;
        }
        self.escape()?;
        self.emu.start_server()?;
        let stored = self
            .emu
            .run_command(&format!("DEPTH \u{2192}LIST '{TEMP}' STO"))?;
        let fetched = self.fetch_var();
        self.emu.stop_server()?;
        if let Some(e) = stored.error {
            bail!("cannot store the stack: {e}");
        }
        Ok(split_runs(&list_items(&fetched?)?, scripts.len())
            .into_iter()
            .map(|r| r.words)
            .collect())
    }

    /// The LCD below the status area (where the TIME menus show a clock).
    fn body(&self) -> Result<String> {
        let (screen, _) = self.emu.screen_text()?;
        Ok(screen.lines().skip(STATUS_ROWS).collect())
    }

    /// Press `keys`; if the screen below the status area did not change,
    /// press them again (the 48SX loses keys while the TIME menus' clock
    /// ticks).
    fn press_changing(&mut self, keys: &str) -> Result<()> {
        let before = self.body()?;
        for _ in 0..PRESS_TRIES {
            self.emu.press_keys(keys)?;
            if self.body()? != before {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Cancel the command line and show the VAR menu (no clock there), as
    /// often as it takes.
    fn escape(&mut self) -> Result<()> {
        for _ in 0..PRESS_TRIES {
            self.emu.press_keys(CANCEL)?;
            self.emu.press_keys("var")?;
            let (screen, _) = self.emu.screen_text()?;
            if read_labels(&screen)? == self.var_labels {
                return Ok(());
            }
        }
        bail!("cannot get back to the VAR menu")
    }

    /// [`Self::run_all`]; if that fails (keys lost while a clock is
    /// shown), each run alone, where one that still fails gives nothing.
    fn run_each(
        &mut self,
        n: u32,
        runs: &[RunKind],
        screens: &[[Label; 6]],
    ) -> Result<Vec<RunResult>> {
        if let Ok(results) = self.run_all(n, runs, screens) {
            return Ok(results);
        }
        let mut out = Vec::with_capacity(runs.len());
        let mut failed = 0;
        for run in runs {
            match self.run_all(n, std::slice::from_ref(run), screens) {
                Ok(mut r) => out.push(r.pop().unwrap_or_default()),
                Err(_) => {
                    failed += 1;
                    out.push(RunResult::default());
                }
            }
        }
        if failed == runs.len() {
            bail!("cannot get back to the VAR menu after any key of menu {n}");
        }
        Ok(out)
    }

    /// After key `k` was pressed on a page that showed `before`: if the
    /// labels still read the same apart from their ■ marks, the key was a
    /// setting that acted at once (CLK in the modes menu, IR/W in the I/O
    /// setup, XYZ among the coordinate modes), so press keys until the
    /// labels are back: a changed key that was marked before (a choice
    /// among several), else the changed key itself (an on/off or a cycle).
    /// Returns false when the menu changed (the key navigated).
    fn untoggle(&mut self, k: usize, before: &[Label; 6]) -> Result<bool> {
        let unmarked = |labels: &[Label; 6], font: &Font| -> Vec<String> {
            labels
                .iter()
                .map(|l| font.read(l).replace('■', ""))
                .collect()
        };
        let texts: Vec<String> = before.iter().map(|l| self.font.read(l)).collect();
        for _ in 0..MAX_TOGGLE_STATES {
            let (screen, _) = self.emu.screen_text()?;
            let now = read_labels(&screen)?;
            if now == *before {
                return Ok(true);
            }
            if unmarked(&now, self.font) != unmarked(before, self.font) {
                return Ok(false);
            }
            let changed: Vec<usize> = (0..6).filter(|&i| now[i] != before[i]).collect();
            let press = changed
                .iter()
                .copied()
                .find(|&i| texts[i].contains('■'))
                .unwrap_or(k);
            self.emu.press_keys(SOFTKEYS[press])?;
        }
        Ok(false)
    }

    /// Do `runs` on menu `n`, whose pages showed `screens`, then fetch
    /// their results.
    fn run_all(
        &mut self,
        n: u32,
        runs: &[RunKind],
        screens: &[[Label; 6]],
    ) -> Result<Vec<RunResult>> {
        // From the saved state: the Kermit exchanges before (fetching the
        // last batch) take wall-clock dependent amounts of emulated time,
        // and the keys must not depend on them.
        self.emu.load_state(&self.state)?;
        self.escape()?;
        self.type_keys("CLEAR\n")?;
        let prog = program_keys(self.emu.model());
        let mut stayed = Vec::with_capacity(runs.len());
        for (i, run) in runs.iter().enumerate() {
            // Everything typed happens in the VAR menu; on the menu under
            // test only single keys are pressed, each checked to have
            // changed the screen. A submenu key's target comes from
            // RCLMENU in a program pushed beforehand (EVAL); whether plain
            // keys stayed on their page is read off the labels.
            self.escape()?;
            self.type_keys(&format!("{}\n", MARKER_BASE + i as u64))?;
            if matches!(run, RunKind::Submenu { .. }) {
                self.emu.press_keys(prog)?;
                self.type_keys("RCLMENU\n")?;
            }
            self.type_keys(&format!("{n}.{:02} MENU\n", run.page() + 1))?;
            match run {
                RunKind::Program {
                    page,
                    keys,
                    shifted,
                } => {
                    self.press_changing(prog)?;
                    let shift = if *shifted { "leftshift " } else { "" };
                    for &k in keys {
                        self.press_changing(&format!("{shift}{}", SOFTKEYS[k]))?;
                        if !self.untoggle(k, &screens[*page])? {
                            break;
                        }
                    }
                    self.press_changing("enter")?;
                    let (screen, _) = self.emu.screen_text()?;
                    stayed.push(Some(read_labels(&screen)? == screens[*page]));
                }
                RunKind::Submenu { key, .. } => {
                    self.press_changing(SOFTKEYS[*key])?;
                    self.press_changing(eval_keys(self.emu.model()))?;
                    stayed.push(None);
                }
            }
        }
        self.escape()?;
        self.emu.start_server()?;
        let stored = self
            .emu
            .run_command(&format!("DEPTH \u{2192}LIST '{TEMP}' STO"))?;
        let fetched = self.fetch_var();
        self.emu.stop_server()?;
        if let Some(e) = stored.error {
            bail!("cannot store the stack: {e}");
        }
        let mut results = split_runs(&list_items(&fetched?)?, runs.len());
        for (r, s) in results.iter_mut().zip(stayed) {
            r.stayed = s;
        }
        Ok(results)
    }
}

/// The results of one run.
#[derive(Clone, Debug, Default)]
struct RunResult {
    /// Whether the run pushed a program.
    program: bool,
    /// For plain keys: whether the page was still shown afterwards.
    stayed: Option<bool>,
    words: Vec<String>,
    menu: Option<f64>,
}

/// Split the fetched stack at the run markers: a program pushed by the run
/// gives its words, the last number its menu.
fn split_runs(items: &[Item], count: usize) -> Vec<RunResult> {
    let mut out = vec![RunResult::default(); count];
    let mut current: Option<usize> = None;
    for item in items {
        match item {
            Item::Number(x)
                if *x >= MARKER_BASE as f64 && *x < (MARKER_BASE + count as u64) as f64 =>
            {
                current = Some((*x - MARKER_BASE as f64) as usize);
            }
            Item::Number(x) => {
                if let Some(c) = current {
                    out[c].menu = Some(*x);
                }
            }
            Item::Program(words) => {
                if let Some(c) = current {
                    out[c].program = true;
                    out[c].words.extend(words.iter().cloned());
                }
            }
            Item::Other => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(kind: LabelKind, cols: &[u8]) -> Label {
        let mut columns = [0u8; LABEL_W];
        columns[3..3 + cols.len()].copy_from_slice(cols);
        Label { kind, columns }
    }

    #[test]
    fn the_font_reads_widest_glyphs_first() {
        let font = Font::from_labels([
            ('I', label(LabelKind::Plain, &[0x1F])),
            ('H', label(LabelKind::Plain, &[0x1F, 0x04, 0x1F])),
            ('l', label(LabelKind::Plain, &[0x1F])),
        ]);
        assert_eq!(font.len(), 3);
        let l = label(LabelKind::Plain, &[0x1F, 0x04, 0x1F, 0, 0x1F, 0, 0x0A]);
        assert_eq!(font.read(&l), "HI?");
    }

    #[test]
    fn stack_items_and_runs() {
        let text = "%%HP: T(1)A(D)F(.);\n{ 100000. « SIN \"A B\" COS » 3.01 100001. 4.01 }";
        let items = list_items(text).unwrap();
        assert_eq!(
            items,
            [
                Item::Number(100000.0),
                Item::Program(vec!["SIN".into(), "\"A B\"".into(), "COS".into()]),
                Item::Number(3.01),
                Item::Number(100001.0),
                Item::Number(4.01)
            ]
        );
        let runs = split_runs(&items, 2);
        assert_eq!(runs[0].words, ["SIN", "\"A B\"", "COS"]);
        assert_eq!(runs[0].menu, Some(3.01));
        assert_eq!(runs[1].menu, Some(4.01));
        assert_eq!(menu_of(4.01), (4, 1));
        assert_eq!(menu_of(12.03), (12, 3));
    }
}
