//! The `gui-pass` probe: a debug build started with [`PROBE_VAR`] set serves `GET /elements` on that
//! loopback address, answering with every element that carries an `id`, a `data-hook` or an ARIA role
//! (explicit, or implied by its tag) — its rect, text, `aria-label` and the ids, hooks and roles of its
//! ancestors, whether it holds focus and, for a text field, its value — plus whether the page is ready
//! and focused, and whether every map container holds a `MapLibre` map that is idle (constructed,
//! loaded and not moving, so nothing queued has yet to reach its canvas). `cargo xtask gui-pass`
//! resolves a scenario's targets over that list, so its scenarios name elements instead of window
//! pixels, and waits for idle maps before a step counts as settled. `GET /hit?x=…&y=…` answers what
//! `document.elementFromPoint` finds at that viewport point, with it and its reported ancestors, so the
//! harness refuses a target that something else covers — a click there would land on whatever is on top.
//!
//! The probe only **observes**. It never dispatches an event, sets a value, scrolls or moves focus:
//! every input a scenario sends reaches the webview as a real X event, so what a scenario proves is
//! what a user's mouse and keyboard would do. A release build ignores the variable, like
//! `VITNI_ASSISTED_NET_REROUTE`, so a shipped binary never listens.
//!
//! Mounted once by the root [`App`](crate::app::App), outside the keyed subtree a restart remounts, so
//! the listener survives a workspace switch. The non-desktop build compiles an inert no-op.

use dioxus::prelude::*;

/// The variable naming the loopback `host:port` the probe listens on.
pub const PROBE_VAR: &str = "VITNI_GUI_PROBE";

/// The probe's listen address from `value` of [`PROBE_VAR`]: honoured only in a `debug` build, and only
/// for a loopback `ip:port`, since the snapshot carries every record name on screen.
#[cfg(any(feature = "desktop", test))]
fn probe_address(value: Option<String>, debug: bool) -> Option<std::net::SocketAddr> {
    if !debug {
        return None;
    }
    let value = value?;
    let Ok(address) = value.trim().parse::<std::net::SocketAddr>() else {
        if !value.trim().is_empty() {
            tracing::error!(%value, "the gui-pass probe address is not ip:port; the probe is off");
        }
        return None;
    };
    if !address.ip().is_loopback() {
        tracing::error!(%address, "the gui-pass probe listens on loopback only; the probe is off");
        return None;
    }
    Some(address)
}

/// What a request head asks the probe for.
#[cfg(any(feature = "desktop", test))]
#[derive(Debug, PartialEq, Eq)]
enum Route {
    /// `GET /elements`: the snapshot.
    Elements,
    /// `GET /hit?x=…&y=…`: what is on top at that viewport point.
    Hit { x: i32, y: i32 },
    /// `GET /hit` without exactly an integer `x` and `y`.
    BadHit,
    /// Anything else.
    NotFound,
}

/// Routes an HTTP request by its first line.
#[cfg(any(feature = "desktop", test))]
fn route(head: &str) -> Route {
    let mut parts = head.lines().next().unwrap_or_default().split_whitespace();
    match (parts.next(), parts.next()) {
        (Some("GET"), Some("/elements")) => Route::Elements,
        (Some("GET"), Some(target)) if target == "/hit" || target.starts_with("/hit?") => {
            hit_point(target.trim_start_matches("/hit").trim_start_matches('?'))
                .map_or(Route::BadHit, |(x, y)| Route::Hit { x, y })
        }
        _ => Route::NotFound,
    }
}

/// The integer `x` and `y` of a `/hit` query, and nothing else. Integers only, so the point can be
/// written into the hit script without quoting.
#[cfg(any(feature = "desktop", test))]
fn hit_point(query: &str) -> Option<(i32, i32)> {
    let (mut x, mut y) = (None, None);
    for pair in query.split('&') {
        match pair.split_once('=')? {
            ("x", value) if x.is_none() => x = Some(value.parse().ok()?),
            ("y", value) if y.is_none() => y = Some(value.parse().ok()?),
            _ => return None,
        }
    }
    Some((x?, y?))
}

