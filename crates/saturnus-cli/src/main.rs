//! `saturnus`: run Saturn calculator ROMs headless, replay key scripts,
//! dump the screen, serve the serial port and the control API, drive a
//! running emulator (`ctl`), disassemble ROM code and fetch ROM images.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod control;
mod reference;
mod rom;
mod serial;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, bail};
use clap::builder::TypedValueParser as _;
use clap::{Parser, Subcommand};
use saturnus::disasm::decode;
use saturnus::{ADDR_MASK, Machine, Model, NEW_CARD_BYTES, Port};

use saturnus_drive::runner::{self, Runner};
use saturnus_drive::session::Session;
use saturnus_drive::{autostart, screen, script};
use saturnus_host::Emulator;
use serde_json::{Value, json};
use serial::{BridgeOptions, SerialPort, SerialSpec, ServeHook};

/// Headless emulator of the HP Saturn calculators (emulates the HP 48SX,
/// 48GX, 49G, 38G, 39G, 40G and 42S).
#[derive(Debug, Parser)]
#[command(name = "saturnus", version)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Run a ROM: optional state load, N cycles, a key script; then dump
    /// the screen and save the state, or (with --serve, --serial or
    /// --control) first serve the serial port and the control API until
    /// Ctrl-C.
    Run(Box<RunArgs>),
    /// Drive a running `saturnus run` through its control API.
    Ctl(Box<control::client::CtlArgs>),
    /// Disassemble ROM code.
    Disasm {
        /// Calculator model.
        #[arg(long, value_parser = model_parser(), default_value = "48sx")]
        model: Model,
        /// Packed ROM image.
        #[arg(long)]
        rom: PathBuf,
        /// Start address, hex (`#`/`0x` prefix optional).
        #[arg(long, value_parser = parse_hex)]
        at: u32,
        /// Number of instructions.
        #[arg(long, default_value_t = 20)]
        count: usize,
    },
    /// Look up a built-in command in the reference (48SX, 48GX, 49G):
    /// description, stack effect, menu, examples run on this emulator,
    /// manual pages. ASCII spellings work (->LIST, SIGMA+, or the
    /// calculator's codes such as \->LIST and \.S); a query that names
    /// several commands lists them and exits with status 2.
    Ref {
        /// The command, e.g. STO, →LIST or ->LIST.
        #[arg(allow_hyphen_values = true)]
        command: String,
        /// Only this model: 48sx, 48gx or 49g (its menu, examples and
        /// manual pages).
        #[arg(long)]
        model: Option<String>,
        /// Print the entry (or the candidates) as JSON.
        #[arg(long)]
        json: bool,
    },
    /// ROM image management.
    Rom {
        #[command(subcommand)]
        command: RomCmd,
    },
}

