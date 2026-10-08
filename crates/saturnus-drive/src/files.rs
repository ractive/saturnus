//! The native hosts' files: ROMs and states read with a size cap, and
//! states and settings written whole (a temporary file renamed over the
//! old one). The page never names a file; the desktop app chooses them in
//! dialogs and the CLI takes them on its command line, and keeps the
//! auto-saved states in its data folder ([`StateDir`]).

use std::path::{Path, PathBuf};

use saturnus::Model;

/// Largest ROM file read: the largest image any model accepts (an unpacked
/// 49G, 4 MiB); anything longer is refused before it is read whole.
pub fn max_rom_file() -> u64 {
    Model::ALL
        .iter()
        .map(|m| 2 * m.rom_bytes() as u64)
        .max()
        .unwrap_or(0)
}

/// Largest state file read (the protocol's cap on a state,
/// `saturnus_host::protocol::MAX_STATE_BYTES`).
pub const MAX_STATE_FILE: u64 = saturnus_host::protocol::MAX_STATE_BYTES as u64;

/// Replace `path` with `bytes` so that it holds either the old content or
/// the new, never a part: write a temporary file beside it with `write`
/// (the seam tests use to fail a write), sync it, then rename it over
/// `path` (`rename` replaces an existing file on Windows too). On failure
/// the temporary file is removed and `path` is untouched.
pub fn write_atomic(
    path: &Path,
    bytes: &[u8],
    write: impl FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
) -> std::io::Result<()> {
    write_atomic_with(path, bytes, std::fs::OpenOptions::new(), write)
}

/// [`write_atomic`] with the temporary file opened through `opts` (a Unix
/// mode, say). The temporary file has a random name and is created with
/// `create_new`, which never follows a link planted at its name nor
/// reuses a file there: another local user who can write the directory
/// cannot redirect the write.
pub fn write_atomic_with(
    path: &Path,
    bytes: &[u8],
    mut opts: std::fs::OpenOptions,
    write: impl FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path.file_name().map_or_else(
        || std::ffi::OsString::from("state"),
        std::ffi::OsStr::to_os_string,
    );
    opts.write(true).create_new(true);
    let mut opened = None;
    for _ in 0..16 {
        let mut tmp_name = std::ffi::OsString::from(".");
        tmp_name.push(&name);
        tmp_name.push(format!(".{:016x}.tmp", random_u64()));
        let tmp = dir.join(tmp_name);
        match opts.open(&tmp) {
            Ok(f) => {
                opened = Some((f, tmp));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    let Some((mut f, tmp)) = opened else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "no free temporary file name",
        ));
    };
    let result = (|| {
        write(&mut f, bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// 64 random bits: std's hasher keys come from the OS random source.
fn random_u64() -> u64 {
    use std::hash::{BuildHasher as _, Hasher as _};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    h.finish()
}

/// Read `path` if it holds at most `cap` bytes; a longer file (or a
/// device that never ends) is refused after reading `cap + 1` bytes. The
/// error is the message for the user.
pub fn read_capped(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let f = std::fs::File::open(path).map_err(|e| format!("cannot read {name}: {e}"))?;
    if f.metadata().is_ok_and(|m| m.is_file() && m.len() > cap) {
        return Err(format!("{name} is larger than {cap} bytes"));
    }
    let mut data = Vec::new();
    f.take(cap + 1)
        .read_to_end(&mut data)
        .map_err(|e| format!("cannot read {name}: {e}"))?;
    if data.len() as u64 > cap {
        return Err(format!("{name} is larger than {cap} bytes"));
    }
    Ok(data)
}

/// Where a native host keeps each model's auto-saved state (iteration 27,
/// `saturnus_host::protocol`'s `Output::Save`): one slot per model, apart
/// from the files the user saves states to.
pub trait StateStore: Send + 'static {
    /// The state kept for `model`, if any. A slot that cannot be read is
    /// none (the machine cold-boots); the error is the message to log.
    fn load(&self, model: &str) -> Result<Option<Vec<u8>>, String>;
    /// Keep `state` as `model`'s.
    fn save(&self, model: &str, state: &[u8]) -> Result<(), String>;
    /// Forget `model`'s state (a fresh start).
    fn clear(&self, model: &str) -> Result<(), String>;
}

/// [`StateStore`] over a directory: `<model>.auto.state`, written whole
/// ([`write_atomic`]), read with the state cap.
#[derive(Clone, Debug)]
pub struct StateDir(pub PathBuf);

impl StateDir {
    /// The file of `model`'s slot; `None` for a name that is not a model's.
    fn file(&self, model: &str) -> Result<PathBuf, String> {
        if !Model::ALL.iter().any(|m| m.name() == model) {
            return Err(format!("no model {model:?}"));
        }
        Ok(self.0.join(format!("{model}.auto.state")))
    }
}

impl StateStore for StateDir {
    fn load(&self, model: &str) -> Result<Option<Vec<u8>>, String> {
        let path = self.file(model)?;
        if !path.exists() {
            return Ok(None);
        }
        read_capped(&path, MAX_STATE_FILE).map(Some)
    }

    fn save(&self, model: &str, state: &[u8]) -> Result<(), String> {
        let path = self.file(model)?;
        std::fs::create_dir_all(&self.0)
            .map_err(|e| format!("cannot make {}: {e}", self.0.display()))?;
        write_atomic(&path, state, |f, b| {
            use std::io::Write as _;
            f.write_all(b)
        })
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    fn clear(&self, model: &str) -> Result<(), String> {
        let path = self.file(model)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("cannot remove {}: {e}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_dir_keeps_one_slot_per_model() {
        let dir = std::env::temp_dir().join(format!("saturnus-states-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = StateDir(dir.join("states"));
        assert_eq!(store.load("48sx").unwrap(), None, "nothing yet");
        store.save("48sx", b"one").unwrap();
        store.save("49g", b"two").unwrap();
        assert_eq!(store.load("48sx").unwrap().as_deref(), Some(&b"one"[..]));
        store.save("48sx", b"three").unwrap();
        assert_eq!(store.load("48sx").unwrap().as_deref(), Some(&b"three"[..]));
        store.clear("48sx").unwrap();
        store.clear("48sx").unwrap();
        assert_eq!(store.load("48sx").unwrap(), None);
        assert_eq!(store.load("49g").unwrap().as_deref(), Some(&b"two"[..]));
        assert!(store.save("../x", b"no").is_err(), "a model's name only");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A link planted where the temporary file would go (the old,
    /// predictable name) or a file squatting on a name is never written
    /// through: the temporary file is new, randomly named, made with
    /// `create_new`.
    #[cfg(unix)]
    #[test]
    fn atomic_writes_do_not_follow_planted_links() {
        let dir = std::env::temp_dir().join(format!("saturnus-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let victim = dir.join("victim");
        std::fs::write(&victim, b"victim").unwrap();
        let file = dir.join("calc.state");
        let old = dir.join(format!(".calc.state.{}.tmp", std::process::id()));
        std::os::unix::fs::symlink(&victim, &old).unwrap();
        write_atomic(&file, b"state", |f, b| {
            use std::io::Write as _;
            f.write_all(b)
        })
        .unwrap();
        assert_eq!(std::fs::read(&victim).unwrap(), b"victim");
        assert_eq!(std::fs::read(&file).unwrap(), b"state");
        assert!(std::fs::symlink_metadata(&file).unwrap().is_file());
        // `create_new` refuses a link at the name it opens.
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        assert!(opts.open(&old).is_err());
        assert_ne!(random_u64(), random_u64());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