/// An HTTP/1.1 response closing the connection after `body`.
#[cfg(any(feature = "desktop", test))]
fn response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// What both scripts share: how an element is named and described. Zero-sized elements are left out
/// of both answers: a `display: none` panel cannot be clicked and has no region to measure.
#[cfg(feature = "desktop")]
const PRELUDE: &str = r"
const collapse = (text) => (text || '').replace(/\s+/g, ' ').trim().slice(0, 200);
const hook = (el) => (el.dataset && el.dataset.hook) || null;
const role = (el) => {
    const explicit = el.getAttribute('role');
    if (explicit) return explicit;
    switch (el.tagName) {
        case 'BUTTON': case 'SUMMARY': return 'button';
        case 'A': return el.hasAttribute('href') ? 'link' : null;
        case 'SELECT': return 'combobox';
        case 'TEXTAREA': return 'textbox';
        case 'TR': return 'row';
        case 'TD': return 'cell';
        case 'TH': return 'columnheader';
        case 'INPUT': {
            const kind = el.type;
            if (kind === 'checkbox' || kind === 'radio') return kind;
            if (kind === 'range') return 'slider';
            if (kind === 'button' || kind === 'submit' || kind === 'reset') return 'button';
            return kind === 'hidden' ? null : 'textbox';
        }
        default: return null;
    }
};
const targets = '[id], [data-hook], [role], button, a[href], input, select, textarea, summary, tr, td, th';
const sized = (el) => {
    const rect = el.getBoundingClientRect();
    return rect.width !== 0 || rect.height !== 0;
};
const describe = (el) => {
    const rect = el.getBoundingClientRect();
    const within = [];
    for (let up = el.parentElement; up; up = up.parentElement) {
        if (up.id) within.push(up.id);
        if (hook(up)) within.push(hook(up));
        if (role(up)) within.push(role(up));
    }
    return {
        id: el.id || null,
        hook: hook(el),
        role: role(el),
        text: collapse(el.innerText),
        label: collapse(el.getAttribute('aria-label') || el.getAttribute('title')),
        rect: [rect.x, rect.y, rect.width, rect.height],
        within,
        active: el === document.activeElement,
        value: el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' ? el.value : null,
    };
};
const viewport = [window.innerWidth, window.innerHeight];
";

/// The snapshot script, after [`PRELUDE`].
#[cfg(feature = "desktop")]
const SNAPSHOT: &str = r"
const elements = [];
for (const el of document.querySelectorAll(targets)) {
    if (sized(el)) elements.push(describe(el));
}
const active = document.activeElement;
return {
    ready: document.readyState === 'complete' && document.querySelector('.app') !== null,
    focused: document.hasFocus(),
    maps_idle: [...document.querySelectorAll('.map-container')]
        .every((el) => el.__geoMap && el.__geoMap.loaded() && !el.__geoMap.isMoving()),
    active: active ? (active.id || hook(active) || role(active) || active.tagName.toLowerCase()) : null,
    viewport,
    elements,
};
";

/// The hit script for the viewport point `(x, y)`, after [`PRELUDE`]: the element on top there, named
/// for a failure message, and it and every ancestor the snapshot would report, nearest first. An inert
/// or `pointer-events: none` element is passed through, as a click would pass through it.
#[cfg(feature = "desktop")]
fn hit_script(x: i32, y: i32) -> String {
    format!(
        r"{PRELUDE}
const top = document.elementFromPoint({x}, {y});
const name = (el) => {{
    if (el.id) return '#' + el.id;
    if (hook(el)) return '[data-hook=' + hook(el) + ']';
    const classes = typeof el.className === 'string' ? el.className.trim().split(/\s+/).filter(Boolean) : [];
    return [el.tagName.toLowerCase(), ...classes].join('.');
}};
const chain = [];
for (let el = top; el; el = el.parentElement) {{
    if (el.matches(targets) && sized(el)) chain.push(describe(el));
}}
return {{ viewport, top: top ? name(top) : null, chain }};
"
    )
}

