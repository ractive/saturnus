//! The little HTTP/1.1 the control API needs, over blocking `TcpStream`s:
//! one request per connection (`Connection: close`), bodies only with
//! `Content-Length`, every read bounded in size and wall time. Shared by
//! the server and the `ctl` client.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::Instant;

/// Most bytes of a request or response head (request line and headers).
pub const MAX_HEAD: usize = 16 * 1024;
/// Most header lines in a head.
pub const MAX_HEADERS: usize = 64;

/// Why reading from a peer stopped.
#[derive(Debug)]
pub enum ReadError {
    /// The deadline passed.
    Timeout,
    /// The head or body grew past its cap.
    TooLarge,
    /// The peer closed the connection early.
    Closed,
    /// Not HTTP as this module speaks it.
    Bad(String),
    /// A socket error.
    Io(std::io::Error),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => f.write_str("timed out"),
            Self::TooLarge => f.write_str("too large"),
            Self::Closed => f.write_str("connection closed early"),
            Self::Bad(m) => f.write_str(m),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

/// A parsed head: the first line split in three, and the headers with
/// lowercase names.
#[derive(Debug)]
pub struct Head {
    /// The request method, or the response's protocol version.
    pub first: String,
    /// The request target, or the response's status code.
    pub second: String,
    /// The request's protocol version, or the response's reason phrase.
    pub third: String,
    /// `(lowercase name, value)` in order.
    pub headers: Vec<(String, String)>,
}

impl Head {
    /// The value of the one header called `name` (lowercase); an error if
    /// it appears more than once.
    pub fn header(&self, name: &str) -> Result<Option<&str>, String> {
        let mut found = self
            .headers
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.as_str());
        let first = found.next();
        if found.next().is_some() {
            return Err(format!("more than one {name} header"));
        }
        Ok(first)
    }

    /// The `Content-Length`, 0 without one; refuses `Transfer-Encoding`.
    pub fn content_length(&self) -> Result<usize, String> {
        if self.header("transfer-encoding")?.is_some() {
            return Err("Transfer-Encoding is not supported: send a Content-Length".into());
        }
        match self.header("content-length")? {
            None => Ok(0),
            Some(v) => v.parse().map_err(|_| format!("bad Content-Length {v:?}")),
        }
    }
}

/// Read from `s` into `buf` until `deadline`; `Ok(0)` is the end.
fn read_some(s: &mut TcpStream, buf: &mut [u8], deadline: Instant) -> Result<usize, ReadError> {
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(ReadError::Timeout);
        }
        s.set_read_timeout(Some(left)).map_err(ReadError::Io)?;
        match s.read(buf) {
            Ok(n) => return Ok(n),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return Err(ReadError::Timeout);
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(ReadError::Io(e)),
        }
    }
}

/// Read a head (up to the empty line) by `deadline`; returns it parsed and
/// the bytes that followed it (the start of the body).
pub fn read_head(s: &mut TcpStream, deadline: Instant) -> Result<(Head, Vec<u8>), ReadError> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = data.split_off(end + 4);
            data.truncate(end);
            return parse_head(&data).map(|h| (h, rest));
        }
        if data.len() > MAX_HEAD {
            return Err(ReadError::TooLarge);
        }
        let n = read_some(s, &mut buf, deadline)?;
        if n == 0 {
            return Err(ReadError::Closed);
        }
        data.extend_from_slice(&buf[..n]);
    }
}

