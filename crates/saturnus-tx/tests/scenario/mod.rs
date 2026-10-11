//! The session `just tx-traces` records from saturnus and the trace tests
//! replay: one script for both, so a replay asks for exactly what was
//! recorded and checks the same results.

use saturnus_tx::names::file_name;
use saturnus_tx::{Entry, Format, Model, Outcome, Request, TreeItem, TxError};

/// The models recorded, with saturnus's model name and the ROM file in
/// `SATURNUS_ROM_DIR`.
pub const MODELS: [(&str, &str); 3] =
    [("48sx", "sxrom-j"), ("48gx", "gxrom-r"), ("49g", "rom.49g")];

/// The seed of every recorded device (the sync markers).
pub const SEED: u64 = 0x5a7_0037;

/// A program in the calculator's text form (translation mode 3: `\<<` is
/// `«`).
pub const PROGRAM: &[u8] = b"%%HP: T(3)A(R)F(.);\r\n\\<< 1 2 + \\>>\r\n";
/// A file that is no calculator object.
pub const NOTE: &[u8] = b"hello, calculator";
/// A name outside ASCII.
pub const ARROW: &str = "A\u{2192}B";

fn model(name: &str) -> Model {
    match name {
        "48sx" => Model::Hp48Sx,
        "48gx" => Model::Hp48Gx,
        _ => Model::Hp49G,
    }
}

/// The driver runs one request to its end; the label names it in the
/// trace (`# request LABEL`).
pub type Driver<'a> = dyn FnMut(&str, Request) -> Result<Outcome, TxError> + 'a;

fn put(name: &str, data: &[u8], replace: bool, as_string: bool) -> Request {
    Request::Put {
        dir: vec!["HOME".into()],
        name: name.into(),
        data: data.to_vec(),
        replace,
        as_string,
    }
}

fn get(name: &str, format: Format) -> Request {
    Request::Get {
        dir: vec!["HOME".into()],
        name: name.into(),
        format,
    }
}

fn stored(outcome: Result<Outcome, TxError>, name: &str) -> Result<(), String> {
    match outcome {
        Ok(Outcome::Stored(n)) if n == name => Ok(()),
        other => Err(format!("put {name}: {other:?}")),
    }
}