/// How long a connection waits for the webview to answer before giving up on it.
#[cfg(feature = "desktop")]
const ANSWER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// The largest request head read; the harness sends a few dozen bytes.
#[cfg(feature = "desktop")]
const MAX_HEAD: usize = 8 * 1024;

/// One request for the app: the script to evaluate, and where its answer goes.
#[cfg(feature = "desktop")]
type Request = (String, std::sync::mpsc::Sender<Result<String, String>>);

/// Serves the probe while the app runs, when [`PROBE_VAR`] is set in a debug build. Renders nothing.
#[cfg(feature = "desktop")]
#[component]
pub fn GuiProbe() -> Element {
    use tokio::sync::mpsc;

    use_hook(|| {
        let Some(address) = probe_address(std::env::var(PROBE_VAR).ok(), cfg!(debug_assertions)) else {
            return;
        };
        let listener = match std::net::TcpListener::bind(address) {
            Ok(listener) => listener,
            Err(error) => {
                tracing::error!(%address, %error, "the gui-pass probe could not listen");
                return;
            }
        };
        let (requests, mut incoming) = mpsc::channel::<Request>(8);
        std::thread::spawn(move || serve(&listener, &requests));
        spawn(async move {
            while let Some((script, answer)) = incoming.recv().await {
                let observed = observe(&script).await;
                let sent = answer.send(
                    observed
                        .map(|value| value.to_string())
                        .map_err(|error| error.to_string()),
                );
                if sent.is_err() {
                    tracing::warn!("a gui-pass probe answer came after its request had timed out");
                }
            }
        });
        tracing::info!(%address, "the gui-pass probe is listening");
    });
    rsx! {}
}

/// How many times [`observe`] runs a script before giving up on `EvalError::Finished`.
#[cfg(feature = "desktop")]
const EVAL_ATTEMPTS: usize = 3;

/// Runs `script` and returns what it saw.
///
/// dioxus-desktop drops an eval's state once the webview reports the script done, so a script that
/// finishes before `join` first polls it reads as `EvalError::Finished` rather than a value. Both
/// scripts only read the DOM, so running one again is safe; anything else is the caller's error.
#[cfg(feature = "desktop")]
async fn observe(script: &str) -> Result<serde_json::Value, document::EvalError> {
    let mut attempt = 1;
    loop {
        match document::eval(script).join::<serde_json::Value>().await {
            Err(document::EvalError::Finished) if attempt < EVAL_ATTEMPTS => attempt += 1,
            outcome => return outcome,
        }
    }
}

/// Answers each connection on `listener` in turn, asking the app to run the routed script through
/// `requests`.
#[cfg(feature = "desktop")]
fn serve(listener: &std::net::TcpListener, requests: &tokio::sync::mpsc::Sender<Request>) {
    use std::io::Write as _;

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                tracing::warn!(%error, "the gui-pass probe dropped a connection");
                continue;
            }
        };
        let answer = match read_head(&mut stream).map(|head| route(&head)) {
            Ok(Route::Elements) => ask(requests, format!("{PRELUDE}{SNAPSHOT}")),
            Ok(Route::Hit { x, y }) => ask(requests, hit_script(x, y)),
            Ok(Route::BadHit) => response(
                "400 Bad Request",
                r#"{"error": "/hit takes exactly an integer x and y"}"#,
            ),
            Ok(Route::NotFound) => response("404 Not Found", "{}"),
            Err(error) => response("400 Bad Request", &serde_json::json!({ "error": error }).to_string()),
        };
        if let Err(error) = stream.write_all(answer.as_bytes()) {
            tracing::warn!(%error, "the gui-pass probe could not answer");
        }
    }
}

