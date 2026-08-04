//! Token and request guards: the access-control checks applied to every
//! `/api` request before any body work happens. Implements spec
//! Security §1-4, §6.

use rand::RngCore;
use subtle::ConstantTimeEq;

/// A bearer token minted once per `serve` run, printed in the boot URL, and
/// required (as a header or a `?token=` query param) on every `/api`
/// request.
#[derive(Debug, Clone)]
pub struct Token {
    raw: String,
}

impl Token {
    /// 32 CSPRNG bytes, URL-safe base64 without padding.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        Self {
            raw: base64_url_no_pad(&bytes),
        }
    }

    pub fn from_raw(raw: impl Into<String>) -> Self {
        Self { raw: raw.into() }
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Constant-time comparison against a presented token. Length is
    /// compared in the clear first (the token's length is public — the
    /// URL it is printed in already discloses it); the byte comparison
    /// itself is constant-time.
    pub fn matches(&self, presented: &str) -> bool {
        self.raw.len() == presented.len()
            && bool::from(self.raw.as_bytes().ct_eq(presented.as_bytes()))
    }
}

/// A hand-rolled ~12-line URL-safe base64 encoder (no padding) so the
/// project does not add a `base64` dependency for one call site.
fn base64_url_no_pad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        let chars = [
            ALPHABET[((n >> 18) & 0x3f) as usize],
            ALPHABET[((n >> 12) & 0x3f) as usize],
            ALPHABET[((n >> 6) & 0x3f) as usize],
            ALPHABET[(n & 0x3f) as usize],
        ];
        match chunk.len() {
            1 => out.push_str(std::str::from_utf8(&chars[..2]).unwrap()),
            2 => out.push_str(std::str::from_utf8(&chars[..3]).unwrap()),
            _ => out.push_str(std::str::from_utf8(&chars).unwrap()),
        }
    }
    out
}

/// The server-side facts needed to admit or deny a request: minted token,
/// the loopback origin/port it is bound to, and whether `--bind-all` is in
/// effect (which relaxes only the Host check — anti-DNS-rebinding still
/// applies to Origin).
pub struct Guard {
    pub token: Token,
    pub port: u16,
    pub bind_all: bool,
}

/// What `check` needs from an incoming request, extracted from headers and
/// the query string before any body work.
pub struct ReqFacts<'a> {
    pub bearer: Option<&'a str>,
    pub query_token: Option<&'a str>,
    pub origin: Option<&'a str>,
    pub host: Option<&'a str>,
    pub client_header: Option<&'a str>,
    pub mutating: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Deny {
    Unauthorized,
    Forbidden(String),
}

/// Loopback names accepted on the bound port when NOT `--bind-all`.
const LOOPBACK_NAMES: &[&str] = &["127.0.0.1", "localhost", "[::1]"];

pub fn check(guard: &Guard, facts: &ReqFacts<'_>) -> Result<(), Deny> {
    // 1. Token: Bearer header first, then the `?token=` query fallback (the
    // wasm editor has no JS-to-wasm header channel for some request paths).
    let presented = facts.bearer.or(facts.query_token);
    match presented {
        Some(token) if guard.token.matches(token) => {}
        _ => return Err(Deny::Unauthorized),
    }

    // 2. Origin: exact-match allowlist when the header is present at all.
    // Never `*`, no Private-Network-Access header. Absent Origin (e.g. a
    // same-origin navigation, or curl) is allowed through to the Host check.
    if let Some(origin) = facts.origin {
        let allowed = expected_origins(guard.port);
        if !allowed.iter().any(|candidate| candidate == origin) {
            return Err(Deny::Forbidden(format!("origin not allowed: {origin}")));
        }
    }

    // 3. Host: anti-rebinding. Under `--bind-all`, any name is accepted as
    // long as the port matches; otherwise the host name itself must be a
    // loopback name.
    if let Some(host) = facts.host {
        if !host_is_acceptable(host, guard.port, guard.bind_all) {
            return Err(Deny::Forbidden(format!("host not allowed: {host}")));
        }
    }

    // 4. Mutating requests must carry the anti-CSRF client header — a
    // cross-site form post cannot set custom headers.
    if facts.mutating && facts.client_header != Some("1") {
        return Err(Deny::Forbidden(
            "mutating requests require X-Waml-Client: 1".to_string(),
        ));
    }

    Ok(())
}