/// Parse a head without its final empty line.
pub fn parse_head(data: &[u8]) -> Result<Head, ReadError> {
    if data.len() > MAX_HEAD {
        return Err(ReadError::TooLarge);
    }
    let text = std::str::from_utf8(data).map_err(|_| ReadError::Bad("head is not UTF-8".into()))?;
    let mut lines = text.split("\r\n");
    let first_line = lines.next().unwrap_or_default();
    let mut parts = first_line.splitn(3, ' ');
    let (Some(first), Some(second), Some(third)) = (parts.next(), parts.next(), parts.next())
    else {
        return Err(ReadError::Bad(format!("bad first line {first_line:?}")));
    };
    let mut headers = Vec::new();
    for line in lines {
        if headers.len() >= MAX_HEADERS {
            return Err(ReadError::TooLarge);
        }
        if line.starts_with([' ', '\t']) {
            return Err(ReadError::Bad(
                "folded header lines are not accepted".into(),
            ));
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(ReadError::Bad("header line without a colon".into()));
        };
        if name.is_empty() || name.contains([' ', '\t']) {
            return Err(ReadError::Bad("bad header name".into()));
        }
        headers.push((name.to_ascii_lowercase(), value.trim().to_string()));
    }
    Ok(Head {
        first: first.to_string(),
        second: second.to_string(),
        third: third.to_string(),
        headers,
    })
}

/// Read a body of exactly `len` bytes, `start` already read, by
/// `deadline`.
pub fn read_body(
    s: &mut TcpStream,
    mut start: Vec<u8>,
    len: usize,
    deadline: Instant,
) -> Result<Vec<u8>, ReadError> {
    if start.len() > len {
        return Err(ReadError::Bad("more bytes than Content-Length".into()));
    }
    start.reserve(len - start.len());
    let mut buf = [0u8; 16 * 1024];
    while start.len() < len {
        let want = (len - start.len()).min(buf.len());
        let n = read_some(s, &mut buf[..want], deadline)?;
        if n == 0 {
            return Err(ReadError::Closed);
        }
        start.extend_from_slice(&buf[..n]);
    }
    Ok(start)
}

/// A response to send.
#[derive(Debug)]
pub struct Response {
    /// Status code.
    pub status: u16,
    /// `Content-Type` of the body.
    pub content_type: &'static str,
    /// Extra headers (`Allow`, `WWW-Authenticate`).
    pub headers: Vec<(&'static str, String)>,
    /// The body.
    pub body: Vec<u8>,
}

/// The reason phrase of the codes this API uses.
pub fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        421 => "Misdirected Request",
        422 => "Unprocessable Content",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown",
    }
}

impl Response {
    /// Write the response (with `Connection: close`; no CORS headers, ever).
    pub fn write_to(&self, w: &mut impl Write) -> std::io::Result<()> {
        let mut head = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\
             Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n",
            self.status,
            reason(self.status),
            self.content_type,
            self.body.len()
        );
        for (name, value) in &self.headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        w.write_all(head.as_bytes())?;
        w.write_all(&self.body)?;
        w.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heads_parse_and_refuse_oddities() {
        let h = parse_head(b"GET /v1/info HTTP/1.1\r\nHost: 127.0.0.1:4840\r\nX-A:  b ").unwrap();
        assert_eq!((h.first.as_str(), h.second.as_str()), ("GET", "/v1/info"));
        assert_eq!(h.header("host").unwrap(), Some("127.0.0.1:4840"));
        assert_eq!(h.header("x-a").unwrap(), Some("b"));
        assert_eq!(h.content_length().unwrap(), 0);
        let two = parse_head(b"GET / HTTP/1.1\r\nHost: a\r\nhost: b").unwrap();
        assert!(two.header("host").is_err());
        let te = parse_head(b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked").unwrap();
        assert!(te.content_length().is_err());
        assert!(parse_head(b"GET /\r\n").is_err());
        assert!(parse_head(b"GET / HTTP/1.1\r\n folded").is_err());
        assert!(parse_head(b"GET / HTTP/1.1\r\nno colon").is_err());
        let many: Vec<u8> = std::iter::once("GET / HTTP/1.1".to_string())
            .chain((0..=MAX_HEADERS).map(|i| format!("X-{i}: 1")))
            .collect::<Vec<_>>()
            .join("\r\n")
            .into_bytes();
        assert!(matches!(parse_head(&many), Err(ReadError::TooLarge)));
    }
}
