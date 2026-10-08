//! Downloading a known ROM image from hpcalc.org on the user's machine,
//! for `saturnus rom fetch` and the desktop app (kb:
//! iterations/iteration-20b-rom-download). saturnus never hosts or proxies
//! an image; the hosts ask the user first.
//!
//! The download runs the system `curl` with its own user agent (hpcalc.org
//! serves junk, a gzip bomb, to agents that pretend to be a browser) and
//! without asking for a compressed transfer, reading at most [`MAX_ZIP`]
//! bytes. The image is taken out of the zip here (a stored or deflated
//! member, at most its expected size), its size and SHA-256 checked, and
//! only then written, whole, into the target directory.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use saturnus_host::romid::KnownRom;
use saturnus_host::sha256;

/// Largest zip downloaded: the published zips are at most about 1 MB.
pub const MAX_ZIP: u64 = 8 * 1024 * 1024;

/// The image to fetch: where it is and what it must be.
#[derive(Clone, Copy, Debug)]
pub struct Wanted<'a> {
    /// The zip's URL.
    pub url: &'a str,
    /// The image's name inside the zip, and on disk.
    pub file: &'a str,
    /// Its size in bytes.
    pub size: usize,
    /// Its SHA-256, lowercase hex.
    pub sha256: &'a str,
}

impl From<&KnownRom> for Wanted<'static> {
    fn from(k: &KnownRom) -> Self {
        Self {
            url: k.url,
            file: k.file,
            size: k.size,
            sha256: k.sha256,
        }
    }
}

/// What [`fetch`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fetched {
    /// The file was there already and verified; nothing was downloaded.
    Present(PathBuf),
    /// Downloaded, verified and stored.
    Downloaded(PathBuf),
}

impl Fetched {
    /// The image's path.
    pub fn path(&self) -> &Path {
        match self {
            Fetched::Present(p) | Fetched::Downloaded(p) => p,
        }
    }
}

/// Check `data` is the wanted image: its size, then its SHA-256.
pub fn verify(w: &Wanted, data: &[u8]) -> Result<(), String> {
    if data.len() != w.size {
        return Err(format!(
            "{} is {} bytes, expected {}",
            w.file,
            data.len(),
            w.size
        ));
    }
    let digest = sha256::hex_digest(data);
    if digest != w.sha256 {
        return Err(format!(
            "{} has SHA-256 {digest}, expected {}",
            w.file, w.sha256
        ));
    }
    Ok(())
}

/// The image `w` in `dir`: kept if `dir/file` already holds it, else
/// downloaded, verified and written there (the directory is made if
/// needed). A file there that does not verify is an error, unless
/// `replace` (the app's own directory), when the download replaces it.
pub fn fetch(w: &Wanted, dir: &Path, replace: bool) -> Result<Fetched, String> {
    fetch_capped(w, dir, replace, MAX_ZIP)
}

fn fetch_capped(w: &Wanted, dir: &Path, replace: bool, cap: u64) -> Result<Fetched, String> {
    let target = dir.join(w.file);
    if target.exists() {
        let have = crate::files::read_capped(&target, crate::files::max_rom_file())
            .and_then(|data| verify(w, &data));
        match have {
            Ok(()) => return Ok(Fetched::Present(target)),
            Err(e) if !replace => {
                return Err(format!(
                    "the existing {} does not verify ({e}); remove it to download it again",
                    target.display()
                ));
            }
            Err(_) => {}
        }
    }
    let zip = get(w.url, cap)?;
    let rom = unzip(&zip, w.file, w.size)?;
    verify(w, &rom)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    crate::files::write_atomic(&target, &rom, std::io::Write::write_all)
        .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
    Ok(Fetched::Downloaded(target))
}

