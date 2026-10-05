//! A private temporary directory for the machine states the generators
//! keep: created fresh (never an existing path, so nothing another user
//! placed in the shared temporary directory is followed), readable by the
//! owner only on Unix, removed with everything in it when dropped.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The directory; removed on drop.
#[derive(Debug)]
pub struct PrivateDir {
    path: PathBuf,
}

impl PrivateDir {
    /// A new directory `<temp>/saturnus-refgen-<pid>-<n>`, `n` counting up
    /// until a name is free.
    pub fn new() -> Result<Self> {
        let base = std::env::temp_dir();
        for n in 0..1000u32 {
            let path = base.join(format!("saturnus-refgen-{}-{n}", std::process::id()));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(PrivateDir { path }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    return Err(e).with_context(|| format!("cannot create {}", path.display()));
                }
            }
        }
        anyhow::bail!("no free private directory name in {}", base.display())
    }

    /// `name` inside the directory.
    pub fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        // Best effort: a leftover directory in the temporary directory.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_are_fresh_private_and_removed() {
        let a = PrivateDir::new().unwrap();
        let b = PrivateDir::new().unwrap();
        assert_ne!(a.path(), b.path());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(a.path()).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
        std::fs::write(a.file("x"), b"1").unwrap();
        let path = a.path().to_path_buf();
        drop(a);
        assert!(!path.exists());
    }
}
