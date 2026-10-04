//! `satx`: transfer files between HP Saturn calculators and a computer.

use clap::Parser;

/// Transfer files between HP Saturn calculators and a computer over serial.
#[derive(Parser)]
#[command(name = "satx", version, about)]
struct Cli {}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    Ok(())
}
