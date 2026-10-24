//! The only code in Ledgeling that may open a network connection for an
//! optional online feature. Every request names a [`Scope`], and `send` refuses
//! it unless the user has switched that scope on. No other file references an
//! HTTP client; `tests/privacy.rs` enforces that.
use super::{
    consent::{Consent, Mode},
    scope::Scope,
};
use serde::Serialize;
use std::{
    io::Read,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};

const USER_AGENT: &str = "Ledgeling";
const MAX_REDIRECTS: usize = 3;
const CHUNK: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NetError {
    /// Stable, machine-readable: `offline`, `refused`, `failed`, `too_large`
    /// or `cancelled`. The UI switches on this, never on the message.
    pub code: &'static str,
    pub message: String,
}

impl NetError {
    fn new(code: &'static str, message: impl Into<String>) -> NetError {
        NetError {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub struct Request {
    pub url: String,
    /// `None` is a GET; a body makes it a POST.
    pub body: Option<Vec<u8>>,
    pub content_type: Option<String>,
    pub bearer: Option<String>,
    /// The response is cut off, and the call fails, beyond this many bytes.
    pub max_bytes: u64,
}

impl Request {
    pub fn get(url: impl Into<String>, max_bytes: u64) -> Request {
        Request {
            url: url.into(),
            body: None,
            content_type: None,
            bearer: None,
            max_bytes,
        }
    }
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Response {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeStatus {
    pub scope: Scope,
    pub label: &'static str,
    pub description: &'static str,
    pub hosts: Vec<String>,
    /// Whether this build knows a server for the scope. Unavailable scopes
    /// cannot be switched on.
    pub available: bool,
    pub mode: Mode,
    /// Whether "until I switch it off" is the only way to turn it on.
    pub always_remembered: bool,
}

pub struct Net {
    consent: Mutex<Consent>,
    /// Bumped when a scope is switched off, so requests already running for it
    /// stop at their next chunk instead of finishing.
    epochs: [AtomicU64; Scope::ALL.len()],
    #[cfg(test)]
    test_hosts: Mutex<Vec<(Scope, String)>>,
}

impl Net {
    pub fn new(consent: Consent) -> Net {
        Net {
            consent: Mutex::new(consent),
            epochs: Default::default(),
            #[cfg(test)]
            test_hosts: Mutex::new(Vec::new()),
        }
    }

    pub fn status(&self) -> Vec<ScopeStatus> {
        let consent = self.consent.lock().unwrap_or_else(|e| e.into_inner());
        Scope::ALL
            .into_iter()
            .map(|scope| ScopeStatus {
                scope,
                label: scope.label(),
                description: scope.description(),
                hosts: scope.hosts(),
                available: scope.available(),
                mode: consent.mode(scope),
                always_remembered: scope.always_remembered(),
            })
            .collect()
    }

    pub fn allows(&self, scope: Scope) -> bool {
        self.consent
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .allows(scope)
    }

    /// Change one scope. Switching it off also stops its requests in flight.
    pub fn set(&self, scope: Scope, mode: Mode) -> Result<(), String> {
        if mode != Mode::Off && self.hosts(scope).is_empty() {
            return Err(format!(
                "{} isn't available in this version of Ledgeling.",
                scope.label()
            ));
        }
        self.consent
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set(scope, mode)?;
        if mode == Mode::Off {
            self.epochs[slot(scope)].fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn get(&self, scope: Scope, url: &str, max_bytes: u64) -> Result<Response, NetError> {
        self.send(scope, Request::get(url, max_bytes))
    }

    /// The single gate: nothing below `gate` runs for a scope that is off.
    pub fn send(&self, scope: Scope, request: Request) -> Result<Response, NetError> {
        self.gate(scope)?;
        let epoch = self.epochs[slot(scope)].load(Ordering::SeqCst);
        let hosts = self.hosts(scope);
        if hosts.is_empty() {
            return Err(NetError::new(
                "unavailable",
                format!(
                    "{} isn't available in this version of Ledgeling.",
                    scope.label()
                ),
            ));
        }
        let url = check_url(&request.url, &hosts)?;
        let allowed = hosts.clone();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(USER_AGENT)
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                let next = attempt.url();
                let on_list = next
                    .host_str()
                    .is_some_and(|h| allowed.iter().any(|a| a == h));
                if attempt.previous().len() >= MAX_REDIRECTS {
                    attempt.error("too many redirects")
                } else if on_list && scheme_allowed(next.scheme()) {
                    attempt.follow()
                } else {
                    // Hand back the redirect itself rather than leave the list.
                    attempt.stop()
                }
            }))
            .build()
            .map_err(|_| NetError::new("failed", "Could not start a network connection."))?;
        let mut builder = match request.body {
            Some(body) => client.post(url).body(body),
            None => client.get(url),
        };
        if let Some(kind) = request.content_type {
            builder = builder.header(reqwest::header::CONTENT_TYPE, kind);
        }
        if let Some(token) = request.bearer {
            builder = builder.bearer_auth(token);
        }
        let mut response = builder
            .send()
            .map_err(|_| NetError::new("failed", "Could not reach the server."))?;
        if response
            .content_length()
            .is_some_and(|n| n > request.max_bytes)
        {
            return Err(too_large(request.max_bytes));
        }
        let status = response.status().as_u16();
        let mut body = Vec::new();
        let mut chunk = vec![0_u8; CHUNK];
        loop {
            if self.epochs[slot(scope)].load(Ordering::SeqCst) != epoch || !self.allows(scope) {
                return Err(NetError::new("cancelled", "You switched this off."));
            }
            let read = response
                .read(&mut chunk)
                .map_err(|_| NetError::new("failed", "The connection was interrupted."))?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..read]);
            if body.len() as u64 > request.max_bytes {
                return Err(too_large(request.max_bytes));
            }
        }
        Ok(Response { status, body })
    }

    fn gate(&self, scope: Scope) -> Result<(), NetError> {
        if self.allows(scope) {
            Ok(())
        } else {
            Err(NetError::new(
                "offline",
                format!("{} is switched off.", scope.label()),
            ))
        }
    }

    fn hosts(&self, scope: Scope) -> Vec<String> {
        #[allow(unused_mut)]
        let mut hosts: Vec<String> = scope.hosts();
        #[cfg(test)]
        hosts.extend(
            self.test_hosts
                .lock()
                .unwrap()
                .iter()
                .filter(|(s, _)| *s == scope)
                .map(|(_, h)| h.clone()),
        );
        hosts
    }

    /// Tests talk to a local server; production code has no such door.
    #[cfg(test)]
    pub fn allow_for_test(&self, scope: Scope, host: &str) {
        self.test_hosts
            .lock()
            .unwrap()
            .push((scope, host.to_string()));
    }
}

fn slot(scope: Scope) -> usize {
    Scope::ALL.iter().position(|s| *s == scope).unwrap_or(0)
}

fn too_large(max: u64) -> NetError {
    NetError::new(
        "too_large",
        format!("The response is larger than the {} MiB limit.", max >> 20),
    )
}

fn scheme_allowed(scheme: &str) -> bool {
    scheme == "https" || (cfg!(test) && scheme == "http")
}

fn check_url(text: &str, hosts: &[String]) -> Result<reqwest::Url, NetError> {
    let refused =
        |why: &str| NetError::new("refused", format!("That address is not allowed: {why}."));
    let url = reqwest::Url::parse(text).map_err(|_| refused("it is not a web address"))?;
    if !scheme_allowed(url.scheme()) {
        return Err(refused("it must start with https://"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(refused("it contains a login"));
    }
    if !cfg!(test) && url.port().is_some() {
        return Err(refused("it names a port"));
    }
    match url.host_str() {
        Some(host) if hosts.iter().any(|h| h == host) => Ok(url),
        _ => Err(refused("it is not one of this feature's servers")),
    }
}
