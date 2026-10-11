//! The computer's files for the page's folder pane (kb/docs/saturnus-tx.md,
//! "The local folder", and [`crate`]'s scope note): the whole file system
//! as the user may use it, read and written for the page's `Folder`
//! (web/tx/folder.js) through the handle shape of web/tx/tauri.js.
//!
//! A place is a list of names from the top of the computer: `["Users",
//! "ann", "hp"]` is `/Users/ann/hp` on macOS and Linux, `["C:", "Users",
//! "ann"]` is `C:\Users\ann` on Windows, whose top lists its drives. A
//! name is one entry: never empty, `.` or `..`, never with a separator or
//! a NUL (nor a `:` on Windows but in the drive), so no list of names
//! leaves the tree it names or reaches a path that is not a plain file or
//! folder of it.
//!
//! Errors carry the File System Access API's names (`NotFoundError`,
//! `TypeMismatchError`, ...), which `folder.js` turns into its plain
//! words, as it does for a browser's handles.

use std::fs;
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::UNIX_EPOCH;

use serde::Serialize;

/// The most of one file read at once: no calculator file comes near it,
/// and a file dragged by mistake (a disk image) must not fill the memory.
pub const MAX_READ: u64 = 256 << 20;

/// A failure, as the page's folder code knows it: `name` a DOMException's
/// name (or `OsError` for the rest), `message` in plain words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsError {
    /// `NotFoundError`, `TypeMismatchError`, `InvalidModificationError`,
    /// `NoModificationAllowedError`, `TypeError` or `OsError`.
    pub name: &'static str,
    /// What went wrong, in words.
    pub message: String,
}

impl FsError {
    fn new(name: &'static str, message: impl Into<String>) -> Self {
        FsError {
            name,
            message: message.into(),
        }
    }

    /// As the page receives it: `Name: message`.
    pub fn to_page(&self) -> String {
        format!("{}: {}", self.name, self.message)
    }
}

impl From<std::io::Error> for FsError {
    fn from(e: std::io::Error) -> Self {
        let (name, message) = match e.kind() {
            ErrorKind::NotFound => ("NotFoundError", "it is not there".to_string()),
            ErrorKind::PermissionDenied => {
                ("OsError", "this user may not read or change it".to_string())
            }
            ErrorKind::AlreadyExists => ("OsError", "that name is already there".to_string()),
            ErrorKind::DirectoryNotEmpty => (
                "InvalidModificationError",
                "the folder is not empty".to_string(),
            ),
            ErrorKind::NotADirectory | ErrorKind::IsADirectory => (
                "TypeMismatchError",
                "a file where a folder was expected, or back".to_string(),
            ),
            ErrorKind::StorageFull | ErrorKind::QuotaExceeded => {
                ("OsError", "the disk is full".to_string())
            }
            ErrorKind::ReadOnlyFilesystem => ("OsError", "the disk is read-only".to_string()),
            ErrorKind::ResourceBusy => (
                "NoModificationAllowedError",
                "it is in use by another program".to_string(),
            ),
            ErrorKind::InvalidFilename => (
                "TypeError",
                "the file system does not take that name".to_string(),
            ),
            _ => ("OsError", e.to_string()),
        };
        FsError::new(name, message)
    }
}

/// Result of this module.
pub type Result<T> = std::result::Result<T, FsError>;

/// Whether `name` is a drive as Windows names it: `C:`.
fn is_drive(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// Throw unless `name` is one entry's name.
pub fn check_name(name: &str) -> Result<()> {
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\0')
        || (cfg!(windows) && (name.contains('\\') || name.contains(':')));
    if bad {
        return Err(FsError::new("TypeError", format!("{name:?} is not a name")));
    }
    Ok(())
}

/// The path that `names` (from the top of the computer) name.
pub fn resolve(names: &[String]) -> Result<PathBuf> {
    let mut path = if cfg!(windows) {
        let Some(drive) = names.first() else {
            return Err(FsError::new(
                "TypeMismatchError",
                "the top of the computer holds only its drives",
            ));
        };
        if !is_drive(drive) {
            return Err(FsError::new(
                "NotFoundError",
                format!("{drive} is not a drive"),
            ));
        }
        PathBuf::from(format!("{}\\", drive.to_ascii_uppercase()))
    } else {
        PathBuf::from("/")
    };
    let rest = if cfg!(windows) { &names[1..] } else { names };
    for name in rest {
        check_name(name)?;
        path.push(name);
    }
    Ok(path)
}

/// The names of `path`, an absolute path, from the top of the computer
/// (what the page's paths hold): the inverse of [`resolve`].
pub fn split(path: &Path) -> Result<Vec<String>> {
    if !path.is_absolute() {
        return Err(FsError::new(
            "NotFoundError",
            format!("{} is not a whole path", path.display()),
        ));
    }
    let mut names = Vec::new();
    for c in path.components() {
        match c {
            Component::RootDir => {}
            Component::Prefix(p) => match p.kind() {
                std::path::Prefix::Disk(d) | std::path::Prefix::VerbatimDisk(d) => {
                    names.push(format!("{}:", char::from(d).to_ascii_uppercase()));
                }
                _ => {
                    return Err(FsError::new(
                        "OsError",
                        format!(
                            "{}: network folders are not offered; choose one on a drive",
                            path.display()
                        ),
                    ));
                }
            },
            Component::Normal(n) => {
                let n = n.to_str().ok_or_else(|| {
                    FsError::new(
                        "TypeError",
                        format!("{} has a name that is not Unicode", path.display()),
                    )
                })?;
                names.push(n.to_string());
            }
            Component::CurDir | Component::ParentDir => {
                return Err(FsError::new(
                    "NotFoundError",
                    format!("{} is not a plain path", path.display()),
                ));
            }
        }
    }
    Ok(names)
}

/// An entry of a listing.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entry {
    /// Its name.
    pub name: String,
    /// Whether it is a folder (a link to one counts).
    pub directory: bool,
    /// A file's size in bytes.
    pub size: Option<u64>,
    /// When a file was last written, in ms since 1970.
    pub modified: Option<f64>,
}

