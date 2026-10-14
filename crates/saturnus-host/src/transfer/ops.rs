//! What a write does and the plans' pieces: names, directories, the text
//! and copy checks.

use super::*;

/// What a write does (`web/protocol.md`, "The user memory, written").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Store `data` (an HP binary file, or text) as `name` in `dir`.
    /// `replace`: `None` leaves a name in use to the calculator (flag -36
    /// clear: it stores `NAME.1`); `Some(false)` refuses a name in use;
    /// `Some(true)` replaces a variable of that name: the file goes in
    /// under a free temporary name first ([`REPLACE_VARIABLE`]), so a
    /// failed transfer leaves the old one, then the old one is purged and
    /// the new one stored under its name. A directory of that name is
    /// never replaced.
    Store {
        dir: Vec<String>,
        name: String,
        data: Vec<u8>,
        replace: Option<bool>,
    },
    /// Fetch variable `name` of `dir`: as an HP binary file, or with
    /// `text` in the calculator's text form (`%%HP:`, ASCII mode: flag
    /// -35 cleared and put back), as the calculator sends it.
    Fetch {
        dir: Vec<String>,
        name: String,
        text: bool,
    },
    /// Purge variable `name` of `dir` (a directory with all it holds).
    Purge { dir: Vec<String>, name: String },
    /// Rename variable `name` of `dir` to `to`.
    Rename {
        dir: Vec<String>,
        name: String,
        to: String,
    },
    /// Create the empty directory `name` in `dir` (`CRDIR`).
    CreateDir { dir: Vec<String>, name: String },
    /// Make `dir` the current directory.
    ChangeDir { dir: Vec<String> },
    /// Copy variable `name` of `dir` (a directory with all it holds) into
    /// directory `to`, replacing a variable of that name there only when
    /// `replace`; `remove` (a move) then purges the original, once the
    /// copy is checked to be the same object.
    Copy {
        dir: Vec<String>,
        name: String,
        to: Vec<String>,
        replace: bool,
        remove: bool,
    },
    /// Set (`on`) or clear flag `flag` (negative: a system flag).
    SetFlag { flag: i32, on: bool },
    /// Set the clock by typing `→TIME` and `→DATE` (no server; see
    /// [`crate::clock`]): the date in the order flag -42 asks for, a
    /// comma for the point under flag -51, and in the 49G's algebraic
    /// mode one `→TIME(…)` and one `→DATE(…)`, each echo dropped.
    /// The time and date are keyed apart: in algebraic mode they are
    /// taken a moment apart, which matters at midnight
    /// ([`crate::Emulator::clock_write`]).
    SetClock {
        time: crate::clock::ClockTime,
        date: crate::clock::ClockTime,
    },
    /// Compile `text` on the calculator and put the object at `target`
    /// (`dir` is the variable's directory; a stack level is in the current
    /// one).
    StoreText {
        dir: Vec<String>,
        target: Target,
        text: String,
    },
    /// Store `screen` as a GROB: as variable `name` in `dir` (replacing a
    /// variable of that name only when `replace`), or with no name on
    /// stack level 1 (`dir` is then not used). It travels under a
    /// temporary name ([`SCREEN_VARIABLE`]), recalled and purged on the
    /// calculator, so the user memory keeps nothing else.
    StoreScreen {
        dir: Vec<String>,
        name: Option<String>,
        replace: bool,
        screen: Graphic,
    },
    /// Ask whether the Kermit server runs (a calculator left in `SERVER`,
    /// say by a saved state): an I packet, which a server answers at once;
    /// nothing is typed and no key is pressed. With `end`, a server that
    /// answers is ended with `G F` and the wait for the stack, as the
    /// writes end theirs. [`TransferResult::server`] says whether it ran.
    Server { end: bool },
    /// Back up HOME: `ARCHIVE` into port 0, recalled into a temporary
    /// variable in HOME ([`BACKUP_VARIABLE`]; port 0 and the variable
    /// purged again, also after an error), fetched as a binary file of
    /// the directory object. First the free memory (`MEM`): refused,
    /// before anything is archived, unless it holds twice HOME and
    /// [`BACKUP_MARGIN`] (what `ARCHIVE` and the recalled copy take).
    Backup,
}

