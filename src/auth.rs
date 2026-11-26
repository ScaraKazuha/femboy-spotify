//! Login: cached credentials first, interactive OAuth in the browser otherwise.

use std::path::PathBuf;

use anyhow::{Context, Result};
use librespot::core::{
    authentication::Credentials, cache::Cache, config::SessionConfig, session::Session,
};
use librespot::oauth::OAuthClientBuilder;

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

/// Logs in with saved credentials, if there are any that still work.
pub async fn connect_cached() -> Result<Option<Session>> {
    let cache = cache()?;
    let Some(creds) = cache.credentials() else {
        return Ok(None);
    };
    let session = Session::new(SessionConfig::default(), Some(cache));
    match session.connect(creds, true).await {
        Ok(()) => Ok(Some(session)),
        Err(_) => Ok(None),
    }
}

/// Opens the browser for a Spotify sign-in and connects with the result.
pub async fn login_interactive() -> Result<Session> {
    let config = SessionConfig::default();
    let client = OAuthClientBuilder::new(&config.client_id, REDIRECT_URI, SCOPES.to_vec())
        .open_in_browser()
        .with_custom_message("all done! you can go back to femboy spotify now <3")
        .build()
        .context("could not start OAuth sign-in")?;
    log("login: waiting for the browser redirect on 127.0.0.1:8989");
    let token = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        client.get_access_token_async(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("sign-in timed out (no answer from the browser in 5 minutes)"))?
    .context("Spotify sign-in failed")?;
    log(&format!(
        "login: got access token (scopes: {})",
        token.scopes.join(" ")
    ));

    let session = Session::new(config, Some(cache()?));
    log("login: connecting to a Spotify access point...");
    tokio::time::timeout(
        std::time::Duration::from_secs(60),
        session.connect(Credentials::with_access_token(token.access_token), true),
    )
    .await
    .map_err(|_| anyhow::anyhow!("connecting to Spotify timed out after 60 seconds"))?
    .context("could not connect to Spotify (is this a Premium account?)")?;
    log(&format!("login: connected as {}", session.username()));
    Ok(session)
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
    let file = data_dir().join("credentials").join("credentials.json");
    if file.exists() {
        std::fs::remove_file(&file)
            .with_context(|| format!("cannot delete {}", file.display()))?;
    }
    Ok(())
}

/// Terminal flow: saved login, otherwise print a hint and open the browser.
pub async fn connect() -> Result<Session> {
    if let Some(session) = connect_cached().await? {
        return Ok(session);
    }
    println!();
    println!("  \x1b[38;2;255;92;170m♡ femboy spotify ♡\x1b[0m");
    println!("  opening your browser to sign in to Spotify (Premium required)...");
    println!("  if it does not open, copy the link printed below into your browser.");
    println!();
    login_interactive().await
}