/// Whether the list leaves `name` (or its metadata) out: hidden files
/// (a leading dot; the hidden attribute on Windows), as the file
/// managers do.
fn hidden(name: &str, meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        if meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0 {
            return true;
        }
    }
    #[cfg(not(windows))]
    let _ = meta;
    name.starts_with('.')
}

fn ms(meta: &fs::Metadata) -> Option<f64> {
    let t = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    #[allow(clippy::cast_precision_loss)]
    Some(t.as_millis() as f64)
}

/// The drives of a Windows computer, as the top's listing.
fn drives() -> Vec<Entry> {
    (b'A'..=b'Z')
        .map(|d| format!("{}:", char::from(d)))
        .filter(|d| fs::metadata(format!("{d}\\")).is_ok())
        .map(|name| Entry {
            name,
            directory: true,
            size: None,
            modified: None,
        })
        .collect()
}

/// The files and folders in `names`: plain files and folders only (no
/// devices, pipes or sockets, no broken links), hidden ones left out.
pub fn list(names: &[String]) -> Result<Vec<Entry>> {
    if cfg!(windows) && names.is_empty() {
        return Ok(drives());
    }
    let dir = resolve(names)?;
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let Ok(entry) = entry else { continue };
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        // The target's metadata: a link to a folder is a folder.
        let Ok(meta) = fs::metadata(entry.path()) else {
            continue;
        };
        if hidden(&name, &meta) {
            continue;
        }
        if meta.is_dir() {
            out.push(Entry {
                name,
                directory: true,
                size: None,
                modified: None,
            });
        } else if meta.is_file() {
            out.push(Entry {
                name,
                directory: false,
                size: Some(meta.len()),
                modified: ms(&meta),
            });
        }
    }
    Ok(out)
}

/// What `names` is: a file (with its size and time) or a folder, as a
/// listing's entry.
pub fn stat(names: &[String]) -> Result<Entry> {
    let name = names.last().cloned().unwrap_or_default();
    let folder = Entry {
        name: name.clone(),
        directory: true,
        size: None,
        modified: None,
    };
    // The top: the root folder, or the list of drives on Windows.
    if names.is_empty() {
        return Ok(folder);
    }
    let meta = fs::metadata(resolve(names)?)?;
    if meta.is_dir() {
        Ok(folder)
    } else if meta.is_file() {
        Ok(Entry {
            name,
            directory: false,
            size: Some(meta.len()),
            modified: ms(&meta),
        })
    } else {
        Err(FsError::new(
            "TypeMismatchError",
            "it is neither a file nor a folder",
        ))
    }
}

/// Bytes `start..end` of file `names` (all of it without `end`), at most
/// [`MAX_READ`].
pub fn read(names: &[String], start: u64, end: Option<u64>) -> Result<Vec<u8>> {
    let path = resolve(names)?;
    let meta = fs::metadata(&path)?;
    if !meta.is_file() {
        return Err(FsError::new("TypeMismatchError", "it is not a file"));
    }
    let end = end.unwrap_or(meta.len()).min(meta.len());
    let len = end.saturating_sub(start);
    if len > MAX_READ {
        return Err(FsError::new(
            "OsError",
            format!(
                "it is {} MB, more than the {} MB read at once",
                len >> 20,
                MAX_READ >> 20
            ),
        ));
    }
    let mut file = fs::File::open(&path)?;
    file.seek(SeekFrom::Start(start))?;
    let mut out = Vec::with_capacity(usize::try_from(len).unwrap_or_default());
    file.take(len).read_to_end(&mut out)?;
    Ok(out)
}

