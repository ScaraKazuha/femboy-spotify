//! Regression test: librespot's HTTPS client used to panic ("no process-level
//! CryptoProvider") because two rustls crypto backends are compiled in.
//! Needs network access to apresolve.spotify.com; no Spotify login.

use librespot::core::{SessionConfig, session::Session};

#[tokio::test]
async fn librespot_https_works_after_init_runtime() {
    femboy_spotify::auth::init_runtime();
    let session = Session::new(SessionConfig::default(), None);
    let ap = session
        .apresolver()
        .resolve("accesspoint")
        .await
        .expect("apresolve request failed");
    println!("access point: {ap:?}");
}
