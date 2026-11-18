//! HTTP with the error messages and semantics of the original fetch-based client.

use serde_json::Value;
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use ureq::{Agent, AgentBuilder, ErrorKind, RedirectAuthHeaders};

const TIMEOUT_MS: u64 = 15_000;
/// Largest response body read into memory; matches the component cache read limit.
const MAX_BODY_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Fail {
    pub message: String,
    pub status: Option<u16>,
    /// True for the failures the JS client raised as SourceError; a mirror is only tried after those.
    pub source_error: bool,
}

impl Fail {
    pub fn source(message: impl Into<String>) -> Self {
        Fail {
            message: message.into(),
            status: None,
            source_error: true,
        }
    }
    pub fn other(message: impl Into<String>) -> Self {
        Fail {
            message: message.into(),
            status: None,
            source_error: false,
        }
    }
}

pub type Res<T> = Result<T, Fail>;

/// Local mock transport for debug test binaries only. No override code is linked in release.
#[cfg(debug_assertions)]
pub fn test_base() -> Option<&'static str> {
    static BASE: OnceLock<Option<String>> = OnceLock::new();
    BASE.get_or_init(|| {
        std::env::var("CLAYUI_TEST_BASE_URL")
            .ok()
            .filter(|base| valid_test_base(base))
    })
    .as_deref()
}

#[cfg(debug_assertions)]
fn valid_test_base(base: &str) -> bool {
    base.strip_prefix("http://127.0.0.1:")
        .filter(|port| !port.is_empty() && port.bytes().all(|c| c.is_ascii_digit()))
        .and_then(|port| port.parse::<u16>().ok())
        .is_some_and(|port| port != 0)
}

#[cfg(not(debug_assertions))]
pub fn test_base() -> Option<&'static str> {
    None
}

pub fn transport_url(url: &str) -> String {
    match test_base() {
        Some(base) => format!(
            "{base}/{}",
            url.split_once("://").map_or(url, |(_, rest)| rest)
        ),
        None => url.to_string(),
    }
}

/// Request budget; a shorter one is honoured only together with the mock server.
pub fn timeout(default_ms: u64) -> Duration {
    let test_ms = test_base()
        .and_then(|_| std::env::var("CLAYUI_TEST_TIMEOUT_MS").ok())
        .and_then(|value| value.parse().ok());
    Duration::from_millis(test_ms.unwrap_or(default_ms))
}

/// Pause between retries; a mock server needs no real backoff.
pub fn backoff(ms: u64) {
    std::thread::sleep(Duration::from_millis(if test_base().is_some() {
        1
    } else {
        ms
    }));
}

pub fn agent(redirects: u32) -> Agent {
    AgentBuilder::new()
        .redirects(redirects)
        .redirect_auth_headers(RedirectAuthHeaders::SameHost)
        .user_agent(concat!("clayui/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn client() -> &'static Agent {
    static CLIENT: OnceLock<Agent> = OnceLock::new();
    CLIENT.get_or_init(|| agent(20))
}

pub fn host_of(url: &str) -> Result<String, String> {
    let rest = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .ok_or("Invalid URL")?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() {
        return Err("Invalid URL".into());
    }
    let host = host.rsplit('@').next().unwrap_or(host).to_ascii_lowercase();
    Ok(host
        .strip_suffix(":443")
        .or(host.strip_suffix(":80"))
        .unwrap_or(&host)
        .to_string())
}

/// The code Node reports in `error.cause.code`, or the error name when it has none.
pub fn error_code(error: &ureq::Transport) -> String {
    let io_kind = std::error::Error::source(error)
        .and_then(|source| source.downcast_ref::<std::io::Error>())
        .map(std::io::Error::kind);
    use std::io::ErrorKind as Io;
    match (error.kind(), io_kind) {
        (_, Some(Io::TimedOut | Io::WouldBlock)) => "TimeoutError".into(),
        (ErrorKind::Dns, _) => "ENOTFOUND".into(),
        (_, Some(Io::ConnectionRefused)) => "ECONNREFUSED".into(),
        (_, Some(Io::ConnectionReset)) => "ECONNRESET".into(),
        (_, Some(Io::BrokenPipe)) => "EPIPE".into(),
        _ => "TypeError".into(),
    }
}

pub struct Reply {
    pub status: u16,
    pub final_url: String,
    pub requested_url: String,
    response: ureq::Response,
    host: String,
}

impl Reply {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn text(self) -> Res<String> {
        let mut bytes = Vec::new();
        let host = self.host;
        self.response
            .into_reader()
            .take(MAX_BODY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                let code = if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) {
                    "TimeoutError"
                } else {
                    "TypeError"
                };
                Fail::other(format!("{host}: {code}"))
            })?;
        if bytes.len() as u64 > MAX_BODY_BYTES {
            return Err(Fail::other(format!("{host}: response too large")));
        }
        let text = String::from_utf8_lossy(&bytes);
        Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
    }
}