fn expected_origins(port: u16) -> Vec<String> {
    LOOPBACK_NAMES
        .iter()
        .map(|name| format!("http://{name}:{port}"))
        .collect()
}

fn host_is_acceptable(host: &str, port: u16, bind_all: bool) -> bool {
    let (name, host_port) = match host.rsplit_once(':') {
        Some((name, port_str)) if !name.is_empty() => match port_str.parse::<u16>() {
            Ok(p) => (name, Some(p)),
            Err(_) => (host, None),
        },
        _ => (host, None),
    };

    if let Some(host_port) = host_port {
        if host_port != port {
            return false;
        }
    }

    if bind_all {
        return true;
    }

    LOOPBACK_NAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard(bind_all: bool) -> Guard {
        Guard {
            token: Token::from_raw("thetoken"),
            port: 8099,
            bind_all,
        }
    }

    fn facts<'a>() -> ReqFacts<'a> {
        ReqFacts {
            bearer: Some("thetoken"),
            query_token: None,
            origin: None,
            host: None,
            client_header: None,
            mutating: false,
        }
    }

    #[test]
    fn a_generated_token_is_long_and_unique() {
        let a = Token::generate();
        let b = Token::generate();
        assert!(a.as_str().len() >= 32);
        assert_ne!(a.as_str(), b.as_str());
    }

    #[test]
    fn accepts_the_token_in_either_position() {
        let g = guard(false);
        let mut f = facts();
        f.bearer = Some("thetoken");
        f.query_token = None;
        assert!(check(&g, &f).is_ok());

        f.bearer = None;
        f.query_token = Some("thetoken");
        assert!(check(&g, &f).is_ok());
    }

    #[test]
    fn rejects_a_missing_or_wrong_token() {
        let g = guard(false);
        let mut f = facts();
        f.bearer = None;
        f.query_token = None;
        assert_eq!(check(&g, &f), Err(Deny::Unauthorized));

        f.bearer = Some("thetokenX");
        assert_eq!(check(&g, &f), Err(Deny::Unauthorized));

        f.bearer = Some("thetoke");
        assert_eq!(check(&g, &f), Err(Deny::Unauthorized));
    }

    #[test]
    fn rejects_a_foreign_origin() {
        let g = guard(false);
        let mut f = facts();
        f.origin = Some("https://evil.example");
        assert!(matches!(check(&g, &f), Err(Deny::Forbidden(_))));

        f.origin = Some("http://127.0.0.1:8099");
        assert!(check(&g, &f).is_ok());
    }

    #[test]
    fn rejects_a_rebound_host() {
        let g = guard(false);
        let mut f = facts();
        f.host = Some("evil.example:8099");
        assert!(matches!(check(&g, &f), Err(Deny::Forbidden(_))));

        f.host = Some("127.0.0.1:9999");
        assert!(matches!(check(&g, &f), Err(Deny::Forbidden(_))));

        f.host = Some("[::1]:8099");
        assert!(check(&g, &f).is_ok());
    }

    #[test]
    fn mutating_requests_need_the_client_header() {
        let g = guard(false);
        let mut f = facts();
        f.mutating = true;
        f.client_header = None;
        assert!(matches!(check(&g, &f), Err(Deny::Forbidden(_))));

        f.client_header = Some("1");
        assert!(check(&g, &f).is_ok());
    }

    #[test]
    fn bind_all_relaxes_only_the_host_check() {
        let g = guard(true);
        let mut f = facts();
        f.host = Some("anything.example:8099");
        assert!(check(&g, &f).is_ok());

        // Origin allowlist is unaffected by bind_all.
        f.origin = Some("https://evil.example");
        assert!(matches!(check(&g, &f), Err(Deny::Forbidden(_))));
    }
}
