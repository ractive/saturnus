//! ROM images: loading with a size check, and `rom fetch` from hpcalc.org
//! (kb: docs/clean-room-rule). ROMs are never bundled; the user downloads
//! them after a confirmation prompt.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use saturnus::Model;

use saturnus_host::sha256;

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

/// The ROM source for `model`, or `None` for the 42S: HP never released
/// the Pioneer ROMs and no site may offer them; the owner dumps their own
/// calculator (kb: iteration 15).
pub fn source(model: Model) -> Option<RomSource> {
    Some(match model {
        // 48SX ROM revision J, observed on 2026-10-05.
        Model::Hp48sx => RomSource {
            url: "https://www.hpcalc.org/hp48/pc/emulators/sxrom-j.zip",
            file: "sxrom-j",
            sha256: "e5eb3af020e4910f35a7580a705cf0a46f3ba9d7ba5516582d98010c93af7c74",
        },
        // 48GX ROM revision R, observed on 2026-10-05.
        Model::Hp48gx => RomSource {
            url: "https://www.hpcalc.org/hp48/pc/emulators/gxrom-r.zip",
            file: "gxrom-r",
            sha256: "de3a5a07b0f00640f4ba3599ea4092e9473113aad75c04bd03d3e37c059b5b33",
        },
        // 38G ROM revision A1.67, observed on 2026-10-05. The image is
        // packed and its I/O window (#00100-#0013F) is already zeroed.
        Model::Hp38g => RomSource {
            url: "https://www.hpcalc.org/hp38/pc/38grom.zip",
            file: "38G_A167.ROM",
            sha256: "3c9f747f637757d3adc414ed14d7f3636033f34f0a72e6e453ee197987f16be7",
        },
        // 49G ROM 2.15, the image the saturnng 49g oracle runs: the whole
        // 2 MB flash, packed. Its readme labels it for the 48gII/49g+/50g
        // and its boot sector is not the original 49G one, but it boots as
        // a 49G (kb: iteration 5). Observed on 2026-10-05. Fallback with
        // the original 49G boot sector: ROM 2.10,
        // https://www.hpcalc.org/hp49/pc/rom/hp4950v210.zip member
        // `rom.49g`, SHA-256
        // 58c3de6b7fc75a0ba65fca7437c4d49d8f26ca9e334a57e4d3bc4f8fb2dc8c11
        // (download by hand; see README).
        Model::Hp49g => RomSource {
            url: "https://www.hpcalc.org/hp49/pc/rom/hp4950emurom.zip",
            file: "rom.49g",
            sha256: "b01c13e24a692f35e6087106d58ec205b4696d5b5e35d57f8f94015f8bb1f1ca",
        },
        // 39G/40G ROM, one image for both models (hpcalc details 6739;
        // the page links `../hp39/pc/rom3940.zip`, observed on
        // 2026-10-05). `rom.39g` holds the 1 MB ROM unpacked, one nibble
        // per byte, with the I/O window of an upload at #00100-#0013F,
        // which the machine zeroes when it loads the image.
        Model::Hp39g | Model::Hp40g => RomSource {
            url: "https://www.hpcalc.org/hp39/pc/rom3940.zip",
            file: "rom.39g",
            sha256: "69220f42d5e90dd8825e7d1596d9eaca490ee6a7a52a3b8b96469a5f3d3f627f",
        },
        Model::Hp42s => return None,
    })
}

pub use saturnus_drive::rom::load;

/// The revision of a known image by SHA-256; the one list of known images
/// is `saturnus_host::romid::KNOWN`, which the pages and the app share.
pub use saturnus_host::romid::revision;

/// Check size and checksum of `data` against `src`.
fn verify(model: Model, src: &RomSource, data: &[u8]) -> Result<()> {
    if !model.accepts_rom_len(data.len()) {
        bail!(
            "{} is {} bytes, expected {}",
            src.file,
            data.len(),
            model.rom_bytes()
        );
    }
    let digest = sha256::hex_digest(data);
    if src.sha256.is_empty() {
        bail!(
            "no checksum recorded for {} yet; refusing to trust the download",
            src.file
        );
    }
    if digest != src.sha256 {
        bail!("{} has SHA-256 {digest}, expected {}", src.file, src.sha256);
    }
    Ok(())
}

/// `saturnus rom fetch`: download, unzip and verify the ROM into `dir`.
/// An existing file that verifies is kept without downloading.
pub fn fetch(model: Model, dir: &Path, yes: bool) -> Result<PathBuf> {
    let Some(src) = source(model) else {
        bail!(
            "no download for the {}: HP never released its ROM; dump your own calculator \
             and pass the 64 KB image with --rom",
            model.name().to_uppercase()
        );
    };
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
        let src = source(Model::Hp48sx).unwrap();
        let e = verify(Model::Hp48sx, &src, &[0; 10]).unwrap_err();
        assert!(e.to_string().contains("262144"), "{e}");
        let e = verify(Model::Hp48sx, &src, &vec![0; 262_144]).unwrap_err();
        assert!(e.to_string().contains("SHA-256"), "{e}");
        let gx = source(Model::Hp48gx).unwrap();
        assert_eq!(gx.file, "gxrom-r");
        let e = verify(Model::Hp48gx, &gx, &vec![0; 262_144]).unwrap_err();
        assert!(e.to_string().contains("524288"), "{e}");
        // The 39G image is unpacked: 2 MB passes the size check.
        let g39 = source(Model::Hp39g).unwrap();
        assert_eq!(g39.file, source(Model::Hp40g).unwrap().file);
        let e = verify(Model::Hp39g, &g39, &vec![0; 2 * 1_048_576]).unwrap_err();
        assert!(e.to_string().contains("SHA-256"), "{e}");
        assert!(source(Model::Hp42s).is_none());
        let e = fetch(Model::Hp42s, Path::new("unused"), true).unwrap_err();
        assert!(e.to_string().contains("--rom"), "{e}");
    }

    #[test]
    fn every_fetched_rom_has_a_known_revision() {
        for m in Model::ALL {
            if let Some(src) = source(m) {
                assert!(revision(src.sha256).is_some(), "{m:?}");
            }
        }
        assert_eq!(revision(&"0".repeat(64)), None);
    }
}