#[derive(Debug, Subcommand)]
enum RomCmd {
    /// Download a model's ROM from hpcalc.org and verify size and SHA-256.
    Fetch {
        /// Calculator model.
        #[arg(long, value_parser = model_parser(), default_value = "48sx")]
        model: Model,
        /// Target directory.
        #[arg(long, default_value = "roms")]
        dir: PathBuf,
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, clap::Args)]
struct RunArgs {
    /// Calculator model.
    #[arg(long, value_parser = model_parser(), default_value = "48sx")]
    model: Model,
    /// Packed ROM image (two nibbles per byte).
    #[arg(long)]
    rom: PathBuf,
    /// CPU cycles to run before the key script (after `--load`).
    #[arg(long, default_value_t = 0, value_parser = parse_count)]
    cycles: u64,
    /// Key script to replay (see README, "Key scripts").
    #[arg(long)]
    keys: Option<PathBuf>,
    /// Write the final screen: `.txt` (131x64 `#`/`.`, 131x16 on the 42S)
    /// or `.png`.
    #[arg(long)]
    screen: Option<PathBuf>,
    /// Write the lit annunciators as one line (names separated by spaces,
    /// `-` for none).
    #[arg(long)]
    annunciators: Option<PathBuf>,
    /// Restore a saved machine state before running (needs the same ROM).
    #[arg(long)]
    load: Option<PathBuf>,
    /// Save the machine state at the end.
    #[arg(long)]
    save: Option<PathBuf>,
    /// Packed RAM-card image for port 1, inserted after `--load` and before
    /// the run. A missing file is created as a zeroed 128 KB card.
    #[arg(long)]
    card1: Option<PathBuf>,
    /// Packed RAM-card image for port 2, as `--card1`.
    #[arg(long)]
    card2: Option<PathBuf>,
    /// Write the card images back to their files at the end of the run.
    #[arg(long)]
    card_writeback: bool,
    /// Record the last N instructions and print them at the end or on a
    /// CPU halt.
    #[arg(long, default_value_t = 0)]
    trace: usize,
    /// After the key script, serve until SIGINT/SIGTERM: the serial port
    /// (`tcp:4841` on models with one) and the control API (port 4840);
    /// `--screen`, `--save` and the card files are written when it stops.
    #[arg(long)]
    serve: bool,
    /// Bridge the serial port (and serve): `tcp:PORT` (listens on
    /// 127.0.0.1), `tcp:HOST:PORT` or `stdio`.
    #[arg(long, value_parser = SerialSpec::parse)]
    serial: Option<SerialSpec>,
    /// Allow `--serial tcp:HOST:PORT` on an address other than loopback
    /// (anyone who reaches it can talk to the calculator).
    #[arg(long, requires = "serial")]
    serial_remote: bool,
    /// With --serve: no serial bridge.
    #[arg(long, conflicts_with = "serial")]
    no_serial: bool,
    /// Before serving, answer the boot prompt with NO (unless `--load`)
    /// and start the Kermit server (ALPHA ALPHA S E R V E R ENTER).
    #[arg(long)]
    autostart: bool,
    /// Stop after the first serial client disconnects (stdin EOF for
    /// `stdio`), then write `--screen`/`--save` as usual.
    #[arg(long)]
    exit_on_disconnect: bool,
    /// Append the serial traffic with emulated timestamps to this file.
    #[arg(long)]
    serial_log: Option<PathBuf>,
    /// Serve the control API here (and serve): PORT or 127.0.0.1:PORT;
    /// with --serve the default is 4840, or SATURNUS_CONTROL. It binds
    /// 127.0.0.1 only.
    #[arg(long)]
    control: Option<String>,
    /// With --serve: no control API.
    #[arg(long, conflicts_with = "control")]
    no_control: bool,
    /// The control API's token file (default: see README, or
    /// SATURNUS_TOKEN_FILE); created with a new token if missing.
    #[arg(long)]
    token_file: Option<PathBuf>,
    /// Report `wait-idle` timings and serial connections on stderr.
    #[arg(long, short)]
    verbose: bool,
}

/// `--model`: a model's short name, parsed by the core (`Model`'s
/// `FromStr`); `--help` lists the names.
fn model_parser() -> impl clap::builder::TypedValueParser<Value = Model> {
    let names = Model::ALL.map(|m| {
        let help = match m {
            Model::Hp40g => "HP 40G (same ROM as the 39G)",
            Model::Hp42s => "HP 42S (Lewis chip; supply your own 64 KB ROM dump)",
            _ => "",
        };
        let v = clap::builder::PossibleValue::new(m.name());
        if help.is_empty() { v } else { v.help(help) }
    });
    clap::builder::PossibleValuesParser::new(names).try_map(|s| s.parse::<Model>())
}

fn parse_hex(s: &str) -> Result<u32> {
    let t = s.trim_start_matches('#').trim_start_matches("0x");
    u32::from_str_radix(t, 16).with_context(|| format!("bad hex address {s:?}"))
}

