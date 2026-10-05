//! The semantic tools' work on the calculator: `eval`, the typed stack and
//! variables, all through the ROM's Kermit server, which these functions
//! enter and leave on demand ([`Emulator::semantic`]).
//!
//! Exact values come from binary GETs: the levels to return are copied
//! into a temporary list variable (`n DUPN n →LIST`), fetched, decoded
//! ([`crate::object`]) and purged; the stack is never disturbed. Programs,
//! algebraics and units take their text from an ASCII GET of the same
//! variable. Objects go to the calculator as RPL text in a host command
//! when they have one that fits a packet, else as a binary SEND (or a
//! string compiled with `STR→`) into a temporary variable that is purged.
//! wiki: protocols/server-commands, protocols/hp-object-format.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use hptx_core::calc::validate_name;
use hptx_core::object::Family;
use hptx_core::reply::{Listing, StackReply};
use hptx_core::{Calculator, TransferMode};
use saturnus::{Machine, Model};
use saturnus_drive::script::{Action, Line};
use serde::Serialize;

use crate::emulator::Emulator;
use crate::link::MAX_BUSY;
use crate::object::{Base, Memory, Object, decode_file, encode_file, fill_sources, to_source};

/// Default emulated-time limit of `eval`.
pub const DEFAULT_EVAL_TIMEOUT: Duration = Duration::from_secs(60);
/// Emulated-time limit of the other semantic tools' host commands.
const OP_TIMEOUT: Duration = Duration::from_secs(60);
/// Temporary variable names, tried in order until one is free.
const TEMP_NAMES: [&str; 4] = ["SATRNTMP", "SATRNTM1", "SATRNTM2", "SATRNTM3"];
/// Encoded bytes of the longest host command that fits one Kermit packet
/// (hptx-core refuses longer ones before sending).
const MAX_COMMAND_BYTES: usize = 77;

/// A calculator error returned by a semantic tool: the message after
/// `Error:` and the stack afterwards.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CalcError {
    /// E.g. `Infinite Result`.
    pub error: String,
    /// Stack depth afterwards.
    pub depth: usize,
    /// Display text of the levels afterwards, level 1 first.
    pub display: Vec<String>,
}

/// Typed stack levels.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Levels {
    /// The whole stack's depth.
    pub depth: usize,
    /// The requested levels, level 1 first.
    pub levels: Vec<Object>,
    /// The display text of the same levels, level 1 first.
    pub display: Vec<String>,
}

/// A variable in [`Vars`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Var {
    /// Name.
    pub name: String,
    /// The calculator's type name, e.g. `Real Number`, `Directory`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Size in bytes.
    pub size: f64,
    /// The calculator's checksum.
    pub checksum: u16,
}

/// The current directory and its variables.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Vars {
    /// The current directory, e.g. `["HOME", "D1"]`.
    pub path: Vec<String>,
    /// Its variables in calculator order.
    pub variables: Vec<Var>,
}

/// The emulated ROM and RAM as seen by the CPU, for ROM pointers.
struct MachineMemory<'a>(&'a Machine);

impl Memory for MachineMemory<'_> {
    fn nibble(&self, addr: u32) -> Option<u8> {
        (addr < 0x10_0000).then(|| self.0.peek(addr))
    }
}

/// Whether `model` has a Kermit server.
pub fn has_server(model: Model) -> bool {
    matches!(model, Model::Hp48sx | Model::Hp48gx | Model::Hp49g)
}

fn family(model: Model) -> Family {
    if model == Model::Hp49g {
        Family::Hp49
    } else {
        Family::Hp48
    }
}

/// `n` as an RPL real (`3.`), which the 49G does not turn into an exact
/// integer.
fn count(n: usize) -> String {
    format!("{n}.")
}

/// Turn a reply carrying an error into one, for internal commands.
fn checked(what: &str, reply: StackReply) -> Result<StackReply> {
    match reply.error {
        Some(e) => bail!("{what}: calculator error: {e}"),
        None => Ok(reply),
    }
}

/// `'NAME'` after hptx's name check.
fn quoted(name: &str) -> Result<String> {
    validate_name(name).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(format!("'{name}'"))
}

