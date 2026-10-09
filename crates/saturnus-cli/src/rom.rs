//! ROM images: loading with a size check, and `rom fetch` from hpcalc.org
//! (kb: docs/clean-room-rule). ROMs are never bundled; the user downloads
//! them after a confirmation prompt.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use saturnus::Model;
use saturnus_drive::fetch::{self, Fetched, Wanted};
use saturnus_host::romid::{self, KnownRom};

pub use saturnus_drive::rom::load;

/// The revision of a known image by SHA-256; the one list of known images,
/// with where hpcalc.org offers them, is `saturnus_host::romid::KNOWN`,
/// which the pages and the app share.
pub use saturnus_host::romid::revision;

/// `saturnus rom fetch`: download, unzip and verify the ROM into `dir`
/// (`saturnus_drive::fetch`). An existing file that verifies is kept
/// without downloading.
pub fn fetch(model: Model, dir: &Path, yes: bool) -> Result<PathBuf> {
    let Some(known) = romid::download(model) else {
        bail!(
            "there is no download for the {}: HP never published its ROM. Read the 64 KB \
             ROM out of your own calculator and pass the file with --rom",
            model.name().to_uppercase()
        );
    };
    let wanted = Wanted::from(known);
    let target = dir.join(known.file);
    // Asked only when there is something to download.
    let present =
        saturnus_drive::files::read_capped(&target, saturnus_drive::files::max_rom_file())
            .is_ok_and(|d| fetch::verify(&wanted, &d).is_ok());
    if !present && target.exists() {
        bail!(
            "{} is not the expected ROM file (wrong size or SHA-256); delete it and \
             run this again",
            target.display()
        );
    }
    if !present && !yes && !confirm(known, dir)? {
        bail!("download cancelled");
    }
    let got = fetch::fetch(&wanted, dir, false)
        .map_err(anyhow::Error::msg)
        .with_context(|| {
            format!(
                "cannot download the {} ROM; download it yourself from {}",
                model.name().to_uppercase(),
                known.page
            )
        })?;
    let what = match got {
        Fetched::Present(_) => "is present and verified",
        Fetched::Downloaded(_) => "downloaded and verified",
    };
    println!("{} {what} (SHA-256 {})", got.path().display(), known.sha256);
    Ok(got.path().to_path_buf())
}

fn confirm(known: &KnownRom, dir: &Path) -> Result<bool> {
    print!(
        "Download {} into {}?\nThe ROM is HP's copyrighted software, hosted by hpcalc.org with \
         HP's permission for use with emulators; it is not part of saturnus. [y/N] ",
        known.url,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_42s_has_no_download() {
        let e = fetch(Model::Hp42s, Path::new("unused"), true).unwrap_err();
        assert!(e.to_string().contains("--rom"), "{e}");
    }

    /// An existing file that does not verify is refused before anything
    /// is downloaded or asked.
    #[test]
    fn a_bad_existing_file_is_refused() {
        let dir = std::env::temp_dir().join(format!("saturnus-cli-rom-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("sxrom-j"), [0u8; 10]).unwrap();
        let e = fetch(Model::Hp48sx, &dir, false).unwrap_err();
        assert!(e.to_string().contains("delete it"), "{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
