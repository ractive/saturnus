//! The control API's per-user token file (kb: docs/control-api-security).
//!
//! `saturnus run` creates it on first use: 256 bits from the operating
//! system's random source, as 64 hex digits. On Unix the file is created
//! with mode 0600 in a directory created with mode 0700, and a file that
//! other users may read is refused. On Windows it lives under the user's
//! profile (`%LOCALAPPDATA%`), whose default ACL gives only the user (and
//! SYSTEM and the administrators) access; no ACL is set by saturnus.
//!
//! The token never leaves this module in readable form except as the
//! `Authorization` header the client sends: its `Debug` is redacted, and
//! no message here contains it.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Overrides the token file's location (for tests and second accounts).
pub const TOKEN_ENV: &str = "SATURNUS_TOKEN_FILE";
/// Hex digits in a token: 256 bits.
const TOKEN_HEX: usize = 64;

/// The shared secret of the control API.
#[derive(Clone)]
pub struct Token(String);

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(redacted)")
    }
}

impl Token {
    /// Whether `presented` (the credentials of an `Authorization: Bearer`
    /// header) is this token, compared in constant time.
    pub fn matches(&self, presented: &[u8]) -> bool {
        constant_time_eq(self.0.as_bytes(), presented)
    }

    /// The `Authorization` header value the client sends.
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }

    /// A fresh token from the OS random source.
    fn generate() -> Result<Self> {
        Ok(Self(random_hex()?))
    }

    /// The server's proof that it holds this token, for the client's
    /// `nonce` on `port` (the port the server is bound to, so a proof
    /// relayed from a server on another port does not match):
    /// HMAC-SHA-256 keyed with the token, as hex. `saturnus ctl` checks it
    /// before it sends the token (`GET /v1/hello`).
    pub fn proof(&self, nonce: &str, port: u16) -> String {
        hmac_sha256(
            self.0.as_bytes(),
            format!("saturnus control hello\n{nonce}\n{port}").as_bytes(),
        )
    }
}

/// 256 bits from the OS random source, as 64 hex digits (a token, a
/// nonce).
pub fn random_hex() -> Result<String> {
    let mut bytes = [0u8; TOKEN_HEX / 2];
    getrandom::fill(&mut bytes)
        .map_err(|e| anyhow::anyhow!("cannot get random bytes from the OS: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// HMAC-SHA-256 (RFC 2104) of `msg` under `key` (at most one 64-byte
/// block, as the token is), as hex.
fn hmac_sha256(key: &[u8], msg: &[u8]) -> String {
    use saturnus_host::sha256::hex_digest;
    let mut k = [0u8; 64];
    for (d, s) in k.iter_mut().zip(key) {
        *d = *s;
    }
    let pad = |b: u8| k.iter().map(move |x| x ^ b);
    let inner: Vec<u8> = pad(0x36).chain(msg.iter().copied()).collect();
    let inner = hex_digest(&inner);
    let inner = (0..inner.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(inner.get(i..i + 2)?, 16).ok());
    let outer: Vec<u8> = pad(0x5c).chain(inner).collect();
    hex_digest(&outer)
}

/// Equal length and equal bytes, in time independent of where they differ.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let diff = a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y));
    std::hint::black_box(diff) == 0
}

/// Where the token file lives by default: `%LOCALAPPDATA%\saturnus\
/// control-token` on Windows, `$XDG_CONFIG_HOME/saturnus/control-token`
/// or `~/.config/saturnus/control-token` elsewhere.
pub fn default_path() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA")
            .filter(|v| !v.is_empty())
            .context("LOCALAPPDATA is not set: pass --token-file or set SATURNUS_TOKEN_FILE")?;
        Ok(PathBuf::from(base).join("saturnus").join("control-token"))
    }
    #[cfg(not(windows))]
    {
        let base = match std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
        {
            Some(p) => p,
            None => std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|h| PathBuf::from(h).join(".config"))
                .context("HOME is not set: pass --token-file or set SATURNUS_TOKEN_FILE")?,
        };
        Ok(base.join("saturnus").join("control-token"))
    }
}

/// The token file: `flag`, else `$SATURNUS_TOKEN_FILE`, else
/// [`default_path`].
pub fn resolve(flag: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = flag {
        return Ok(p.to_path_buf());
    }
    match std::env::var_os(TOKEN_ENV).filter(|v| !v.is_empty()) {
        Some(p) => Ok(PathBuf::from(p)),
        None => default_path(),
    }
}