impl Emulator {
    /// Run `f` with the Kermit server up: start it if needed (pressing ON
    /// first to clear a command line or a form), and stop it afterwards
    /// unless `keep_server`, so the screen shows the stack again. On the
    /// 38G, 39G, 40G and 42S, an error.
    pub fn semantic<T>(
        &mut self,
        keep_server: bool,
        f: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        if !has_server(self.model()) {
            bail!(
                "no Kermit server on this model: the {} has none, so eval, the typed stack and \
                 the variable tools work on the 48SX, 48GX and 49G only (use press_keys, \
                 type_text and screen)",
                self.model().name().to_uppercase()
            );
        }
        if !self.server_running() {
            self.enter_server()?;
        }
        let result = f(self);
        if !keep_server && self.server_running() {
            let left = self.stop_server();
            if let (Ok(_), Err(e)) = (&result, left) {
                return Err(e.context("the tool worked, but leaving Kermit server mode failed"));
            }
        }
        result
    }

    /// Press ON (clears a command line or a form), then type SERVER.
    fn enter_server(&mut self) -> Result<()> {
        self.run_lines(&[Line {
            number: 1,
            action: Action::Press {
                key: saturnus::io::Key::On,
                hold_ms: saturnus_drive::script::DEFAULT_HOLD_MS,
            },
        }])?;
        self.start_server()
            .context("cannot enter Kermit server mode")?;
        Ok(())
    }

    fn calc(&mut self) -> Result<&mut Calculator> {
        self.kermit()
    }

    /// A host command bounded by `limit` of emulated time. On the limit
    /// the calculator is interrupted with ON, which ends server mode.
    fn host(&mut self, command: &str, limit: Duration) -> Result<StackReply> {
        self.host_eval(command, limit, &[command])
    }

    /// [`Self::host`] for an evaluation: after an interrupt, the server is
    /// entered again and a string on level 1 that equals one of
    /// `leftovers` is dropped (the 48SX ROM puts the evaluated text back
    /// as a string when ON stops it).
    fn host_eval(
        &mut self,
        command: &str,
        limit: Duration,
        leftovers: &[&str],
    ) -> Result<StackReply> {
        self.core()?.set_read_cap(Some(limit));
        let result = self.calc()?.run(command);
        let hit = {
            let mut core = self.core()?;
            core.set_read_cap(None);
            core.take_read_cap_hit()
        };
        match result {
            Ok(reply) => Ok(reply),
            Err(_) if hit => {
                self.interrupt()?;
                let cleanup = self.drop_leftover(leftovers);
                let what = match cleanup {
                    Ok(true) => "the evaluated text, which the ROM had put back on level 1 as a \
                                 string, was dropped; anything else the evaluation pushed stays"
                        .to_string(),
                    Ok(false) => {
                        "whatever the evaluation had pushed stays on the stack".to_string()
                    }
                    Err(e) => format!(
                        "server mode could not be entered again to look at the stack ({e:#}); \
                         the evaluated text may be on level 1 as a string"
                    ),
                };
                bail!(
                    "no result within {} ms of emulated time: the calculator was interrupted \
                     with ON, which also ends its Kermit server; {what} (look with stack). On \
                     the 49G integer literals evaluate exactly or symbolically and can take \
                     minutes: write reals with a dot (2. instead of 2), or raise timeout_ms",
                    limit.as_millis()
                )
            }
            Err(e) => Err(anyhow::anyhow!("{e}")).context("Kermit host command failed"),
        }
    }

    /// After an interrupt: enter server mode again and drop level 1 if it
    /// shows one of `leftovers` as a string. Returns whether it did.
    fn drop_leftover(&mut self, leftovers: &[&str]) -> Result<bool> {
        self.enter_server()?;
        let reply = checked("reading the stack", self.host("", OP_TIMEOUT)?)?;
        // The display shows HP characters (Σ) where the source may have
        // trigraphs (\GS): compare in the calculator's characters.
        let ours = reply.level(1).is_some_and(|l| {
            leftovers
                .iter()
                .any(|t| shows_string(l, as_calculator_text(t).trim()))
        });
        if ours {
            checked("DROP", self.host("DROP", OP_TIMEOUT)?)?;
        }
        Ok(ours)
    }

    /// Press ON to stop a busy calculator; the server counts as stopped.
    fn interrupt(&mut self) -> Result<()> {
        self.forget_server();
        self.core()?.discard_input();
        self.run_lines(&[Line {
            number: 1,
            action: Action::Press {
                key: saturnus::io::Key::On,
                hold_ms: saturnus_drive::script::DEFAULT_HOLD_MS,
            },
        }])?;
        Ok(())
    }