/// GET `url` with the system `curl`, at most `cap` bytes.
fn get(url: &str, cap: u64) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("curl");
    // `-q` first: no `.curlrc`, so no configured user agent or
    // compression. curl's own user agent, no Accept-Encoding.
    cmd.args([
        "-q",
        "--fail",
        "--location",
        "--silent",
        "--show-error",
        "--proto",
        "=http,https",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "30",
        // The app holds the page's command turn while this runs;
        // the zips are 1-3 MB, so two minutes is generous.
        "--max-time",
        "120",
        "--max-filesize",
    ])
    .arg(cap.to_string())
    .args(["--output", "-", "--"])
    .arg(url)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    // No console window flashing up from the desktop app.
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cannot run curl ({e}); is it installed?"))?;
    let mut body = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or("curl gave no output")?
        .take(cap + 1)
        .read_to_end(&mut body);
    if read.is_err() || body.len() as u64 > cap {
        let _ = child.kill();
        let _ = child.wait();
        return Err(match read {
            Err(e) => format!("the download failed: {e}"),
            Ok(_) => format!("the download is larger than {cap} bytes; refused"),
        });
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("curl failed: {e}"))?;
    // curl's "maximum file size exceeded", from the announced length.
    if out.status.code() == Some(63) {
        return Err(format!("the download is larger than {cap} bytes; refused"));
    }
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        let why = why.trim();
        return Err(if why.is_empty() {
            format!("the download failed (curl: {})", out.status)
        } else {
            format!("the download failed ({why})")
        });
    }
    Ok(body)
}

fn u16_at(b: &[u8], at: usize) -> Option<usize> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(usize::from(u16::from_le_bytes([s[0], s[1]])))
}

