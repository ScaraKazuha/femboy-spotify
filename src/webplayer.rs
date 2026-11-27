//! The built-in player, shared by both desktop versions.
//!
//! Spotify refuses audio keys to librespot, so the apps carry Spotify's own web player
//! in a WebView2 nobody sees. WebView2 has Edge's DRM, so it plays fine, and it shows
//! up as a Spotify Connect device ("Web Player (Microsoft Edge)") that `remote.rs`
//! drives like any other. The page gets no access to the apps' commands.

/// Where the player lives.
pub const HOME: &str = "https://open.spotify.com/";

/// Spotify's sign-in, continuing to the player.
pub const LOGIN: &str = "https://accounts.spotify.com/login?continue=https%3A%2F%2Fopen.spotify.com%2F";

/// Every webview of an app shares one WebView2 environment, and both versions share
/// the same data folder (one sign-in for both), so they all must use exactly these
/// arguments (`tauri.conf.json` repeats them for the main window). The first part is
/// Tauri's default; the rest lets the player start music without a click and keeps it
/// from being throttled while nobody sees it.
pub const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
     --autoplay-policy=no-user-gesture-required --disable-background-timer-throttling \
     --disable-renderer-backgrounding --disable-backgrounding-occluded-windows";

/// The WebView2 data folder (cookies: the player's Spotify sign-in), the one Tauri
/// uses for the main version.
pub fn data_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("dev.scarakazuha.femboyspotify")
}

/// Sites the player may visit: Spotify and its "continue with" sign-in providers.
pub fn may_visit(url: &str) -> bool {
    if url == "about:blank" {
        return true; // used to unload the player on exit
    }
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#', ':']).next().unwrap_or_default();
    ["spotify.com", "google.com", "apple.com", "facebook.com", "gstatic.com"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Runs in the player. Nobody sees Spotify's web player, yet it would draw ~200
/// animations at the screen's refresh rate, which costs a few hundred MB in WebView2's
/// GPU process. On open.spotify.com every animation and transition is cut to 1 ms (so
/// the ones the page waits for still finish and fire their events). Spotify's sign-in
/// pages are on other hosts and keep theirs.
pub const CALM_SCRIPT: &str = r#"
(() => {
  if (location.hostname !== 'open.spotify.com') return;
  const add = () => {
    if (document.getElementById('femboy-calm')) return;
    const s = document.createElement('style');
    s.id = 'femboy-calm';
    s.textContent = '*, *::before, *::after { animation-duration: 1ms !important; animation-delay: 0s !important;'
      + ' animation-iteration-count: 1 !important; transition-duration: 1ms !important; transition-delay: 0s !important; }';
    (document.head || document.documentElement).append(s);
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', add); else add();
})();
"#;

/// Script that sets the player's volume (0..=100) right on its page: instant, unlike a
/// Spotify Connect volume command, which goes through Spotify's servers and which the
/// web player often undoes when the volume goes up.
pub fn volume_script(volume: u16) -> String {
    let v = f64::from(volume.min(100)) / 100.0;
    // the slider snaps to 10 % steps unless its step is relaxed while we set it
    format!(
        "(() => {{ const i = [...document.querySelectorAll('input[type=range]')].find((x) => x.max === '1'); \
         if (!i) return; const step = i.getAttribute('step'); i.setAttribute('step', 'any'); \
         Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, '{v}'); \
         i.dispatchEvent(new Event('input', {{ bubbles: true }})); i.dispatchEvent(new Event('change', {{ bubbles: true }})); \
         if (step) i.setAttribute('step', step); }})()"
    )
}

/// The player's own device in Spotify Connect.
pub fn is_player_device(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with("web player")
}

#[cfg(test)]
mod tests {
    use super::may_visit;

    #[test]
    fn visits() {
        assert!(may_visit("https://open.spotify.com/"));
        assert!(may_visit("https://accounts.spotify.com/login?x=1"));
        assert!(may_visit("https://accounts.google.com/o/oauth2"));
        assert!(may_visit("about:blank"));
        assert!(!may_visit("http://open.spotify.com/"));
        assert!(!may_visit("https://evilspotify.com/"));
        assert!(!may_visit("https://spotify.com.evil.net/"));
    }
}
