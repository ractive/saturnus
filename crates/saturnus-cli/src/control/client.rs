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

    /// One request; `content_type` goes with a body.
    pub fn send(
        &self,
        method: &str,
        target: &str,
        headers: &[(&str, &str)],
        body: &[u8],
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
        let mut head = format!(
            "{method} {target} HTTP/1.1\r\nHost: {}\r\nAuthorization: {}\r\nConnection: close\r\nContent-Length: {}\r\n",
            self.addr,
            self.token.bearer(),
            body.len()
        );
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
                let (a, b): (String, String) = (
                    text.chars().take(cursor).collect(),
                    text.chars().skip(cursor).collect(),
                );
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
        CtlCmd::Stack | CtlCmd::Tree | CtlCmd::Flags => {
            let path = match args.command {
                CtlCmd::Stack => "/v1/stack",
                CtlCmd::Tree => "/v1/tree",
                _ => "/v1/flags",
            };
            let v = c.call("GET", path, None)?;
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
