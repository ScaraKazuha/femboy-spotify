//! Live check against the saved login (read-only). Run by hand:
//! `cargo test --release --test library_live -- --ignored --nocapture`

use std::time::Instant;

use femboy_spotify::{api::Api, auth};

#[tokio::test]
#[ignore]
async fn library_loads_for_saved_login() {
    auth::init_runtime();
    let login = auth::connect_cached().await.unwrap().expect("no saved login");
    // product info arrives shortly after connecting
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let mut attrs: Vec<_> = login.session.user_data().attributes.into_iter().collect();
    attrs.sort();
    for (k, v) in attrs.iter().filter(|(k, _)| ["type", "catalogue", "financial-product", "streaming-rules", "license"].contains(&k.as_str())) {
        println!("attribute {k} = {v}");
    }
    let api = Api::new(&login);

    let t = Instant::now();
    let me = api.me().await.unwrap();
    println!("me: {} avatar={}", me.name, me.avatar.is_some());

    let pls = api.playlists().await.unwrap();
    println!("playlists: {} in {:?}", pls.len(), t.elapsed());
    for p in pls.iter().take(20) {
        println!("  {:<40} {:>4} tracks  owner={:<10} image={}", p.name, p.total, p.owner, p.image.is_some());
    }
    assert!(!pls.is_empty());

    let t = Instant::now();
    let tracks = api.playlist_tracks(&pls[0].id).await.unwrap();
    println!("'{}': {} tracks in {:?}", pls[0].name, tracks.len(), t.elapsed());
    for tr in tracks.iter().take(3) {
        println!("  {} — {} [{}] {}ms img={}", tr.name, tr.artists, tr.album, tr.duration_ms, tr.image.is_some());
    }

    let t = Instant::now();
    let liked = api.liked().await.unwrap();
    println!("liked: {} tracks in {:?}", liked.len(), t.elapsed());

    let t = Instant::now();
    let found = api.search("pink sky").await.unwrap();
    println!("search 'pink sky': {} in {:?}", found.len(), t.elapsed());
    for tr in found.iter().take(3) {
        println!("  {} — {}", tr.name, tr.artists);
    }

    if let Some(tr) = liked.first() {
        let ly = api.lyrics(&tr.id).await.unwrap();
        println!("lyrics for '{}': {:?} lines", tr.name, ly.map(|l| l.len()));
    }
}
