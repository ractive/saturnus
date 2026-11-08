//! Files over XSERV on the emulated 49G (`rom.49g`, ROM 2.15): the device
//! finds XSERV by itself, lists, makes, renames and purges, puts and gets
//! binary files byte for byte (a replace too), goes through the Kermit
//! server nested in XSERV for a text file, the graphics screen and a
//! backup, and comes back to XSERV each time; the disconnect ends XSERV.
//! Gated by `SATURNUS_ROM_DIR` (the test policy).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::print_stderr)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use saturnus_drive::script::parse_keys;
use saturnus_drive::session::Session;
use saturnus_tx::{
    Action, Device, DeviceOptions, Format, Outcome, Request, Server, TreeItem, TxError,
};
use satx_core::transport::{EmulatorModel, SaturnusTransport, Transport};

/// The string "hello" as the 49G's binary transfer file.
const HELLO: &[u8] = b"HPHP49-C\x2c\x2a\xf0\x00\x00hello";
/// The string "world".
const WORLD: &[u8] = b"HPHP49-C\x2c\x2a\xf0\x00\x00world";
const PROGRAM: &[u8] = b"%%HP: T(3)A(R)F(.);\r\n\\<< 6 7 * \\>>\r\n";
const DIR: &str = "SATXXS";

struct Calculator {
    device: Device,
    transport: SaturnusTransport,
    /// XSERV `L` bytes written.
    listings: usize,
}

impl Calculator {
    /// The 49G with its Kermit server ended and XSERV typed.
    fn boot() -> Option<Self> {
        let mut c = Self::at_stack()?;
        c.type_xserv();
        Some(c)
    }

    /// Type `XSERV` and ENTER.
    fn type_xserv(&mut self) {
        self.keys("wait 1000\nalpha alpha x sin e sqrt eex\nalpha\nenter\nwait 1000");
    }

    /// The stack (level 1 first) and whether flag -95 (algebraic mode) is
    /// set, from the RAM.
    fn stack_and_mode(&mut self) -> (Vec<String>, bool) {
        let machine = self.transport.machine();
        let stack = saturnus_objects::ram::stack_objects(machine)
            .unwrap()
            .iter()
            .map(|o| format!("{o:?}"))
            .collect();
        let flags = saturnus_objects::ram::flags(machine).unwrap();
        (stack, flags.get(-95) == Some(true))
    }

    /// The 49G with its Kermit server ended, at its stack.
    fn at_stack() -> Option<Self> {
        let rom = std::env::var_os("SATURNUS_ROM_DIR").map(PathBuf::from)?;
        let rom = rom.join("rom.49g");
        if !rom.is_file() {
            eprintln!("skipped: {} not found", rom.display());
            return None;
        }
        let model = EmulatorModel::from_name("49g").unwrap();
        let mut c = Self {
            device: Device::new(Instant::now(), DeviceOptions::default(), 7),
            transport: SaturnusTransport::open(model, &rom).unwrap(),
            listings: 0,
        };
        // The boot types SERVER: connect to it and end it.
        let kermit = c.run(Request::Connect(Default::default()));
        let Ok(Outcome::Connected(conn)) = kermit else {
            panic!("{kermit:?}");
        };
        assert_eq!(conn.server, Server::Kermit);
        assert!(matches!(
            c.run(Request::Disconnect),
            Ok(Outcome::Disconnected)
        ));
        Some(c)
    }

    fn keys(&mut self, script: &str) {
        let lines = parse_keys(script).unwrap();
        let mut session = Session::new(self.transport.machine().clone(), 0, false);
        session.set_echo_warnings(false);
        for line in &lines {
            session.apply(line).unwrap();
        }
        *self.transport.machine() = session.machine;
    }

