//! Login: cached credentials first, interactive OAuth in the browser otherwise.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use librespot::core::{
    authentication::Credentials, cache::Cache, config::SessionConfig, session::Session,
};
use librespot::oauth::{OAuthClient, OAuthClientBuilder, OAuthToken};

const REDIRECT_URI: &str = "http://127.0.0.1:8989/login";

const SCOPES: &[&str] = &[
    "streaming",
    "user-read-private",
    "user-read-email",
    "user-library-read",
    "user-library-modify",
    "user-read-recently-played",
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-read-playback-state",
    "user-modify-playback-state",
];

pub fn data_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("femboy-spotify")
}

fn cache() -> Result<Cache> {
    let dir = data_dir();
    Cache::new(
        Some(dir.join("credentials")),
        Some(dir.join("volume")),
        Some(dir.join("audio")),
        Some(1024 * 1024 * 1024), // 1 GiB audio cache
    )
    .map_err(|e| anyhow::anyhow!("cannot create cache in {}: {e}", dir.display()))
}

/// A logged-in user: the librespot session (audio) plus Web API access.
pub struct Login {
    pub session: Session,
    pub web: Arc<WebAuth>,
}

/// Error text the UI looks for to show the "connect your Client ID" card.
pub const WEB_NOT_SET_UP: &str = "web-api-not-set-up";

/// Address of the Spotify page where people create their own Client ID.
pub const DASHBOARD_URL: &str = "https://developer.spotify.com/dashboard";

/// Scopes the Web API part needs (profile, library, playlists, history).
const WEB_SCOPES: &[&str] = &[
    "user-read-private",
    "user-read-email",
    "user-library-read",
    "user-library-modify",
    "user-read-recently-played",
    "playlist-read-private",
    "playlist-read-collaborative",
];

/// Access to api.spotify.com with the user's *own* developer-app Client ID.
///
/// librespot's shared Client ID is permanently rate limited (HTTP 429) on the
/// Web API, and its keymaster tokens are refused (403), so profile, playlists
/// and search go through a Client ID the user creates in the Spotify dashboard.
/// Audio playback keeps using librespot.
pub struct WebAuth {
    inner: tokio::sync::Mutex<Option<WebToken>>,
}

struct WebToken {
    client_id: String,
    client: OAuthClient,
    token: OAuthToken,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SavedWeb {
    client_id: String,
    refresh_token: String,
}

fn web_file() -> PathBuf {
    data_dir().join("credentials").join("web_auth.json")
}

fn save_web(client_id: &str, refresh_token: &str) {
    if refresh_token.is_empty() {
        return;
    }
    let _ = std::fs::create_dir_all(data_dir().join("credentials"));
    let saved = SavedWeb {
        client_id: client_id.to_string(),
        refresh_token: refresh_token.to_string(),
    };
    if let Err(e) = std::fs::write(web_file(), serde_json::to_string(&saved).unwrap_or_default()) {
        log(&format!("could not save web login: {e}"));
    }
}

/// The Client ID saved last time (if any), so the setup card can prefill it.
pub fn saved_client_id() -> Option<String> {
    let text = std::fs::read_to_string(web_file()).ok()?;
    let saved: SavedWeb = serde_json::from_str(&text).ok()?;
    Some(saved.client_id)
}

fn web_client(client_id: &str, open_browser: bool) -> Result<OAuthClient> {
    let mut b = OAuthClientBuilder::new(client_id, REDIRECT_URI, WEB_SCOPES.to_vec())
        .with_custom_message("all done! you can go back to femboy spotify now <3");
    if open_browser {
        b = b.open_in_browser();
    }
    b.build().context("could not start OAuth sign-in")
}

impl WebAuth {
    /// Restores the saved Web API login, or starts "not set up".
    async fn restore() -> Self {
        let web = Self {
            inner: tokio::sync::Mutex::new(None),
        };
        let Ok(text) = std::fs::read_to_string(web_file()) else {
            return web;
        };
        let Ok(saved) = serde_json::from_str::<SavedWeb>(&text) else {
            return web;
        };
        let Ok(client) = web_client(&saved.client_id, false) else {
            return web;
        };
        match client.refresh_token_async(&saved.refresh_token).await {
            Ok(mut token) => {
                if token.refresh_token.is_empty() {
                    token.refresh_token = saved.refresh_token;
                }
                save_web(&saved.client_id, &token.refresh_token);
                log("web: restored saved Web API login");
                *web.inner.lock().await = Some(WebToken {
                    client_id: saved.client_id,
                    client,
                    token,
                });
            }
            Err(e) => log(&format!("web: saved Web API login expired: {e}")),
        }
        web
    }

    pub async fn is_ready(&self) -> bool {
        self.inner.lock().await.is_some()
    }

    /// Opens the browser so the user approves their own app, then keeps the token.
    pub async fn connect(&self, client_id: &str) -> Result<()> {
        let client_id = client_id.trim();
        if client_id.len() != 32 || !client_id.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!("that doesn't look like a Client ID (it is 32 letters/digits, like 1a2b3c...)");
        }
        let client = web_client(client_id, true)?;
        log("web: waiting for the browser to approve the app");
        let token = tokio::time::timeout(Duration::from_secs(300), client.get_access_token_async())
            .await
            .map_err(|_| anyhow::anyhow!("timed out waiting for the browser (5 minutes)"))?
            .context("Spotify did not accept the app (check the Client ID and the Redirect URI)")?;
        save_web(client_id, &token.refresh_token);
        log("web: connected with own Client ID");
        *self.inner.lock().await = Some(WebToken {
            client_id: client_id.to_string(),
            client,
            token,
        });
        Ok(())
    }

