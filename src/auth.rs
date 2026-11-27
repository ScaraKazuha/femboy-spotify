//! Login: cached credentials first, interactive OAuth in the browser otherwise.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use librespot::core::{
    authentication::Credentials, cache::Cache, config::SessionConfig, session::Session,
};
use librespot::oauth::{OAuthClient, OAuthClientBuilder};

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

/// A logged-in user (the librespot session is used for audio and for library data).
pub struct Login {
    pub session: Session,
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

/// Logs in with what was saved last time. `Ok(None)`: nothing saved, or Spotify
/// turned the saved login down. `Err`: Spotify couldn't be reached (e.g. right
/// after Windows starts, before the network is up) — the saved login is kept.
pub async fn connect_cached() -> Result<Option<Login>> {
    let Some(creds) = cache()?.credentials() else {
        return Ok(None);
    };
    let mut last = None;
    for attempt in 1..=4 {
        match connect_session(creds.clone()).await {
            Ok(session) => return Ok(Some(Login { session })),
            Err(e) => {
                let msg = format!("{e:#}");
                log(&format!("saved login, attempt {attempt}: {msg}"));
                let lower = msg.to_lowercase();
                if lower.contains("bad credentials") || lower.contains("permission denied") || lower.contains("premium") && lower.contains("required") {
                    return Ok(None); // the saved login itself is no good
                }
                last = Some(e);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("could not reach Spotify")))
}

// ---------- sign-in inside the app (OAuth 2 with PKCE) ----------
//
// The sign-in page opens in the app's own window (the built-in player's), so one
// sign-in also logs the built-in player in. The app catches the redirect to
// REDIRECT_URI in that window itself; nothing has to listen on the port.

pub const OAUTH_REDIRECT: &str = REDIRECT_URI;

pub struct OAuthStart {
    /// page to open: Spotify's sign-in, continuing to the consent/redirect
    pub url: String,
    pub verifier: String,
    pub state: String,
}

fn random_string(len: usize) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    (0..len).map(|_| CHARS[fastrand::usize(..CHARS.len())] as char).collect()
}

pub fn oauth_start() -> OAuthStart {
    use base64::Engine;
    use sha2::Digest;
    let verifier = random_string(64);
    let state = random_string(24);
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
    let enc = crate::api::encode_query;
    let authorize = format!(
        "https://accounts.spotify.com/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge_method=S256&code_challenge={}&state={}",
        SessionConfig::default().client_id,
        enc(REDIRECT_URI),
        enc(&SCOPES.join(" ")),
        challenge,
        state,
    );
    let url = format!("https://accounts.spotify.com/login?continue={}", enc(&authorize));
    OAuthStart { url, verifier, state }
}

/// Trades the code from the redirect for a token and connects (the login is saved).
pub async fn oauth_finish(code: &str, verifier: &str) -> Result<Login> {
    let enc = crate::api::encode_query;
    let body = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
        enc(code),
        enc(REDIRECT_URI),
        SessionConfig::default().client_id,
        enc(verifier),
    );
    let resp = reqwest::Client::new()
        .post("https://accounts.spotify.com/api/token")
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .context("could not reach Spotify to finish signing in")?;
    let status = resp.status();
    let json: serde_json::Value = resp.json().await.context("odd answer from Spotify")?;
    let Some(token) = json["access_token"].as_str() else {
        anyhow::bail!("Spotify did not accept the sign-in ({status}): {}", json["error_description"].as_str().unwrap_or("no details"));
    };
    log("login: got access token (in-app sign-in)");
    let session = connect_session(Credentials::with_access_token(token.to_string())).await?;
    Ok(Login { session })
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
    Ok(Login { session })
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
        // leftovers of earlier builds that used the public Web API
        data_dir().join("credentials").join("web_auth.json"),
        data_dir().join("credentials").join("web_refresh_token"),
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
