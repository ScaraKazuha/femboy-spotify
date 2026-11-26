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
        .with_custom_message("all done! you can go back to femboy spotify now â™¡")
        .build()
        .context("could not start OAuth sign-in")?;
    let token = client
        .get_access_token_async()
        .await
        .context("Spotify sign-in failed")?;

    let session = Session::new(config, Some(cache()?));
    session
        .connect(Credentials::with_access_token(token.access_token), true)
        .await
        .context("could not connect to Spotify (is this a Premium account?)")?;
    Ok(session)
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
    println!("  \x1b[38;2;255;92;170mâ™¡ femboy spotify â™¡\x1b[0m");
    println!("  opening your browser to sign in to Spotify (Premium required)...");
    println!("  if it does not open, copy the link printed below into your browser.");
    println!();
    login_interactive().await
}