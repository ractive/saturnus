//! The ROMs the app remembers: one file per model, chosen once in a
//! dialog, kept by path in a small settings file in the platform's config
//! directory (kb: iterations/iteration-20-remember-roms).
//!
//! When a ROM is chosen, the other regular files in its directory (not
//! below it) are identified by content (`saturnus_web::romid`) and
//! assigned or offered by its shared rules; the scan is bounded in
//! entries, files and bytes, and reads each file with the runner's cap.
//! Paths stay here: the page learns `{model, fileName, revision, state}`
//! per slot and the offers by number, never a path or a directory. A
//! remembered file that is gone, or whose content changed, is reported
//! and asked for again; nothing else boots in its place.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use saturnus::Model;
use saturnus_drive::runner::{max_rom_file, read_capped};
use saturnus_web::model_from_name;
use saturnus_web::romid::{self, Candidate, RomId};
use serde_json::{Value, json};

/// Name of the settings file in the app's config directory.
pub const SETTINGS_FILE: &str = "settings.json";
/// Largest settings file read.
const MAX_SETTINGS: u64 = 64 * 1024;
/// Directory entries the scan looks at.
pub const SCAN_ENTRIES: usize = 256;
/// Files of a ROM's size the scan reads at most.
pub const SCAN_FILES: usize = 16;
/// Bytes the scan reads at most, all files together (eight unpacked 49G
/// images; the seven published images take 7.3 MB).
pub const SCAN_BYTES: u64 = 32 * 1024 * 1024;

/// A model's remembered file.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Remembered {
    path: PathBuf,
    sha256: String,
    revision: Option<String>,
}

/// A file found beside a chosen one that only fits: the user decides.
#[derive(Clone, Debug)]
struct Offer {
    id: u64,
    models: Vec<Model>,
    path: PathBuf,
    sha256: String,
}

/// What a ROM command leads to: a model to boot from a file, and a line
/// for the user.
#[derive(Debug, Default)]
pub struct Step {
    pub boot: Option<(Model, PathBuf)>,
    pub notice: String,
}

/// The remembered ROMs, the settings file they live in, and the offers of
/// the last choice.
#[derive(Debug)]
pub struct Library {
    /// `None`: nothing is written (no config directory).
    file: Option<PathBuf>,
    boot_last: bool,
    last_model: Option<Model>,
    roms: HashMap<Model, Remembered>,
    /// Slots whose file no longer holds what was remembered.
    changed: HashSet<Model>,
    offers: Vec<Offer>,
    next_offer: u64,
    /// Why the settings could not be read or written, once.
    note: Option<String>,
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

fn title(m: Model) -> String {
    m.name().to_uppercase()
}

impl Library {
    /// The library kept in `file` (read now; a missing file is an empty
    /// library, an unreadable one too, with a note).
    pub fn open(file: Option<PathBuf>) -> Self {
        let mut lib = Self {
            file,
            boot_last: true,
            last_model: None,
            roms: HashMap::new(),
            changed: HashSet::new(),
            offers: Vec::new(),
            next_offer: 1,
            note: None,
        };
        let Some(path) = lib.file.clone() else {
            return lib;
        };
        if !path.exists() {
            return lib;
        }
        let parsed = read_capped(&path, MAX_SETTINGS).and_then(|b| {
            serde_json::from_slice::<Value>(&b).map_err(|e| format!("{SETTINGS_FILE}: {e}"))
        });
        match parsed {
            Ok(v) => lib.load(&v),
            Err(e) => {
                lib.note = Some(format!(
                    "the remembered ROMs could not be read ({e}); choose them again"
                ))
            }
        }
        lib
    }

    fn load(&mut self, v: &Value) {
        self.boot_last = v.get("bootLast").and_then(Value::as_bool).unwrap_or(true);
        self.last_model = v
            .get("lastModel")
            .and_then(Value::as_str)
            .and_then(|m| model_from_name(m).ok());
        let Some(roms) = v.get("roms").and_then(Value::as_object) else {
            return;
        };
        for (name, r) in roms {
            let (Ok(model), Some(path), Some(sha256)) = (
                model_from_name(name),
                r.get("path").and_then(Value::as_str),
                r.get("sha256").and_then(Value::as_str),
            ) else {
                continue;
            };
            self.roms.insert(
                model,
                Remembered {
                    path: PathBuf::from(path),
                    sha256: sha256.to_string(),
                    revision: romid::revision(sha256).map(str::to_string),
                },
            );
        }
    }