fn file(outcome: Result<Outcome, TxError>, name: &str, format: Format) -> Result<Vec<u8>, String> {
    match outcome {
        Ok(Outcome::File(f)) if f.name == name && f.format == format => {
            if f.file_name != file_name(name, format) {
                return Err(format!("get {name}: file name {}", f.file_name));
            }
            Ok(f.data)
        }
        other => Err(format!("get {name}: {other:?}")),
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// The recorded session on the calculator `model_name` (fresh memory).
pub fn run(model_name: &str, driver: &mut Driver<'_>) -> Result<(), String> {
    let header: &[u8] = if model_name == "49g" {
        b"HPHP49-"
    } else {
        b"HPHP48-"
    };
    match driver("connect", Request::Connect(Default::default())) {
        Ok(Outcome::Connected(c)) => {
            if c.model != model(model_name) {
                return Err(format!("connect: model {:?}", c.model));
            }
            if c.settings != saturnus_tx::LinkSettings::DEFAULT {
                return Err(format!("connect: settings {}", c.settings));
            }
        }
        other => return Err(format!("connect: {other:?}")),
    }
    match driver("list HOME", Request::List { dir: vec![] }) {
        Ok(Outcome::Listing(l)) if l.path.as_deref() == Some(&["HOME".to_string()][..]) => {}
        other => return Err(format!("list: {other:?}")),
    }
    stored(
        driver("put PRG text", put("PRG", PROGRAM, false, false)),
        "PRG",
    )?;
    stored(
        driver("put NOTE string", put("NOTE", NOTE, false, true)),
        "NOTE",
    )?;
    stored(
        driver("put A→B string", put(ARROW, NOTE, false, true)),
        ARROW,
    )?;

    let prg = file(
        driver("get PRG binary", get("PRG", Format::Binary)),
        "PRG",
        Format::Binary,
    )?;
    if !prg.starts_with(header) {
        return Err(format!("get PRG: header {:?}", &prg[..prg.len().min(8)]));
    }
    stored(
        driver("put PRG2 binary", put("PRG2", &prg, false, false)),
        "PRG2",
    )?;
    let text = file(
        driver("get PRG2 text", get("PRG2", Format::Text)),
        "PRG2",
        Format::Text,
    )?;
    if !text.starts_with(b"%%HP:") || !contains(&text, b"1 2 +") {
        return Err(format!("get PRG2 text: {}", String::from_utf8_lossy(&text)));
    }
    let note = file(
        driver("get NOTE binary", get("NOTE", Format::Binary)),
        "NOTE",
        Format::Binary,
    )?;
    if !note.starts_with(header) || !contains(&note, NOTE) {
        return Err(format!("get NOTE: {note:?}"));
    }
    file(
        driver("get A→B binary", get(ARROW, Format::Binary)),
        ARROW,
        Format::Binary,
    )?;

    match driver("put PRG clash", put("PRG", PROGRAM, false, false)) {
        Err(TxError::NameClash(n)) if n == "PRG" => {}
        other => return Err(format!("put PRG again: {other:?}")),
    }
    stored(
        driver("put PRG replace", put("PRG", &prg, true, false)),
        "PRG",
    )?;
    match driver("list HOME again", Request::List { dir: vec![] }) {
        Ok(Outcome::Listing(l)) => {
            for name in ["PRG", "PRG2", "NOTE", ARROW] {
                if !l.entries.iter().any(|e| e.name == name) {
                    return Err(format!("list again: no {name} in {:?}", l.entries));
                }
            }
            // BYTES counts the name too; the checksum is the object's.
            let checksum = |n: &str| l.entries.iter().find(|e| e.name == n).map(|e| e.checksum);
            if checksum("PRG") != checksum("PRG2") || checksum("NOTE") != checksum(ARROW) {
                return Err(format!("list again: PRG and PRG2 differ: {:?}", l.entries));
            }
        }
        other => return Err(format!("list again: {other:?}")),
    }
    match driver("disconnect", Request::Disconnect) {
        Ok(Outcome::Disconnected) => Ok(()),
        other => Err(format!("disconnect: {other:?}")),
    }
}

/// Where the ops session runs: requests, and the two things done on the
/// calculator itself.
pub trait Env {
    /// Run one request to its end; the label names it in the trace.
    fn run(&mut self, label: &str, request: Request) -> Result<Outcome, TxError>;
    /// A freshly booted calculator of the same model in server mode, and a
    /// new device for it (`# fresh calculator` in the trace).
    fn fresh(&mut self);
    /// `SERVER` typed on the calculator, after the restore.
    fn server(&mut self);
}

fn home() -> Vec<String> {
    vec!["HOME".into()]
}

fn item(path: &[&str], data: Option<&[u8]>) -> TreeItem {
    TreeItem {
        path: path.iter().map(|s| s.to_string()).collect(),
        data: data.map(<[u8]>::to_vec),
    }
}

fn tree_of(outcome: Result<Outcome, TxError>, what: &str) -> Result<Vec<TreeItem>, String> {
    match outcome {
        Ok(Outcome::Tree(t)) => Ok(t.items),
        other => Err(format!("{what}: {other:?}")),
    }
}

fn listing(outcome: Result<Outcome, TxError>, what: &str) -> Result<Vec<Entry>, String> {
    match outcome {
        Ok(Outcome::Listing(l)) => Ok(l.entries),
        other => Err(format!("{what}: {other:?}")),
    }
}

/// The names of a tree's items, `/` between the parts, `/` after a
/// directory.
pub fn tree_names(items: &[TreeItem]) -> Vec<String> {
    items
        .iter()
        .map(|i| {
            let mut p = i.path.join("/");
            if i.is_directory() {
                p.push('/');
            }
            p
        })
        .collect()
}

/// The ops session on the calculator `model_name` (fresh memory):
/// [`ops_dirs`], then [`ops_backup`].
pub fn ops(model_name: &str, env: &mut dyn Env) -> Result<(), String> {
    let d2 = ops_dirs(model_name, env)?;
    ops_backup(model_name, env, &d2)
}

/// The label of the first request of [`ops_backup`].
pub const BACKUP_PART: &str = "request memory";

fn header(model_name: &str) -> &'static [u8] {
    if model_name == "49g" {
        b"HPHP49-"
    } else {
        b"HPHP48-"
    }
}

/// Make, rename, purge with the count, a directory got, purged, put back
/// and got again byte for byte, a tree put into HOME refused and
/// replaced, PICT. Returns the tree of D2, the directory left.
pub fn ops_dirs(model_name: &str, env: &mut dyn Env) -> Result<Vec<TreeItem>, String> {
    let header = header(model_name);
    let connected = |o: Result<Outcome, TxError>, what: &str| match o {
        Ok(Outcome::Connected(c)) if c.model == model(model_name) => Ok(()),
        other => Err(format!("{what}: {other:?}")),
    };
    connected(
        env.run("connect", Request::Connect(Default::default())),
        "connect",
    )?;
    let mkdir = |name: &str| Request::Mkdir {
        dir: home(),
        name: name.into(),
    };
    match env.run("mkdir EMPTY", mkdir("EMPTY")) {
        Ok(Outcome::Created(n)) if n == "EMPTY" => {}
        other => return Err(format!("mkdir: {other:?}")),
    }
    match env.run("mkdir EMPTY clash", mkdir("EMPTY")) {
        Err(TxError::NameClash(n)) if n == "EMPTY" => {}
        other => return Err(format!("mkdir again: {other:?}")),
    }

    let first = vec![
        item(&["PRG"], Some(PROGRAM)),
        item(&["NOTE"], Some(NOTE)),
        item(&["INNER"], None),
        item(&["INNER", ARROW], Some(NOTE)),
    ];
    let put_tree = |name: &str, items: Vec<TreeItem>| Request::PutTree {
        dir: home(),
        name: Some(name.into()),
        items,
        replace: false,
        as_string: true,
    };
    match env.run("put-tree D1", put_tree("D1", first)) {
        Ok(Outcome::TreeStored {
            directories: 2,
            variables: 3,
        }) => {}
        other => return Err(format!("put-tree D1: {other:?}")),
    }
    let get_tree = || Request::GetTree {
        dir: vec!["HOME".into(), "D1".into()],
        format: Format::Binary,
        skip: Vec::new(),
    };
    let got = tree_of(env.run("get-tree D1", get_tree()), "get-tree D1")?;
    let names = tree_names(&got);
    if names != ["PRG", "NOTE", "INNER/", &format!("INNER/{ARROW}")] {
        return Err(format!("get-tree D1: {names:?}"));
    }
    for i in got.iter().filter_map(|i| i.data.as_deref()) {
        if !i.starts_with(header) {
            return Err(format!("get-tree D1: header {:?}", &i[..i.len().min(8)]));
        }
    }

    let purge = |name: &str, count: Option<usize>| Request::Purge {
        dir: home(),
        name: name.into(),
        count,
    };
    match env.run("purge D1", purge("D1", None)) {
        Err(TxError::NotEmpty { name, count: 4 }) if name == "D1" => {}
        other => return Err(format!("purge D1: {other:?}")),
    }
    match env.run("purge D1 count", purge("D1", Some(4))) {
        Ok(Outcome::Purged { count: 4, .. }) => {}
        other => return Err(format!("purge D1 with its count: {other:?}")),
    }
    match env.run("put-tree D1 again", put_tree("D1", got.clone())) {
        Ok(Outcome::TreeStored {
            directories: 2,
            variables: 3,
        }) => {}
        other => return Err(format!("put-tree D1 again: {other:?}")),
    }
    let again = tree_of(env.run("get-tree D1 again", get_tree()), "get-tree again")?;
    if again != got {
        return Err(format!(
            "get-tree D1 again: not the same bytes: {:?}",
            tree_names(&again)
        ));
    }

    let rename = |from: &str, to: &str| Request::Rename {
        dir: home(),
        from: from.into(),
        to: to.into(),
    };
    match env.run("rename D1 D2", rename("D1", "D2")) {
        Ok(Outcome::Renamed { to, .. }) if to == "D2" => {}
        other => return Err(format!("rename: {other:?}")),
    }
    match env.run("rename EMPTY clash", rename("EMPTY", "D2")) {
        Err(TxError::NameClash(n)) if n == "D2" => {}
        other => return Err(format!("rename to a name in use: {other:?}")),
    }
    match env.run("rename missing", rename("NOPE", "X")) {
        Err(TxError::NotFound(n)) if n == "NOPE" => {}
        other => return Err(format!("rename a missing name: {other:?}")),
    }
    match env.run("purge EMPTY", purge("EMPTY", None)) {
        Ok(Outcome::Purged { count: 0, .. }) => {}
        other => return Err(format!("purge EMPTY: {other:?}")),
    }

    // Into HOME itself: D2 exists and is entered, D2/PRG is in use.
    let into_home = |replace: bool| Request::PutTree {
        dir: home(),
        name: None,
        items: vec![item(&["D2"], None), item(&["D2", "PRG"], Some(PROGRAM))],
        replace,
        as_string: false,
    };
    match env.run("put-tree HOME clash", into_home(false)) {
        Err(TxError::NamesInUse(names)) if names == ["D2/PRG"] => {}
        other => return Err(format!("put-tree into HOME: {other:?}")),
    }
    match env.run("put-tree HOME replace", into_home(true)) {
        Ok(Outcome::TreeStored {
            directories: 0,
            variables: 1,
        }) => {}
        other => return Err(format!("put-tree into HOME, replacing: {other:?}")),
    }
    let d2 = tree_of(
        env.run(
            "get-tree D2",
            Request::GetTree {
                dir: vec!["HOME".into(), "D2".into()],
                format: Format::Binary,
                skip: Vec::new(),
            },
        ),
        "get-tree D2",
    )?;
    // PRG compiled from the same text again: the same object.
    if d2 != got {
        return Err(format!("get-tree D2: {:?}", tree_names(&d2)));
    }

    match env.run("pict", Request::Pict) {
        Ok(Outcome::Pict(p)) if p.data.starts_with(header) => {}
        other => return Err(format!("pict: {other:?}")),
    }
    Ok(d2)
}

/// The memory, a backup, and the backup restored onto a fresh calculator
/// with the same HOME after (D2 is `d2` again).
pub fn ops_backup(model_name: &str, env: &mut dyn Env, d2: &[TreeItem]) -> Result<(), String> {
    let header = header(model_name);
    let connected = |o: Result<Outcome, TxError>, what: &str| match o {
        Ok(Outcome::Connected(c)) if c.model == model(model_name) => Ok(()),
        other => Err(format!("{what}: {other:?}")),
    };
    match env.run("memory", Request::Memory) {
        Ok(Outcome::Memory(m)) if m.home > 0.0 && m.fits() => {}
        other => return Err(format!("memory: {other:?}")),
    }
    let before = listing(env.run("list HOME", Request::List { dir: home() }), "list")?;
    let backup = match env.run("backup", Request::Backup) {
        Ok(Outcome::Backup(b)) if b.data.starts_with(header) && b.model == model(model_name) => {
            b.data
        }
        other => return Err(format!("backup: {other:?}")),
    };
    match env.run("disconnect", Request::Disconnect) {
        Ok(Outcome::Disconnected) => {}
        other => return Err(format!("disconnect: {other:?}")),
    }

    env.fresh();
    connected(
        env.run("connect fresh", Request::Connect(Default::default())),
        "connect to the fresh calculator",
    )?;
    let fresh = listing(
        env.run("list fresh HOME", Request::List { dir: home() }),
        "list fresh",
    )?;
    if fresh.iter().any(|e| e.name == "D2") {
        return Err(format!("the fresh calculator has D2: {fresh:?}"));
    }
    match env.run("restore", Request::Restore { data: backup }) {
        Ok(Outcome::Restored) => {}
        other => return Err(format!("restore: {other:?}")),
    }
    match env.run("list after restore", Request::List { dir: home() }) {
        Err(TxError::Restarted) => {}
        other => return Err(format!("a request after the restore: {other:?}")),
    }
    env.server();
    connected(
        env.run(
            "connect after restore",
            Request::Connect(Default::default()),
        ),
        "connect after the restore",
    )?;
    let after = listing(
        env.run("list restored HOME", Request::List { dir: home() }),
        "list restored",
    )?;
    // Names, sizes and checksums.
    if after != before {
        return Err(format!(
            "the restored HOME differs:\n  backed up {before:?}\n  restored  {after:?}"
        ));
    }
    let restored = tree_of(
        env.run(
            "get-tree D2 restored",
            Request::GetTree {
                dir: vec!["HOME".into(), "D2".into()],
                format: Format::Binary,
                skip: Vec::new(),
            },
        ),
        "get-tree D2 restored",
    )?;
    if restored != *d2 {
        return Err(format!("the restored D2: {:?}", tree_names(&restored)));
    }
    match env.run("disconnect restored", Request::Disconnect) {
        Ok(Outcome::Disconnected) => Ok(()),
        other => Err(format!("disconnect: {other:?}")),
    }
}
