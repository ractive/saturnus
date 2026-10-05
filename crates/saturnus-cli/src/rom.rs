//! ROM images: loading with a size check, and `rom fetch` from hpcalc.org
//! (kb: docs/clean-room-rule). ROMs are never bundled; the user downloads
//! them after a confirmation prompt.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use saturnus::Model;

use crate::sha256;

/// Where to get a model's ROM and what to expect.
#[derive(Clone, Copy, Debug)]
pub struct RomSource {
    /// Download URL (a zip with one file).
    pub url: &'static str,
    /// Name of the ROM file inside the zip and on disk.
    pub file: &'static str,
    /// Expected SHA-256 of the unpacked ROM, lowercase hex.
    pub sha256: &'static str,
}

/// The ROM source for `model`.
pub fn source(model: Model) -> RomSource {
    match model {
        // 48SX ROM revision J, observed on 2026-10-05.
        Model::Hp48sx => RomSource {
            url: "https://www.hpcalc.org/hp48/pc/emulators/sxrom-j.zip",
            file: "sxrom-j",
            sha256: "e5eb3af020e4910f35a7580a705cf0a46f3ba9d7ba5516582d98010c93af7c74",
        },
    }
}

/// Read a packed ROM image and check its size for `model`. The checksum is
/// not enforced, so other ROM revisions of the same size load.
pub fn load(model: Model, path: &Path) -> Result<Vec<u8>> {
    let rom = std::fs::read(path).with_context(|| format!("cannot read ROM {}", path.display()))?;
    if rom.len() != model.rom_bytes() {
        bail!(
            "ROM {} is {} bytes, the {model:?} needs a packed image of {} bytes",
            path.display(),
            rom.len(),
            model.rom_bytes()
        );
    }
    Ok(rom)
}

/// Check size and checksum of `data` against `src`.
fn verify(model: Model, src: &RomSource, data: &[u8]) -> Result<()> {
    if data.len() != model.rom_bytes() {
        bail!(
            "{} is {} bytes, expected {}",
            src.file,
            data.len(),
            model.rom_bytes()
        );
    }
    let digest = sha256::hex_digest(data);
    if digest != src.sha256 {
        bail!("{} has SHA-256 {digest}, expected {}", src.file, src.sha256);
    }
    Ok(())
}

/// `saturnus rom fetch`: download, unzip and verify the ROM into `dir`.
/// An existing file that verifies is kept without downloading.
pub fn fetch(model: Model, dir: &Path, yes: bool) -> Result<PathBuf> {
    let src = source(model);
    let target = dir.join(src.file);
    if let Ok(data) = std::fs::read(&target) {
        verify(model, &src, &data).with_context(|| {
            format!(
                "existing {} does not verify; remove it to re-download",
                target.display()
            )
        })?;
        println!(
            "{} is present and verified (SHA-256 {})",
            target.display(),
            src.sha256
        );
        return Ok(target);
    }
    if !yes && !confirm(&src, dir)? {
        bail!("download cancelled");
    }
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let zip = dir.join(format!("{}.zip", src.file));
    // curl's own user agent: hpcalc.org serves junk to browser-like agents.
    run(
        Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--output",
            ])
            .arg(&zip)
            .arg(src.url),
        "curl",
    )?;
    extract(&zip, src.file, dir)?;
    let data = std::fs::read(&target)
        .with_context(|| format!("{} not found in {}", src.file, zip.display()))?;
    verify(model, &src, &data)?;
    std::fs::remove_file(&zip).with_context(|| format!("cannot remove {}", zip.display()))?;
    println!(
        "{} downloaded and verified (SHA-256 {})",
        target.display(),
        src.sha256
    );
    Ok(target)
}

fn confirm(src: &RomSource, dir: &Path) -> Result<bool> {
    print!(
        "Download {} into {}?\nThe ROM is HP's copyrighted software, which HP allows to be downloaded \
         from hpcalc.org for use with emulators. [y/N] ",
        src.url,
        dir.display()
    );
    std::io::stdout().flush().context("cannot write prompt")?;
    let mut answer = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut answer)
        .context("cannot read the answer")?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES" | "Yes"))
}

/// Unzip `member` from `zip` into `dir` with the system `unzip`, falling
/// back to `tar` (bsdtar reads zip files on macOS and Windows).
fn extract(zip: &Path, member: &str, dir: &Path) -> Result<()> {
    let unzip = Command::new("unzip")
        .arg("-o")
        .arg("-q")
        .arg(zip)
        .arg(member)
        .arg("-d")
        .arg(dir)
        .status();
    match unzip {
        Ok(s) if s.success() => Ok(()),
        _ => run(
            Command::new("tar")
                .arg("-xf")
                .arg(zip)
                .arg("-C")
                .arg(dir)
                .arg(member),
            "unzip or tar",
        ),
    }
}

fn run(cmd: &mut Command, what: &str) -> Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("cannot run {what}; is it installed?"))?;
    if !status.success() {
        bail!("{what} failed ({status})");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_checks_size_then_digest() {
        let src = source(Model::Hp48sx);
        let e = verify(Model::Hp48sx, &src, &[0; 10]).unwrap_err();
        assert!(e.to_string().contains("262144"), "{e}");
        let e = verify(Model::Hp48sx, &src, &vec![0; 262_144]).unwrap_err();
        assert!(e.to_string().contains("SHA-256"), "{e}");
    }

    #[test]
    fn load_rejects_wrong_size() {
        let dir = std::env::temp_dir().join(format!("saturnus-rom-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("short.rom");
        std::fs::write(&p, [0u8; 100]).unwrap();
        assert!(load(Model::Hp48sx, &p).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
