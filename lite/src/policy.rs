//! Where the window may go. Spotify's web player and its sign-in pages open inside
//! the app; every other link opens in the default browser; anything that isn't a
//! plain web address is refused.

use url::Url;

#[derive(Debug, PartialEq, Eq)]
pub enum Destination {
    Embedded,
    Browser,
    Blocked,
}

/// Only top-level navigation is filtered; the site's own resources stay subject to
/// WebView2's normal origin and TLS protections.
pub fn destination(url: &Url) -> Destination {
    if !matches!(url.scheme(), "http" | "https") || !url.username().is_empty() || url.password().is_some() {
        return Destination::Blocked;
    }
    let host = url.host_str().unwrap_or_default();
    let secure = url.scheme() == "https" && url.port_or_known_default() == Some(443);
    let spotify = matches!(host, "open.spotify.com" | "accounts.spotify.com" | "challenge.spotify.com");
    // "Continue with Google / Apple / Facebook" on Spotify's sign-in page
    let providers = matches!(host, "accounts.google.com" | "appleid.apple.com" | "www.facebook.com" | "m.facebook.com")
        && !url.path().starts_with("/download");
    if secure && (spotify || providers) { Destination::Embedded } else { Destination::Browser }
}

/// Google refuses sign-in inside some embedded browsers.
pub fn sign_in_rejected(url: &Url) -> bool {
    url.host_str() == Some("accounts.google.com") && (url.path().contains("/rejected") || url.path().contains("/denied"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_trust_boundary() {
        for address in [
            "https://open.spotify.com/",
            "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
            "https://accounts.spotify.com/en/login?continue=https%3A%2F%2Fopen.spotify.com%2F",
            "https://challenge.spotify.com/c/abc",
            "https://accounts.google.com/o/oauth2/v2/auth",
            "https://appleid.apple.com/auth/authorize",
        ] {
            assert_eq!(destination(&Url::parse(address).unwrap()), Destination::Embedded, "{address}");
        }
        for address in [
            "https://open.spotify.com.evil.example/",
            "https://example.com/?next=https://open.spotify.com",
            "http://open.spotify.com/",
            "https://open.spotify.com:8443/",
            "https://www.spotify.com/download/",
            "https://support.spotify.com/",
        ] {
            assert_eq!(destination(&Url::parse(address).unwrap()), Destination::Browser, "{address}");
        }
        for address in [
            "file:///C:/Windows/system.ini",
            "javascript:alert(1)",
            "data:text/html,test",
            "spotify:track:4uLU6hMCjMI75M1A2tKUQC",
            "ms-settings:privacy",
            "https://user:password@open.spotify.com/",
        ] {
            assert_eq!(destination(&Url::parse(address).unwrap()), Destination::Blocked, "{address}");
        }
    }
}