    fn run(&mut self, request: Request) -> Result<Outcome, TxError> {
        assert!(self.device.start(Instant::now(), request).is_ok());
        let mut buf = [0u8; 2048];
        loop {
            while let Some(action) = self.device.poll_action() {
                if let Action::Write(bytes) = action {
                    self.listings += usize::from(bytes == b"L");
                    if let Err(e) = self.transport.write_packet(&bytes) {
                        self.device.handle_link_error(Instant::now(), &e);
                    }
                }
            }
            while self.device.poll_event().is_some() {}
            if let Some(result) = self.device.poll_result() {
                return result;
            }
            let wait = self
                .device
                .next_timeout()
                .map_or(Duration::from_millis(100), |d| {
                    d.saturating_duration_since(Instant::now())
                });
            match self
                .transport
                .read(&mut buf, wait.max(Duration::from_millis(1)))
            {
                Ok(0) => self.device.handle_timeout(Instant::now()),
                Ok(n) => self.device.handle_input(Instant::now(), &buf[..n]),
                Err(e) => self.device.handle_link_error(Instant::now(), &e),
            }
        }
    }

    fn list(&mut self, dir: &[&str]) -> Vec<(String, String, f64)> {
        let dir = dir.iter().map(|s| s.to_string()).collect();
        match self.run(Request::List { dir }) {
            Ok(Outcome::Listing(l)) => l
                .entries
                .into_iter()
                .map(|e| (e.name, e.kind, e.size))
                .collect(),
            other => panic!("list: {other:?}"),
        }
    }

    fn names(&mut self, dir: &[&str]) -> Vec<String> {
        self.list(dir).into_iter().map(|(n, _, _)| n).collect()
    }

    fn put(
        &mut self,
        dir: &[&str],
        name: &str,
        data: &[u8],
        replace: bool,
    ) -> Result<Outcome, TxError> {
        self.run(Request::Put {
            dir: dir.iter().map(|s| s.to_string()).collect(),
            name: name.into(),
            data: data.to_vec(),
            replace,
            as_string: false,
        })
    }

    fn get(&mut self, dir: &[&str], name: &str, format: Format) -> Vec<u8> {
        let got = self.run(Request::Get {
            dir: dir.iter().map(|s| s.to_string()).collect(),
            name: name.into(),
            format,
        });
        match got {
            Ok(Outcome::File(file)) => file.data,
            other => panic!("get {name}: {other:?}"),
        }
    }
}

