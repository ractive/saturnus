//! `satx`: transfer files between HP Saturn calculators and a computer.
// The argument docs are clap's --help text, where `<UTC time>` is a
// placeholder, not an HTML tag.
#![allow(rustdoc::invalid_html_tags)]

mod commands;
mod convert;
mod error;
mod offline;
mod output;
mod port;
mod repl;
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod testkit;
mod util;
mod xmodem;
mod xserv;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

use crate::output::Format;

const ABOUT: &str =
    "Talk to an HP 48/49 in Kermit server mode: list, get, put, run, repl, pict, backup";

const LONG_ABOUT: &str = "\
Talk to an HP 48SX/GX or 49G over a serial cable (or to an emulator over TCP).

Start the Kermit server on the calculator first: run SERVER (the display
shows \"Awaiting Server Cmd.\"). satx then lists, copies, renames and deletes
variables, runs RPL commands, fetches the graphics screen PICT and backs up
HOME. All
transfers run at 9600 baud. `object`, `grob`, `chars` and `completions` work
on this computer and need no calculator.

Output is text on a terminal and JSON when piped (`--format` overrides):
{\"results\": ..., \"total\": N, \"hints\": [{\"description\", \"cmd\"}]}.
Errors go to stderr (as {\"error\", \"hint\"} in JSON mode) with a non-zero
exit code.";

const EXAMPLES: &str = "\
Examples:
  satx ports                          # find the cable
  satx info                           # model, ROM version, free memory, IOPAR
  satx ls                             # variables in the current directory
  satx ls HOME/GAMES                  # change to HOME/GAMES and list it
  satx get PRG -o prg.hp              # download PRG (binary)
  satx put prg.hp --as PRG2           # upload a file as PRG2
  satx put prg.hp --protocol xmodem   # XRECV typed on a 48G/GX or 49G
  satx run '6 7 *'                    # evaluate RPL, print the stack
  satx repl                           # RPL line by line on one link; :help
  satx pict -o plot.png               # the graphics screen PICT as PNG
  satx backup -o home.hp              # archive HOME to a file
  satx --port tcp://localhost:4848 ls # an emulator
  satx ls --json | jq '.results[].name'
  satx object inspect prg.hp          # type and size of a file (offline)
  satx grob to-png pic.hp             # a GROB file as PNG (offline)
  satx chars                          # trigraphs for → « » √ π ... (offline)
  satx completions zsh > _satx        # shell completions

Port: --port PATH, else SATX_PORT, else the only USB serial port.";

/// Command-line interface.
#[derive(Parser, Debug)]
#[command(
    name = "satx",
    version,
    about = ABOUT,
    long_about = LONG_ABOUT,
    after_help = EXAMPLES,
    max_term_width = 100
)]
pub struct Cli {
    #[command(flatten)]
    pub global: Global,
    #[command(subcommand)]
    pub command: Command,
}

/// Options for every command.
#[derive(Args, Debug, Clone)]
pub struct Global {
    /// Serial device (/dev/ttyUSB0, /dev/cu.usbserial-X, COM3) or tcp://host:port.
    /// Default: the only USB serial port.
    #[arg(
        long,
        short = 'p',
        env = "SATX_PORT",
        hide_env_values = true,
        global = true,
        value_name = "PORT",
        help_heading = "Connection"
    )]
    pub port: Option<String>,
    /// Change to this calculator directory first, from HOME: HOME/GAMES or GAMES.
    /// The calculator stays there afterwards.
    #[arg(long, global = true, value_name = "PATH", help_heading = "Connection")]
    pub dir: Option<String>,
    /// Seconds to wait for each Kermit packet before resending (1-600); a host
    /// command (run, rm, mv, ...) is never resent.
    #[arg(
        long,
        global = true,
        default_value_t = 20,
        value_parser = clap::value_parser!(u64).range(1..=600),
        value_name = "SECS",
        help_heading = "Connection"
    )]
    pub timeout: u64,
    /// Resends per packet before giving up (0-50); host commands get none.
    #[arg(
        long,
        global = true,
        default_value_t = 5,
        value_parser = clap::value_parser!(u32).range(0..=50),
        value_name = "N",
        help_heading = "Connection"
    )]
    pub retries: u32,
    /// Output format [default: text on a terminal, json when piped].
    #[arg(long, global = true, value_enum, help_heading = "Output")]
    pub format: Option<Format>,
    /// Same as --format json.
    #[arg(
        long,
        global = true,
        conflicts_with = "format",
        help_heading = "Output"
    )]
    pub json: bool,
    /// jq filter over the JSON envelope, e.g. '.results[].name' or '.total'.
    #[arg(long, global = true, value_name = "FILTER", help_heading = "Output")]
    pub jq: Option<String>,
}