/// Where `storeText` puts its object, and what `editText` reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// The variable of this name.
    Variable(String),
    /// This stack level (1 is the top).
    Level(usize),
}

/// The name of the string variable `storeText` sends its text in (a
/// number is added when the directory holds one already).
pub const TEXT_VARIABLE: &str = "SATEDIT";
/// The name `storeScreen` sends its GROB under (a number is added when
/// the directory holds one already).
pub const SCREEN_VARIABLE: &str = "SATSCRN";
/// The name a replacing `storeFile` sends its file under (a number is
/// added when the directory holds one already).
pub const REPLACE_VARIABLE: &str = "SATREPL";
/// The name a backup's archive has in port 0 and in HOME (a number is
/// added when HOME holds one already).
pub const BACKUP_VARIABLE: &str = "SATBKUP";
/// Bytes a backup needs free on top of two copies of HOME: `ARCHIVE`
/// takes HOME's size and about 80 bytes, the recalled copy as much again
/// (measured on the emulated 48SX, 48GX and 49G; saturnus-tx's
/// `BACKUP_MARGIN`, `crates/saturnus-tx/src/device.rs`).
pub const BACKUP_MARGIN: f64 = 1024.0;
/// The start of a backup's refusal for want of memory: "a backup needs N
/// bytes free and the calculator has M".
pub const BACKUP_NO_ROOM: &str = "a backup needs ";

/// Whether a Kermit server may run on `m`, for [`Op::Server`]: a model
/// with one, its port on (the server opens it; it stays on after the
/// server until the calculator is turned off) and no command line open.
/// Otherwise no packet need ask.
pub fn may_serve(m: &Machine) -> bool {
    no_server(m.model()).is_none()
        && m.serial_on()
        && !cmdline::command_line(m).is_ok_and(|l| l.active)
}

/// Why `model`'s user memory cannot be written, or `None`.
pub(super) fn no_server(model: Model) -> Option<String> {
    (!matches!(model, Model::Hp48sx | Model::Hp48gx | Model::Hp49g)).then(|| {
        format!(
            "the {} has no Kermit server and no RPL user memory to write",
            model.name().to_uppercase()
        )
    })
}

/// `name` as a quoted global name for a host command.
pub(super) fn quoted(name: &str) -> String {
    format!("'{name}'")
}

pub(super) fn check_name(name: &str) -> crate::Result<()> {
    if is_plain_name(name) {
        Ok(())
    } else {
        Err(format!(
            "{name:?} is not a plain variable name (1 to 127 characters, no digit or point first, \
             no spaces, delimiters or operators)"
        )
        .into())
    }
}

/// `dir` without its leading `HOME`, every component a plain name.
pub(super) fn components(dir: &[String]) -> crate::Result<&[String]> {
    let rest = match dir.split_first() {
        Some((home, rest)) if home == "HOME" => rest,
        _ => dir,
    };
    for name in rest {
        check_name(name)?;
    }
    Ok(rest)
}

/// The host command that makes `dir` current (`HOME A B`).
pub(super) fn cd_command(dir: &[String]) -> String {
    std::iter::once("HOME")
        .chain(dir.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The variables of `dir` (components below HOME), or why not.
pub(super) fn directory<'a>(tree: &'a [Variable], dir: &[String]) -> crate::Result<&'a [Variable]> {
    let mut vars = tree;
    let mut path = String::from("HOME");
    for name in dir {
        path.push(' ');
        path.push_str(name);
        vars = vars
            .iter()
            .find(|v| &v.name == name)
            .and_then(|v| v.variables.as_deref())
            .ok_or_else(|| format!("no directory {{ {path} }}"))?;
    }
    Ok(vars)
}

/// Whether `path` (components below HOME) lies inside `dir`/`name`.
pub(super) fn inside(path: &[String], dir: &[String], name: &str) -> bool {
    path.len() > dir.len() && path.starts_with(dir) && path[dir.len()] == name
}

/// The flag's range on `model`: -64..64 on the 48s, -128..128 on the 49G.
pub(super) fn flag_range(model: Model) -> i32 {
    if model == Model::Hp49g { 128 } else { 64 }
}

