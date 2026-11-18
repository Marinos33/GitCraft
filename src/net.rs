//! One HTTPS transport for hosting services and AI providers.
//!
//! Requests carry credentials, so the client never follows redirects: a 3xx answer is handed
//! back to the caller, which reports it instead of forwarding the credential elsewhere.

use reqwest::header::HeaderMap;
use std::time::{Duration, Instant};

/// The shared client. Building it once keeps one connection pool and one TLS setup.
pub fn client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("GitCraft")
            .build()
            .expect("HTTP transport")
    })
}

/// A fully read response. Bodies are bounded by [`Limits::max_bytes`].
pub struct Response {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<String> {
        self.headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned)
    }
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    pub fn is_redirect(&self) -> bool {
        (300..400).contains(&self.status)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The request could not be sent or its headers were not received.
    Unreachable,
    /// The body could not be read completely.
    Unreadable,
    /// The body exceeded the caller's budget.
    TooLarge,
    TimedOut,
    /// The caller's cancellation check failed; carries its message.
    Cancelled(String),
}

pub struct Limits<'a> {
    pub timeout: Duration,
    pub max_bytes: usize,
    /// Limits concurrent requests of one kind; `None` for no limit.
    pub slots: Option<&'a tokio::sync::Semaphore>,
}

/// Send `request` and read its whole body. `cancelled` is polled while waiting so a dropped
/// consumer interrupts connect, header and body waits without leaving a detached thread.
/// Must be called from a blocking context (never from inside an async task).
pub fn send(
    request: reqwest::RequestBuilder,
    limits: Limits,
    cancelled: &dyn Fn() -> Result<(), String>,
) -> Result<Response, Error> {
    let request = request.timeout(limits.timeout);
    tauri::async_runtime::block_on(async {
        let mut operation = Box::pin(async {
            let _slot = match limits.slots {
                Some(slots) => Some(slots.acquire().await.map_err(|_| Error::Unreachable)?),
                None => None,
            };
            let mut response = request.send().await.map_err(|e| if e.is_timeout() { Error::TimedOut } else { Error::Unreachable })?;
            let status = response.status().as_u16();
            let headers = response.headers().clone();
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| if e.is_timeout() { Error::TimedOut } else { Error::Unreadable })? {
                if body.len() + chunk.len() > limits.max_bytes { return Err(Error::TooLarge); }
                body.extend_from_slice(&chunk);
            }
            Ok(Response { status, headers, body })
        });
        let started = Instant::now();
        loop {
            cancelled().map_err(Error::Cancelled)?;
            if started.elapsed() >= limits.timeout { return Err(Error::TimedOut); }
            if let Ok(result) = tokio::time::timeout(Duration::from_millis(20), &mut operation).await {
                return result;
            }
        }
    })
}

/// No cancellation: the request runs until it completes or times out.
pub fn never_cancelled() -> Result<(), String> { Ok(()) }

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    fn serve_once(reply: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let _: Vec<_> = BufReader::new(&mut socket).lines().map_while(Result::ok).take_while(|l| !l.is_empty()).collect();
            socket.write_all(reply.as_bytes()).unwrap();
        });
        url
    }

    fn limits(max_bytes: usize) -> Limits<'static> {
        Limits { timeout: Duration::from_secs(5), max_bytes, slots: None }
    }

    #[test]
    fn redirects_are_returned_instead_of_followed() {
        let url = serve_once("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        let response = send(client().get(url).bearer_auth("secret"), limits(1024), &never_cancelled).unwrap();
        assert!(response.is_redirect());
        assert_eq!(response.header("location").as_deref(), Some("http://127.0.0.1:1/steal"));
    }

    #[test]
    fn bodies_over_budget_are_rejected() {
        let url = serve_once("HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\n0123456789");
        assert_eq!(send(client().get(url), limits(4), &never_cancelled).err(), Some(Error::TooLarge));
    }

    #[test]
    fn cancellation_interrupts_the_wait() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let result = send(client().get(url), limits(1024), &|| Err("cancelled".into()));
        assert_eq!(result.err(), Some(Error::Cancelled("cancelled".into())));
        drop(listener);
    }
}