fn parse_count(s: &str) -> Result<u64> {
    s.replace('_', "")
        .parse()
        .with_context(|| format!("bad count {s:?}"))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Run(args) => run(&args),
        Cmd::Ctl(args) => control::client::run(&args),
        Cmd::Disasm {
            model,
            rom,
            at,
            count,
        } => disasm(model, &rom, at, count),
        Cmd::Ref {
            command,
            model,
            json,
        } => {
            match reference::lookup(&command, model.as_deref())? {
                reference::Found::One(entry) if json => {
                    println!("{}", serde_json::to_string_pretty(&entry)?);
                }
                reference::Found::One(entry) => print!("{}", reference::text(&entry)),
                reference::Found::Many(names) => {
                    if json {
                        println!("{}", serde_json::json!({ "candidates": names }));
                    } else {
                        eprint!("{}", reference::candidates_text(&command, &names));
                    }
                    std::process::exit(2);
                }
            }
            Ok(())
        }
        Cmd::Rom {
            command: RomCmd::Fetch { model, dir, yes },
        } => rom::fetch(model, &dir, yes).map(|_| ()),
    }
}

/// What a serving run serves: the serial bridge's spec and whether the
/// control API runs.
#[derive(Debug)]
struct Serving {
    serial: Option<SerialSpec>,
    control: bool,
}

/// The mode of a run. A run serves with `--serve`, `--serial` or
/// `--control`; anything else is a batch run exactly as before the control
/// API (iteration 17): it runs and writes its results, then stops. A
/// serving run bridges `--serial` (with `--serve`, by default
/// `tcp:127.0.0.1:4841` on models with a serial port) and serves the
/// control API with `--control` (with `--serve`, by default on port 4840).
fn serving(args: &RunArgs, model: Model) -> Result<Option<Serving>> {
    if !(args.serve || args.serial.is_some() || args.control.is_some()) {
        let only = [
            (args.no_serial, "--no-serial"),
            (args.no_control, "--no-control"),
            (args.token_file.is_some(), "--token-file"),
        ];
        if let Some((_, flag)) = only.iter().find(|(on, _)| *on) {
            bail!("{flag} applies to a serving run: add --serve");
        }
        return Ok(None);
    }
    if args.serial.is_some() && !model.has_serial() {
        bail!(
            "the {} has no serial port: --serial is not supported",
            model.name().to_uppercase()
        );
    }
    let serial = match &args.serial {
        Some(spec) => Some(spec.clone()),
        None if args.serve && !args.no_serial && model.has_serial() => Some(SerialSpec::Tcp(
            format!("127.0.0.1:{}", control::DEFAULT_SERIAL_PORT),
        )),
        None => None,
    };
    if let Some(SerialSpec::Tcp(addr)) = &serial
        && !serial::is_loopback(addr)
    {
        if !args.serial_remote {
            bail!(
                "--serial {addr} is not a loopback address: anyone who reaches it can talk \
                 to the calculator; add --serial-remote to mean it"
            );
        }
        eprintln!(
            "warning: the serial bridge listens on {addr}, beyond this machine; it has no \
             authentication"
        );
    }
    let control = args.control.is_some() || (args.serve && !args.no_control);
    Ok(Some(Serving { serial, control }))
}