/// File `names`, made empty if it is not there (an existing one is left
/// as it is).
pub fn create(names: &[String]) -> Result<()> {
    let path = resolve(names)?;
    if fs::metadata(&path).is_ok_and(|m| m.is_dir()) {
        return Err(FsError::new("TypeMismatchError", "it is a folder"));
    }
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    Ok(())
}

static TEMP: AtomicU32 = AtomicU32::new(0);

/// Write `bytes` as file `names`, replacing what is there only once all of
/// it is written (a temporary file beside it, renamed over it).
pub fn write(names: &[String], bytes: &[u8]) -> Result<()> {
    let path = resolve(names)?;
    let (Some(dir), Some(name)) = (path.parent(), names.last()) else {
        return Err(FsError::new(
            "TypeMismatchError",
            "a file is needed, not the top",
        ));
    };
    if fs::metadata(&path).is_ok_and(|m| m.is_dir()) {
        return Err(FsError::new("TypeMismatchError", "it is a folder"));
    }
    let temp = dir.join(format!(
        ".{name}.{}-{}.saturnus-tx",
        std::process::id(),
        TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&temp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&temp, &path)
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&temp);
        return Err(e.into());
    }
    Ok(())
}

/// Folder `names`, made if it is not there.
pub fn mkdir(names: &[String]) -> Result<()> {
    let path = resolve(names)?;
    match fs::metadata(&path) {
        Ok(m) if m.is_dir() => Ok(()),
        Ok(_) => Err(FsError::new("TypeMismatchError", "it is a file")),
        Err(_) => Ok(fs::create_dir(&path)?),
    }
}

/// Remove `names`: a file, a link (not what it points to), or a folder,
/// one that is not empty only when `recursive`. Never the top or a drive.
pub fn remove(names: &[String], recursive: bool) -> Result<()> {
    if names.is_empty() || (cfg!(windows) && names.len() == 1) {
        return Err(FsError::new(
            "NoModificationAllowedError",
            "the top of a drive is not removed",
        ));
    }
    let path = resolve(names)?;
    let meta = fs::symlink_metadata(&path)?;
    if meta.file_type().is_symlink() {
        // A link to a folder is a directory entry on Windows.
        return Ok(fs::remove_file(&path).or_else(|_| fs::remove_dir(&path))?);
    }
    if meta.is_dir() {
        if recursive {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_dir(&path).map_err(|e| {
                // Not every system says DirectoryNotEmpty.
                if fs::read_dir(&path).is_ok_and(|mut d| d.next().is_some()) {
                    FsError::new("InvalidModificationError", "the folder is not empty")
                } else {
                    e.into()
                }
            })?;
        }
        return Ok(());
    }
    Ok(fs::remove_file(&path)?)
}