/// What the file is stored as: binary files as they are; text, when it is
/// UTF-8 in the calculator's character set, in that set (a PC's `«` is two
/// UTF-8 bytes, the calculator's one), otherwise as it is. Returns the
/// bytes and whether they travel in binary mode (flag -35 set).
pub fn store_bytes(data: &[u8]) -> (Vec<u8>, bool) {
    if tfile::is_binary_file(data) {
        return (data.to_vec(), true);
    }
    let encoded = std::str::from_utf8(data)
        .ok()
        .and_then(|t| charset::encode(t).ok());
    (encoded.unwrap_or_else(|| data.to_vec()), false)
}

/// A received binary file cut to its object's length: the calculator pads
/// the last packet.
pub fn trim_fetched(mut data: Vec<u8>) -> Vec<u8> {
    if tfile::is_binary_file(&data) {
        let nibbles = tfile::unpack(&data[tfile::HEADER_LEN..]);
        if let Ok(size) = prolog::object_size(&nibbles, 0) {
            data.truncate(tfile::HEADER_LEN + size.div_ceil(2));
        }
    }
    data
}

/// Why `text` cannot be compiled inside the list `storeText` wraps it in,
/// or `None`. The wrapper must end where the calculator reads its end, so
/// what could move that end is refused before anything runs:
///
/// - a `}` that closes more than the text opened (the list would end early
///   and what follows would run);
/// - a string left open (it would swallow the wrapper's `}`);
/// - a `"` or `@` right after a word's character: the ROM starts a string
///   or a comment there, inside the word (`X@ 1` is `X` and a comment,
///   `A"B"` is `A` and `"B"`; wiki: protocols/server-commands), which is
///   easy to misread, so it is refused rather than followed.
///
/// Strings (`"` to `"`) and comments (`@` to the next `@` or the line's
/// end) are skipped as the ROM reads them.
pub(super) fn text_refusal(text: &str) -> Option<String> {
    let mut depth = 0usize;
    let mut string = false;
    let mut comment = false;
    let mut prev: Option<char> = None;
    for c in text.chars() {
        let mid_word = prev.is_some_and(|p| {
            !p.is_whitespace()
                && !matches!(
                    p,
                    '{' | '}' | '[' | ']' | '(' | ')' | '«' | '»' | '\'' | '"'
                )
        });
        match c {
            '"' | '@' if !string && !comment && mid_word => {
                return Some(format!(
                    "a {c} right after a word starts a new token on the calculator: put a space before it"
                ));
            }
            '"' if !comment => string = !string,
            '@' if !string => comment = !comment,
            '\n' if comment => comment = false,
            '{' if !string && !comment => depth += 1,
            '}' if !string && !comment => match depth.checked_sub(1) {
                Some(d) => depth = d,
                None => return Some("a } closes a list the text did not open".into()),
            },
            _ => {}
        }
        prev = Some(c);
    }
    string.then(|| "the text leaves a string open (a \" is missing)".into())
}

/// What `copy` and `move` must know of a variable and its target.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CopyCase {
    /// The variable is a directory.
    pub(super) is_dir: bool,
    /// The target holds a variable of that name: `Some(true)` a directory.
    pub(super) taken: Option<bool>,
    /// The user said to replace it.
    pub(super) replace: bool,
    /// A move: the original is purged after the copy.
    pub(super) remove: bool,
    /// The variable is a directory that holds the current one.
    pub(super) holds_here: bool,
}