fn run(args: &RunArgs) -> Result<()> {
    let model = args.model;
    let serving = serving(args, model)?;
    let spec = serving.as_ref().and_then(|s| s.serial.clone());
    let serial_only = [
        (args.autostart, "--autostart"),
        (args.exit_on_disconnect, "--exit-on-disconnect"),
        (args.serial_log.is_some(), "--serial-log"),
    ];
    if spec.is_none()
        && let Some((_, flag)) = serial_only.iter().find(|(on, _)| *on)
    {
        bail!(
            "{flag} needs the serial bridge (--serial, or --serve on a model with a serial port)"
        );
    }
    // Checked before anything runs, so an unsupported model fails at once.
    let autostart = if spec.is_some() && args.autostart {
        autostart::autostart_script(model, args.load.is_none())?
    } else {
        Vec::new()
    };
    let image = rom::load(model, &args.rom)?;
    let machine = Machine::new(model, &image).context("cannot build the machine")?;
    let script = match &args.keys {
        Some(p) => {
            let text = std::fs::read_to_string(p)
                .with_context(|| format!("cannot read key script {}", p.display()))?;
            script::parse(&text).with_context(|| format!("in {}", p.display()))?
        }
        None => Vec::new(),
    };
    let mut s = Session::new(machine, args.trace, args.verbose);
    let script_name = args.keys.as_deref().map(|p| p.display().to_string());
    s.check_keys(&script)
        .with_context(|| format!("in {}", script_name.unwrap_or_default()))?;
    s.check_keys(&autostart)?;
    if let Some(p) = &args.load {
        load_state(&mut s.machine, p)?;
    }
    let cards = [(Port::One, &args.card1), (Port::Two, &args.card2)];
    for (port, path) in cards {
        if let Some(p) = path {
            insert_card(&mut s.machine, port, p)?;
        }
    }
    s.run(args.cycles)?;
    for line in &script {
        s.apply(line)?;
    }
    let machine = if serving.is_none() {
        if args.trace > 0 {
            eprintln!("{}", s.trace_text().trim_start());
        }
        s.machine
    } else {
        for line in &autostart {
            s.apply(line)?;
        }
        if args.trace > 0 {
            // The trace covers the run before serving.
            eprintln!("{}", s.trace_text().trim_start());
        }
        serve(
            args,
            s.machine,
            &image,
            spec,
            serving.is_some_and(|s| s.control),
        )?
    };
    // Every output is attempted: a failing one (a bad --screen path) must
    // not lose the RAM cards, which go first. The failures are reported
    // together.
    let mut errors = Vec::new();
    if args.card_writeback {
        for (port, path) in cards {
            if let Some(p) = path {
                errors.extend(write_card(&machine, port, p).err());
            }
        }
    }
    if let Some(p) = &args.save {
        errors.extend(save_state(&machine, p).err());
    }
    let fb = machine.framebuffer();
    if let Some(p) = &args.screen {
        errors.extend(screen::write(&fb.pixels, p).err());
    }
    if let Some(p) = &args.annunciators {
        errors.extend(
            std::fs::write(p, format!("{}\n", fb.annunciator_line()))
                .with_context(|| format!("cannot write {}", p.display()))
                .err(),
        );
    }
    if !errors.is_empty() {
        let text: Vec<String> = errors.iter().map(|e| format!("{e:#}")).collect();
        anyhow::bail!("{}", text.join("\n"));
    }
    if args.verbose {
        eprintln!("stopped at cycle {}", machine.cycles());
    }
    Ok(())
}

/// Events of the machine thread while serving: errors and a halt go to
/// stderr; frames and keys have no viewer here.
#[derive(Debug)]
struct LogSink;

impl runner::Sink for LogSink {
    fn event(&self, msg: Value) {
        match msg["type"].as_str() {
            Some("error") => eprintln!("saturnus: {}", msg["message"].as_str().unwrap_or("?")),
            Some("status") => {
                if let Some(h) = msg["halted"].as_str() {
                    eprintln!("saturnus: {h}");
                }
            }
            _ => {}
        }
    }
}