fn u32_at(b: &[u8], at: usize) -> Option<usize> {
    let s = b.get(at..at.checked_add(4)?)?;
    usize::try_from(u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok()
}

/// The member `name` of `zip`, stored or deflated, at most `cap` bytes
/// unpacked. Reads the central directory; no zip64, no encryption.
pub fn unzip(zip: &[u8], name: &str, cap: usize) -> Result<Vec<u8>, String> {
    const EOCD: usize = 0x0605_4b50;
    const CENTRAL: usize = 0x0201_4b50;
    const LOCAL: usize = 0x0403_4b50;
    let bad = || "the download is not a readable zip file".to_string();
    // The end record is the last 22 bytes, or followed by a comment.
    let floor = zip.len().saturating_sub(22 + 0xffff);
    let end = (floor..=zip.len().saturating_sub(22))
        .rev()
        .find(|&i| u32_at(zip, i) == Some(EOCD))
        .ok_or_else(bad)?;
    let entries = u16_at(zip, end + 10).ok_or_else(bad)?;
    let mut at = u32_at(zip, end + 16).ok_or_else(bad)?;
    for _ in 0..entries {
        if u32_at(zip, at) != Some(CENTRAL) {
            return Err(bad());
        }
        let field = |off: usize| u16_at(zip, at + off).ok_or_else(bad);
        let (flags, method) = (field(8)?, field(10)?);
        let packed = u32_at(zip, at + 20).ok_or_else(bad)?;
        let unpacked = u32_at(zip, at + 24).ok_or_else(bad)?;
        let (name_len, extra_len, comment_len) = (field(28)?, field(30)?, field(32)?);
        let local = u32_at(zip, at + 42).ok_or_else(bad)?;
        let entry_name = zip.get(at + 46..at + 46 + name_len).ok_or_else(bad)?;
        at += 46 + name_len + extra_len + comment_len;
        if entry_name != name.as_bytes() {
            continue;
        }
        if flags & 1 != 0 {
            return Err(format!("{name} is encrypted in the zip"));
        }
        if unpacked > cap {
            return Err(format!(
                "{name} is {unpacked} bytes in the zip, expected {cap}"
            ));
        }
        if u32_at(zip, local) != Some(LOCAL) {
            return Err(bad());
        }
        let data = local
            + 30
            + u16_at(zip, local + 26).ok_or_else(bad)?
            + u16_at(zip, local + 28).ok_or_else(bad)?;
        let raw = zip.get(data..data + packed).ok_or_else(bad)?;
        return match method {
            0 => Ok(raw.to_vec()),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(raw, cap)
                .map_err(|e| format!("{name} cannot be unpacked: {e}")),
            m => Err(format!("{name} is packed with method {m}, not read")),
        };
    }
    Err(format!("{name} is not in the zip"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::net::TcpListener;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "saturnus-fetch-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// A zip of one member, deflated or stored (no CRC: nothing here reads it).
    fn zip_of(name: &str, data: &[u8], deflate: bool) -> Vec<u8> {
        let body = if deflate {
            miniz_oxide::deflate::compress_to_vec(data, 6)
        } else {
            data.to_vec()
        };
        let method: u16 = if deflate { 8 } else { 0 };
        let n = u16::try_from(name.len()).unwrap();
        let (packed, unpacked) = (body.len() as u32, data.len() as u32);
        let mut z = Vec::new();
        z.extend(0x0403_4b50u32.to_le_bytes());
        z.extend([20, 0, 0, 0]);
        z.extend(method.to_le_bytes());
        z.extend([0; 8]); // time, date, crc
        z.extend(packed.to_le_bytes());
        z.extend(unpacked.to_le_bytes());
        z.extend(n.to_le_bytes());
        z.extend([0, 0]);
        z.extend(name.as_bytes());
        z.extend(&body);
        let central = z.len() as u32;
        z.extend(0x0201_4b50u32.to_le_bytes());
        z.extend([20, 0, 20, 0, 0, 0]);
        z.extend(method.to_le_bytes());
        z.extend([0; 8]);
        z.extend(packed.to_le_bytes());
        z.extend(unpacked.to_le_bytes());
        z.extend(n.to_le_bytes());
        z.extend([0; 12]); // extra, comment, disk, attributes
        z.extend(0u32.to_le_bytes()); // local header at 0
        z.extend(name.as_bytes());
        let size = z.len() as u32 - central;
        z.extend(0x0605_4b50u32.to_le_bytes());
        z.extend([0, 0, 0, 0, 1, 0, 1, 0]);
        z.extend(size.to_le_bytes());
        z.extend(central.to_le_bytes());
        z.extend([0, 0]);
        z
    }

    /// An HTTP server on 127.0.0.1 answering each connection with the next
    /// of `responses` (status, body); the request heads come back.
    fn serve(responses: Vec<(u16, Vec<u8>)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut heads = Vec::new();
            for (status, body) in responses {
                let (mut s, _) = listener.accept().unwrap();
                let mut head = Vec::new();
                let mut byte = [0u8];
                while !head.ends_with(b"\r\n\r\n") && s.read(&mut byte).unwrap() == 1 {
                    head.push(byte[0]);
                }
                heads.push(String::from_utf8_lossy(&head).into_owned());
                let _ = write!(
                    s,
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(&body);
            }
            heads
        });
        (url, handle)
    }

    fn image() -> Vec<u8> {
        (0..4096u32).map(|i| (i * 7 % 251) as u8).collect()
    }

    fn wanted<'a>(url: &'a str, sha: &'a str) -> Wanted<'a> {
        Wanted {
            url,
            file: "test.rom",
            size: 4096,
            sha256: sha,
        }
    }

    #[test]
    fn unzip_reads_stored_and_deflated_members() {
        let rom = image();
        for deflate in [false, true] {
            let z = zip_of("test.rom", &rom, deflate);
            assert_eq!(unzip(&z, "test.rom", 4096).unwrap(), rom);
            let e = unzip(&z, "other", 4096).unwrap_err();
            assert!(e.contains("not in the zip"), "{e}");
            let e = unzip(&z, "test.rom", 100).unwrap_err();
            assert!(e.contains("4096 bytes"), "{e}");
        }
        // A deflated member that claims less than it holds stops at the cap.
        let mut z = zip_of("test.rom", &rom, true);
        let central = u32_at(&z, z.len() - 6).unwrap();
        z[central + 24..central + 28].copy_from_slice(&100u32.to_le_bytes());
        assert!(unzip(&z, "test.rom", 100).is_err());
        assert!(unzip(b"not a zip at all, just text", "x", 10).is_err());
        assert!(unzip(&[], "x", 10).is_err());
    }

    /// Downloaded from a local stub with curl's own user agent and no
    /// compression asked for, verified, stored; a second fetch finds it.
    #[test]
    fn fetch_downloads_verifies_and_keeps() {
        let rom = image();
        let sha = sha256::hex_digest(&rom);
        let (base, server) = serve(vec![(200, zip_of("test.rom", &rom, true))]);
        let url = format!("{base}/rom.zip");
        let dir = temp("ok");
        let got = fetch(&wanted(&url, &sha), &dir, false).unwrap();
        assert_eq!(got, Fetched::Downloaded(dir.join("test.rom")));
        assert_eq!(std::fs::read(got.path()).unwrap(), rom);
        let heads = server.join().unwrap();
        assert!(heads[0].starts_with("GET /rom.zip "), "{}", heads[0]);
        let head = heads[0].to_ascii_lowercase();
        assert!(head.contains("\r\nuser-agent: curl/"), "{head}");
        assert!(!head.contains("accept-encoding"), "{head}");
        // Present and verified: no request (nothing listens there now).
        let got = fetch(&wanted(&url, &sha), &dir, false).unwrap();
        assert_eq!(got, Fetched::Present(dir.join("test.rom")));
        // A file there that does not verify: refused, or replaced.
        std::fs::write(dir.join("test.rom"), b"junk").unwrap();
        let e = fetch(&wanted(&url, &sha), &dir, false).unwrap_err();
        assert!(e.contains("remove it"), "{e}");
        let (base, server) = serve(vec![(200, zip_of("test.rom", &rom, false))]);
        let url = format!("{base}/rom.zip");
        let got = fetch(&wanted(&url, &sha), &dir, true).unwrap();
        assert_eq!(std::fs::read(got.path()).unwrap(), rom);
        server.join().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Nothing is stored when the download fails: a wrong checksum, an
    /// HTTP error, a body over the cap, a zip without the member.
    #[test]
    fn nothing_stored_on_failure() {
        let rom = image();
        let sha = sha256::hex_digest(&rom);
        let dir = temp("bad");
        let mut other = rom.clone();
        other[0] ^= 1;
        let (base, server) = serve(vec![
            (200, zip_of("test.rom", &other, true)),
            (404, b"no such file".to_vec()),
            (200, vec![0; 5000]),
            (200, zip_of("else.rom", &rom, true)),
        ]);
        let url = format!("{base}/rom.zip");
        let e = fetch(&wanted(&url, &sha), &dir, false).unwrap_err();
        assert!(e.contains("SHA-256"), "{e}");
        let e = fetch(&wanted(&url, &sha), &dir, false).unwrap_err();
        assert!(e.contains("404"), "{e}");
        let e = fetch_capped(&wanted(&url, &sha), &dir, false, 4000).unwrap_err();
        assert!(e.contains("larger than 4000"), "{e}");
        let e = fetch(&wanted(&url, &sha), &dir, false).unwrap_err();
        assert!(e.contains("not in the zip"), "{e}");
        server.join().unwrap();
        assert!(!dir.exists(), "nothing was stored");
        // Only http and https.
        let e = fetch(&wanted("file:///etc/passwd", &sha), &dir, false).unwrap_err();
        assert!(e.contains("download failed"), "{e}");
        assert!(!dir.exists());
    }

    #[test]
    fn known_images_convert() {
        let k = saturnus_host::romid::download(saturnus::Model::Hp48gx).unwrap();
        let w = Wanted::from(k);
        assert_eq!((w.file, w.size), ("gxrom-r", 524_288));
        let e = verify(&w, &vec![0; 524_288]).unwrap_err();
        assert!(e.contains("SHA-256"), "{e}");
        let e = verify(&w, &[0; 10]).unwrap_err();
        assert!(e.contains("524288"), "{e}");
    }
}