pub type Headers = Vec<(&'static str, String)>;

pub fn request(url: &str, headers: &Headers) -> Res<Reply> {
    let host = host_of(url).map_err(Fail::other)?;
    let target = transport_url(url);
    let mut call = client().get(&target).timeout(timeout(TIMEOUT_MS));
    for (name, value) in headers {
        call = call.set(name, value);
    }
    let response = match call.call() {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(ureq::Error::Transport(error)) => {
            return Err(Fail::source(format!("{host}: {}", error_code(&error))))
        }
    };
    Ok(Reply {
        status: response.status(),
        final_url: response.get_url().to_string(),
        requested_url: target,
        response,
        host,
    })
}

fn fetch_ok(url: &str, headers: &Headers) -> Res<Reply> {
    let reply = request(url, headers)?;
    if !reply.ok() {
        return Err(Fail {
            message: format!(
                "{}: HTTP {}",
                host_of(url).unwrap_or_default(),
                reply.status
            ),
            status: Some(reply.status),
            source_error: true,
        });
    }
    Ok(reply)
}

pub fn fetch_text_with(url: &str, headers: &Headers) -> Res<String> {
    fetch_ok(url, headers)?.text()
}

pub fn fetch_text(url: &str) -> Res<String> {
    fetch_text_with(url, &Vec::new())
}

pub fn fetch_json_with(url: &str, headers: &Headers) -> Res<Value> {
    let text = fetch_text_with(url, headers)?;
    serde_json::from_str(&text).map_err(|_| {
        Fail::source(format!(
            "{}: invalid JSON",
            host_of(url).unwrap_or_default()
        ))
    })
}

pub fn fetch_json(url: &str) -> Res<Value> {
    fetch_json_with(url, &Vec::new())
}

/// Runs `worker` over `items` on at most `limit` threads, keeping input order; stops at the first failure.
pub fn map_limit<T: Sync, R: Send>(
    items: &[T],
    limit: usize,
    worker: impl Fn(&T) -> Res<R> + Sync,
) -> Res<Vec<R>> {
    let next = AtomicUsize::new(0);
    let failure: Mutex<Option<Fail>> = Mutex::new(None);
    let results: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::with_capacity(items.len()));
    std::thread::scope(|scope| {
        for _ in 0..limit.min(items.len()) {
            scope.spawn(|| loop {
                if failure.lock().map_or(true, |slot| slot.is_some()) {
                    return;
                }
                let index = next.fetch_add(1, Ordering::SeqCst);
                let Some(item) = items.get(index) else { return };
                match worker(item) {
                    Ok(value) => {
                        if let Ok(mut done) = results.lock() {
                            done.push((index, value));
                        }
                    }
                    Err(error) => {
                        if let Ok(mut slot) = failure.lock() {
                            slot.get_or_insert(error);
                        }
                    }
                }
            });
        }
    });
    if let Some(error) = failure.into_inner().ok().flatten() {
        return Err(error);
    }
    let mut done = results
        .into_inner()
        .map_err(|_| Fail::other("worker failed"))?;
    if done.len() != items.len() {
        return Err(Fail::other("worker failed"));
    }
    done.sort_by_key(|(index, _)| *index);
    Ok(done.into_iter().map(|(_, value)| value).collect())
}

/// Runs two tasks concurrently and returns both outcomes.
pub fn join<A: Send, B: Send>(
    first: impl FnOnce() -> A + Send,
    second: impl FnOnce() -> B + Send,
) -> (A, B) {
    std::thread::scope(|scope| {
        let left = scope.spawn(first);
        let right = second();
        match left.join() {
            Ok(value) => (value, right),
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn mock_authority_cannot_escape_loopback() {
        #[cfg(debug_assertions)]
        {
            assert!(super::valid_test_base("http://127.0.0.1:1234"));
            for value in [
                "http://127.0.0.1:1@HOST",
                "http://127.0.0.1:1/path",
                "http://127.0.0.1:0",
                "https://other",
            ] {
                assert!(!super::valid_test_base(value));
            }
        }
        #[cfg(not(debug_assertions))]
        assert!(super::test_base().is_none());
    }
}
