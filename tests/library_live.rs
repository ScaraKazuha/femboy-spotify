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

/// Artist page, album page and song radio for the first liked song (read-only).
#[tokio::test]
#[ignore]
async fn artist_album_radio_load() {
    auth::init_runtime();
    let login = auth::connect_cached().await.unwrap().expect("no saved login");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let api = Api::new(&login);
    let liked = api.liked().await.unwrap();
    let tr = liked.first().expect("no liked songs");
    println!("song: {} — artists {:?} album {:?}", tr.name, tr.artist_list.iter().map(|a| &a.name).collect::<Vec<_>>(), tr.album_uri);

    let t = Instant::now();
    let artist = api.artist(&tr.artist_list[0].uri).await.unwrap();
    println!("artist '{}' in {:?}: image={} top={} albums={} singles={}", artist.name, t.elapsed(), artist.image.is_some(), artist.top.len(), artist.albums.len(), artist.singles.len());
    for s in artist.top.iter().take(3) { println!("  top: {}", s.name); }
    for a in artist.singles.iter().chain(artist.albums.iter()).take(3) { println!("  release: {} ({}) img={}", a.name, a.year, a.image.is_some()); }
    assert!(!artist.top.is_empty() || !artist.singles.is_empty() || !artist.albums.is_empty());

    let t = Instant::now();
    let album = api.album(tr.album_uri.as_deref().unwrap()).await.unwrap();
    println!("album '{}' ({}) in {:?}: {} tracks img={}", album.name, album.year, t.elapsed(), album.tracks.len(), album.image.is_some());
    assert!(!album.tracks.is_empty());

    let t = Instant::now();
    match api.context_tracks(&format!("spotify:station:track:{}", tr.id), 100).await {
        Ok(r) => println!("radio: {} tracks in {:?}, first: {:?}", r.len(), t.elapsed(), r.first().map(|x| &x.name)),
        Err(e) => println!("radio failed: {e:#}"),
    }
}

/// What owner names look like next to our own username (read-only).
#[tokio::test]
#[ignore]
async fn playlist_owner_names() {
    use librespot::protocol::playlist4_external::SelectedListContent;
    use protobuf::Message;
    auth::init_runtime();
    let login = auth::connect_cached().await.unwrap().expect("no saved login");
    let s = &login.session;
    println!("session username: {:?}", s.username());
    let root = SelectedListContent::parse_from_bytes(&s.spclient().get_rootlist(0, Some(20)).await.unwrap()).unwrap();
    for item in root.contents.items.iter().take(6) {
        let Some(id) = item.uri().strip_prefix("spotify:playlist:") else { continue };
        let sid = librespot::core::SpotifyId::from_base62(id).unwrap();
        let pl = SelectedListContent::parse_from_bytes(&s.spclient().get_playlist(&sid).await.unwrap()).unwrap();
        println!("{:<30} owner={:?} can_edit={:?}", pl.attributes.name(), pl.owner_username(), pl.abuse_reporting_enabled);
    }
}