/// The token in `path` for the server: read it, or create the file with a
/// new token if there is none.
pub fn load_or_create(path: &Path) -> Result<Token> {
    // Another process may be creating it right now: a short file is read
    // again a few times before it counts as broken.
    for attempt in 0..5 {
        match read(path) {
            Ok(t) => return Ok(t),
            Err(ReadError::Missing) => match create(path) {
                Ok(t) => return Ok(t),
                Err(e) if is_exists(&e) => {}
                Err(e) => return Err(e),
            },
            Err(ReadError::Malformed) if attempt < 4 => {}
            Err(e) => return Err(e.into_error(path)),
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err(ReadError::Malformed.into_error(path))
}

/// The token in `path` for the client.
pub fn load(path: &Path) -> Result<Token> {
    read(path).map_err(|e| match e {
        ReadError::Missing => anyhow::anyhow!(
            "no token file at {}: start `saturnus run --serve` first, which creates it (or pass \
             --token-file / set {TOKEN_ENV})",
            path.display()
        ),
        e => e.into_error(path),
    })
}

fn is_exists(e: &anyhow::Error) -> bool {
    e.downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::AlreadyExists)
}

#[derive(Debug)]
enum ReadError {
    Missing,
    Malformed,
    Open(std::io::Error),
    NotAFile,
    #[cfg(unix)]
    Exposed(u32),
}

impl ReadError {
    fn into_error(self, path: &Path) -> anyhow::Error {
        let p = path.display();
        match self {
            Self::Missing => anyhow::anyhow!("no token file at {p}"),
            Self::Malformed => anyhow::anyhow!(
                "the token file {p} is malformed (64 hex digits expected); delete it, and \
                 the next `saturnus run --serve` makes a new one"
            ),
            Self::Open(e) => anyhow::anyhow!("cannot read the token file {p}: {e}"),
            Self::NotAFile => anyhow::anyhow!("the token file {p} is not a regular file"),
            #[cfg(unix)]
            Self::Exposed(mode) => anyhow::anyhow!(
                "the token file {p} has mode {mode:o}: other users may read it; run \
                 `chmod 600 {p}` (or delete it for a new token)"
            ),
        }
    }
}

fn read(path: &Path) -> std::result::Result<Token, ReadError> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ReadError::Missing),
        Err(e) => return Err(ReadError::Open(e)),
    };
    if !meta.is_file() {
        return Err(ReadError::NotAFile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(ReadError::Exposed(mode));
        }
    }
    let text = std::fs::read_to_string(path).map_err(ReadError::Open)?;
    let t = text.trim();
    if t.len() != TOKEN_HEX || !t.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ReadError::Malformed);
    }
    Ok(Token(t.to_ascii_lowercase()))
}

/// Create `path` with a new token, failing if it exists.
fn create(path: &Path) -> Result<Token> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        create_dir(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let token = Token::generate()?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let mut f = match opts.open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(e.into()),
        Err(e) => {
            return Err(e)
                .with_context(|| format!("cannot create the token file {}", path.display()));
        }
    };
    let written = f
        .write_all(format!("{}\n", token.0).as_bytes())
        .and_then(|()| f.sync_all());
    if let Err(e) = written {
        drop(f);
        let _ = std::fs::remove_file(path);
        return Err(e).with_context(|| format!("cannot write the token file {}", path.display()));
    }
    Ok(token)
}

#[cfg(unix)]
fn create_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(not(unix))]
fn create_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// RFC 4231 test case 2; and a proof depends on the nonce and port.
    #[test]
    fn hmac_and_proofs() {
        assert_eq!(
            hmac_sha256(b"Jefe", b"what do ya want for nothing?"),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        let t = Token(random_hex().unwrap());
        let n = random_hex().unwrap();
        assert_eq!(t.proof(&n, 4840), t.proof(&n, 4840));
        assert_ne!(t.proof(&n, 4840), t.proof(&n, 4841));
        assert_ne!(t.proof(&n, 4840), t.proof(&random_hex().unwrap(), 4840));
        assert_ne!(
            t.proof(&n, 4840),
            Token(random_hex().unwrap()).proof(&n, 4840)
        );
    }

    /// The token of hex `s`.
    pub(crate) fn token_of(s: &str) -> Token {
        Token(s.to_string())
    }

    /// A temporary directory removed on drop.
    pub(crate) struct TempDir(pub PathBuf);

    impl TempDir {
        pub(crate) fn new(tag: &str) -> Self {
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!("saturnus-{tag}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn created_once_then_read_back() {
        let dir = TempDir::new("token");
        let p = dir.0.join("sub").join("control-token");
        assert!(
            load(&p)
                .unwrap_err()
                .to_string()
                .contains("start `saturnus run --serve`")
        );
        let t = load_or_create(&p).unwrap();
        assert!(t.bearer().starts_with("Bearer "));
        assert_eq!(t.bearer().len(), 7 + TOKEN_HEX);
        let again = load_or_create(&p).unwrap();
        assert!(again.matches(t.0.as_bytes()));
        assert!(load(&p).unwrap().matches(t.0.as_bytes()));
        assert_eq!(format!("{t:?}"), "Token(redacted)");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            let dmode = std::fs::metadata(p.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dmode, 0o700);
        }
    }

    #[test]
    fn two_tokens_differ() {
        let a = Token::generate().unwrap();
        let b = Token::generate().unwrap();
        assert!(!a.matches(b.0.as_bytes()));
    }

    #[test]
    fn malformed_and_exposed_files_are_refused_without_their_content() {
        let dir = TempDir::new("token-bad");
        let p = dir.0.join("control-token");
        std::fs::write(&p, "secret-but-short").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let e = load(&p).unwrap_err().to_string();
        assert!(e.contains("malformed"), "{e}");
        assert!(!e.contains("secret"), "{e}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::write(&p, "a".repeat(64)).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
            let e = load_or_create(&p).unwrap_err().to_string();
            assert!(e.contains("chmod 600"), "{e}");
            assert!(!e.contains(&"a".repeat(64)), "{e}");
        }
    }

    #[test]
    fn constant_time_eq_compares_whole_strings() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"", b"a"));
    }
}
