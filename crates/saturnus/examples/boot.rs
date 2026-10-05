//! Bring-up tool: boot a ROM image headless and report the machine state.
//!
//! ```text
//! cargo run --release -p saturnus --example boot -- <rom-file>
//!     [--cycles N] [--keys "name@cycle name ..."] [--trace N]
//!     [--watch-pc HEX] [--screen]
//! ```
//!
//! `--keys`: each item presses a key at an absolute machine cycle
//! (`name@cycle`) or 2,000,000 cycles after the previous item (`name`),
//! holds it for 200,000 cycles and releases it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;

use saturnus::cpu::{ADDR_MASK, Decoded, decode, disassemble};
use saturnus::io::Key;
use saturnus::{Machine, Model};

const DEFAULT_CYCLES: u64 = 20_000_000;
const KEY_GAP: u64 = 2_000_000;
const KEY_HOLD: u64 = 200_000;
const WATCH_PRINT_LIMIT: u64 = 20;

#[derive(Debug, Default)]
struct Args {
    rom: String,
    cycles: u64,
    keys: Vec<(u64, Key, bool)>,
    trace: usize,
    watch_pc: Option<u32>,
    screen: bool,
}

fn usage() -> String {
    "usage: boot <rom-file> [--cycles N] [--keys \"seq\"] [--trace N] [--watch-pc HEX] [--screen]"
        .to_string()
}

fn parse_keys(seq: &str) -> Result<Vec<(u64, Key, bool)>, String> {
    let mut events = Vec::new();
    let mut prev = 0u64;
    for item in seq.split_whitespace() {
        let (name, at) = match item.split_once('@') {
            Some((n, c)) => (
                n,
                c.replace('_', "")
                    .parse::<u64>()
                    .map_err(|e| format!("bad cycle in {item:?}: {e}"))?,
            ),
            None => (item, prev + KEY_GAP),
        };
        let key = Key::from_name(name).ok_or_else(|| format!("unknown key {name:?}"))?;
        events.push((at, key, true));
        events.push((at + KEY_HOLD, key, false));
        prev = at;
    }
    events.sort_by_key(|&(at, _, down)| (at, down));
    Ok(events)
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        cycles: DEFAULT_CYCLES,
        ..Args::default()
    };
    let mut it = std::env::args().skip(1);
    let mut rom = None;
    while let Some(a) = it.next() {
        let mut value = |flag: &str| it.next().ok_or_else(|| format!("{flag} needs a value"));
        match a.as_str() {
            "--cycles" => {
                args.cycles = value("--cycles")?
                    .replace('_', "")
                    .parse()
                    .map_err(|e| format!("--cycles: {e}"))?;
            }
            "--keys" => args.keys = parse_keys(&value("--keys")?)?,
            "--trace" => {
                args.trace = value("--trace")?
                    .parse()
                    .map_err(|e| format!("--trace: {e}"))?;
            }
            "--watch-pc" => {
                let v = value("--watch-pc")?;
                let v = v.trim_start_matches('#').trim_start_matches("0x");
                args.watch_pc =
                    Some(u32::from_str_radix(v, 16).map_err(|e| format!("--watch-pc: {e}"))?);
            }
            "--screen" => args.screen = true,
            "-h" | "--help" => return Err(usage()),
            _ if a.starts_with("--") => return Err(format!("unknown option {a}\n{}", usage())),
            _ => rom = Some(a),
        }
    }
    args.rom = rom.ok_or_else(usage)?;
    Ok(args)
}

fn print_trace(ring: &VecDeque<(u32, Decoded)>) {
    if ring.is_empty() {
        return;
    }
    println!("--- last {} instructions ---", ring.len());
    for (pc, d) in ring {
        println!("#{pc:05X}  {}", disassemble(&d.instr));
    }
}

fn print_state(m: &Machine) {
    let r = &m.cpu.regs;
    let t = &m.hw.io.timers;
    println!("pc                 #{:05X}", r.pc);
    println!("cycles             {}", m.cycles());
    println!("shutdown           {}", m.is_shutdown());
    println!("in_interrupt       {}", r.in_interrupt);
    println!("interrupts_enabled {}", r.interrupts_enabled);
    println!("memory controller  {:?}", m.hw.mc);
    println!(
        "TIMER1             #{:X} ctrl #{:X}",
        t.t1,
        t.read_t1_ctrl()
    );
    println!(
        "TIMER2             #{:08X} ctrl #{:X}",
        t.t2,
        t.read_t2_ctrl()
    );
    println!("OUT                #{:03X}", m.hw.out());
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let rom =
        std::fs::read(&args.rom).unwrap_or_else(|e| panic!("cannot read ROM {}: {e}", args.rom));
    // The model follows from the ROM size.
    let model = Model::ALL
        .into_iter()
        .find(|m| m.rom_bytes() == rom.len())
        .unwrap_or(Model::Hp48sx);
    let mut m = Machine::new(model, &rom).unwrap_or_else(|e| panic!("{e}"));

    let mut events: VecDeque<_> = args.keys.iter().copied().collect();
    let mut ring: VecDeque<(u32, Decoded)> = VecDeque::with_capacity(args.trace);
    let mut watch_hits = 0u64;
    let mut halted = None;

    while m.cycles() < args.cycles {
        while let Some(&(at, key, down)) = events.front() {
            if at > m.cycles() {
                break;
            }
            if down {
                m.key_down(key);
            } else {
                m.key_up(key);
            }
            events.pop_front();
        }
        if !m.is_shutdown() {
            let pc = m.cpu.regs.pc;
            if args.watch_pc == Some(pc) {
                watch_hits += 1;
                if watch_hits <= WATCH_PRINT_LIMIT {
                    println!("watch #{pc:05X} hit {watch_hits} at cycle {}", m.cycles());
                }
            }
            if args.trace > 0 {
                if ring.len() == args.trace {
                    ring.pop_front();
                }
                ring.push_back((pc, decode(|a| m.peek(a & ADDR_MASK), pc)));
            }
        }
        // While shut down, sleep only up to the next key event so a press
        // is not skipped over together with its release.
        let result = if m.is_shutdown() {
            let until = events.front().map_or(args.cycles, |e| e.0.min(args.cycles));
            m.run_cycles(until.saturating_sub(m.cycles()).max(1))
        } else {
            m.step().map(|_| ())
        };
        if let Err(h) = result {
            halted = Some(h);
            break;
        }
    }

    if let Some(h) = halted {
        println!("HALT: {h}");
    }
    if args.watch_pc.is_some() {
        println!("watch hits         {watch_hits}");
    }
    print_trace(&ring);
    print_state(&m);
    if args.screen {
        print!("{}", m.lcd().to_text());
    }
}