/// Rename `from` in folder `dir` to `to`, never over another entry (a
/// change of case only is the same entry where case does not count).
pub fn rename(dir: &[String], from: &str, to: &str) -> Result<()> {
    check_name(from)?;
    check_name(to)?;
    let base = resolve(dir)?;
    let src = base.join(from);
    let dst = base.join(to);
    fs::symlink_metadata(&src)?;
    if fs::symlink_metadata(&dst).is_ok() && from.to_lowercase() != to.to_lowercase() {
        return Err(FsError::new("OsError", format!("{to} is already there")));
    }
    Ok(fs::rename(&src, &dst)?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn names(path: &Path) -> Vec<String> {
        split(path).unwrap()
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("saturnus-tx-files-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // macOS's temp dir is behind a link (/var -> /private/var).
        fs::canonicalize(&dir).unwrap()
    }

    fn with(base: &[String], more: &[&str]) -> Vec<String> {
        let mut v = base.to_vec();
        v.extend(more.iter().map(|s| (*s).to_string()));
        v
    }

    /// A path and its names are one another; no name leaves its folder.
    #[test]
    fn names_resolve_to_paths_and_back() {
        let dir = temp_dir("names");
        let n = names(&dir);
        assert_eq!(resolve(&n).unwrap(), dir);
        for bad in ["", ".", "..", "a/b", "a\0b"] {
            assert!(resolve(&with(&n, &[bad])).is_err(), "{bad:?}");
        }
        assert!(split(Path::new("relative/path")).is_err());
        #[cfg(unix)]
        assert_eq!(resolve(&[]).unwrap(), PathBuf::from("/"));
        #[cfg(windows)]
        {
            assert!(
                resolve(&["C:".into(), "a:b".into()]).is_err(),
                "no stream names"
            );
            assert!(resolve(&["C:".into(), "a\\b".into()]).is_err());
            assert_eq!(
                resolve(&["c:".into(), "x".into()]).unwrap(),
                PathBuf::from("C:\\x")
            );
            assert_eq!(
                split(Path::new("C:\\Users\\x")).unwrap(),
                ["C:", "Users", "x"]
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// Write, list, read, rename, remove: what folder.js does through them.
    #[test]
    fn files_and_folders_round_trip() {
        let dir = temp_dir("trip");
        let n = names(&dir);
        mkdir(&with(&n, &["D1"])).unwrap();
        mkdir(&with(&n, &["D1"])).unwrap();
        create(&with(&n, &["A.hp"])).unwrap();
        assert_eq!(read(&with(&n, &["A.hp"]), 0, None).unwrap(), b"");
        write(&with(&n, &["A.hp"]), b"HPHP48-R hello").unwrap();
        create(&with(&n, &["A.hp"])).unwrap();
        assert_eq!(
            read(&with(&n, &["A.hp"]), 0, None).unwrap(),
            b"HPHP48-R hello",
            "create leaves it"
        );
        assert_eq!(read(&with(&n, &["A.hp"]), 0, Some(8)).unwrap(), b"HPHP48-R");
        assert_eq!(read(&with(&n, &["A.hp"]), 9, Some(99)).unwrap(), b"hello");
        fs::write(dir.join(".hidden"), b"x").unwrap();
        let mut l = list(&n).unwrap();
        l.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(
            l.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["A.hp", "D1"]
        );
        assert_eq!((l[0].directory, l[0].size), (false, Some(14)));
        assert!(l[0].modified.unwrap() > 1.0e12);
        assert!(l[1].directory);
        assert!(stat(&with(&n, &["D1"])).unwrap().directory);
        let a = stat(&with(&n, &["A.hp"])).unwrap();
        assert_eq!(
            (a.name.as_str(), a.directory, a.size),
            ("A.hp", false, Some(14))
        );
        assert!(stat(&[]).unwrap().directory, "the top");
        assert_eq!(
            stat(&with(&n, &["nope"])).unwrap_err().name,
            "NotFoundError"
        );
        assert_eq!(
            read(&with(&n, &["D1"]), 0, None).unwrap_err().name,
            "TypeMismatchError"
        );
        assert_eq!(
            mkdir(&with(&n, &["A.hp"])).unwrap_err().name,
            "TypeMismatchError"
        );
        assert_eq!(
            write(&with(&n, &["D1"]), b"x").unwrap_err().name,
            "TypeMismatchError"
        );

        write(&with(&n, &["D1", "B.hp"]), b"b").unwrap();
        assert_eq!(
            remove(&with(&n, &["D1"]), false).unwrap_err().name,
            "InvalidModificationError"
        );
        rename(&n, "D1", "D2").unwrap();
        assert!(
            rename(&n, "A.hp", "D2").is_err(),
            "never over another entry"
        );
        rename(&n, "A.hp", "a.hp").unwrap();
        assert_eq!(
            read(&with(&n, &["a.hp"]), 0, None).unwrap(),
            b"HPHP48-R hello"
        );
        remove(&with(&n, &["D2"]), true).unwrap();
        remove(&with(&n, &["a.hp"]), false).unwrap();
        assert_eq!(
            remove(&with(&n, &["a.hp"]), false).unwrap_err().name,
            "NotFoundError"
        );
        assert!(remove(&[], true).is_err(), "never the top");
        // No temporary file is left.
        let left: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, [std::ffi::OsString::from(".hidden")]);
        let _ = fs::remove_dir_all(&dir);
    }

    /// A link to a folder lists as a folder; removing it removes the link.
    #[cfg(unix)]
    #[test]
    fn a_link_is_removed_not_its_target() {
        let dir = temp_dir("link");
        let n = names(&dir);
        mkdir(&with(&n, &["real"])).unwrap();
        write(&with(&n, &["real", "keep.hp"]), b"k").unwrap();
        std::os::unix::fs::symlink(dir.join("real"), dir.join("link")).unwrap();
        std::os::unix::fs::symlink(dir.join("gone"), dir.join("broken")).unwrap();
        let l = list(&n).unwrap();
        assert!(l.iter().any(|e| e.name == "link" && e.directory));
        assert!(
            !l.iter().any(|e| e.name == "broken"),
            "a broken link is left out"
        );
        remove(&with(&n, &["link"]), true).unwrap();
        assert_eq!(
            read(&with(&n, &["real", "keep.hp"]), 0, None).unwrap(),
            b"k"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Devices are not files: /dev/zero is neither listed nor read.
    #[cfg(unix)]
    #[test]
    fn devices_are_left_out() {
        let dev = vec!["dev".to_string()];
        assert!(!list(&dev).unwrap().iter().any(|e| e.name == "zero"));
        assert_eq!(
            read(&with(&dev, &["zero"]), 0, None).unwrap_err().name,
            "TypeMismatchError"
        );
    }
}
