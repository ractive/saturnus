//! Loading ROM images with a size check. ROMs are never bundled (kb:
//! docs/clean-room-rule); `saturnus rom fetch` downloads them.

use std::path::Path;

use anyhow::{Context, Result, bail};
use saturnus::Model;

/// Read a packed ROM image and check its size for `model`. The checksum is
/// not enforced, so other ROM revisions of the same size load.
pub fn load(model: Model, path: &Path) -> Result<Vec<u8>> {
    let rom = std::fs::read(path).with_context(|| format!("cannot read ROM {}", path.display()))?;
    // The 49G also takes its flash image unpacked (one nibble per byte),
    // the form of the ROM 1.19-6 emulator image.
    let unpacked_ok = model == Model::Hp49g && rom.len() == 2 * model.rom_bytes();
    if rom.len() != model.rom_bytes() && !unpacked_ok {
        bail!(
            "ROM {} is {} bytes, the {model:?} needs a packed image of {} bytes",
            path.display(),
            rom.len(),
            model.rom_bytes()
        );
    }
    Ok(rom)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

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