/// The commands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// List serial ports; USB ones are candidates for auto-pick.
    Ports,
    /// Model, ROM version, free memory, current directory, IOPAR, transfer mode.
    Info,
    /// List a directory: name, size, type, checksum.
    #[command(after_help = "\
PATH is absolute from HOME (HOME/GAMES or GAMES) and makes it the current
directory on the calculator. Without PATH: the current directory.

Examples:
  satx ls
  satx ls HOME/GAMES
  satx ls --json | jq -r '.results[] | select(.type == \"Program\") | .name'")]
    Ls {
        /// Directory to change to and list.
        path: Option<String>,
    },
    /// Download a variable to a file (binary unless --ascii).
    #[command(after_help = "\
Binary files start with HPHP48-x or HPHP49-x and keep the object byte for
byte; ASCII files start with %%HP: and are text. Without -o the file is
NAME.hp (binary) or NAME.txt (--ascii) in the working directory; characters
a file system refuses (/ \\ : * ? \" < > | %, control characters, a trailing
dot or space, the first letter of a Windows device name such as AUX) are
written %XX, one per UTF-8 byte, and put reads them back. Existing files are
kept unless --force.

--protocol xmodem (48G/GX and 49G; the 48S/SX has no XModem): the Kermit
server cannot start XSEND, so satx ends server mode and prints what to type
on the calculator, 'NAME' XSEND then ENTER, and waits --start-timeout
seconds for it. satx cuts the padding of the last block by walking the
object; a file it cannot walk keeps every byte received. Afterwards type
SERVER on the calculator again. A 49G in algebraic mode is switched to RPN
first (-95 SF switches back). --dry-run shows the plan and keeps the server.

Examples:
  satx get PRG                 # writes ./PRG.hp
  satx get 'A?B'               # writes ./A%3FB.hp
  satx get NOTES --ascii       # writes ./NOTES.txt
  satx get PRG -o prg.hp
  satx get PRG -o - | xxd | head
  satx get PRG -o prg.hp --protocol xmodem")]
    Get(commands::GetArgs),
    /// Upload a file as a variable (binary; ASCII for %%HP: text files).
    #[command(after_help = "\
The name defaults to the file name without extension, %XX codes read back
as get writes them (A%3FB.hp stores A?B). A file without an
HPHP48/HPHP49 header is stored as a String in binary mode. A %%HP: text file
goes in ASCII mode unless --binary. If the name exists, put refuses unless
--overwrite, which sends the file as SATXPT first and only then replaces the
old variable (a failed transfer leaves it alone); --dry-run shows what would
happen.

--protocol xmodem (48G/GX and 49G; the 48S/SX has no XModem): the Kermit
server cannot start XRECV, so satx ends server mode and prints what to type
on the calculator, 'NAME' XRECV then ENTER, and waits --start-timeout
seconds for it. The file goes byte for byte (no --ascii, --binary). XRECV
never replaces a variable: the 49G stores the object as NAME.1 and the
48G/GX refuses, so satx refuses an existing NAME (no --overwrite). A failed
transfer on a 48G/GX can leave NAME holding an empty string. Afterwards type
SERVER on the calculator again. A 49G in algebraic mode is switched to RPN
first (-95 SF switches back). --dry-run shows the plan and keeps the server.

Examples:
  satx put PRG.hp              # stores PRG
  satx put prg.hp --as PRG2
  satx put prg.hp --overwrite --dry-run
  satx put prg.hp --protocol xmodem --start-timeout 120")]
    Put(commands::PutArgs),
    /// Delete variables; directories go with their contents.
    #[command(after_help = "\
Every name is checked in the listing first; nothing is deleted if one is
missing.

