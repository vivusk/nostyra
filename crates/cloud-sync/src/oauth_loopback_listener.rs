//! Loopback redirect target of the browser sign-in: a `TcpListener` on `127.0.0.1`
//! (never a host name — `localhost` may resolve to `::1`) on port 0 or the first
//! free registered port. Accepts one `GET /?code=…&state=…`, checks `state`,
//! answers a tiny "you can close this tab" page. Other paths (`/favicon.ico`) get a
//! 404 and are ignored. Non-blocking accept polled every 200 ms so the cancel flag
//! and the timeout are honoured. The request line (it holds the code) is never logged.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use percent_encoding::percent_decode_str;

use crate::oauth_endpoints::{LoopbackRedirect, PortRule};
use crate::oauth_error::SignInError;
use crate::secret_string::SecretString;

const ACCEPT_POLL: Duration = Duration::from_millis(200);
/// One browser connection must deliver its request line + headers within this.
const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_REQUEST_BYTES: usize = 16 * 1024;

const PAGE_DONE: &str = "Signed in. You can close this tab and return to Nostyra.";
const PAGE_CANCELLED: &str = "Sign-in cancelled. You can close this tab.";
const PAGE_FAILED: &str = "Sign-in failed. Close this tab and try again in Nostyra.";

/// Bound listener + the redirect URI to send to the provider.
pub struct LoopbackListener {
    listener: TcpListener,
    redirect_uri: String,
}

impl LoopbackListener {
    /// Binds `127.0.0.1` per the provider's port rule.
    pub fn bind(redirect: &LoopbackRedirect) -> Result<Self, SignInError> {
        let listener = match redirect.ports {
            PortRule::Any => TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?,
            PortRule::Fixed(ports) => ports
                .iter()
                .find_map(|port| TcpListener::bind((Ipv4Addr::LOCALHOST, *port)).ok())
                .ok_or_else(|| SignInError::PortsBusy(port_list(ports)))?,
        };
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        Ok(Self {
            listener,
            redirect_uri: redirect.uri(port),
        })
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    /// Waits for the redirect carrying `expected_state`; returns the code.
    pub fn wait_for_code(
        &self,
        expected_state: &str,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<SecretString, SignInError> {
        let started = Instant::now();
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(SignInError::Cancelled);
            }
            if started.elapsed() >= timeout {
                return Err(SignInError::TimedOut);
            }
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if let Some(code) = answer(stream, expected_state)? {
                        return Ok(code);
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(ACCEPT_POLL);
                }
                Err(err) => return Err(err.into()),
            }
        }
    }
}

/// Handles one connection: `Ok(Some(code))` on success, `Ok(None)` for requests to
/// ignore (favicon, empty / broken connections), `Err` for a final answer.
fn answer(
    mut stream: TcpStream,
    expected_state: &str,
) -> Result<Option<SecretString>, SignInError> {
    let Some(target) = read_request_target(&mut stream) else {
        return Ok(None);
    };
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    if path != "/" {
        respond(&mut stream, "404 Not Found", "Not found.");
        return Ok(None);
    }
    let params = parse_query(query);
    let get = |key: &str| {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    let (code, error) = (get("code"), get("error"));
    if code.is_none() && error.is_none() {
        respond(&mut stream, "404 Not Found", "Not found.");
        return Ok(None);
    }
    if get("state") != Some(expected_state) {
        respond(&mut stream, "400 Bad Request", PAGE_FAILED);
        return Err(SignInError::StateMismatch);
    }
    match (code, error) {
        (_, Some("access_denied")) => {
            respond(&mut stream, "200 OK", PAGE_CANCELLED);
            Err(SignInError::Cancelled)
        }
        (_, Some(error)) => {
            respond(&mut stream, "200 OK", PAGE_FAILED);
            Err(SignInError::Refused(sanitize_error_code(error)))
        }
        (Some(code), None) => {
            respond(&mut stream, "200 OK", PAGE_DONE);
            Ok(Some(SecretString::new(code)))
        }
        (None, None) => Ok(None),
    }
}

/// Request target of a `GET` (e.g. `/?code=…`), `None` for anything else.
fn read_request_target(stream: &mut TcpStream) -> Option<String> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(REQUEST_READ_TIMEOUT)).ok()?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < MAX_REQUEST_BYTES {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let mut parts = head.lines().next()?.split(' ');
    (parts.next()? == "GET").then_some(())?;
    parts.next().map(str::to_string)
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    let decode = |s: &str| {
        percent_decode_str(&s.replace('+', " "))
            .decode_utf8_lossy()
            .into_owned()
    };
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect()
}

/// OAuth error codes are `[a-z_]`; anything else is cut so it is safe to show / log.
fn sanitize_error_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(64)
        .collect()
}

fn respond(stream: &mut TcpStream, status: &str, message: &str) {
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Nostyra</title></head>\
         <body style=\"font-family:sans-serif;margin:3em\"><p>{message}</p></body></html>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    // The browser may already be gone; the sign-in result does not depend on it.
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn port_list(ports: &[u16]) -> String {
    ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
#[path = "oauth_loopback_listener_tests.rs"]
mod tests;
