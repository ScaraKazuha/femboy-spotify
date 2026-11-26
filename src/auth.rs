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

pub async fn connect() -> Result<Session> {
    let config = SessionConfig::default();
    let cache = cache()?;

    if let Some(creds) = cache.credentials() {
        let session = Session::new(config.clone(), Some(cache.clone()));
        match session.connect(creds, true).await {
            Ok(()) => return Ok(session),
            Err(e) => eprintln!("saved login did not work ({e}), signing in again..."),
        }
    }

    println!();
    println!("  \x1b[38;2;255;92;170m♡ femboy spotify ♡\x1b[0m");
    println!("  opening your browser to sign in to Spotify (Premium required)...");
    println!("  if it does not open, copy the link printed below into your browser.");
    println!();

    let client = OAuthClientBuilder::new(&config.client_id, REDIRECT_URI, SCOPES.to_vec())
        .open_in_browser()
        .with_custom_message("all done! you can go back to femboy spotify now ♡")
        .build()
        .context("could not start OAuth sign-in")?;
    let token = client
        .get_access_token_async()
        .await
        .context("Spotify sign-in failed")?;

    let session = Session::new(config, Some(cache));
    session
        .connect(Credentials::with_access_token(token.access_token), true)
        .await
        .context("could not connect to Spotify (is this a Premium account?)")?;
    Ok(session)
}
