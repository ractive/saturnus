//! The folders the desktop app remembers (kb/iterations/iteration-49-tx-desktop-roots.md):
//! `folders.json` in the app's config folder (`ch.ractive.saturnus`; iteration 51), written only here (mode
//! 0600 on Unix), as `{"version": 1, "folders": [{names, picked, used}]}`
//! newest first.
//!
//! - An entry with `picked` is a folder the user chose in the app's
//!   dialog (`folder_pick`): a root ([`files::Roots`]) in this run and the
//!   next ones. Only [`Places::pick`] sets it; [`Places::used`] (the page
//!   reporting a folder its pane showed) never does, so a page cannot
//!   widen the roots by reporting a path.
//! - The pane's "Recent folders" are the newest [`RECENT`] entries that
//!   are there ([`Places::recent`]). Entries past them are dropped unless
//!   picked (at most [`MAX_PICKED`] of those, the oldest dropped first),
//!   so a picked folder stays a root after the list moves on.
//! - A picked folder that is not there when the file is read (a USB stick
//!   not plugged in) is no root in this run; it stays in the file, and
//!   becomes one again when it is back ([`Places::check`] looks again
//!   before it refuses).
//! - [`Places::clear`] forgets them all: the roots are the home folder
//!   alone again.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::files::{self, FsError, Roots};

/// The file's name in the app's config folder.
pub const FOLDERS_FILE: &str = "folders.json";
/// The most of it read: a few dozen entries come nowhere near it.
const MAX_FILE: u64 = 64 * 1024;
/// The folders "Recent folders" shows.
pub const RECENT: usize = 8;
/// The most picked folders kept.
pub const MAX_PICKED: usize = 32;

/// One remembered folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place {
    /// Its names from the top of the computer (as the page's paths).
    pub names: Vec<String>,
    /// Whether the user picked it in the app's dialog (a root).
    #[serde(default)]
    pub picked: bool,
    /// When the pane last showed it, in ms since 1970.
    #[serde(default)]
    pub used: f64,
}

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    folders: Vec<Place>,
}

/// Now, in ms since 1970.
fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or_default()
}

/// Whether `a` holds `b` or the other way round (one of them a folder
/// inside the other, or both the same).
fn on_one_line(a: &[String], b: &[String]) -> bool {
    let n = a.len().min(b.len());
    a[..n] == b[..n]
}

/// The remembered folders and the roots made of them.
#[derive(Debug)]
pub struct Places {
    file: Option<PathBuf>,
    home: PathBuf,
    places: Vec<Place>,
    roots: Roots,
}

impl Places {
    /// The places of `file` (none when it is missing, too big or not
    /// ours), the roots `home` and the picked folders that are there.
    pub fn load(file: Option<PathBuf>, home: &Path) -> Self {
        let places = file.as_deref().map(read_file).unwrap_or_default();
        let mut p = Places {
            file,
            home: home.to_path_buf(),
            places,
            roots: Roots::new(home),
        };
        p.rebuild_roots();
        p
    }

    /// The roots now.
    pub fn roots(&self) -> &Roots {
        &self.roots
    }

    /// The roots again from the home folder and the picked folders there.
    fn rebuild_roots(&mut self) {
        let mut roots = Roots::new(&self.home);
        for p in self.places.iter().filter(|p| p.picked) {
            if let Ok(path) = files::resolve(&p.names) {
                // Not there now: no root, until it is back.
                let _ = roots.add(&path);
            }
        }
        self.roots = roots;
    }

    /// Run `check` on the roots; a refusal is checked again after the
    /// picked folders are looked for once more (one may be back).
    pub fn check(&mut self, check: impl Fn(&Roots) -> files::Result<()>) -> files::Result<()> {
        match check(&self.roots) {
            Err(e) if e.message == files::OUTSIDE => {
                self.rebuild_roots();
                check(&self.roots)
            }
            other => other,
        }
    }

    /// Folder `path` picked in the app's dialog: a root from now on,
    /// remembered, at the top of the list. Resolves to its names.
    pub fn pick(&mut self, path: &Path) -> files::Result<Vec<String>> {
        let names = files::split(path)?;
        self.roots.add(path)?;
        self.put(names.clone(), true, true);
        Ok(names)
    }

    /// Folder `names` shown in the pane: to the top of the list (never a
    /// root by this). A walk down or up from the newest entry replaces it
    /// unless that one was picked, so opening `a`, then `a/b`, then
    /// `a/b/c` leaves one entry. The home folder itself is not listed.
    pub fn used(&mut self, names: Vec<String>) -> files::Result<()> {
        let path = files::resolve(&names)?;
        if fs::canonicalize(&path).is_ok_and(|p| p == self.roots.home()) {
            return Ok(());
        }
        if !fs::metadata(&path).is_ok_and(|m| m.is_dir()) {
            return Err(FsError {
                name: "NotFoundError",
                message: "it is not a folder there".into(),
            });
        }
        // Already the newest: the order is the same, nothing to write.
        if let Some(top) = self.places.first_mut()
            && top.names == names
        {
            top.used = now_ms();
            return Ok(());
        }
        if let Some(top) = self.places.first()
            && !top.picked
            && top.names != names
            && on_one_line(&top.names, &names)
        {
            self.places.remove(0);
        }
        self.put(names, false, false);
        Ok(())
    }

