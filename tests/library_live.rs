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

/// Probe of read-only endpoints used for search tabs, history and "date added".
#[tokio::test]
#[ignore]
async fn probe_endpoints() {
    use librespot::protocol::playlist4_external::SelectedListContent;
    use protobuf::Message;
    use reqwest::Method;
    auth::init_runtime();
    let login = auth::connect_cached().await.unwrap().expect("no saved login");
    let s = &login.session;
    let user = s.username();
    let get = |ep: String| {
        let s = s.clone();
        async move {
            match s.spclient().request_as_json(&Method::GET, &ep, None, None).await {
                Ok(b) => String::from_utf8_lossy(&b).to_string(),
                Err(e) => format!("ERR {e}"),
            }
        }
    };
    let show = |name: &str, body: &str| println!("== {name}: {} bytes\n{}\n", body.len(), &body.chars().take(900).collect::<String>());

    let country = s.country();
    let b = get(format!("/searchview/km/v4/search/pink%20sky?entityVersion=2&limit=5&catalogue=premium&country={country}&locale=en")).await;
    show("searchview v4", &b);
    let b = get(format!("/recently-played/v3/user/{user}/recently-played?format=json&limit=10&filter=default,collection-new-episodes")).await;
    show("recently-played v3", &b);
    let b = get(format!("/context-resolve/v1/spotify:user:{user}:collection")).await;
    show("liked context (first page)", &b);

    let root = SelectedListContent::parse_from_bytes(&s.spclient().get_rootlist(0, Some(5)).await.unwrap()).unwrap();
    if let Some(id) = root.contents.items.iter().find_map(|i| i.uri().strip_prefix("spotify:playlist:")) {
        let sid = librespot::core::SpotifyId::from_base62(id).unwrap();
        let pl = SelectedListContent::parse_from_bytes(&s.spclient().get_playlist(&sid).await.unwrap()).unwrap();
        for it in pl.contents.items.iter().take(3) {
            println!("playlist item {} timestamp={:?} added_by={:?}", it.uri(), it.attributes.timestamp, it.attributes.added_by);
        }
    }
}

/// Which full-search endpoint answers (read-only).
#[tokio::test]
#[ignore]
async fn probe_search() {
    use reqwest::Method;
    auth::init_runtime();
    let login = auth::connect_cached().await.unwrap().expect("no saved login");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let s = &login.session;
    let (user, country) = (s.username(), s.country());
    let tries = [
        format!("/searchview/km/v4/search/pink%20sky?entityVersion=2&limit=5&imageSize=large&catalogue=premium&country={country}&locale=en&username={user}"),
        format!("/searchview/km/v2/search/pink%20sky?entityVersion=2&limit=5&imageSize=large&catalogue=premium&country={country}&locale=en&username={user}"),
        format!("/searchview/km/v4/search/pink%20sky?limit=5&country={country}&locale=en&username={user}&catalogue=&entityVersion=2&platform=desktop"),
        "/searchview/km/v3/search/pink%20sky?limit=5".to_string(),
        format!("/search/v1/search?q=pink%20sky&limit=5&country={country}"),
    ];
    for ep in tries {
        let r = s.spclient().request_as_json(&Method::GET, &ep, None, None).await;
        match r {
            Ok(b) => println!("OK {ep}\n{}\n", String::from_utf8_lossy(&b).chars().take(700).collect::<String>()),
            Err(e) => println!("ERR {ep}: {e}"),
        }
    }
}

/// History, full search and collection sets (read-only).
#[tokio::test]
#[ignore]
async fn recent_search_collection() {
    auth::init_runtime();
    let login = auth::connect_cached().await.unwrap().expect("no saved login");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let api = Api::new(&login);
    let t = Instant::now();
    let recent = api.recent().await.unwrap();
    println!("recent: {} in {:?}", recent.len(), t.elapsed());
    for c in recent.iter().take(5) { println!("  {} {} — {} img={}", c.kind, c.name, c.sub, c.image.is_some()); }
    let t = Instant::now();
    let s = api.search_all("hatsune miku").await.unwrap();
    println!("search: {} tracks, {} artists, {} albums in {:?}", s.tracks.len(), s.artists.len(), s.albums.len(), t.elapsed());
    for c in s.artists.iter().take(3) { println!("  artist {} img={}", c.name, c.image.is_some()); }
    let artists = api.collection_set("artist").await.unwrap();
    let coll = api.collection_set("collection").await.unwrap();
    println!("followed artists: {}  collection: {} ({} albums)", artists.len(), coll.len(), coll.iter().filter(|u| u.starts_with("spotify:album:")).count());
    let liked = api.liked().await.unwrap();
    println!("liked first added_at: {:?}", liked.first().map(|t| t.added_at));
}