/// The app's answer to `script`, or a `503` naming why it gave none.
#[cfg(feature = "desktop")]
fn ask(requests: &tokio::sync::mpsc::Sender<Request>, script: String) -> String {
    let (answer, answered) = std::sync::mpsc::channel();
    if requests.blocking_send((script, answer)).is_err() {
        return response("503 Service Unavailable", r#"{"error": "the app has stopped"}"#);
    }
    match answered.recv_timeout(ANSWER_TIMEOUT) {
        Ok(Ok(body)) => response("200 OK", &body),
        Ok(Err(error)) => response(
            "503 Service Unavailable",
            &serde_json::json!({ "error": error }).to_string(),
        ),
        Err(error) => response(
            "503 Service Unavailable",
            &serde_json::json!({ "error": error.to_string() }).to_string(),
        ),
    }
}

/// Reads a request up to the blank line ending its head.
#[cfg(feature = "desktop")]
fn read_head(stream: &mut std::net::TcpStream) -> Result<String, String> {
    use std::io::Read as _;

    stream
        .set_read_timeout(Some(ANSWER_TIMEOUT))
        .map_err(|error| error.to_string())?;
    let mut head = Vec::new();
    let mut byte = [0_u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() >= MAX_HEAD {
            return Err("request head too long".to_owned());
        }
        stream.read_exact(&mut byte).map_err(|error| error.to_string())?;
        head.extend_from_slice(&byte);
    }
    Ok(String::from_utf8_lossy(&head).into_owned())
}

/// The non-desktop no-op: there is no webview to observe.
#[cfg(not(feature = "desktop"))]
#[component]
pub fn GuiProbe() -> Element {
    rsx! {}
}

#[cfg(test)]
mod tests {
    use super::{Route, probe_address, response, route};

    #[test]
    fn only_a_debug_build_honours_the_probe_address() {
        let address = Some("127.0.0.1:4100".to_owned());
        assert_eq!(
            probe_address(address.clone(), true).map(|address| address.to_string()),
            Some("127.0.0.1:4100".to_owned())
        );
        assert_eq!(probe_address(address, false), None, "a release build never listens");
        assert_eq!(
            probe_address(Some("  ".to_owned()), true),
            None,
            "a blank value is unset"
        );
        assert_eq!(probe_address(None, true), None);
    }

    #[test]
    fn the_probe_listens_on_loopback_only() {
        assert_eq!(
            probe_address(Some("0.0.0.0:4100".to_owned()), true),
            None,
            "every interface would expose the screen's records"
        );
        assert_eq!(probe_address(Some("localhost".to_owned()), true), None, "not ip:port");
        assert!(probe_address(Some("[::1]:4100".to_owned()), true).is_some());
    }

    #[test]
    fn only_the_probes_get_routes_are_served() {
        assert_eq!(route("GET /elements HTTP/1.1\r\nHost: x\r\n\r\n"), Route::Elements);
        assert_eq!(
            route("POST /elements HTTP/1.1\r\n\r\n"),
            Route::NotFound,
            "the probe takes no input"
        );
        assert_eq!(route("GET / HTTP/1.1\r\n\r\n"), Route::NotFound);
        assert_eq!(route(""), Route::NotFound);
    }

    #[test]
    fn get_hit_carries_the_viewport_point_to_test() {
        assert_eq!(
            route("GET /hit?x=120&y=-4 HTTP/1.1\r\n\r\n"),
            Route::Hit { x: 120, y: -4 },
            "a point above the viewport still parses; the webview answers it with nothing"
        );
        assert_eq!(route("GET /hit?y=7&x=3 HTTP/1.1\r\n\r\n"), Route::Hit { x: 3, y: 7 });
    }

    #[test]
    fn a_hit_without_two_integer_coordinates_is_a_bad_request() {
        for head in [
            "GET /hit HTTP/1.1\r\n\r\n",
            "GET /hit?x=1 HTTP/1.1\r\n\r\n",
            "GET /hit?x=1&y=2.5 HTTP/1.1\r\n\r\n",
            "GET /hit?x=1&y=alert(1) HTTP/1.1\r\n\r\n",
            "GET /hit?x=1&y=2&z=3 HTTP/1.1\r\n\r\n",
        ] {
            assert_eq!(route(head), Route::BadHit, "{head:?}");
        }
    }

    #[test]
    fn a_response_carries_its_status_and_body_length() {
        assert_eq!(
            response("200 OK", "{}"),
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
        );
    }
}