#[test]
fn files_over_xserv() {
    let Some(mut c) = Calculator::boot() else {
        return;
    };
    let t0 = Instant::now();
    let connected = c.run(Request::Connect(Default::default()));
    let Ok(Outcome::Connected(conn)) = connected else {
        panic!("{connected:?}");
    };
    assert_eq!(conn.server, Server::Xserv);
    assert_eq!(conn.settings.baud, 9600);
    eprintln!("connected to XSERV in {:?}", t0.elapsed());

    // HOME, whatever directory the calculator was in; IOPAR sized as the
    // Kermit server sizes it.
    let home = c.list(&["HOME"]);
    let iopar = home.iter().find(|(n, _, _)| n == "IOPAR").expect("IOPAR");
    assert_eq!((iopar.1.as_str(), iopar.2), ("List", 29.5), "{home:?}");

    // A directory, a file in it byte for byte, a refused clash, a replace.
    assert!(matches!(
        c.run(Request::Mkdir {
            dir: vec![],
            name: DIR.into()
        }),
        Ok(Outcome::Created(_))
    ));
    let dir = ["HOME", DIR];
    assert!(c.names(&dir).is_empty());
    assert!(matches!(
        c.put(&dir, "S1", HELLO, false),
        Ok(Outcome::Stored(_))
    ));
    assert_eq!(c.get(&dir, "S1", Format::Binary), HELLO);
    assert!(matches!(
        c.put(&dir, "S1", WORLD, false),
        Err(TxError::NameClash(_))
    ));
    assert!(matches!(
        c.put(&dir, "S1", WORLD, true),
        Ok(Outcome::Stored(_))
    ));
    assert_eq!(c.get(&dir, "S1", Format::Binary), WORLD);
    assert_eq!(c.names(&dir), ["S1"]);
    let missing = c.run(Request::Get {
        dir: dir.iter().map(|s| s.to_string()).collect(),
        name: "NOSUCH".into(),
        format: Format::Binary,
    });
    assert!(matches!(missing, Err(TxError::NotFound(_))), "{missing:?}");

    // Rename, a subdirectory, a walk down and back up.
    assert!(matches!(
        c.run(Request::Rename {
            dir: dir.iter().map(|s| s.to_string()).collect(),
            from: "S1".into(),
            to: "S2".into(),
        }),
        Ok(Outcome::Renamed { .. })
    ));
    assert_eq!(c.names(&dir), ["S2"]);
    let sub = c.run(Request::Mkdir {
        dir: dir.iter().map(|s| s.to_string()).collect(),
        name: "SUBD".into(),
    });
    assert!(matches!(sub, Ok(Outcome::Created(_))), "{sub:?}");
    assert!(matches!(
        c.put(&["HOME", DIR, "SUBD"], "S3", HELLO, false),
        Ok(Outcome::Stored(_))
    ));
    assert_eq!(c.names(&["HOME", DIR, "SUBD"]), ["S3"]);
    assert!(c.names(&["HOME"]).iter().any(|n| n == DIR));

    // Through the Kermit server: a text file in and out, and XSERV after it.
    assert!(matches!(
        c.put(&dir, "PRG", PROGRAM, false),
        Ok(Outcome::Stored(_))
    ));
    let text = c.get(&dir, "PRG", Format::Text);
    let text = String::from_utf8_lossy(&text);
    assert!(text.contains("6 7 *"), "{text}");
    assert!(c.names(&dir).contains(&"PRG".to_string()));

    // A tree got by XSERV, and put back as a new directory. One listing
    // per directory (an `L` sent again to a busy calculator aside), none
    // before each variable.
    let listed = c.listings;
    let tree = match c.run(Request::GetTree {
        dir: dir.iter().map(|s| s.to_string()).collect(),
        format: Format::Binary,
        skip: vec![],
    }) {
        Ok(Outcome::Tree(t)) => t,
        other => panic!("tree: {other:?}"),
    };
    assert_eq!(tree.items.len(), 4, "{:?}", tree.items);
    assert!(c.listings - listed <= 3, "{} listings", c.listings - listed);
    let items: Vec<TreeItem> = tree.items.clone();
    let stored = c.run(Request::PutTree {
        dir: vec![],
        name: Some("SATXXT".into()),
        items,
        replace: false,
        as_string: false,
    });
    assert!(
        matches!(
            stored,
            Ok(Outcome::TreeStored {
                directories: 2,
                variables: 3
            })
        ),
        "{stored:?}"
    );
    assert_eq!(
        c.get(&["HOME", "SATXXT", "SUBD"], "S3", Format::Binary),
        HELLO
    );

    // The graphics screen and the memory.
    assert!(matches!(c.run(Request::Pict), Ok(Outcome::Pict(_))));
    let memory = match c.run(Request::Memory) {
        Ok(Outcome::Memory(m)) => m,
        other => panic!("memory: {other:?}"),
    };
    assert!(memory.free > 100_000.0 && memory.home > 0.0, "{memory:?}");

    // Purges with their counts.
    for (name, count) in [(DIR, 4), ("SATXXT", 4)] {
        let refused = c.run(Request::Purge {
            dir: vec![],
            name: name.into(),
            count: None,
        });
        assert!(
            matches!(refused, Err(TxError::NotEmpty { count: n, .. }) if n == count),
            "{refused:?}"
        );
        let purged = c.run(Request::Purge {
            dir: vec![],
            name: name.into(),
            count: Some(count),
        });
        assert!(matches!(purged, Ok(Outcome::Purged { .. })), "{purged:?}");
    }
    let names = c.names(&["HOME"]);
    assert!(
        !names.iter().any(|n| n == DIR || n == "SATXXT"),
        "{names:?}"
    );

    // A backup through the Kermit server, then XSERV again.
    let backup = match c.run(Request::Backup) {
        Ok(Outcome::Backup(b)) => b,
        other => panic!("backup: {other:?}"),
    };
    assert!(backup.data.starts_with(b"HPHP49-"));
    assert!(c.names(&["HOME"]).iter().any(|n| n == "IOPAR"));

    // Fast transfers are for the Kermit server.
    let fast = c.run(Request::XmodemGet {
        dir: vec![],
        name: "IOPAR".into(),
        wait: satx_core::time::Duration::from_secs(1),
    });
    assert!(matches!(fast, Err(TxError::NotForXmodem(_))), "{fast:?}");

    let quit = c.run(Request::Disconnect);
    assert!(
        matches!(quit, Ok(Outcome::XservQuit { ended: true })),
        "{quit:?}"
    );
    eprintln!("whole session {:?}", t0.elapsed());
}