    /// `names` to the top, `picked` kept or set; the list cut; saved
    /// (flushed to the disk when `durable`: a pick, not a folder shown).
    fn put(&mut self, names: Vec<String>, picked: bool, durable: bool) {
        let mut picked = picked;
        if let Some(i) = self.places.iter().position(|p| p.names == names) {
            picked |= self.places.remove(i).picked;
        }
        self.places.insert(
            0,
            Place {
                names,
                picked,
                used: now_ms(),
            },
        );
        let mut picked_kept = 0;
        let mut i = 0;
        self.places.retain(|p| {
            i += 1;
            if p.picked {
                picked_kept += 1;
                picked_kept <= MAX_PICKED
            } else {
                i <= RECENT
            }
        });
        self.save(durable);
    }

    /// The newest [`RECENT`] folders that are there, newest first.
    pub fn recent(&self) -> Vec<Place> {
        self.places
            .iter()
            .filter(|p| files::resolve(&p.names).is_ok_and(|path| path.is_dir()))
            .take(RECENT)
            .cloned()
            .collect()
    }

    /// Forget every folder: the list empty, the roots the home folder.
    pub fn clear(&mut self) {
        self.places.clear();
        self.rebuild_roots();
        self.save(true);
    }

    /// Write the file (best effort: a failure keeps the list for this
    /// run), flushed to the disk when `durable`.
    fn save(&self, durable: bool) {
        let Some(path) = &self.file else { return };
        let file = File {
            version: 1,
            folders: self.places.clone(),
        };
        if let Ok(bytes) = serde_json::to_vec_pretty(&file)
            && let Err(e) = write_private(path, &bytes, durable)
        {
            eprintln!("saturnus: {}: {e}", path.display());
        }
    }
}