Examples:
  satx rm OLD TMP --dry-run
  satx rm OLD TMP")]
    Rm {
        /// Variables to delete.
        #[arg(required = true, value_name = "NAME")]
        names: Vec<String>,
        /// Show what would be deleted, delete nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Create a directory.
    Mkdir {
        /// Name of the new directory.
        name: String,
    },
    /// Rename a variable or directory (TO must not exist).
    #[command(after_help = "\
Examples:
  satx mv PRG OLDPRG
  satx --dir HOME/GAMES mv TETRIS TET")]
    Mv {
        /// Current name.
        from: String,
        /// New name.
        to: String,
    },
    /// Run RPL on the calculator and print the stack.
    #[command(after_help = "\
The words are joined with spaces and sent as one host command; quote RPL
that your shell would expand. Results stay on the calculator's stack. satx
prints the stack as the calculator displays it: the 49G truncates long
values and shows lists with commas. For exact values store the result and
`satx get` it. Unicode (→, «, ») and ASCII trigraphs (\\->, \\<<, \\>>) both
work. One command must fit in one Kermit packet (77 encoded bytes).

Trigraphs: → \\->  « \\<<  » \\>>  √ \\v/  π \\pi  Σ \\GS  ∫ \\.S  ∂ \\.d  ∞ \\oo
           ≤ \\<=  ≥ \\>=  ≠ \\=/   `satx chars` lists all of them.

Examples:
  satx run '6 7 *'
  satx run DROP
  satx run -35 SF              # negative numbers are RPL, not options
  satx run \"{ 1 2 3 } 'L' STO\"")]
    Run {
        /// RPL words. A word that starts with - must be a number (-35) or come after --.
        #[arg(required = true, value_name = "RPL", allow_negative_numbers = true)]
        words: Vec<String>,
    },
    /// Type RPL line by line on one link and see the stack; :help for commands.
    #[command(after_help = repl::AFTER_HELP)]
    Repl,
    /// Save the graphics screen PICT (plots, drawings) as a PNG.
    #[command(after_help = "\
PICT is the calculator's graphics screen: what PLOT, DRAW, PIXON or LINE
drew. It persists while the calculator serves, so a plot made before SERVER
can be fetched. satx recalls PICT into SATXTMP, sends it and deletes it; if
a variable SATXTMP already exists, pict stops and changes nothing. A PICT
nothing was ever drawn into is empty (0x0) and gives an error.

The display itself cannot be captured over the link: in server mode it only
shows the server's banner. Capture it by hand: type LCD\u{2192} 'S' STO on the
calculator before SERVER, then `satx get S` and `satx grob to-png S`.

Examples:
  satx pict                       # satx-pict-<UTC time>.png
  satx pict -o plot.png --force
  satx pict -o - > plot.png
  satx run 'ERASE { # 10d # 10d } PIXON' && satx pict -o dot.png")]
    Pict {
        /// Output file [default: satx-pict-<UTC time>.png]; - for stdout.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Replace an existing file.
        #[arg(long)]
        force: bool,
    },
    /// Archive HOME into a file (via :0:SATXBK; needs free memory for a copy).
    #[command(after_help = "\
The backup is a binary Directory object (HPHP48-x / HPHP49-x header) that
`satx restore` puts back. ARCHIVE goes through port 0, so the calculator
needs free memory for one copy of HOME. Stop the clock display first: a
ticking clock can corrupt the archive.")]
    Backup {
        /// Output file [default: satx-backup-<UTC time>.hp].
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Replace an existing file.
        #[arg(long)]
        force: bool,
    },
    /// Replace HOME with a backup; the calculator warm-starts.
    #[command(after_help = "\
restore REPLACES EVERYTHING IN HOME. It asks first unless --yes.

satx uploads the backup as SATXRS, copies it to port 0 and runs
:0:SATXRS RESTORE. The calculator warm-starts and leaves server mode, and
:0:SATXRS stays in port 0. Run SERVER on the calculator again; the next satx
command on the same port from this computer deletes :0:SATXRS, or run
`satx restore --cleanup`.

Examples:
  satx restore home.hp --dry-run
  satx restore home.hp
  satx restore --cleanup")]
    Restore(commands::RestoreArgs),
    /// Show or change IOPAR and the transfer mode (flag -35).
    #[command(after_help = "\