/// The host commands of `copy` (and `move`) of `name` from `from` into
/// `into` (components below HOME), or why not. The copy is the
/// calculator's `RCL` and `STO`; a move then compares the two objects'
/// `BYTES` (size and checksum) and purges the original only when they
/// agree, else fails with the original kept.
pub(super) fn copy_plan(
    from: &[String],
    name: &str,
    into: &[String],
    c: &CopyCase,
) -> crate::Result<Vec<Step>> {
    let src = cd_command(from);
    let dst = cd_command(into);
    if from == into {
        return Err(format!("{name} is in {{ {dst} }} already").into());
    }
    if c.is_dir && inside(into, from, name) {
        return Err(format!("{name} cannot go into itself or a directory inside it").into());
    }
    if c.remove && c.holds_here {
        return Err(
            format!("{name} holds the current directory: change to another one first").into(),
        );
    }
    match c.taken {
        Some(_) if !c.replace => {
            return Err(format!("{name} already exists in {{ {dst} }}").into());
        }
        Some(true) => {
            return Err(format!(
                "{name} in {{ {dst} }} is a directory, which a copy does not replace: purge it first"
            )
            .into());
        }
        Some(false) if c.is_dir => {
            return Err(format!(
                "a directory does not replace the variable {name} in {{ {dst} }}: purge it first"
            )
            .into());
        }
        _ => {}
    }
    // One command per packet: the levels a failed one leaves are dropped
    // (the baseline), so the steps may pass objects on the stack.
    let q = quoted(name);
    let mut texts = vec![
        src.clone(),
        format!("{q} RCL"),
        dst.clone(),
        format!("{q} STO"),
    ];
    if c.remove {
        let verb = if c.is_dir { "PGDIR" } else { "PURGE" };
        texts.extend([
            format!("{q} RCL BYTES"),
            src,
            format!("{q} RCL BYTES"),
            "ROT == 3 ROLLD == AND".into(),
            format!("« {q} {verb} » « \"{COPY_DIFFERS}\" DOERR » IFTE"),
        ]);
    }
    Ok(texts
        .into_iter()
        .map(|text| Step::Host {
            text,
            cleanup: false,
        })
        .collect())
}

/// The calculator's error when a move's copy is not the same object: the
/// original is not purged.
pub(super) const COPY_DIFFERS: &str = "Copy differs";

/// The string `storeText` sends: the text in a list, which compiles
/// without running anything in it.
pub(super) fn wrapped(text: &str) -> String {
    format!("{{\n{text}\n}}")
}

/// `base`, or with a number added, whichever `vars` lacks.
pub(super) fn free_name(vars: &[Variable], base: &str) -> String {
    (1..)
        .map(|n| match n {
            1 => base.to_string(),
            n => format!("{base}{n}"),
        })
        .find(|name| vars.iter().all(|v| &v.name != name))
        .unwrap_or_default()
}

/// A number as a stack level shows it, in any display mode: `12345.`,
/// `12345`, `1.2345E4`, `12345,00` (a comma for the point under flag
/// -51), with a sign; `None` for anything else.
pub(super) fn parse_number(level: &str) -> Option<f64> {
    let t = level.trim().replace(',', ".");
    let t = t.strip_suffix('.').unwrap_or(&t);
    t.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Whether `data` is a binary file of a directory object (a stored file
/// that `PGDIR`, not `PURGE`, removes).
pub(super) fn holds_directory(data: &[u8]) -> bool {
    if !tfile::is_binary_file(data) {
        return false;
    }
    let head = &data[tfile::HEADER_LEN..data.len().min(tfile::HEADER_LEN + 3)];
    prolog::read_field(&tfile::unpack(head), 0, 5) == Some(ObjectType::Directory.prolog())
}

/// The count `storeText`'s compile replies with, 0, 1 or 2 (two or more),
/// read in any display mode (`1`, `1.`, `1.000`, `1,000`).
pub(super) fn reply_count(level: &str) -> Option<u32> {
    let first = level.trim().chars().next()?;
    first.to_digit(10).filter(|&n| n <= 2)
}

/// The object at `address` as `size:checksum` (nibbles, hex CRC).
pub(super) fn identity(u: &UserMemory<'_>, address: u32) -> crate::Result<String> {
    let (size, crc) = u.identity_at(address)?;
    Ok(format!("{size}:{crc:04X}"))
}

/// Whether the 49G is in algebraic mode (system flag -95).
pub(super) fn algebraic(model: Model, flags: &Flags) -> bool {
    model == Model::Hp49g && flags.get(-95) == Some(true)
}

/// Whether the calculator's flag -95 reads clear now (RPN mode).
pub(super) fn rpn_now(m: &Machine) -> bool {
    UserMemory::of(m)
        .and_then(|u| u.flags())
        .is_ok_and(|f| f.get(-95) == Some(false))
}