    /// A name for a temporary variable that is free in the current
    /// directory.
    fn temp_name(&mut self) -> Result<&'static str> {
        let listing = self.calc()?.list().map_err(|e| anyhow::anyhow!("{e}"))?;
        TEMP_NAMES
            .into_iter()
            .find(|n| !listing.entries.iter().any(|e| e.name == *n))
            .context("all temporary variable names are taken in this directory")
    }

    /// Fetch variable `name` and decode it, with the sources of programs
    /// and the like from an ASCII GET.
    fn fetch(&mut self, name: &str) -> Result<Object> {
        let data = self
            .calc()?
            .get(name, TransferMode::Binary)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .with_context(|| format!("cannot fetch {name}"))?;
        let mut obj = {
            let core = self.core()?;
            decode_file(&data, &MachineMemory(&core.session.machine))?
        };
        if obj.needs_source() {
            let text = self
                .calc()?
                .get(name, TransferMode::Ascii)
                .map_err(|e| anyhow::anyhow!("{e}"))
                .with_context(|| format!("cannot fetch {name} as text"))?;
            fill_sources(&mut obj, &hptx_core::charset::decode(&text));
        }
        Ok(obj)
    }

    /// The binary integer display base (flags -11, -12): push `#0`, read
    /// its suffix, drop it.
    fn base(&mut self) -> Result<Option<Base>> {
        let reply = checked("#0", self.host("#0", OP_TIMEOUT)?)?;
        let base = reply.level(1).and_then(Base::from_display);
        checked("DROP", self.host("DROP", OP_TIMEOUT)?)?;
        Ok(base)
    }

    /// Levels 1..=n as typed objects (level 1 first), without changing the
    /// stack. `display` is the stack's display text, level 1 first.
    fn typed_levels(&mut self, n: usize, display: &[String]) -> Result<Vec<Object>> {
        if n == 0 {
            return Ok(Vec::new());
        }
        let tmp = self.temp_name()?;
        let copy = format!("{} DUPN {} \u{2192}LIST '{tmp}' STO", count(n), count(n));
        checked("copying the stack", self.host(&copy, OP_TIMEOUT)?)?;
        let fetched = self.fetch(tmp);
        let purged = self.host(&format!("'{tmp}' PURGE"), OP_TIMEOUT);
        let list = fetched?;
        checked("purging the temporary copy", purged?)?;
        let Object::List { mut items } = list else {
            bail!("the stack copy is not a list: {list:?}");
        };
        items.reverse();
        let mut base = None;
        for (i, item) in items.iter_mut().enumerate() {
            if !item.has_binary() {
                continue;
            }
            let b = match display.get(i).and_then(|d| Base::from_display(d)) {
                Some(b) => Some(b),
                None => match base {
                    Some(b) => Some(b),
                    None => self.base()?,
                },
            };
            if let Some(b) = b {
                base = Some(b);
                item.set_base(b);
            }
        }
        Ok(items)
    }

    /// Evaluate RPL `source` and return levels 1..=`levels` typed, or the
    /// calculator's error. `source` that the calculator rejects as `Invalid
    /// Syntax` is tried again as an algebraic (`'SIN(0.5)' EVAL`) when it
    /// holds no quotes. Source too long for one packet travels as a string
    /// that `STR→` compiles. `limit` bounds the emulated time.
    pub fn eval(
        &mut self,
        source: &str,
        levels: usize,
        limit: Duration,
    ) -> Result<std::result::Result<Levels, CalcError>> {
        if source.trim().is_empty() {
            bail!("source is empty");
        }
        let limit = limit.min(MAX_BUSY);
        let mut reply = self.run_source(source, limit)?;
        if reply.error.as_deref() == Some("Invalid Syntax") && self.left_as_string(source, &reply) {
            let algebraic = format!("'{}' EVAL", source.trim());
            if !source.contains(['\'', '"']) && command_fits(&algebraic) {
                checked("DROP", self.host("DROP", OP_TIMEOUT)?)?;
                reply = self.host_eval(&algebraic, limit, &[&algebraic, source])?;
                if reply.error.as_deref() == Some("Invalid Syntax")
                    && self.left_as_string(&algebraic, &reply)
                {
                    // Not an algebraic either: report the first error.
                    reply = checked("DROP", self.host("DROP", OP_TIMEOUT)?)?;
                    reply.error = Some("Invalid Syntax".into());
                }
            } else {
                // Leave the stack as it was.
                reply = checked("DROP", self.host("DROP", OP_TIMEOUT)?)?;
                reply.error = Some("Invalid Syntax".into());
            }
        }
        if let Some(error) = reply.error {
            return Ok(Err(CalcError {
                error,
                depth: reply.levels.len(),
                display: reply.levels,
            }));
        }
        let depth = reply.levels.len();
        let n = levels.min(depth);
        let objects = self.typed_levels(n, &reply.levels)?;
        let mut display = reply.levels;
        display.truncate(n);
        Ok(Ok(Levels {
            depth,
            levels: objects,
            display,
        }))
    }

    /// Whether a syntax error left `text` on level 1 as a string, as the
    /// calculator does with a command line it cannot parse.
    fn left_as_string(&self, text: &str, reply: &StackReply) -> bool {
        reply.level(1).is_some_and(|l| shows_string(l, text.trim()))
    }

    /// Run `source` as a host command, or as a string compiled by `STR→`
    /// when it does not fit one packet.
    fn run_source(&mut self, source: &str, limit: Duration) -> Result<StackReply> {
        if command_fits(source) {
            return self.host_eval(source, limit, &[source]);
        }
        let data =
            hptx_core::charset::encode_command(source).map_err(|e| anyhow::anyhow!("{e}"))?;
        let tmp = self.put_temp(&data)?;
        let command = format!("{tmp} '{tmp}' PURGE STR\u{2192}");
        self.host_eval(&command, limit, &[source, &command])
    }

    /// Store `data` (a binary object file, or bytes that become a string)
    /// in a free temporary variable and return its name.
    fn put_temp(&mut self, data: &[u8]) -> Result<&'static str> {
        let tmp = self.temp_name()?;
        let stored = self
            .calc()?
            .put(tmp, data, TransferMode::Binary)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .context("cannot send the object")?;
        if stored != tmp {
            // Best effort: the mismatch is the error to report.
            let _ = self.host(&format!("'{stored}' PURGE"), OP_TIMEOUT);
            bail!("the calculator stored the object as {stored}, not {tmp}");
        }
        Ok(tmp)
    }

    /// The stack: levels 1..=`levels` (default all) typed.
    pub fn typed_stack(&mut self, levels: Option<usize>) -> Result<Levels> {
        let reply = checked("reading the stack", self.host("", OP_TIMEOUT)?)?;
        let depth = reply.levels.len();
        let n = levels.unwrap_or(depth).min(depth);
        let objects = self.typed_levels(n, &reply.levels)?;
        let mut display = reply.levels;
        display.truncate(n);
        Ok(Levels {
            depth,
            levels: objects,
            display,
        })
    }

    /// Put `obj` on the stack.
    pub fn push(&mut self, obj: &Object) -> Result<std::result::Result<Levels, CalcError>> {
        let before = self.depth()?;
        let reply = self.place(obj, "")?;
        if let Some(error) = reply.error {
            return Ok(Err(CalcError {
                error,
                depth: reply.levels.len(),
                display: reply.levels,
            }));
        }
        let depth = reply.levels.len();
        check_depth("push", before + 1, depth)?;
        Ok(Ok(Levels {
            depth,
            levels: Vec::new(),
            display: reply.levels.into_iter().take(1).collect(),
        }))
    }

    /// Put `obj` on the stack and run `then` (e.g. `'X' STO`) after it in
    /// the same host command when possible. A syntax error drops the text
    /// the calculator left on the stack.
    fn place(&mut self, obj: &Object, then: &str) -> Result<StackReply> {
        check_sources(obj, true)?;
        let fam = family(self.model());
        let source = to_source(obj, fam)?;
        if let Some(src) = &source {
            let command = format!("{src} {then}");
            if command_fits(&command) {
                let reply = self.host(command.trim(), OP_TIMEOUT)?;
                if reply.error.as_deref() == Some("Invalid Syntax")
                    && self.left_as_string(command.trim(), &reply)
                {
                    let mut r = checked("DROP", self.host("DROP", OP_TIMEOUT)?)?;
                    r.error = Some("Invalid Syntax".into());
                    return Ok(r);
                }
                return Ok(reply);
            }
        }
        if let Some(file) = encode_file(obj, fam)? {
            let tmp = self.put_temp(&file)?;
            return self.host(
                format!("'{tmp}' RCL '{tmp}' PURGE {then}").trim(),
                OP_TIMEOUT,
            );
        }
        let Some(src) = source else {
            bail!("this object can be sent neither as text nor as a binary object");
        };
        let data = hptx_core::charset::encode_command(&src).map_err(|e| anyhow::anyhow!("{e}"))?;
        let tmp = self.put_temp(&data)?;
        self.host(
            format!("{tmp} '{tmp}' PURGE STR\u{2192} {then}").trim(),
            OP_TIMEOUT,
        )
    }

    /// The stack depth.
    fn depth(&mut self) -> Result<usize> {
        Ok(checked("reading the stack", self.host("", OP_TIMEOUT)?)?
            .levels
            .len())
    }

    /// Remove level 1 and return it typed.
    pub fn pop(&mut self) -> Result<Levels> {
        let reply = checked("reading the stack", self.host("", OP_TIMEOUT)?)?;
        if reply.levels.is_empty() {
            bail!("the stack is empty");
        }
        let objects = self.typed_levels(1, &reply.levels)?;
        let after = checked("DROP", self.host("DROP", OP_TIMEOUT)?)?;
        Ok(Levels {
            depth: after.levels.len(),
            levels: objects,
            display: reply.levels.into_iter().take(1).collect(),
        })
    }

    /// Drop `n` levels; an error if there are fewer.
    pub fn drop_levels(&mut self, n: usize) -> Result<std::result::Result<usize, CalcError>> {
        let command = match n {
            0 => bail!("count must be at least 1"),
            1 => "DROP".to_string(),
            _ => format!("{} DROPN", count(n)),
        };
        let reply = self.host(&command, OP_TIMEOUT)?;
        Ok(match reply.error {
            Some(error) => Err(CalcError {
                error,
                depth: reply.levels.len(),
                display: reply.levels,
            }),
            None => Ok(reply.levels.len()),
        })
    }

    /// Empty the stack.
    pub fn clear_stack(&mut self) -> Result<()> {
        checked("CLEAR", self.host("CLEAR", OP_TIMEOUT)?)?;
        Ok(())
    }

    /// Variable `name` of the current directory, typed.
    pub fn get_var(&mut self, name: &str) -> Result<Object> {
        validate_name(name).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut obj = self.fetch(name)?;
        if obj.has_binary()
            && let Some(b) = self.base()?
        {
            obj.set_base(b);
        }
        Ok(obj)
    }

    /// Store `obj` as variable `name` in the current directory (replacing
    /// it; `STO`).
    pub fn set_var(
        &mut self,
        name: &str,
        obj: &Object,
    ) -> Result<std::result::Result<(), CalcError>> {
        let q = quoted(name)?;
        let before = self.depth()?;
        let reply = self.place(obj, &format!("{q} STO"))?;
        if let Some(error) = reply.error {
            // The object may still be on the stack (STO refused it).
            return Ok(Err(CalcError {
                error,
                depth: reply.levels.len(),
                display: reply.levels,
            }));
        }
        check_depth("set_var", before, reply.levels.len())?;
        Ok(Ok(()))
    }

    /// The current directory and its variables.
    pub fn list_vars(&mut self) -> Result<Vars> {
        let calc = self.calc()?;
        let listing: Listing = calc.list().map_err(|e| anyhow::anyhow!("{e}"))?;
        let path = match listing.path.clone() {
            Some(p) => p,
            None => calc.path().map_err(|e| anyhow::anyhow!("{e}"))?,
        };
        Ok(Vars {
            path,
            variables: listing
                .entries
                .into_iter()
                .map(|e| Var {
                    name: e.name,
                    kind: e.kind,
                    size: e.size,
                    checksum: e.checksum,
                })
                .collect(),
        })
    }

    /// Change directory: `HOME/A/B` (absolute), `A/B` (relative), `..`
    /// (parent); returns the new path.
    pub fn cd(&mut self, path: &str) -> Result<Vec<String>> {
        let parts: Vec<&str> = path
            .split(|c: char| c == '/' || c.is_whitespace())
            .filter(|p| !p.is_empty())
            .collect();
        let calc = self.calc()?;
        let mut target = match parts.first() {
            Some(&"HOME") => vec!["HOME".to_string()],
            _ => calc.path().map_err(|e| anyhow::anyhow!("{e}"))?,
        };
        for p in parts
            .iter()
            .skip(usize::from(parts.first() == Some(&"HOME")))
        {
            match *p {
                ".." => {
                    if target.len() > 1 {
                        target.pop();
                    }
                }
                "." => {}
                name => target.push(name.to_string()),
            }
        }
        let refs: Vec<&str> = target.iter().map(String::as_str).collect();
        calc.cd(&refs).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(target)
    }
}

