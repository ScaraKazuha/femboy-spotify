//! The built-in player: its settings and the scripts it runs.
//!
//! Spotify refuses audio keys to librespot, so the app carries Spotify's own web player
//! in a WebView2 nobody sees. WebView2 has Edge's DRM, so it plays fine, and it shows
//! up as a Spotify Connect device ("Web Player (Microsoft Edge)") that `remote.rs`
//! drives like any other. The page gets no access to the app's commands.

/// Where the player lives.
pub const HOME: &str = "https://open.spotify.com/";

/// Spotify's sign-in, continuing to the player.
pub const LOGIN: &str = "https://accounts.spotify.com/login?continue=https%3A%2F%2Fopen.spotify.com%2F";

/// The main window's WebView2 arguments (`tauri.conf.json` has them; every webview in
/// one data folder must use the same). Tauri's default, plus: music may start without a
/// click, and nothing gets throttled while the window is in the background.
pub const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
     --autoplay-policy=no-user-gesture-required --disable-background-timer-throttling \
     --disable-renderer-backgrounding --disable-backgrounding-occluded-windows";

/// The app's WebView2 data folder (Tauri's, for the main window).
pub fn data_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("dev.scarakazuha.femboyspotify")
}

/// The player runs in a WebView2 environment of its own, tuned for a page nobody ever
/// sees: it draws without the graphics card (sharing the window's GPU process cost a
/// few hundred MB; the window keeps the GPU for its animations) and in Chromium's
/// low-memory mode for weak devices (smaller caches and memory budget). Measured with
/// a song playing: 1008 MB for the whole app before, 864 MB after.
pub const PLAYER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
     --autoplay-policy=no-user-gesture-required --disable-background-timer-throttling \
     --disable-renderer-backgrounding --disable-backgrounding-occluded-windows --disable-gpu \
     --enable-low-end-device-mode";

/// The player's own data folder (an environment with other arguments needs one).
pub fn player_data_dir() -> std::path::PathBuf {
    data_dir().join("player")
}

/// First start with the player's own folder: takes its Spotify sign-in (cookies)
/// from the shared folder earlier versions used. Windows ties their encryption to
/// the user, not the folder, so they keep working; nothing to sign in to again.
/// Must run before any webview starts (they lock the cookies file).
pub fn take_sign_in() {
    let done = player_data_dir().join("sign-in-taken");
    if done.exists() {
        return;
    }
    let to = player_data_dir().join("EBWebView");
    let existed = to.exists();
    let _ = std::fs::create_dir_all(player_data_dir());
    let from = data_dir().join("EBWebView");
    let files = ["Local State", "Default/Network/Cookies", "Default/Network/Cookies-journal"];
    if !from.join(files[1]).exists() {
        let _ = std::fs::write(&done, "1"); // nothing to take (a new install)
        return;
    }
    let mut ok = true;
    for f in files {
        let src = from.join(f);
        if !src.exists() {
            continue;
        }
        let dst = to.join(f);
        if let Some(dir) = dst.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::copy(&src, &dst) {
            crate::auth::log(&format!("player: couldn't take the sign-in ({f}): {e}"));
            ok = false;
        }
    }
    if ok {
        let _ = std::fs::write(&done, "1");
        crate::auth::log("player: took the Spotify sign-in into its own folder");
    } else if !existed {
        // half a profile is worse than none: start clean (tried again next start)
        let _ = std::fs::remove_dir_all(&to);
    }
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
  // Spotify's service worker only caches the web page, in a process of its own
  const sw = navigator.serviceWorker;
  if (sw) {
    sw.register = () => Promise.reject(new Error("no service worker in femboy spotify"));
    sw.getRegistrations().then((list) => list.forEach((r) => r.unregister())).catch(() => {});
  }
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