    fn to_json(&self) -> Value {
        let roms: serde_json::Map<String, Value> = self
            .roms
            .iter()
            .map(|(m, r)| {
                (
                    m.name().to_string(),
                    json!({"path": r.path.to_string_lossy(), "sha256": r.sha256}),
                )
            })
            .collect();
        json!({
            "version": 1,
            "bootLast": self.boot_last,
            "lastModel": self.last_model.map(Model::name),
            "roms": roms,
        })
    }

    /// Write the settings file; a failure is kept as the note (the slots
    /// still work until the app quits).
    fn save(&mut self) {
        let Some(path) = &self.file else {
            return;
        };
        let text = format!("{:#}\n", self.to_json());
        if let Err(e) = write_private(path, text.as_bytes()) {
            self.note = Some(format!(
                "the ROMs cannot be remembered ({e}); they work until the app quits"
            ));
        }
    }

    /// The slots for the page: `{slots: [{model, fileName, revision,
    /// state}], offers: [{id, models, fileName}], lastModel, bootLast,
    /// remembered, note}`; `state` is `empty`, `ready`, `missing` or
    /// `changed`.
    pub fn slots(&self) -> Value {
        let slots: Vec<Value> = Model::ALL
            .into_iter()
            .map(|m| match self.roms.get(&m) {
                None => json!({"model": m.name(), "fileName": null, "revision": null, "state": "empty"}),
                Some(r) => {
                    let state = if self.changed.contains(&m) {
                        "changed"
                    } else if r.path.is_file() {
                        "ready"
                    } else {
                        "missing"
                    };
                    json!({"model": m.name(), "fileName": file_name(&r.path), "revision": r.revision, "state": state})
                }
            })
            .collect();
        let offers: Vec<Value> = self
            .offers
            .iter()
            .map(|o| {
                json!({
                    "id": o.id,
                    "models": o.models.iter().map(|m| m.name()).collect::<Vec<_>>(),
                    "fileName": file_name(&o.path),
                })
            })
            .collect();
        json!({
            "slots": slots,
            "offers": offers,
            "lastModel": self.last_model.map(Model::name),
            "bootLast": self.boot_last,
            "remembered": self.file.is_some(),
            "note": self.note,
        })
    }

    /// A directory to open the dialog in: where a remembered ROM is.
    pub fn folder(&self, model: Model) -> Option<PathBuf> {
        let mine = self.roms.get(&model).into_iter();
        mine.chain(self.roms.values())
            .filter_map(|r| r.path.parent())
            .find(|d| d.is_dir())
            .map(Path::to_path_buf)
    }

    /// `path` was chosen for `selected`: identify it and the files beside
    /// it, assign them, remember, and say what to boot.
    pub fn choose(&mut self, selected: Model, path: &Path) -> Result<Step, String> {
        let rom = read_capped(path, max_rom_file())?;
        let mut files = vec![(path.to_path_buf(), romid::identify(&rom))];
        files.extend(scan(path));
        let candidates: Vec<Candidate> = files
            .iter()
            .enumerate()
            .map(|(n, (p, id))| Candidate {
                name: file_name(p),
                identity: id.identity.clone(),
                chosen: n == 0,
            })
            .collect();
        let filled: Vec<Model> = Model::ALL
            .into_iter()
            .filter(|m| self.slot_ready(*m))
            .collect();
        let plan = romid::plan(selected, &candidates, &filled);
        for &(m, n) in &plan.assign {
            self.remember(m, &files[n].0, &files[n].1);
        }
        self.offers = plan
            .offer
            .iter()
            .map(|(models, n)| {
                let id = self.next_offer;
                self.next_offer += 1;
                Offer {
                    id,
                    models: models.clone(),
                    path: files[*n].0.clone(),
                    sha256: files[*n].1.sha256.clone(),
                }
            })
            .collect();
        self.save();
        let boot = plan
            .boot
            .and_then(|m| Some((m, self.roms.get(&m)?.path.clone())));
        Ok(Step {
            boot,
            notice: plan.notice,
        })
    }