/// Serve the serial port (`spec`) and the control API, the machine paced
/// to wall-clock time, until SIGINT/SIGTERM (or the serial client leaves
/// with `--exit-on-disconnect`); returns the machine.
fn serve(
    args: &RunArgs,
    machine: Machine,
    rom_image: &[u8],
    spec: Option<SerialSpec>,
    with_control: bool,
) -> Result<Machine> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    ctrlc::set_handler(move || flag.store(true, Ordering::Relaxed))
        .context("cannot install the SIGINT/SIGTERM handler")?;
    // With the serial port on stdio, stdout carries its bytes.
    let stdio = spec == Some(SerialSpec::Stdio);
    let say = |line: String| -> Result<()> {
        if stdio {
            eprintln!("{line}");
        } else {
            println!("{line}");
            std::io::Write::flush(&mut std::io::stdout()).context("cannot flush stdout")?;
        }
        Ok(())
    };
    let clock_hz = machine.model().clock_hz();
    let (port, serial_endpoint) = match spec {
        Some(spec) => {
            let opts = BridgeOptions {
                spec,
                exit_on_disconnect: args.exit_on_disconnect,
                log: args.serial_log.as_deref(),
                verbose: args.verbose,
            };
            let (port, endpoint) = SerialPort::open(&opts, clock_hz)?;
            say(format!("serial bridged on {endpoint}"))?;
            (Some(port), Some(endpoint))
        }
        None => (None, None),
    };
    let control = if !with_control {
        None
    } else {
        let addr = control::resolve_control(args.control.as_deref())?;
        let path = control::token::resolve(args.token_file.as_deref())?;
        let token = control::token::load_or_create(&path)?;
        let listener = control::server::bind(addr)?;
        let port = listener
            .local_addr()
            .context("the control listener has no address")?
            .port();
        say(format!(
            "control API on http://127.0.0.1:{port} (token file: {})",
            path.display()
        ))?;
        Some((listener, port, token))
    };
    let sha = saturnus_host::sha256::hex_digest(rom_image);
    let rom_name = args
        .rom
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (tx, rx) = std::sync::mpsc::sync_channel(control::server::QUEUE_DEPTH);
    let mut r = Runner::for_host(LogSink, "http");
    r.set_info(json!({
        "romSha256": sha,
        "romRevision": rom::revision(&sha),
        "serial": serial_endpoint,
        "control": control.as_ref().map(|(_, port, _)| format!("http://127.0.0.1:{port}")),
    }));
    let hook = ServeHook::new(stop, port);
    let errors = hook.errors();
    r.set_hook(Box::new(hook));
    r.start(Emulator::from_machine(machine), &rom_name);
    if let Some((listener, port, token)) = control {
        control::server::spawn(
            listener,
            control::server::Config {
                port,
                token,
                tx: tx.clone(),
                reply_timeout: control::server::REPLY_TIMEOUT,
            },
        )
        .context("cannot start the control API thread")?;
    }
    // Keeps the channel open without the API; the hook ends the run.
    let _keep = tx;
    let r = r.run(&rx);
    if let Some(e) = errors.lock().ok().and_then(|mut e| e.take()) {
        return Err(e.context("serial bridge"));
    }
    r.into_emulator()
        .map(Emulator::into_machine)
        .context("the machine thread lost its machine")
}

fn load_state(m: &mut Machine, p: &Path) -> Result<()> {
    let data = saturnus_drive::files::read_capped(p, saturnus_drive::files::MAX_STATE_FILE)
        .map_err(anyhow::Error::msg)
        .with_context(|| format!("cannot read state {}", p.display()))?;
    m.load_state(&data)
        .with_context(|| format!("cannot load state {}", p.display()))
}

fn save_state(m: &Machine, p: &Path) -> Result<()> {
    write_atomic(p, &m.save_state()).with_context(|| format!("cannot write state {}", p.display()))
}

/// Replace `p` whole (never a half-written state or card; no link
/// followed at the temporary name). The user named `p`: a symlink there
/// is followed to the real file, which is replaced beside itself, and an
/// existing file keeps its permissions. (A dangling link is replaced.)
fn write_atomic(p: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let real = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let perms = std::fs::metadata(&real).ok().map(|m| m.permissions());
    saturnus_drive::runner::write_atomic(&real, bytes, |f, b| {
        f.write_all(b)?;
        match perms {
            Some(perms) => f.set_permissions(perms),
            None => Ok(()),
        }
    })
}

