//! `saturnus`: run Saturn calculator ROMs headless, replay key scripts,
//! dump the screen, disassemble ROM code and fetch ROM images.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod rom;
mod serial;
mod sha256;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use saturnus::cpu::{ADDR_MASK, decode, disassemble};
use saturnus::machine::NEW_CARD_BYTES;
use saturnus::{Machine, Model, Port};

use saturnus_drive::session::Session;
use saturnus_drive::{autostart, screen, script};
use serial::{BridgeOptions, SerialSpec};

/// Headless emulator of the HP Saturn calculators (emulates the HP 48SX,
/// 48GX, 49G, 38G, 39G and 40G).
#[derive(Debug, Parser)]
#[command(name = "saturnus", version)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Run a ROM: optional state load, N cycles, a key script, then dump
    /// the screen and optionally save the state.
    Run(Box<RunArgs>),
    /// Disassemble ROM code.
    Disasm {
        /// Calculator model.
        #[arg(long, value_enum, default_value_t = ModelArg::Hp48sx)]
        model: ModelArg,
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
        #[arg(long, value_enum, default_value_t = ModelArg::Hp48sx)]
        model: ModelArg,
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
    #[arg(long, value_enum, default_value_t = ModelArg::Hp48sx)]
    model: ModelArg,
    /// Packed ROM image (two nibbles per byte).
    #[arg(long)]
    rom: PathBuf,
    /// CPU cycles to run before the key script (after `--load`).
    #[arg(long, default_value_t = 0, value_parser = parse_count)]
    cycles: u64,
    /// Key script to replay (see README, "Key scripts").
    #[arg(long)]
    keys: Option<PathBuf>,
    /// Write the final screen: `.txt` (131x64 `#`/`.`) or `.png`.
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
    /// After the key script, bridge the serial port and run paced to
    /// wall-clock time until SIGINT/SIGTERM: `tcp:PORT` (listens on
    /// 127.0.0.1), `tcp:HOST:PORT` or `stdio`.
    #[arg(long, value_parser = SerialSpec::parse)]
    serial: Option<SerialSpec>,
    /// Before bridging, answer the boot prompt with NO (unless `--load`)
    /// and start the Kermit server (ALPHA ALPHA S E R V E R ENTER).
    #[arg(long, requires = "serial")]
    autostart: bool,
    /// Stop after the first serial client disconnects (stdin EOF for
    /// `stdio`), then write `--screen`/`--save` as usual.
    #[arg(long, requires = "serial")]
    exit_on_disconnect: bool,
    /// Append the serial traffic with emulated timestamps to this file.
    #[arg(long, requires = "serial")]
    serial_log: Option<PathBuf>,
    /// Report `wait-idle` timings and serial connections on stderr.
    #[arg(long, short)]
    verbose: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ModelArg {
    /// HP 48SX.
    #[value(name = "48sx")]
    Hp48sx,
    /// HP 48GX.
    #[value(name = "48gx")]
    Hp48gx,
    /// HP 38G.
    #[value(name = "38g")]
    Hp38g,
    /// HP 49G.
    #[value(name = "49g")]
    Hp49g,
    /// HP 39G.
    #[value(name = "39g")]
    Hp39g,
    /// HP 40G (same ROM as the 39G).
    #[value(name = "40g")]
    Hp40g,
}

impl From<ModelArg> for Model {
    fn from(m: ModelArg) -> Self {
        match m {
            ModelArg::Hp48sx => Model::Hp48sx,
            ModelArg::Hp48gx => Model::Hp48gx,
            ModelArg::Hp38g => Model::Hp38g,
            ModelArg::Hp49g => Model::Hp49g,
            ModelArg::Hp39g => Model::Hp39g,
            ModelArg::Hp40g => Model::Hp40g,
        }
    }
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
        Cmd::Disasm {
            model,
            rom,
            at,
            count,
        } => disasm(model.into(), &rom, at, count),
        Cmd::Rom {
            command: RomCmd::Fetch { model, dir, yes },
        } => rom::fetch(model.into(), &dir, yes).map(|_| ()),
    }
}

fn run(args: &RunArgs) -> Result<()> {
    let model: Model = args.model.into();
    // Checked before anything runs, so an unsupported model fails at once.
    let autostart = if args.serial.is_some() && args.autostart {
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
    if let Some(spec) = &args.serial {
        for line in &autostart {
            s.apply(line)?;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        ctrlc::set_handler(move || flag.store(true, Ordering::Relaxed))
            .context("cannot install the SIGINT/SIGTERM handler")?;
        let opts = BridgeOptions {
            spec: spec.clone(),
            exit_on_disconnect: args.exit_on_disconnect,
            log: args.serial_log.as_deref(),
            verbose: args.verbose,
        };
        serial::bridge(&mut s, &opts, &stop)?;
    }
    let fb = s.machine.framebuffer();
    if let Some(p) = &args.screen {
        screen::write(&fb.pixels, p)?;
    }
    if let Some(p) = &args.annunciators {
        std::fs::write(p, format!("{}\n", fb.annunciator_line()))
            .with_context(|| format!("cannot write {}", p.display()))?;
    }
    if let Some(p) = &args.save {
        save_state(&s.machine, p)?;
    }
    if args.card_writeback {
        for (port, path) in cards {
            if let Some(p) = path {
                write_card(&s.machine, port, p)?;
            }
        }
    }
    if args.trace > 0 {
        eprintln!("{}", s.trace_text().trim_start());
    }
    if args.verbose {
        eprintln!("stopped at cycle {}", s.machine.cycles());
    }
    Ok(())
}

fn load_state(m: &mut Machine, p: &Path) -> Result<()> {
    let data = std::fs::read(p).with_context(|| format!("cannot read state {}", p.display()))?;
    m.load_state(&data)
        .with_context(|| format!("cannot load state {}", p.display()))
}

fn save_state(m: &Machine, p: &Path) -> Result<()> {
    std::fs::write(p, m.save_state()).with_context(|| format!("cannot write state {}", p.display()))
}

/// Insert the card image at `p` into `port`; a missing file becomes a
/// zeroed 128 KB card.
fn insert_card(m: &mut Machine, port: Port, p: &Path) -> Result<()> {
    let image = if p.exists() {
        std::fs::read(p).with_context(|| format!("cannot read card {}", p.display()))?
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
    std::fs::write(p, image).with_context(|| format!("cannot write card {}", p.display()))
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
        let raw: String = (0..u32::from(d.len))
            .map(|i| format!("{:X}", fetch(pc.wrapping_add(i))))
            .collect();
        println!("#{pc:05X}  {raw:<21}  {}", disassemble(&d.instr));
        pc = pc.wrapping_add(u32::from(d.len)) & ADDR_MASK;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
