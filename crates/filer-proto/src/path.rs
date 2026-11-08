//! DOS paths as a user types them (`C:/_DAT/PHONE.PDB`, `sub\note.txt`)
//! to the canonical bytes the Filer protocol carries (`C:\_DAT\PHONE.PDB`).

use std::fmt;

/// Why a path was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    /// A character outside printable ASCII.
    NotAscii(char),
    /// A drive part that is not one letter and a colon.
    BadDrive(String),
    /// `..` above the root.
    AboveRoot,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::NotAscii(c) => write!(f, "{c:?} is not printable ASCII"),
            PathError::BadDrive(d) => write!(f, "{d:?} is not a drive letter and a colon"),
            PathError::AboveRoot => f.write_str("`..` goes above the root"),
        }
    }
}

impl std::error::Error for PathError {}

/// The canonical form of `input`: upper case, `\` as separator, a drive
/// (`default_drive` when it has none), absolute from the root, `.` and
/// `..` resolved, no trailing separator except on the root. `/` is taken
/// as `\` so that shells need no quoting. DOS wildcards pass through.
pub fn normalize(input: &str, default_drive: u8) -> Result<Vec<u8>, PathError> {
    if let Some(c) = input.chars().find(|c| !(' '..='~').contains(c)) {
        return Err(PathError::NotAscii(c));
    }
    let bytes = input.trim().as_bytes();
    let (drive, rest) = match bytes.iter().position(|&b| b == b':') {
        Some(1) if bytes[0].is_ascii_alphabetic() => (bytes[0], &bytes[2..]),
        Some(i) => {
            return Err(PathError::BadDrive(
                String::from_utf8_lossy(&bytes[..=i]).into_owned(),
            ));
        }
        None => (default_drive, bytes),
    };
    let mut parts: Vec<Vec<u8>> = Vec::new();
    for part in rest.split(|&b| b == b'\\' || b == b'/') {
        match part {
            b"" | b"." => {}
            b".." => {
                parts.pop().ok_or(PathError::AboveRoot)?;
            }
            p => parts.push(p.to_ascii_uppercase()),
        }
    }
    let mut out = vec![drive.to_ascii_uppercase(), b':', b'\\'];
    out.extend(parts.join(&b'\\'));
    Ok(out)
}

/// `dir` and `name` joined with one `\`.
pub fn join(dir: &[u8], name: &[u8]) -> Vec<u8> {
    let mut out = dir.to_vec();
    if !out.ends_with(b"\\") {
        out.push(b'\\');
    }
    out.extend_from_slice(name);
    out
}

/// The last component (`C:\A\B.TXT` gives `B.TXT`; the root gives ``).
pub fn file_name(path: &[u8]) -> &[u8] {
    match path.iter().rposition(|&b| b == b'\\' || b == b':') {
        Some(i) => &path[i + 1..],
        None => path,
    }
}

/// The directory a canonical path is in (`C:\A\B` gives `C:\A`, `C:\A`
/// gives `C:\`); `None` for a root.
pub fn parent(path: &[u8]) -> Option<&[u8]> {
    if path.len() <= 3 {
        return None;
    }
    let i = path.iter().rposition(|&b| b == b'\\')?;
    Some(if i <= 2 { &path[..3] } else { &path[..i] })
}

/// Whether a component holds a DOS wildcard.
pub fn has_wildcard(name: &[u8]) -> bool {
    name.iter().any(|&b| b == b'*' || b == b'?')
}

/// The bytes as text (Latin-1 for 0x80-0xFF).
pub fn display(path: &[u8]) -> String {
    path.iter().map(|&b| char::from(b)).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        let n = |s| normalize(s, b'C').unwrap();
        assert_eq!(n(""), b"C:\\");
        assert_eq!(n("a:"), b"A:\\");
        assert_eq!(n("c:/_dat/phone.pdb"), b"C:\\_DAT\\PHONE.PDB");
        assert_eq!(n("sub\\note.txt"), b"C:\\SUB\\NOTE.TXT");
        assert_eq!(n("C:\\A\\..\\B\\.\\*.*"), b"C:\\B\\*.*");
        assert_eq!(n("/x/"), b"C:\\X");
        assert_eq!(normalize("..", b'C'), Err(PathError::AboveRoot));
        assert!(matches!(
            normalize("AB:x", b'C'),
            Err(PathError::BadDrive(_))
        ));
        assert_eq!(normalize("é", b'C'), Err(PathError::NotAscii('é')));
        assert_eq!(join(b"C:\\", b"X"), b"C:\\X");
        assert_eq!(join(b"C:\\A", b"X"), b"C:\\A\\X");
        assert_eq!(file_name(b"C:\\A\\B.TXT"), b"B.TXT");
        assert_eq!(file_name(b"C:\\"), b"");
        assert_eq!(parent(b"C:\\A\\B"), Some(&b"C:\\A"[..]));
        assert_eq!(parent(b"C:\\A"), Some(&b"C:\\"[..]));
        assert_eq!(parent(b"C:\\"), None);
    }
}
