//! A local stand-in for Digitalarkivet that `gui-pass` drives the assisted import against, so a
//! scenario never reaches the real archive.
//!
//! It serves the bundled `vitni-digitalarkivet` census pages (invented, ADR 0042) by path, the way the
//! plugin host's own assisted test mounts them: every census person page is the fixture person, every
//! residence the fixture household, a scan viewer the fixture viewer, and an image a minimal JPEG. The
//! GUI reaches it through `VITNI_ASSISTED_NET_REROUTE`, which only a debug build honours: the plugin
//! still asks for `https://www.digitalarkivet.no/…`, and the host sends the request here instead.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The variable a debug build of the GUI reads its reroute origin from.
pub const REROUTE_VAR: &str = "VITNI_ASSISTED_NET_REROUTE";

/// Where the census fixture pages live.
const FIXTURES: &str = "crates/vitni-digitalarkivet/tests/fixtures/census";

/// A minimal valid JPEG body (SOI + EOI markers) — enough for `media-store` to store and checksum.
const SCAN_JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xD9];

/// Starts the server on a free loopback port, serving until the process exits, and returns its origin
/// (`http://localhost:PORT`).
///
/// # Errors
///
/// Fails if no loopback port can be bound.
pub fn start() -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").context("binding the archive stand-in")?;
    let port = listener
        .local_addr()
        .context("reading the archive stand-in's port")?
        .port();
    let root = PathBuf::from(FIXTURES);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if let Err(error) = serve(stream, &root) {
                eprintln!("gui-pass: the archive stand-in failed a request: {error:#}");
            }
        }
    });
    Ok(format!("http://localhost:{port}"))
}

/// Answers one request with the page its path names, or a 404.
fn serve(stream: TcpStream, root: &Path) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone().context("sharing the connection")?);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .context("reading the request line")?;
    let mut header = String::new();
    while reader.read_line(&mut header).context("reading a header")? > 2 {
        header.clear();
    }
    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let (status, content_type, body) = match page(path) {
        Some(Page::Html(name)) => {
            let file = root.join(name);
            let body = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            ("200 OK", "text/html; charset=utf-8", body)
        }
        Some(Page::Scan) => ("200 OK", "image/jpeg", SCAN_JPEG.to_vec()),
        None => ("404 Not Found", "text/plain", b"not found".to_vec()),
    };
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .context("writing the response head")?;
    stream.write_all(&body).context("writing the response body")?;
    Ok(())
}

/// What the stand-in serves for a path.
#[derive(Debug, PartialEq, Eq)]
enum Page {
    /// A fixture page, by its file name.
    Html(&'static str),
    /// The scan image.
    Scan,
}

/// The page `path` names, if any.
fn page(path: &str) -> Option<Page> {
    let path = path.split('?').next().unwrap_or(path);
    if Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("jpg"))
    {
        return Some(Page::Scan);
    }
    if path.contains("/census/person/") {
        return Some(Page::Html("person.html"));
    }
    if path.contains("/census/rural-residence/") || path.contains("/census/urban-residence/") {
        return Some(Page::Html("bosted.html"));
    }
    let viewer = path
        .strip_prefix("/fs")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()));
    viewer.then_some(Page::Html("viewer.html"))
}

#[cfg(test)]
mod tests {
    use super::{Page, page};

    #[test]
    fn each_census_path_names_its_fixture_page() {
        assert_eq!(page("/census/person/pf01099901000101"), Some(Page::Html("person.html")));
        assert_eq!(
            page("/census/rural-residence/bf01099901000100"),
            Some(Page::Html("bosted.html"))
        );
        assert_eq!(page("/fs10000099901042"), Some(Page::Html("viewer.html")));
        assert_eq!(page("/URN:NBN:no-a1450-fs10000099901042.jpg"), Some(Page::Scan));
        assert_eq!(page("/census/district/tf01099901000001"), None);
        assert_eq!(page("/fsx"), None);
    }
}