/// The Kermit server nested in XSERV leaves the stack and algebraic mode
/// alone: the 49G in algebraic mode (flag -95) with three numbers on its
/// stack, XSERV typed, two nested Kermit cycles (a text get, PICT) and the
/// disconnect; and a rename that fails on the calculator (`PICT` takes
/// only a graphics object) leaves nothing on the stack.
#[test]
fn the_nested_kermit_server_leaves_the_stack_and_algebraic_mode() {
    let Some(mut c) = Calculator::at_stack() else {
        return;
    };
    c.keys("1\nenter\n2\nenter\n3\nenter\nwait 500");
    let before = c.stack_and_mode();
    assert!(before.1, "the 49G boots in algebraic mode");
    // In algebraic mode each entry is the result and the line typed.
    assert!(before.0.len() >= 6, "{before:?}");
    c.type_xserv();
    let connected = c.run(Request::Connect(Default::default()));
    assert!(
        matches!(&connected, Ok(Outcome::Connected(conn)) if conn.server == Server::Xserv),
        "{connected:?}"
    );
    // XSERV keeps the user's stack out of sight while it runs (the RAM
    // shows only its own objects): what counts is the stack after it.
    let text = c.get(&["HOME"], "IOPAR", Format::Text);
    assert!(text.starts_with(b"%%HP"), "{text:?}");
    assert!(matches!(c.run(Request::Pict), Ok(Outcome::Pict(_))));
    assert!(c.stack_and_mode().1, "algebraic mode after two nests");

    // A rename the calculator refuses: the recalled object and the name
    // do not stay on the stack (both read after a listing).
    assert!(matches!(
        c.put(&["HOME"], "SATXS", HELLO, false),
        Ok(Outcome::Stored(_))
    ));
    let idle = c.stack_and_mode();
    let refused = c.run(Request::Rename {
        dir: vec![],
        from: "SATXS".into(),
        to: "PICT".into(),
    });
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(c.stack_and_mode(), idle);
    assert!(matches!(
        c.run(Request::Purge {
            dir: vec![],
            name: "SATXS".into(),
            count: None,
        }),
        Ok(Outcome::Purged { .. })
    ));

    let quit = c.run(Request::Disconnect);
    assert!(
        matches!(quit, Ok(Outcome::XservQuit { ended: true })),
        "{quit:?}"
    );
    c.keys("wait 1000");
    // Typing XSERV added its own entry (the line and its result); below
    // it the stack is as it was, not packed into a list, and the 49G is
    // still in algebraic mode.
    let after = c.stack_and_mode();
    assert!(after.1, "algebraic mode after XSERV");
    assert_eq!(after.0.get(2..), Some(&before.0[..]), "{after:?}");
}