Without options: show. IOPAR changes are stored in HOME and take effect when
SERVER starts again. satx always talks at 9600 baud. get and put set the
transfer mode themselves; --mode matters for the calculator's own SEND/RECV.

Examples:
  satx settings
  satx settings --checksum 3 --mode ascii")]
    Settings(commands::SettingsArgs),
    /// End server mode on the calculator (Kermit FINISH).
    Finish,
    /// XSERV commands for a 49g+/50g running XSERV (UNVERIFIED on hardware).
    #[command(after_help = "\
UNVERIFIED ON HARDWARE: the XSERV framing follows HP's own client code as
the wiki describes it; no calculator has answered satx yet (the emulated 49G
has no XSERV). Reports welcome.

Start XSERV on the calculator instead of SERVER. --dir is sent as an E
command (HOME A B) first. get and put run XModem with HP's CRC and 1k blocks;
--timeout is the wait for each answer.

Examples:
  satx xserv ls
  satx xserv get PRG -o prg.hp
  satx xserv put prg.hp --as PRG2
  satx xserv eval 'HOME GAMES' --dry-run   # the bytes it would send
  satx xserv mem")]
    Xserv {
        #[command(subcommand)]
        command: xserv::XservCmd,
    },
    /// Inspect or convert object files on this computer (no calculator needed).
    Object {
        #[command(subcommand)]
        command: offline::ObjectCommand,
    },
    /// Turn GROB files into PNG images (no calculator needed).
    Grob {
        #[command(subcommand)]
        command: offline::GrobCommand,
    },
    /// List the HP characters (→ « √ π ...) and the ASCII trigraphs that type them.
    #[command(after_help = "\
The characters 128-255 that have a mnemonic trigraph, with code, character
and trigraph; the other codes 160-255 are \\nnn (three decimal digits).
Trigraphs work in `satx run`, REPL lines (:chars lists them there) and %%HP:
text files: `satx run '\\<< 2 \\v/ \\>>'` sends « 2 √ ». Unicode works too. Needs
no calculator.

Examples:
  satx chars
  satx chars --json | jq -r '.results[] | select(.char == \"π\") | .trigraph'")]
    Chars,
    /// Print a shell completion script.
    #[command(after_help = "\
