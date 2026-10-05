# saturnus

Core library of [saturnus](https://github.com/ractive/saturnus), a
headless emulator of the HP Saturn calculators: HP 48SX, 48GX, 49G, 38G,
39G and 40G. It models the Saturn CPU, the bus with its memory-mapped
modules, the display, keyboard, timers and serial port, and the
per-model machine wiring. No I/O and no threads: the crate builds for
`wasm32-unknown-unknown` and has no dependencies.

It is a clean-room implementation, written from public hardware
documentation and observed behaviour, not from the source code of other
emulators. No ROM is included: you supply the ROM image of a calculator
you own (or one HP allows to be downloaded for emulator use).

## Usage

```rust,no_run
use saturnus::io::Key;
use saturnus::{Machine, Model};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rom = std::fs::read("gxrom-r")?;
    let mut m = Machine::new(Model::Hp48gx, &rom)?;

    // Run two seconds of machine time (the 48GX runs at 4 MHz).
    m.run_cycles(8_000_000).map_err(|h| format!("halted: {h:?}"))?;

    // Press and release a key.
    m.key_down(Key::Enter)?;
    m.run_cycles(200_000).map_err(|h| format!("halted: {h:?}"))?;
    m.key_up(Key::Enter)?;

    // The screen as text (one character per pixel)...
    println!("{}", m.lcd().to_text());

    // ...and the serial port, byte by byte.
    m.serial_push(b"\x01");
    let reply: Vec<u8> = m.serial_drain();
    println!("{} bytes from the calculator", reply.len());
    Ok(())
}
```

## Licence

MIT.
