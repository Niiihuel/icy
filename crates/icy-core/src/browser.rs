//! Pure address-bar logic for the native browser: turns typed text into URLs.

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Turns text the user typed into an absolute URL, or `None` when it is not a URL.
pub fn normalize_url(input: &str) -> Option<String> {
    let text = input.trim();
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    if ["http://", "https://", "file://", "about:", "data:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
    {
        return Some(text.to_string());
    }
    let host = host_of(&lower);
    if host.starts_with('[') {
        // IPv6 literal: only a complete `[...]` host is a URL.
        return host.ends_with(']').then(|| format!("http://{text}"));
    }
    if host == "localhost" || host.ends_with(".localhost") || is_ipv4(host) {
        return Some(format!("http://{text}"));
    }
    if lower.starts_with("www.") {
        return Some(format!("https://{text}"));
    }
    None
}

/// Resolves URL-bar text: a URL, a bare domain, or a search query.
pub fn address_url(input: &str) -> Option<String> {
    let text = input.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(url) = normalize_url(text) {
        return Some(url);
    }
    if !text.chars().any(char::is_whitespace) && looks_like_domain(host_of(text)) {
        return Some(format!("https://{text}"));
    }
    Some(format!("https://duckduckgo.com/?q={}", encode_query(text)))
}

/// The host part of `text`: everything before the first `/`, `?` or `#`, without the port.
fn host_of(text: &str) -> &str {
    let authority = text.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.starts_with('[') {
        return authority
            .find(']')
            .map_or(authority, |end| &authority[..=end]);
    }
    authority.split(':').next().unwrap_or_default()
}

fn is_ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && part.parse::<u8>().is_ok()
        })
}

fn looks_like_domain(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| !label.is_empty())
        && labels
            .last()
            .is_some_and(|tld| tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic()))
}

/// Percent-encodes a query string; spaces become `+`.
fn encode_query(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(b))
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(char::from(HEX[usize::from(b >> 4)]));
                out.push(char::from(HEX[usize::from(b & 0xF)]));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_url_adds_http_to_local_hosts() {
        assert_eq!(
            normalize_url("localhost:3000").as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(
            normalize_url("127.0.0.1:5173/app").as_deref(),
            Some("http://127.0.0.1:5173/app")
        );
        assert_eq!(
            normalize_url("[::1]:8080").as_deref(),
            Some("http://[::1]:8080")
        );
        assert_eq!(
            normalize_url("app.localhost:8000").as_deref(),
            Some("http://app.localhost:8000")
        );
    }

    #[test]
    fn normalize_url_keeps_explicit_schemes() {
        assert_eq!(
            normalize_url("https://x.dev").as_deref(),
            Some("https://x.dev")
        );
        assert_eq!(
            normalize_url("HTTP://x.dev").as_deref(),
            Some("HTTP://x.dev")
        );
        assert_eq!(
            normalize_url("file:///tmp/a.html").as_deref(),
            Some("file:///tmp/a.html")
        );
        assert_eq!(normalize_url("about:blank").as_deref(), Some("about:blank"));
        assert_eq!(
            normalize_url("data:text/plain,hi").as_deref(),
            Some("data:text/plain,hi")
        );
    }

    #[test]
    fn normalize_url_prefixes_www_with_https() {
        assert_eq!(
            normalize_url("www.rust-lang.org").as_deref(),
            Some("https://www.rust-lang.org")
        );
    }

    #[test]
    fn normalize_url_rejects_plain_words_files_and_blanks() {
        assert_eq!(normalize_url("main.rs"), None);
        assert_eq!(normalize_url("foo bar"), None);
        assert_eq!(normalize_url("[::1"), None);
        assert_eq!(normalize_url(""), None);
        assert_eq!(normalize_url("   "), None);
    }

    #[test]
    fn address_url_adds_https_to_domains() {
        assert_eq!(
            address_url("example.com").as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            address_url("example.com/docs?x=1").as_deref(),
            Some("https://example.com/docs?x=1")
        );
    }

    #[test]
    fn address_url_searches_everything_else() {
        assert_eq!(
            address_url("rust lang").as_deref(),
            Some("https://duckduckgo.com/?q=rust+lang")
        );
        assert_eq!(
            address_url("a&b").as_deref(),
            Some("https://duckduckgo.com/?q=a%26b")
        );
        assert_eq!(address_url("main.rs").as_deref(), Some("https://main.rs"));
        assert_eq!(
            address_url("100%/ok").as_deref(),
            Some("https://duckduckgo.com/?q=100%25%2Fok")
        );
    }

    #[test]
    fn address_url_rejects_blank_input() {
        assert_eq!(address_url("  "), None);
        assert_eq!(address_url(""), None);
    }
}