/// Insert the card image at `p` into `port`; a missing file becomes a
/// zeroed 128 KB card.
fn insert_card(m: &mut Machine, port: Port, p: &Path) -> Result<()> {
    let image = if p.exists() {
        saturnus_drive::files::read_capped(p, saturnus::CARD_MAX_BYTES as u64)
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("cannot read card {}", p.display()))?
    } else {
        let image = vec![0u8; NEW_CARD_BYTES];
        std::fs::write(p, &image).with_context(|| format!("cannot create card {}", p.display()))?;
        eprintln!(
            "created {} as an empty {} KB RAM card",
            p.display(),
            NEW_CARD_BYTES / 1024
        );
        image
    };
    m.insert_card(port, &image).with_context(|| {
        format!(
            "cannot insert card {} into port {}",
            p.display(),
            port.number()
        )
    })
}

fn write_card(m: &Machine, port: Port, p: &Path) -> Result<()> {
    let image = m
        .card_image(port)
        .with_context(|| format!("port {} has no card to write back", port.number()))?;
    write_atomic(p, &image).with_context(|| format!("cannot write card {}", p.display()))
}

fn disasm(model: Model, rom_path: &Path, at: u32, count: usize) -> Result<()> {
    let image = rom::load(model, rom_path)?;
    // Addresses are offsets into the image: banked models (49G, 39G, 40G)
    // show the bank their file offset falls in. An unpacked image holds
    // one nibble per byte.
    let nibbles: Vec<u8> = if image.len() == 2 * model.rom_bytes() {
        image.iter().map(|&b| b & 0xF).collect()
    } else {
        image.iter().flat_map(|&b| [b & 0xF, b >> 4]).collect()
    };
    let fetch = |a: u32| {
        nibbles
            .get((a & ADDR_MASK) as usize % nibbles.len())
            .copied()
            .unwrap_or(0)
    };
    let mut pc = at & ADDR_MASK;
    for _ in 0..count {
        let d = decode(fetch, pc);
        let raw: String = (0..u32::from(d.nibbles()))
            .map(|i| format!("{:X}", fetch(pc.wrapping_add(i))))
            .collect();
        println!("#{pc:05X}  {raw:<21}  {d}");
        pc = pc.wrapping_add(u32::from(d.nibbles())) & ADDR_MASK;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--save` and card write-back through a symlink replace the real
    /// file (the link stays a link) and keep its permissions.
    #[cfg(unix)]
    #[test]
    fn outputs_follow_links_and_keep_permissions() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = control::token::tests::TempDir::new("outputs");
        let real = dir.0.join("real.state");
        std::fs::write(&real, b"old").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o640)).unwrap();
        let link = dir.0.join("link.state");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        write_atomic(&link, b"new").unwrap();
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&real).unwrap(), b"new");
        let mode = std::fs::metadata(&real).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
        // A plain file keeps its mode too.
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        write_atomic(&real, b"newer").unwrap();
        assert_eq!(
            std::fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn serial_bridge_refuses_the_42s_before_loading_anything() {
        let cli = Cli::try_parse_from([
            "saturnus",
            "run",
            "--model",
            "42s",
            "--rom",
            "no-such.rom",
            "--serial",
            "stdio",
        ])
        .unwrap();
        let Cmd::Run(args) = cli.command else {
            panic!("not a run command");
        };
        let e = run(&args).unwrap_err().to_string();
        assert!(e.contains("42S has no serial port"), "{e}");
    }

    #[test]
    fn missing_card_file_becomes_a_zeroed_card_and_writes_back() {
        let dir = std::env::temp_dir().join(format!("saturnus-card-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("card.img");
        // A synthetic ROM is enough: the card path does not run code.
        let mut m = Machine::new(Model::Hp48sx, &vec![0u8; Model::Hp48sx.rom_bytes()]).unwrap();
        insert_card(&mut m, Port::One, &p).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), vec![0u8; NEW_CARD_BYTES]);
        assert!(m.card_image(Port::One).is_some());
        write_card(&m, Port::One, &p).unwrap();
        assert!(write_card(&m, Port::Two, &p).is_err());
        std::fs::write(&p, [0u8; 100]).unwrap();
        assert!(insert_card(&mut m, Port::Two, &p).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