    fn slot_ready(&self, m: Model) -> bool {
        self.roms
            .get(&m)
            .is_some_and(|r| !self.changed.contains(&m) && r.path.is_file())
    }

    fn remember(&mut self, m: Model, path: &Path, id: &RomId) {
        self.changed.remove(&m);
        self.roms.insert(
            m,
            Remembered {
                path: path.to_path_buf(),
                sha256: id.sha256.clone(),
                revision: id.revision().map(str::to_string),
            },
        );
    }

    /// Take offer `id` for `model`: the file is read again and must still
    /// be what was offered.
    pub fn take_offer(&mut self, model: Model, id: u64) -> Result<Step, String> {
        let pos = self
            .offers
            .iter()
            .position(|o| o.id == id && o.models.contains(&model))
            .ok_or("that offer is no longer open")?;
        let offer = self.offers[pos].clone();
        let rom = read_capped(&offer.path, max_rom_file())?;
        let rid = romid::identify(&rom);
        if rid.sha256 != offer.sha256 {
            self.offers.remove(pos);
            return Err(format!(
                "{} changed since it was found; choose it again",
                file_name(&offer.path)
            ));
        }
        self.remember(model, &offer.path, &rid);
        // An offer for several models stays open for the others.
        let o = &mut self.offers[pos];
        o.models.retain(|&m| m != model);
        if o.models.is_empty() {
            self.offers.remove(pos);
        }
        self.save();
        Ok(Step {
            boot: Some((model, offer.path)),
            notice: String::new(),
        })
    }

    /// The file to boot `model` from, checked: it must be there and hold
    /// what was remembered. Otherwise the slot is marked and the reason
    /// returned; nothing else is offered in its place.
    pub fn boot_file(&mut self, model: Model) -> Result<PathBuf, String> {
        let r = self
            .roms
            .get(&model)
            .ok_or_else(|| format!("no ROM is remembered for the {}", title(model)))?
            .clone();
        let name = file_name(&r.path);
        if !r.path.is_file() {
            return Err(format!(
                "{name}, the {} ROM, is no longer where it was; choose it again",
                title(model)
            ));
        }
        let rom = read_capped(&r.path, max_rom_file())?;
        if romid::identify(&rom).sha256 != r.sha256 {
            self.changed.insert(model);
            return Err(format!(
                "{name}, the {} ROM, has changed since it was chosen; choose it again",
                title(model)
            ));
        }
        self.changed.remove(&model);
        Ok(r.path)
    }

    /// `model` booted: it is the one to start with next time.
    pub fn booted(&mut self, model: Model) {
        if self.last_model != Some(model) {
            self.last_model = Some(model);
            self.save();
        }
    }

    /// Forget `model`'s ROM, or every ROM; the offers go too.
    pub fn forget(&mut self, model: Option<Model>) {
        match model {
            Some(m) => {
                self.roms.remove(&m);
                self.changed.remove(&m);
            }
            None => {
                self.roms.clear();
                self.changed.clear();
                self.last_model = None;
            }
        }
        self.offers.clear();
        self.save();
    }

    /// Whether the last model boots at start.
    pub fn set_boot_last(&mut self, on: bool) {
        self.boot_last = on;
        self.save();
    }
}

/// The regular files beside `chosen` (not below, not `chosen` itself, no
/// links) whose size a model takes, identified, by name. Bounded: the
/// first [`SCAN_ENTRIES`] entries, at most [`SCAN_FILES`] files and
/// [`SCAN_BYTES`] bytes, each read with the runner's ROM cap. Unreadable
/// entries are skipped.
pub fn scan(chosen: &Path) -> Vec<(PathBuf, RomId)> {
    let Some(dir) = chosen.parent() else {
        return Vec::new();
    };
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let chosen_name = chosen.file_name();
    let mut found: Vec<(PathBuf, u64)> = entries
        .take(SCAN_ENTRIES)
        .filter_map(Result::ok)
        .filter(|e| Some(e.file_name().as_os_str()) != chosen_name)
        // The entry's own type: a link is not followed.
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| Some((e.path(), e.metadata().ok()?.len())))
        .filter(|&(_, len)| {
            usize::try_from(len).is_ok_and(|l| Model::ALL.iter().any(|m| m.accepts_rom_len(l)))
        })
        .collect();
    found.sort();
    let mut bytes = 0;
    let mut out = Vec::new();
    for (path, len) in found.into_iter().take(SCAN_FILES) {
        if bytes + len > SCAN_BYTES {
            break;
        }
        bytes += len;
        if let Ok(rom) = read_capped(&path, max_rom_file()) {
            out.push((path, romid::identify(&rom)));
        }
    }
    out
}