/// Whether `display` shows the string `text`: `"text"`, or a prefix of it
/// cut at the display width without the closing quote (the 49G truncates
/// long values).
fn shows_string(display: &str, text: &str) -> bool {
    let Some(inner) = display.strip_prefix('"') else {
        return false;
    };
    match inner.strip_suffix('"') {
        Some(whole) => whole == text,
        None => inner.chars().count() >= 16 && text.starts_with(inner),
    }
}

/// `text` with ASCII trigraphs read as the calculator reads them.
fn as_calculator_text(text: &str) -> String {
    hptx_core::charset::encode_command(text)
        .map(|b| hptx_core::charset::decode(&b))
        .unwrap_or_else(|_| text.to_string())
}

/// A tool that must leave `want` levels left `got`: the object's text did
/// not compile to exactly one object.
fn check_depth(tool: &str, want: usize, got: usize) -> Result<()> {
    if want != got {
        bail!(
            "{tool} left {got} stack levels instead of {want}: the object did not compile to \
             exactly one object; look at the stack"
        );
    }
    Ok(())
}

/// Whether `command` fits one Kermit host command packet.
fn command_fits(command: &str) -> bool {
    hptx_core::charset::encode_command(command).is_ok_and(|b| b.len() <= MAX_COMMAND_BYTES)
}