/// The entries of `path`; none when it is missing, over [`MAX_FILE`] or
/// not a file of ours.
fn read_file(path: &Path) -> Vec<Place> {
    let Ok(f) = fs::File::open(path) else {
        return Vec::new();
    };
    let mut bytes = Vec::new();
    if f.take(MAX_FILE + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > MAX_FILE {
        return Vec::new();
    }
    match serde_json::from_slice::<File>(&bytes) {
        Ok(f) if f.version == 1 => f
            .folders
            .into_iter()
            .filter(|p| files::resolve(&p.names).is_ok())
            .collect(),
        _ => Vec::new(),
    }
}

/// Write `bytes` as `path`, readable by this user only on Unix (folder
/// 0700, file 0600), replacing it only once all is written (flushed to
/// the disk first when `durable`).
pub fn write_private(path: &Path, bytes: &[u8], durable: bool) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)?;
    let temp = dir.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id()
    ));
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let written = (|| {
        let mut f = opts.open(&temp)?;
        f.write_all(bytes)?;
        if durable {
            f.sync_all()?;
        }
        fs::rename(&temp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("saturnus-tx-places-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        if cfg!(windows) {
            dir
        } else {
            fs::canonicalize(&dir).unwrap()
        }
    }

    /// A home, a folder outside it and the config folder, in one temp dir.
    fn setup(tag: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let dir = temp_dir(tag);
        let home = dir.join("home");
        let outside = dir.join("stick");
        let config = dir.join("config");
        fs::create_dir_all(home.join("hp")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        (dir, home, outside, config.join(FOLDERS_FILE))
    }

    fn file_in(dir: &Path) -> PathBuf {
        dir.join("x.hp")
    }

    #[test]
    fn a_picked_folder_is_a_root_after_a_restart() {
        let (dir, home, stick, file) = setup("restart");
        let mut p = Places::load(Some(file.clone()), &home);
        assert!(p.roots().check(&file_in(&stick)).is_err());
        let names = p.pick(&stick).unwrap();
        assert_eq!(names, files::split(&stick).unwrap());
        p.roots().check(&file_in(&stick)).unwrap();

        // A new run: a new Places from the file.
        let mut again = Places::load(Some(file.clone()), &home);
        again.roots().check(&file_in(&stick)).unwrap();
        assert_eq!(again.recent().len(), 1);
        assert!(again.recent()[0].picked);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        // Cleared: the home folder alone, after a restart too.
        again.clear();
        assert!(again.roots().check(&file_in(&stick)).is_err());
        assert!(again.recent().is_empty());
        let third = Places::load(Some(file), &home);
        assert!(third.roots().check(&file_in(&stick)).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_used_folder_is_listed_but_never_a_root() {
        let (dir, home, stick, file) = setup("used");
        let mut p = Places::load(Some(file.clone()), &home);
        p.used(files::split(&stick).unwrap()).unwrap();
        assert_eq!(p.recent()[0].names, files::split(&stick).unwrap());
        assert!(!p.recent()[0].picked);
        assert!(p.check(|r| r.check(&file_in(&stick))).is_err());
        let again = Places::load(Some(file), &home);
        assert!(again.roots().check(&file_in(&stick)).is_err());
        assert_eq!(again.recent().len(), 1);
        // The home folder itself is not listed; a file is not a folder.
        p.used(files::split(&home).unwrap()).unwrap();
        assert_eq!(p.recent().len(), 1);
        fs::write(home.join("f"), b"").unwrap();
        assert!(p.used(files::split(&home.join("f")).unwrap()).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_list_is_cut_at_eight_and_picked_folders_stay() {
        let (dir, home, stick, file) = setup("cut");
        let mut p = Places::load(Some(file.clone()), &home);
        p.pick(&stick).unwrap();
        for i in 0..12 {
            let d = home.join(format!("d{i}"));
            fs::create_dir_all(&d).unwrap();
            p.used(files::split(&d).unwrap()).unwrap();
        }
        let recent = p.recent();
        assert_eq!(recent.len(), RECENT);
        assert_eq!(recent[0].names.last().unwrap(), "d11");
        assert!(recent.iter().all(|r| !r.picked));
        // Off the list, the picked folder is still a root (now and next run).
        p.roots().check(&file_in(&stick)).unwrap();
        Places::load(Some(file), &home)
            .roots()
            .check(&file_in(&stick))
            .unwrap();
        // Used again: back at the top, still picked.
        p.used(files::split(&stick).unwrap()).unwrap();
        assert!(p.recent()[0].picked);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_walk_down_replaces_the_newest_entry() {
        let (dir, home, _, file) = setup("walk");
        let mut p = Places::load(Some(file), &home);
        let a = home.join("a");
        fs::create_dir_all(a.join("b").join("c")).unwrap();
        p.used(files::split(&a).unwrap()).unwrap();
        p.used(files::split(&a.join("b")).unwrap()).unwrap();
        p.used(files::split(&a.join("b").join("c")).unwrap())
            .unwrap();
        let recent = p.recent();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].names.last().unwrap(), "c");
        // Shown again while newest: the file is not written.
        let file = p.file.clone().unwrap();
        fs::remove_file(&file).unwrap();
        p.used(files::split(&a.join("b").join("c")).unwrap())
            .unwrap();
        assert!(!file.exists());
        p.used(files::split(&home.join("hp")).unwrap()).unwrap();
        assert_eq!(p.recent().len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vanished_folder_is_dropped_and_comes_back() {
        let (dir, home, stick, file) = setup("vanish");
        let mut p = Places::load(Some(file.clone()), &home);
        p.pick(&stick).unwrap();
        fs::rename(&stick, dir.join("away")).unwrap();
        // Unplugged: not listed, no root.
        let mut again = Places::load(Some(file.clone()), &home);
        assert!(again.recent().is_empty());
        assert!(again.check(|r| r.check(&file_in(&stick))).is_err());
        // Plugged in again: a root once more, in the same run.
        fs::rename(dir.join("away"), &stick).unwrap();
        again.check(|r| r.check(&file_in(&stick))).unwrap();
        assert_eq!(again.recent().len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_malformed_or_huge_file_reads_as_empty() {
        let (dir, home, stick, file) = setup("bad");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        let picked = serde_json::json!({"version": 1, "folders": [
            {"names": files::split(&stick).unwrap(), "picked": true, "used": 1.0},
        ]});
        for bad in [
            b"not json".to_vec(),
            br#"{"version": 2, "folders": []}"#.to_vec(),
            br#"{"folders": 3}"#.to_vec(),
        ] {
            fs::write(&file, bad).unwrap();
            let p = Places::load(Some(file.clone()), &home);
            assert!(p.recent().is_empty());
        }
        // Over the cap: none, even with a picked folder in it.
        let mut big = serde_json::to_vec(&picked).unwrap();
        big.extend(std::iter::repeat_n(b' ', MAX_FILE as usize));
        fs::write(&file, big).unwrap();
        let p = Places::load(Some(file.clone()), &home);
        assert!(p.roots().check(&file_in(&stick)).is_err());
        // A `..` in a name never makes an entry.
        let mut dotted = files::split(&home).unwrap();
        dotted.extend(["..".to_string(), "stick".to_string()]);
        let dots = serde_json::json!({"version": 1, "folders": [
            {"names": dotted, "picked": true},
        ]});
        fs::write(&file, serde_json::to_vec(&dots).unwrap()).unwrap();
        let p = Places::load(Some(file.clone()), &home);
        assert!(p.recent().is_empty());
        // The well-formed one: a root.
        fs::write(&file, serde_json::to_vec(&picked).unwrap()).unwrap();
        Places::load(Some(file), &home)
            .roots()
            .check(&file_in(&stick))
            .unwrap();
        let _ = fs::remove_dir_all(&dir);
    }
}