/// Write `bytes` to `path` readable by the user only (0600 on Unix, in a
/// directory of 0700 if it has to be made), replacing it whole: a
/// temporary file beside it is renamed over it.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)?;
    let tmp = dir.join(format!(".{SETTINGS_FILE}.{}.tmp", std::process::id()));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let result = (|| {
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// The model of a message's `model` field.
pub fn model_field(msg: &Value) -> Result<Model, String> {
    model_from_name(
        msg.get("model")
            .and_then(Value::as_str)
            .ok_or("missing \"model\"")?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const KB: usize = 1024;

    /// A fresh directory under the system's temp directory.
    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "saturnus-roms-{tag}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A synthetic image of `len` bytes, distinct per `seed`.
    fn image(len: usize, seed: u8) -> Vec<u8> {
        let mut v = vec![0x20u8; len];
        v[0] = seed;
        v
    }

    fn slot(lib: &Library, model: &str) -> Value {
        lib.slots()["slots"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["model"] == model)
            .unwrap()
            .clone()
    }

    #[test]
    fn choose_scan_remember_and_reopen() {
        let dir = temp("scan");
        let roms = dir.join("roms");
        std::fs::create_dir_all(roms.join("below")).unwrap();
        std::fs::write(roms.join("sx.bin"), image(256 * KB, 1)).unwrap();
        // Synthetic images fit, never exact: the 512 KB one could be the
        // 48GX or the 38G, so it is offered, not assigned.
        std::fs::write(roms.join("gx-or-38.bin"), image(512 * KB, 2)).unwrap();
        std::fs::write(roms.join("readme.txt"), b"not a rom").unwrap();
        std::fs::write(roms.join("below").join("deep.bin"), image(256 * KB, 3)).unwrap();
        let settings = dir.join("config").join(SETTINGS_FILE);
        let mut lib = Library::open(Some(settings.clone()));
        let step = lib.choose(Model::Hp48sx, &roms.join("sx.bin")).unwrap();
        assert_eq!(step.boot.as_ref().unwrap().0, Model::Hp48sx);
        assert_eq!(step.notice, "gx-or-38.bin could be the 48GX or 38G ROM.");
        assert_eq!(slot(&lib, "48sx")["state"], "ready");
        assert_eq!(slot(&lib, "48sx")["fileName"], "sx.bin");
        assert_eq!(slot(&lib, "48gx")["state"], "empty");
        let offers = lib.slots()["offers"].clone();
        assert_eq!(offers[0]["models"], json!(["48gx", "38g"]));
        assert_eq!(offers[0]["fileName"], "gx-or-38.bin");
        // No path reaches the page.
        let text = lib.slots().to_string();
        assert!(!text.contains(roms.to_str().unwrap()), "{text}");
        // Taking the offer for the 38G remembers it and boots it.
        let id = offers[0]["id"].as_u64().unwrap();
        let step = lib.take_offer(Model::Hp38g, id).unwrap();
        assert_eq!(step.boot.unwrap().0, Model::Hp38g);
        assert_eq!(slot(&lib, "38g")["state"], "ready");
        assert_eq!(lib.slots()["offers"][0]["models"], json!(["48gx"]));
        lib.booted(Model::Hp38g);
        // The settings file is the user's only, and a new library reads it.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&settings).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let mut again = Library::open(Some(settings.clone()));
        assert_eq!(again.slots()["lastModel"], "38g");
        assert_eq!(again.slots()["bootLast"], true);
        assert_eq!(again.boot_file(Model::Hp48sx).unwrap(), roms.join("sx.bin"));
        assert!(again.slots()["offers"].as_array().unwrap().is_empty());
        // Missing: reported, not replaced.
        std::fs::rename(roms.join("sx.bin"), dir.join("away.bin")).unwrap();
        assert_eq!(slot(&again, "48sx")["state"], "missing");
        let e = again.boot_file(Model::Hp48sx).unwrap_err();
        assert!(e.contains("no longer where it was"), "{e}");
        // Changed: the same name with other content.
        std::fs::write(roms.join("sx.bin"), image(256 * KB, 9)).unwrap();
        let e = again.boot_file(Model::Hp48sx).unwrap_err();
        assert!(e.contains("has changed"), "{e}");
        assert_eq!(slot(&again, "48sx")["state"], "changed");
        // Forget one, then all.
        again.forget(Some(Model::Hp48sx));
        assert_eq!(slot(&again, "48sx")["state"], "empty");
        again.forget(None);
        let v: Value = serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
        assert_eq!(v["roms"], json!({}));
        assert_eq!(v["lastModel"], Value::Null);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A chosen file that fits no model boots nothing; a corrupt settings
    /// file is an empty library with a note.
    #[test]
    fn refusals() {
        let dir = temp("refuse");
        std::fs::write(dir.join("a.txt"), b"hello").unwrap();
        let settings = dir.join(SETTINGS_FILE);
        std::fs::write(&settings, b"{ not json").unwrap();
        let mut lib = Library::open(Some(settings));
        assert!(
            lib.slots()["note"]
                .as_str()
                .unwrap()
                .contains("could not be read")
        );
        let step = lib.choose(Model::Hp48sx, &dir.join("a.txt")).unwrap();
        assert!(step.boot.is_none());
        assert!(step.notice.contains("not a ROM image"), "{}", step.notice);
        assert!(lib.take_offer(Model::Hp48gx, 99).is_err());
        let mut none = Library::open(None);
        assert_eq!(none.slots()["remembered"], false);
        none.set_boot_last(false);
        assert_eq!(none.slots()["bootLast"], false);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The scan's bounds: files only up to the count, and links are not
    /// followed.
    #[test]
    fn scan_is_bounded() {
        let dir = temp("bounds");
        for n in 0..(SCAN_FILES + 4) {
            std::fs::write(dir.join(format!("{n:02}.rom")), image(64 * KB, n as u8)).unwrap();
        }
        let found = scan(&dir.join("00.rom"));
        assert_eq!(found.len(), SCAN_FILES);
        assert!(
            found
                .iter()
                .all(|(p, _)| p.file_name().unwrap() != "00.rom")
        );
        #[cfg(unix)]
        {
            let other = temp("bounds-target");
            std::fs::write(other.join("t.rom"), image(256 * KB, 7)).unwrap();
            std::os::unix::fs::symlink(other.join("t.rom"), dir.join("aa-link.rom")).unwrap();
            let found = scan(&dir.join("00.rom"));
            assert!(
                found
                    .iter()
                    .all(|(p, _)| p.file_name().unwrap() != "aa-link.rom")
            );
            std::fs::remove_dir_all(&other).unwrap();
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The real images (`SATURNUS_ROM_DIR`, read only): choosing the 48SX
    /// ROM fills the other models but the 42S, whose dump is offered.
    #[test]
    fn rom_dir_folder() {
        let Some(roms) = std::env::var_os("SATURNUS_ROM_DIR").map(PathBuf::from) else {
            return;
        };
        if !roms.join("sxrom-j").is_file() {
            return;
        }
        let mut lib = Library::open(None);
        let step = lib.choose(Model::Hp48sx, &roms.join("sxrom-j")).unwrap();
        assert_eq!(step.boot.unwrap().0, Model::Hp48sx);
        for m in ["48gx", "38g", "49g", "39g", "40g"] {
            let s = slot(&lib, m);
            if s["state"] != "ready" {
                eprintln!("{m}: no image in SATURNUS_ROM_DIR");
            }
        }
        assert_eq!(slot(&lib, "39g")["fileName"], slot(&lib, "40g")["fileName"]);
        assert_eq!(slot(&lib, "42s")["state"], "empty");
        eprintln!("{}", step.notice);
    }
}