    pub async fn access_token(&self) -> Result<String> {
        let mut guard = self.inner.lock().await;
        let Some(w) = guard.as_mut() else {
            anyhow::bail!(WEB_NOT_SET_UP);
        };
        if w.token.expires_at <= Instant::now() + Duration::from_secs(60) {
            log("web: token expired, refreshing");
            let keep = w.token.refresh_token.clone();
            let mut new = w
                .client
                .refresh_token_async(&keep)
                .await
                .context("could not refresh the Spotify token, please log in again")?;
            if new.refresh_token.is_empty() {
                new.refresh_token = keep;
            }
            save_web(&w.client_id, &new.refresh_token);
            w.token = new;
        }
        Ok(w.token.access_token.clone())
    }
}

fn librespot_client() -> Result<OAuthClient> {
    let config = SessionConfig::default();
    OAuthClientBuilder::new(&config.client_id, REDIRECT_URI, SCOPES.to_vec())
        .open_in_browser()
        .with_custom_message("all done! you can go back to femboy spotify now <3")
        .build()
        .context("could not start OAuth sign-in")
}

async fn connect_session(creds: Credentials) -> Result<Session> {
    let session = Session::new(SessionConfig::default(), Some(cache()?));
    log("login: connecting to a Spotify access point...");
    tokio::time::timeout(Duration::from_secs(60), session.connect(creds, true))
        .await
        .map_err(|_| anyhow::anyhow!("connecting to Spotify timed out after 60 seconds"))?
        .context("could not connect to Spotify (is this a Premium account?)")?;
    log(&format!("login: connected as {}", session.username()));
    Ok(session)
}

/// Logs in with what was saved last time, if it still works.
pub async fn connect_cached() -> Result<Option<Login>> {
    let Some(creds) = cache()?.credentials() else {
        return Ok(None);
    };
    match connect_session(creds).await {
        Ok(session) => Ok(Some(Login {
            session,
            web: Arc::new(WebAuth::restore().await),
        })),
        Err(e) => {
            log(&format!("saved login did not work: {e:#}"));
            Ok(None)
        }
    }
}

/// Opens the browser for a Spotify sign-in and connects with the result.
pub async fn login_interactive() -> Result<Login> {
    let client = librespot_client()?;
    log("login: waiting for the browser redirect on 127.0.0.1:8989");
    let token = tokio::time::timeout(Duration::from_secs(300), client.get_access_token_async())
        .await
        .map_err(|_| anyhow::anyhow!("sign-in timed out (no answer from the browser in 5 minutes)"))?
        .context("Spotify sign-in failed")?;
    log("login: got access token");
    let session = connect_session(Credentials::with_access_token(token.access_token)).await?;
    Ok(Login {
        session,
        web: Arc::new(WebAuth::restore().await),
    })
}

// ---------- tiny file log (also receives librespot's own log messages) ----------

pub fn log_path() -> PathBuf {
    data_dir().join("femboy-spotify.log")
}

pub fn log(line: &str) {
    use std::io::Write;
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::create_dir_all(data_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = writeln!(f, "[{secs}] {line}");
    }
}

struct FileLogger;

impl ::log::Log for FileLogger {
    fn enabled(&self, m: &::log::Metadata) -> bool {
        let ours = m.target().starts_with("librespot") || m.target().starts_with("femboy");
        m.level() <= if ours { ::log::Level::Debug } else { ::log::Level::Warn }
    }
    fn log(&self, r: &::log::Record) {
        if self.enabled(r.metadata()) {
            log(&format!("{} {}: {}", r.level(), r.target(), r.args()));
        }
    }
    fn flush(&self) {}
}

/// Sends `log` crate messages (librespot's internals) to the log file.
pub fn init_file_log() {
    if ::log::set_boxed_logger(Box::new(FileLogger)).is_ok() {
        ::log::set_max_level(::log::LevelFilter::Debug);
    }
}

/// Must run first thing in `main`.
///
/// Both rustls crypto backends (ring from librespot, aws-lc-rs from reqwest) end up
/// in the build, and rustls then panics on the first TLS connection unless one of
/// them is picked explicitly. Panics are also written to the log file, because the
/// GUI build has no console to print them to.
pub fn init_runtime() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log(&format!("PANIC: {info}"));
        default_hook(info);
    }));
}

/// Forgets the saved login.
pub fn logout() -> Result<()> {
    for file in [
        data_dir().join("credentials").join("credentials.json"),
        web_file(),
        data_dir().join("credentials").join("web_refresh_token"), // from v0.2 builds
    ] {
        if file.exists() {
            std::fs::remove_file(&file)
                .with_context(|| format!("cannot delete {}", file.display()))?;
        }
    }
    Ok(())
}

/// Terminal flow: saved login, otherwise print a hint and open the browser.
pub async fn connect() -> Result<Login> {
    if let Some(login) = connect_cached().await? {
        return Ok(login);
    }
    println!();
    println!("  \x1b[38;2;255;92;170m♡ femboy spotify ♡\x1b[0m");
    println!("  opening your browser to sign in to Spotify (Premium required)...");
    println!("  if it does not open, copy the link printed below into your browser.");
    println!();
    login_interactive().await
}
