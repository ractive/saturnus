//! `saturnus ctl`: the control API's client. A blocking HTTP/1.1 request
//! over `TcpStream` per call, with the token from the token file; text for
//! people, `--json` for scripts; files only on this side (the server never
//! takes a path).

use std::io::Write as _;
use std::net::{SocketAddr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::http;
use super::token::{self, Token};

/// Time to connect to the API.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Time to wait for a response: longer than the server's own wait for the
/// machine.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);
/// Largest response read (a snapshot, with room).
const MAX_RESPONSE: usize = 16 * 1024 * 1024;

/// `saturnus ctl` options.
#[derive(Debug, Args)]
pub struct CtlArgs {
    /// The API's address: PORT or 127.0.0.1:PORT (default 4840, or
    /// SATURNUS_CONTROL).
    #[arg(long, global = true)]
    control: Option<String>,
    /// The token file (default: see README, or SATURNUS_TOKEN_FILE).
    #[arg(long, global = true)]
    token_file: Option<PathBuf>,
    /// Print the API's result as JSON.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: CtlCmd,
}

#[derive(Debug, Subcommand)]
enum CtlCmd {
    /// Print the screen as text (`#` dark, `.` light), or save a PNG.
    Screen {
        /// Write a 1-bit PNG here instead.
        #[arg(long)]
        png: Option<PathBuf>,
        /// PNG pixels per LCD pixel, 1-8.
        #[arg(long, default_value_t = 1, requires = "png")]
        scale: u32,
    },
    /// Run a key script and wait until the calculator is idle; several
    /// arguments are several lines ("2 ENTER 3 +" is one line of four
    /// presses).
    Keys {
        /// Script lines (see README, "Key scripts").
        #[arg(required_unless_present_any = ["down", "up", "release_all"])]
        script: Vec<String>,
        /// Press KEY and keep it down (returns at once).
        #[arg(long, value_name = "KEY", conflicts_with_all = ["script", "up", "release_all"])]
        down: Option<String>,
        /// Release KEY (returns at once).
        #[arg(long, value_name = "KEY", conflicts_with_all = ["script", "release_all"])]
        up: Option<String>,
        /// Release every key and drop queued presses.
        #[arg(long, conflicts_with = "script")]
        release_all: bool,
    },
    /// Type text into the command line by key presses (any character
    /// the model can type; a newline is the calculator's newline): insert
    /// it at the cursor (or start a line), or with --run also press ENTER,
    /// or with --replace clear the line being edited first.
    Type {
        /// The text.
        text: String,
        /// Then press ENTER; report whether the line closed and the
        /// calculator's error message.
        #[arg(long, conflicts_with = "replace")]
        run: bool,
        /// Clear the command line being edited (staying in it) first.
        #[arg(long)]
        replace: bool,
    },
    /// The command line being edited, read from RAM without a key press.
    Cmdline,
    /// Read or write memory (nibbles, through the current mapping).
    Mem {
        #[command(subcommand)]
        op: MemOp,
    },
    /// Save the machine state to a file, or load it from one.
    Snapshot {
        #[command(subcommand)]
        op: SnapshotOp,
    },
    /// Model, ROM, speed, display and endpoints.
    Info,
    /// Cycles and timing counters.
    Cycles,
    /// The model: clock, display size, serial port, keys.
    Model,
    /// The stack, read from RAM (48SX, 48GX, 49G).
    Stack,
    /// HOME's variables, read from RAM (48SX, 48GX, 49G).
    Tree,
    /// The flags, read from RAM (48SX, 48GX, 49G).
    Flags,
    /// The object at ADDR (a variable's `address` from `tree`), read from
    /// RAM (48SX, 48GX, 49G).
    Object {
        /// Its address, hex (`#`/`0x` optional).
        addr: String,
    },
    /// Store FILE (an HP binary file, or text) as a variable, through the
    /// calculator's Kermit server (48SX, 48GX, 49G).
    Store {
        /// The file.
        file: PathBuf,
        /// The variable's name (default: the file's name without its
        /// extension).
        #[arg(long)]
        name: Option<String>,
        /// The directory, as HOME/A/B (default: the current one).
        #[arg(long)]
        dir: Option<String>,
    },
    /// Fetch variable NAME into FILE as an HP binary file.
    Fetch {
        /// The variable.
        name: String,
        /// Where to write it.
        file: PathBuf,
        /// The directory, as HOME/A/B (default: the current one).
        #[arg(long)]
        dir: Option<String>,
    },
    /// Purge variable NAME (a directory with everything in it).
    Purge {
        /// The variable.
        name: String,
        /// The directory, as HOME/A/B (default: the current one).
        #[arg(long)]
        dir: Option<String>,
    },
    /// Rename variable NAME to TO.
    Rename {
        /// The variable.
        name: String,
        /// Its new name.
        to: String,
        /// The directory, as HOME/A/B (default: the current one).
        #[arg(long)]
        dir: Option<String>,
    },
    /// Create the empty directory NAME.
    Mkdir {
        /// The new directory's name.
        name: String,
        /// The directory it goes in, as HOME/A/B (default: the current
        /// one).
        #[arg(long)]
        dir: Option<String>,
    },
    /// Make DIR (HOME/A/B) the current directory.
    Cd {
        /// The directory.
        dir: String,
    },
    /// Print the text of variable NAME (or of stack level N) as the
    /// palette's editor gets it; with `--set`, compile TEXT on the
    /// calculator and put the object there instead (48SX, 48GX, 49G).
    Text {
        /// The variable.
        #[arg(required_unless_present = "level", conflicts_with = "level")]
        name: Option<String>,
        /// A stack level instead (1 is the top).
        #[arg(long)]
        level: Option<usize>,
        /// The new text.
        #[arg(long)]
        set: Option<String>,
        /// The directory, as HOME/A/B (default: the current one).
        #[arg(long)]
        dir: Option<String>,
    },
    /// Set or clear flag N (negative: a system flag).
    Flag {
        /// The flag.
        #[arg(allow_negative_numbers = true)]
        flag: i32,
        /// `set` or `clear`.
        #[arg(value_parser = ["set", "clear"])]
        state: String,
    },
}

#[derive(Debug, Subcommand)]
enum MemOp {
    /// Print LEN nibbles from ADDR (hex) as hex digits.
    Read {
        /// Start address, hex (`#`/`0x` optional).
        addr: String,
        /// Number of nibbles (decimal).
        len: u64,
    },
    /// Write hex digits from ADDR (hex) on.
    Write {
        /// Start address, hex (`#`/`0x` optional).
        addr: String,
        /// The nibbles, as hex digits.
        nibbles: String,
    },
}

#[derive(Debug, Subcommand)]
enum SnapshotOp {
    /// Save the state into FILE.
    Get {
        /// Where to write the state.
        file: PathBuf,
    },
    /// Load the state from FILE (same model and ROM).
    Put {
        /// The state to load.
        file: PathBuf,
    },
}

/// A connection target with its token.
#[derive(Debug)]
pub struct Client {
    addr: SocketAddrV4,
    token: Token,
}

/// A response: status, content type and body.
#[derive(Debug)]
pub struct Reply {
    /// The HTTP status.
    pub status: u16,
    /// The `Content-Type`.
    pub content_type: String,
    /// The body.
    pub body: Vec<u8>,
}

impl Client {
    /// A client for `addr` with `token`.
    pub fn new(addr: SocketAddrV4, token: Token) -> Self {
        Self { addr, token }
    }

    /// One request with the token; `content_type` goes with a body.
    pub fn send(
        &self,
        method: &str,
        target: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<Reply> {
        self.request(method, target, headers, body, true)
    }

    /// Make sure the server on the port holds our token before the token
    /// goes to it: another local user may have bound the port first. The
    /// server answers a fresh nonce with an HMAC of it and its bound port
    /// under the token, which only a holder of the token can compute.
    pub fn verify(&self) -> Result<()> {
        let nonce = token::random_hex()?;
        let r = self.request("GET", &format!("/v1/hello?nonce={nonce}"), &[], b"", false)?;
        let v: Value = serde_json::from_slice(&r.body).unwrap_or(Value::Null);
        let proof = v["result"]["proof"].as_str().unwrap_or_default();
        let expected = self.token.proof(&nonce, self.addr.port());
        if r.status != 200 || !token::constant_time_eq(proof.as_bytes(), expected.as_bytes()) {
            bail!(
                "the server on {} did not prove that it holds the token, so the token was not \
                 sent: is it a `saturnus run` of this user with this token file?",
                self.addr
            );
        }
        Ok(())
    }

    fn request(
        &self,
        method: &str,
        target: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        with_token: bool,
    ) -> Result<Reply> {
        let mut s = TcpStream::connect_timeout(&SocketAddr::V4(self.addr), CONNECT_TIMEOUT)
            .with_context(|| {
                format!(
                    "no saturnus control API on {} (is `saturnus run` running? \
                     --control or SATURNUS_CONTROL select another port)",
                    self.addr
                )
            })?;
        s.set_write_timeout(Some(RESPONSE_TIMEOUT))
            .context("cannot set a write timeout")?;
        let mut head = format!("{method} {target} HTTP/1.1\r\nHost: {}\r\n", self.addr);
        if with_token {
            head.push_str(&format!("Authorization: {}\r\n", self.token.bearer()));
        }
        head.push_str(&format!(
            "Connection: close\r\nContent-Length: {}\r\n",
            body.len()
        ));
        for (k, v) in headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str("\r\n");
        // The head carries the token: an error here must not quote it.
        s.write_all(head.as_bytes())
            .and_then(|()| s.write_all(body))
            .context("cannot send the request")?;
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let (h, rest) = http::read_head(&mut s, deadline)
            .map_err(|e| anyhow::anyhow!("bad response from the API: {e}"))?;
        let status: u16 = h
            .second
            .parse()
            .with_context(|| format!("bad status {:?}", h.second))?;
        let len = h
            .content_length()
            .map_err(|e| anyhow::anyhow!("bad response from the API: {e}"))?;
        if len > MAX_RESPONSE {
            bail!("response of {len} bytes is too large");
        }
        let body = http::read_body(&mut s, rest, len, deadline)
            .map_err(|e| anyhow::anyhow!("bad response from the API: {e}"))?;
        let content_type = h
            .header("content-type")
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_string();
        Ok(Reply {
            status,
            content_type,
            body,
        })
    }

    /// A request answered with a protocol reply; its `result`, or the
    /// API's error message as the error.
    pub fn call(&self, method: &str, target: &str, body: Option<&Value>) -> Result<Value> {
        let bytes = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
        let headers: &[(&str, &str)] = if body.is_some() {
            &[("Content-Type", "application/json")]
        } else {
            &[]
        };
        let r = self.send(method, target, headers, &bytes)?;
        result_of(&r)
    }
}

/// The `result` of a reply, or an error with the API's message.
fn result_of(r: &Reply) -> Result<Value> {
    if r.status == 401 {
        bail!("the API refused the token (401): is the token file the one `saturnus run` uses?");
    }
    let v: Value = serde_json::from_slice(&r.body).unwrap_or(Value::Null);
    if r.status == 200 && v["ok"] == true {
        return Ok(v["result"].clone());
    }
    let message = v["error"].as_str().unwrap_or("no message").to_string();
    bail!("{message} (HTTP {} {})", r.status, http::reason(r.status))
}

fn parse_hex(s: &str) -> Result<u64> {
    let t = s.trim_start_matches('#').trim_start_matches("0x");
    u64::from_str_radix(t, 16).with_context(|| format!("bad hex address {s:?}"))
}

/// Print `v` as `key: value` lines (strings without quotes).
fn print_fields(v: &Value) {
    if let Value::Object(m) = v {
        for (k, x) in m {
            match x {
                Value::String(s) => println!("{k}: {s}"),
                other => println!("{k}: {other}"),
            }
        }
    } else {
        println!("{v}");
    }
}

/// Write `bytes` to `path` (whole or not at all, beside it first).
fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    saturnus_drive::runner::write_atomic(path, bytes, |f, b| f.write_all(b))
        .with_context(|| format!("cannot write {}", path.display()))
}

/// Run `saturnus ctl`.
pub fn run(args: &CtlArgs) -> Result<()> {
    let addr = super::resolve_control(args.control.as_deref())?;
    let token = token::load(&token::resolve(args.token_file.as_deref())?)?;
    let c = Client::new(addr, token);
    c.verify()?;
    let show = |v: &Value| {
        if args.json {
            println!("{v}");
        }
    };
    let warn = |v: &Value| {
        for w in v["warnings"].as_array().into_iter().flatten() {
            eprintln!("warning: {}", w.as_str().unwrap_or_default());
        }
    };
    match &args.command {
        CtlCmd::Screen {
            png: Some(p),
            scale,
        } => {
            let r = c.send(
                "GET",
                &format!("/v1/screen?scale={scale}"),
                &[("Accept", "image/png")],
                b"",
            )?;
            if r.status != 200 || r.content_type != "image/png" {
                result_of(&r)?;
                bail!("the API sent {} instead of a PNG", r.content_type);
            }
            write_file(p, &r.body)?;
            show(&json!({"file": p.display().to_string(), "bytes": r.body.len()}));
        }
        CtlCmd::Screen { png: None, .. } => {
            let v = c.call("GET", "/v1/screen", None)?;
            if args.json {
                show(&v);
            } else {
                for row in v["rows"].as_array().into_iter().flatten() {
                    println!("{}", row.as_str().unwrap_or_default());
                }
            }
        }
        CtlCmd::Keys {
            script,
            down,
            up,
            release_all,
        } => {
            let msg = if let Some(k) = down {
                json!({"cmd": "keyDown", "key": k.to_ascii_lowercase()})
            } else if let Some(k) = up {
                json!({"cmd": "keyUp", "key": k.to_ascii_lowercase()})
            } else if *release_all {
                json!({"cmd": "releaseAll"})
            } else {
                json!({"cmd": "keyScript", "script": script.join("\n")})
            };
            let v = c.call("POST", "/v1/keys", Some(&msg))?;
            warn(&v);
            show(&v);
        }
        CtlCmd::Type { text, run, replace } => {
            let cmd = match (run, replace) {
                (true, _) => "run",
                (_, true) => "replace",
                _ => "insert",
            };
            let v = c.call("POST", "/v1/type", Some(&json!({"cmd": cmd, "text": text})))?;
            if args.json {
                show(&v);
            } else {
                println!(
                    "typed {} characters with {} keys in {:.1} s of emulated time",
                    v["typed"],
                    v["keys"],
                    v["emulatedMs"].as_f64().unwrap_or(0.0) / 1000.0
                );
                if let Some(closed) = v["closed"].as_bool() {
                    let line = if closed { "closed" } else { "still open" };
                    println!("command line {line}");
                }
                if let Some(e) = v["error"].as_str() {
                    println!("calculator error: {e}");
                }
                if v["running"] == true {
                    println!("the calculator is still busy");
                }
            }
        }
        CtlCmd::Cmdline => {
            let v = c.call("GET", "/v1/cmdline", None)?;
            if args.json {
                show(&v);
            } else if v["active"] == true {
                let text = v["text"].as_str().unwrap_or_default();
                let cursor = v["cursor"].as_u64().unwrap_or(0) as usize;
                let (a, b) = text.split_at(cursor_offset(text, cursor));
                println!("{a}\u{2502}{b}");
            } else {
                println!("(no command line)");
            }
        }
        CtlCmd::Mem {
            op: MemOp::Read { addr, len },
        } => {
            let a = parse_hex(addr)?;
            let v = c.call("GET", &format!("/v1/mem?address={a}&length={len}"), None)?;
            if args.json {
                show(&v);
            } else {
                println!("{}", v["nibbles"].as_str().unwrap_or_default());
            }
        }
        CtlCmd::Mem {
            op: MemOp::Write { addr, nibbles },
        } => {
            let a = parse_hex(addr)?;
            let msg = json!({"cmd": "poke", "address": a, "nibbles": nibbles});
            show(&c.call("POST", "/v1/mem", Some(&msg))?);
        }
        CtlCmd::Snapshot {
            op: SnapshotOp::Get { file },
        } => {
            let r = c.send("GET", "/v1/snapshot", &[], b"")?;
            if r.status != 200 || r.content_type != "application/octet-stream" {
                result_of(&r)?;
                bail!("the API sent {} instead of a state", r.content_type);
            }
            write_file(file, &r.body)?;
            show(&json!({"file": file.display().to_string(), "bytes": r.body.len()}));
        }
        CtlCmd::Snapshot {
            op: SnapshotOp::Put { file },
        } => {
            let cap = super::server::MAX_SNAPSHOT_BODY as u64;
            let data =
                saturnus_drive::runner::read_capped(file, cap).map_err(anyhow::Error::msg)?;
            let r = c.send(
                "PUT",
                "/v1/snapshot",
                &[("Content-Type", "application/octet-stream")],
                &data,
            )?;
            show(&result_of(&r)?);
        }
        CtlCmd::Info | CtlCmd::Cycles => {
            let path = match args.command {
                CtlCmd::Info => "/v1/info",
                _ => "/v1/cycles",
            };
            let v = c.call("GET", path, None)?;
            if args.json {
                show(&v);
            } else {
                print_fields(&v);
            }
        }
        CtlCmd::Model => {
            let v = c.call("GET", "/v1/model", None)?;
            if args.json {
                show(&v);
            } else {
                println!("model: {}", v["model"].as_str().unwrap_or_default());
                println!("clockHz: {}", v["clockHz"]);
                println!("display: {}x{}", v["width"], v["height"]);
                println!("serial: {}", v["hasSerial"]);
                let keys: Vec<&str> = v["layout"]["keys"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|k| k["name"].as_str())
                    .collect();
                println!("keys: {}", keys.join(" "));
            }
        }
        CtlCmd::Store { file, name, dir } => {
            let data =
                saturnus_drive::runner::read_capped(file, saturnus_drive::runner::MAX_FILE as u64)
                    .map_err(anyhow::Error::msg)?;
            let name = name
                .clone()
                .unwrap_or_else(|| saturnus_drive::runner::variable_name(file));
            let mut msg = json!({"cmd": "storeFile", "name": name, "data": saturnus_host::host::base64(&data)});
            with_dir(&mut msg, dir.as_deref());
            let v = c.call("POST", "/v1/memory", Some(&msg))?;
            show(&v);
            if !args.json {
                println!("stored as {}", v["name"].as_str().unwrap_or_default());
            }
        }
        CtlCmd::Fetch { name, file, dir } => {
            let mut msg = json!({"cmd": "fetchFile", "name": name});
            with_dir(&mut msg, dir.as_deref());
            let v = c.call("POST", "/v1/memory", Some(&msg))?;
            let data = saturnus_host::host::base64_decode(v["data"].as_str().unwrap_or_default())
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            write_file(file, &data)?;
            show(&json!({"file": file.display().to_string(), "bytes": data.len()}));
        }
        CtlCmd::Purge { name, dir }
        | CtlCmd::Rename { name, dir, .. }
        | CtlCmd::Mkdir { name, dir } => {
            let mut msg = match &args.command {
                CtlCmd::Rename { to, .. } => json!({"cmd": "rename", "name": name, "to": to}),
                CtlCmd::Mkdir { .. } => json!({"cmd": "createDir", "name": name}),
                _ => json!({"cmd": "purge", "name": name}),
            };
            with_dir(&mut msg, dir.as_deref());
            show(&c.call("POST", "/v1/memory", Some(&msg))?);
        }
        CtlCmd::Cd { dir } => {
            let mut msg = json!({"cmd": "changeDir"});
            with_dir(&mut msg, Some(dir));
            show(&c.call("POST", "/v1/memory", Some(&msg))?);
        }
        CtlCmd::Text {
            name,
            level,
            set,
            dir,
        } => {
            let mut msg = match set {
                Some(text) => json!({"cmd": "storeText", "text": text}),
                None => json!({"cmd": "editText"}),
            };
            match (name, level) {
                (Some(n), _) => msg["name"] = json!(n),
                (None, l) => msg["level"] = json!(l),
            }
            with_dir(&mut msg, dir.as_deref());
            let v = c.call("POST", "/v1/memory", Some(&msg))?;
            if let Some(e) = v["error"].as_str() {
                anyhow::bail!("the calculator says: {e}");
            }
            if args.json || set.is_some() {
                show(&v);
            } else {
                println!("{}", v["text"].as_str().unwrap_or_default());
            }
        }
        CtlCmd::Flag { flag, state } => {
            let msg = json!({"cmd": "setFlag", "flag": flag, "on": state == "set"});
            show(&c.call("POST", "/v1/memory", Some(&msg))?);
        }
        CtlCmd::Stack | CtlCmd::Tree | CtlCmd::Flags | CtlCmd::Object { .. } => {
            let path = match &args.command {
                CtlCmd::Stack => "/v1/stack".to_owned(),
                CtlCmd::Tree => "/v1/tree".to_owned(),
                CtlCmd::Object { addr } => format!("/v1/object?address={}", parse_hex(addr)?),
                _ => "/v1/flags".to_owned(),
            };
            let v = c.call("GET", &path, None)?;
            if args.json {
                show(&v);
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&v).context("cannot format the result")?
                );
            }
        }
    }
    Ok(())
}

