//! The harness side of the GUI's probe: a debug build started with [`PROBE_VAR`] set serves
//! `GET /elements` on that loopback address (`vitni-ui-dioxus`, `shell/gui_probe.rs`), answering with a
//! [`Snapshot`] of every element that carries an `id`, a `data-hook` or an ARIA role.
//!
//! The probe only observes. Every input a scenario sends still reaches the GUI as a real X event from
//! `xdotool`; nothing here clicks, types, scrolls or focuses through the page.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use anyhow::{Context, Result, bail};

use super::target::Snapshot;

/// The variable a debug build of the GUI reads the probe's listen address from. A copy of
/// `vitni-ui-dioxus`'s own, as [`crate::archive_server::REROUTE_VAR`] is of the reroute variable.
pub const PROBE_VAR: &str = "VITNI_GUI_PROBE";

/// How long one request may take, connecting and reading together. A snapshot is one `eval` in the
/// webview; seconds means the GUI is wedged, not slow. Longer than the probe's own 5 s answer timeout,
/// so its `503` naming why it gave up arrives before this side stops reading.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

/// A loopback address no listener holds right now, for one worker's GUI to bind.
///
/// The port is released before the GUI binds it, so another process could take it in between; the
/// GUI then fails to bind and [`snapshot`] reports the probe unreachable at this address.
///
/// # Errors
///
/// Fails if no loopback port can be bound.
pub fn free_address() -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").context("finding a free port for the GUI probe")?;
    let address = listener.local_addr().context("reading the probe port")?;
    Ok(address.to_string())
}

/// The GUI's elements, their rects moved into the pixels of a window `height` tall.
///
/// # Errors
///
/// Fails if nothing answers at `address`, the answer is not `200 OK`, or its body is not a snapshot.
pub fn snapshot(address: &str, height: u32) -> Result<Snapshot> {
    let body = get(address, "/elements")?;
    let snapshot: Snapshot =
        serde_json::from_str(&body).with_context(|| format!("parsing the GUI probe's answer from {address}"))?;
    Ok(snapshot.in_window(height))
}

/// The body of `GET path` from `address`.
fn get(address: &str, path: &str) -> Result<String> {
    let socket = address
        .parse()
        .with_context(|| format!("the GUI probe address {address:?} is not host:port"))?;
    let mut stream = TcpStream::connect_timeout(&socket, REQUEST_TIMEOUT)
        .with_context(|| format!("the GUI probe is not reachable on {address} — is this a debug build?"))?;
    stream
        .set_read_timeout(Some(REQUEST_TIMEOUT))
        .context("setting the probe read timeout")?;
    let request = format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .with_context(|| format!("asking the GUI probe on {address}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .with_context(|| format!("reading the GUI probe's answer from {address}"))?;
    body(&response).map(str::to_owned)
}

/// The body of an HTTP response, failing on any status but `200`.
fn body(response: &str) -> Result<&str> {
    let Some((head, body)) = response.split_once("\r\n\r\n") else {
        bail!("the GUI probe sent no complete HTTP response: {response:?}");
    };
    let status = head.lines().next().unwrap_or_default();
    if status.split_whitespace().nth(1) != Some("200") {
        bail!("the GUI probe answered {status:?}: {body}");
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::{body, free_address, snapshot};
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;

    /// Serves `response` to one connection, returning the address and the request it received.
    fn serve_once(response: String) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("address").to_string();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = Vec::new();
            let mut byte = [0_u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).expect("read");
                request.push(byte[0]);
            }
            stream.write_all(response.as_bytes()).expect("write");
            String::from_utf8_lossy(&request).into_owned()
        });
        (address, handle)
    }

    fn ok(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn a_snapshot_is_fetched_and_moved_into_window_pixels() {
        let json = r#"{"ready": true, "focused": false, "maps_idle": false, "active": "global-search",
            "viewport": [1800, 1170], "elements": [{"id": null, "hook": "rail-item", "text": "People 2", "label": "",
            "rect": [0, 160, 230, 30], "within": ["rail"]}]}"#;
        let (address, served) = serve_once(ok(json));
        let snapshot = snapshot(&address, 1200).expect("a snapshot");
        assert!(snapshot.ready && !snapshot.focused);
        assert!(!snapshot.maps_idle, "a map still drawing is reported");
        assert_eq!(snapshot.active.as_deref(), Some("global-search"));
        assert_eq!(snapshot.elements[0].rect, [0.0, 190.0, 230.0, 30.0]);
        let request = served.join().expect("served");
        assert!(request.starts_with("GET /elements HTTP/1.1\r\n"), "{request}");
    }

    #[test]
    fn a_status_other_than_200_is_an_error_quoting_it() {
        let error = body("HTTP/1.1 503 Service Unavailable\r\n\r\n{\"error\": \"timed out\"}").expect_err("503");
        let error = format!("{error:#}");
        assert!(error.contains("503 Service Unavailable"), "{error}");
        assert!(error.contains("timed out"), "the probe's own reason is quoted: {error}");
    }

    #[test]
    fn a_truncated_response_is_an_error() {
        assert!(body("HTTP/1.1 200 OK\r\nContent-Length: 10").is_err());
    }

    #[test]
    fn a_body_that_is_not_a_snapshot_is_an_error() {
        let (address, _served) = serve_once(ok(r#"{"elements": "nope"}"#));
        let error = snapshot(&address, 1200).expect_err("not a snapshot");
        assert!(
            format!("{error:#}").contains("parsing the GUI probe's answer"),
            "{error:#}"
        );
    }

    #[test]
    fn nothing_listening_is_an_error_naming_the_address() {
        let address = free_address().expect("a free port");
        let error = snapshot(&address, 1200).expect_err("no probe");
        assert!(format!("{error:#}").contains(&address), "{error:#}");
    }
}