Prints the script to stdout. Install it where your shell looks:
  bash:        satx completions bash > ~/.local/share/bash-completion/completions/satx
  zsh:         satx completions zsh > ~/.local/share/zsh/site-functions/_satx
  fish:        satx completions fish > ~/.config/fish/completions/satx.fish
  powershell:  satx completions powershell >> $PROFILE")]
    Completions {
        /// Target shell.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

fn main() -> ExitCode {
    commands::main_entry()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn global_options_after_the_command() {
        let cli = Cli::try_parse_from(["satx", "ls", "--json", "--port", "tcp://h:1"]).unwrap();
        assert!(cli.global.json);
        assert_eq!(cli.global.port.as_deref(), Some("tcp://h:1"));
        assert!(matches!(cli.command, Command::Ls { path: None }));
    }

    #[test]
    fn run_takes_rpl_with_hyphens() {
        let cli = Cli::try_parse_from(["satx", "run", "-35", "SF", "--json"]).unwrap();
        assert!(cli.global.json);
        let Command::Run { words } = cli.command else {
            panic!("not run")
        };
        assert_eq!(words, ["-35", "SF"]);
        let cli = Cli::try_parse_from(["satx", "run", "--", "-1", "--x"]).unwrap();
        let Command::Run { words } = cli.command else {
            panic!("not run")
        };
        assert_eq!(words, ["-1", "--x"]);
    }

    #[test]
    fn timeout_and_retries_are_bounded() {
        for t in ["0", "601", "18446744073709551615"] {
            assert!(
                Cli::try_parse_from(["satx", "ls", "--timeout", t]).is_err(),
                "--timeout {t}"
            );
        }
        for r in ["51", "4294967295"] {
            assert!(
                Cli::try_parse_from(["satx", "ls", "--retries", r]).is_err(),
                "--retries {r}"
            );
        }
        let cli =
            Cli::try_parse_from(["satx", "ls", "--timeout", "600", "--retries", "0"]).unwrap();
        assert_eq!((cli.global.timeout, cli.global.retries), (600, 0));
        let cli = Cli::try_parse_from(["satx", "ls", "--timeout", "1", "--retries", "50"]).unwrap();
        assert_eq!((cli.global.timeout, cli.global.retries), (1, 50));
    }

    #[test]
    fn format_and_json_conflict() {
        assert!(Cli::try_parse_from(["satx", "ls", "--json", "--format", "text"]).is_err());
    }

    #[test]
    fn protocol_and_start_timeout() {
        let cli = Cli::try_parse_from(["satx", "put", "f.hp", "--protocol", "xmodem"]).unwrap();
        let Command::Put(args) = cli.command else {
            panic!("not put")
        };
        assert_eq!(args.xmodem.protocol, commands::Protocol::Xmodem);
        assert_eq!(args.xmodem.start_timeout().as_secs(), 60);
        let cli = Cli::try_parse_from(["satx", "get", "X"]).unwrap();
        let Command::Get(args) = cli.command else {
            panic!("not get")
        };
        assert_eq!(args.xmodem.protocol, commands::Protocol::Kermit);
        for t in ["0", "601"] {
            assert!(
                Cli::try_parse_from(["satx", "get", "X", "--start-timeout", t]).is_err(),
                "--start-timeout {t}"
            );
        }
        let cli = Cli::try_parse_from(["satx", "get", "X", "--start-timeout", "600"]).unwrap();
        let Command::Get(args) = cli.command else {
            panic!("not get")
        };
        // Accepted by the parser, refused for Kermit when run.
        assert!(args.xmodem.check(&[]).is_err());
        let x = commands::XmodemArgs {
            protocol: commands::Protocol::Xmodem,
            start_timeout: None,
        };
        assert!(x.check(&[(false, "--ascii")]).is_ok());
        let err = x.check(&[(true, "--overwrite")]).unwrap_err();
        assert!(err.to_string().contains("--overwrite"));
    }

    #[test]
    fn xserv_subcommands_parse() {
        let cli = Cli::try_parse_from(["satx", "xserv", "eval", "-35", "SF"]).unwrap();
        let Command::Xserv {
            command: xserv::XservCmd::Eval { words, dry_run },
        } = cli.command
        else {
            panic!("not xserv eval")
        };
        assert_eq!((words, dry_run), (vec!["-35".into(), "SF".into()], false));
        assert!(Cli::try_parse_from(["satx", "xserv", "ls"]).is_ok());
        assert!(Cli::try_parse_from(["satx", "xserv", "get", "X", "-o", "x.hp"]).is_ok());
        assert!(Cli::try_parse_from(["satx", "xserv", "put", "x.hp", "--as", "X"]).is_ok());
        assert!(Cli::try_parse_from(["satx", "xserv", "mem"]).is_ok());
    }

    #[test]
    fn repl_parses_with_global_options() {
        let cli =
            Cli::try_parse_from(["satx", "repl", "--port", "tcp://h:1", "--timeout", "3"]).unwrap();
        assert!(matches!(cli.command, Command::Repl));
        assert_eq!(cli.global.timeout, 3);
        assert!(Cli::try_parse_from(["satx", "repl", "extra"]).is_err());
    }

    #[test]
    fn chars_parses_and_is_in_the_help() {
        let cli = Cli::try_parse_from(["satx", "chars", "--json"]).unwrap();
        assert!(matches!(cli.command, Command::Chars));
        assert!(Cli::try_parse_from(["satx", "chars", "x"]).is_err());
        let mut cmd = Cli::command();
        let run = cmd.find_subcommand_mut("run").unwrap().render_long_help();
        assert!(run.to_string().contains("`satx chars` lists all"));
        assert!(EXAMPLES.contains("satx chars"));
    }

    #[test]
    fn restore_needs_a_file_or_cleanup() {
        assert!(Cli::try_parse_from(["satx", "restore"]).is_err());
        assert!(Cli::try_parse_from(["satx", "restore", "--cleanup"]).is_ok());
        assert!(Cli::try_parse_from(["satx", "restore", "f.hp", "--cleanup"]).is_err());
    }
}