/// A command is only accepted inside a composite: on its own a host
/// command would run it. (Sources that could run anything else are
/// refused by [`to_source`].)
fn check_sources(obj: &Object, top: bool) -> Result<()> {
    match obj {
        Object::Command { .. } if top => {
            bail!("a command cannot be pushed on its own; run it with eval");
        }
        Object::List { items } => {
            for i in items {
                check_sources(i, false)?;
            }
        }
        Object::Tagged { object, .. } => check_sources(object, top)?,
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::Real;

    #[test]
    fn sources_are_checked_before_sending() {
        let c = Object::Command {
            name: None,
            address: None,
            source: Some("+".into()),
        };
        assert!(check_sources(&c, true).is_err());
        let l = Object::List {
            items: vec![c, Object::Real { value: Real::ZERO }],
        };
        assert!(check_sources(&l, true).is_ok());
        let t = Object::Tagged {
            tag: "T".into(),
            object: Box::new(Object::Command {
                name: None,
                address: None,
                source: None,
            }),
        };
        assert!(check_sources(&t, true).is_err());
    }

    #[test]
    fn syntax_error_strings() {
        assert!(shows_string("\"SIN(0.5)\"", "SIN(0.5)"));
        assert!(!shows_string("\"SIN(0.5)\"", "SIN(0.6)"));
        assert!(!shows_string("5", "5"));
        // 49G: cut at the display width, no closing quote.
        assert!(shows_string(
            "\"SIN(0.5)+COS(0.3)*TAN",
            "SIN(0.5)+COS(0.3)*TAN(1.2)"
        ));
        assert!(!shows_string("\"SIN(", "SIN(0.5)"));
    }

    #[test]
    fn leftovers_compare_in_calculator_characters() {
        let src = "'\\GS(X=1,10,X)' EVAL";
        assert_eq!(as_calculator_text(src), "'\u{3a3}(X=1,10,X)' EVAL");
        assert!(shows_string(
            "\"'\u{3a3}(X=1,10,X)' EVAL\"",
            &as_calculator_text(src)
        ));
    }

    #[test]
    fn command_length() {
        assert!(command_fits("1 2 +"));
        assert!(command_fits(&"1".repeat(77)));
        assert!(!command_fits(&"1".repeat(78)));
        // A trigraph is one byte.
        assert!(command_fits(&format!("{}\\->", "1".repeat(76))));
        assert_eq!(count(3), "3.");
        assert!(has_server(Model::Hp49g) && !has_server(Model::Hp39g));
    }
}