/// Put `dir` (`HOME/A/B`) into `msg` as the protocol's `dir` array.
fn with_dir(msg: &mut Value, dir: Option<&str>) {
    if let Some(d) = dir {
        let parts: Vec<&str> = d.split('/').filter(|p| !p.is_empty()).collect();
        msg["dir"] = json!(parts);
    }
}

/// The byte offset of `cursor` calculator characters into `text`: each
/// character is one Unicode scalar, except `x̄`, which is `x` and a
/// combining macron (web/protocol.md, "Typing").
fn cursor_offset(text: &str, cursor: usize) -> usize {
    let mut chars = text.char_indices().peekable();
    for _ in 0..cursor {
        let Some((_, c)) = chars.next() else {
            return text.len();
        };
        if c == 'x' && chars.peek().is_some_and(|&(_, m)| m == '\u{0304}') {
            chars.next();
        }
    }
    chars.peek().map_or(text.len(), |&(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_counts_calculator_characters() {
        let t = "x\u{0304}12";
        assert_eq!(t.split_at(cursor_offset(t, 1)), ("x\u{0304}", "12"));
        assert_eq!(t.split_at(cursor_offset(t, 2)), ("x\u{0304}1", "2"));
        assert_eq!(cursor_offset(t, 0), 0);
        assert_eq!(cursor_offset(t, 9), t.len());
        assert_eq!("«x»".split_at(cursor_offset("«x»", 2)), ("«x", "»"));
    }
}
