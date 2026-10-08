//! The text the ROM's Kermit server returns, already translated from the
//! HP character set: the stack display after a host command (`C`) and the
//! directory listing (`G D`). wiki: protocols/server-commands.
//!
//! The 48SX, 48GX and 49G differ in details: the 49G prints reals with a
//! trailing dot (`10777.`), shows names without quotes and cuts long
//! values at the display width; only the 48GX and 49G put the path and
//! the free memory in front of a listing.

use anyhow::{Context, Result, bail};
pub use saturnus_objects::transfer::{StackReply, parse_stack};

/// A `G D` directory listing.
#[derive(Clone, Debug, PartialEq)]
pub struct Listing {
    /// The current directory, e.g. `["HOME", "D1"]` (48GX and 49G only).
    pub path: Option<Vec<String>>,
    /// The variables in calculator order.
    pub entries: Vec<Entry>,
}

/// One variable of a [`Listing`].
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The name.
    pub name: String,
    /// Size in bytes; can be a half (`29.5`).
    pub size: f64,
    /// The type name, e.g. `Real Number`, `Directory`.
    pub kind: String,
    /// The calculator's checksum of the object.
    pub checksum: u16,
}

/// Parse a `G D` listing: an optional header line `{ HOME D1 } 1234.`
/// (path and free memory), then one `NAME SIZE TYPE... CHECKSUM` line per
/// variable.
pub fn parse_listing(text: &str) -> Result<Listing> {
    let mut listing = Listing {
        path: None,
        entries: Vec::new(),
    };
    for (i, line) in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .enumerate()
    {
        if i == 0 && line.starts_with('{') {
            let end = line
                .find('}')
                .with_context(|| format!("bad directory header: {line:?}"))?;
            listing.path = Some(
                parse_list(&line[..=end])
                    .with_context(|| format!("bad directory header: {line:?}"))?,
            );
            continue;
        }
        listing.entries.push(parse_entry(line)?);
    }
    Ok(listing)
}

fn parse_entry(line: &str) -> Result<Entry> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let [name, size, kind @ .., checksum] = tokens.as_slice() else {
        bail!("bad directory line: {line:?}");
    };
    let size = parse_real(size).filter(|s| *s >= 0.0);
    let checksum = parse_real(checksum)
        .filter(|c| c.fract() == 0.0 && (0.0..=65535.0).contains(c))
        .map(|c| c as u16);
    match (size, checksum) {
        (Some(size), Some(checksum)) if !kind.is_empty() => Ok(Entry {
            name: (*name).to_string(),
            size,
            kind: kind.join(" "),
            checksum,
        }),
        _ => bail!("bad directory line: {line:?}"),
    }
}

/// A displayed real such as `127828.`, `1.5`, `-3` or `1.E-3`.
fn parse_real(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty()
        || !s
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'E' | 'e' | '+' | '-'))
    {
        return None;
    }
    s.parse().ok()
}

/// A displayed flat list as its items: `{ HOME D1 }`, `{9600.,0.}`, `{ }`.
/// `None` unless the text is one `{ }` group without nesting or quotes.
pub fn parse_list(s: &str) -> Option<Vec<String>> {
    let inner = s.trim().strip_prefix('{')?.strip_suffix('}')?;
    if inner.contains(['{', '}', '"', '\'']) {
        return None;
    }
    Some(
        inner
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listings() {
        let l =
            parse_listing("{ HOME D1 } 1234.5\r\nX 10.5 Real Number 12345\r\nD 5 Directory 7\r\n")
                .unwrap();
        assert_eq!(l.path, Some(vec!["HOME".into(), "D1".into()]));
        assert_eq!(l.entries.len(), 2);
        assert_eq!(l.entries[0].kind, "Real Number");
        assert_eq!(l.entries[0].size, 10.5);
        assert_eq!(l.entries[0].checksum, 12345);
        let sx = parse_listing("X 10.5 Real Number 12345\n").unwrap();
        assert_eq!(sx.path, None);
        assert!(parse_listing("X 10.5 12\n").is_err());
        assert!(parse_listing("X 1 Real 70000\n").is_err());
        assert_eq!(parse_list("{9600.,0.}").unwrap(), ["9600.", "0."]);
        assert_eq!(parse_list("{ }").unwrap(), Vec::<String>::new());
        assert_eq!(parse_list("{ { 1 } }"), None);
    }
}
