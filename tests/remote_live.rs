//! Live check of the Spotify Connect observer (does not change playback). Run by hand:
//! `cargo test --release --test remote_live -- --ignored --nocapture`

use femboy_spotify::{
    auth,
    remote::{DJ_URI, Remote},
};

/// Plays Spotify DJ for a few seconds on a computer, then pauses. Run by hand.
#[tokio::test]
#[ignore]
async fn dj_starts() {
    auth::init_runtime();
    let login = auth::connect_cached().await.unwrap().expect("no saved login");
    let remote = Remote::start(login.session.clone(), |_| {}).await.unwrap();
    let st = remote.state();
    let device = st
        .active_device
        .clone()
        .or_else(|| st.devices.iter().find(|d| d.kind == "computer").map(|d| d.id.clone()))
        .expect("no device online");
    let res = remote.play_context(DJ_URI, Some(&device)).await;
    println!("play DJ: {res:?}");
    for _ in 0..4 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let st = remote.state();
        println!(
            "context={} track={:?} playing={} pos={} next={:?}",
            st.context_uri,
            st.track_uri,
            st.playing,
            st.position_ms,
            st.next_uris.iter().take(3).collect::<Vec<_>>()
        );
    }
    println!("pause: {:?}", remote.simple("pause").await);
}

#[tokio::test]
#[ignore]
async fn connect_cluster_is_visible() {
    auth::init_runtime();
    let login = auth::connect_cached().await.unwrap().expect("no saved login");
    let remote = Remote::start(login.session.clone(), |st| {
        println!(
            "update: active={:?} playing={} track={:?} pos={}ms/{}ms devices={}",
            st.active_device_name,
            st.playing,
            st.track_uri,
            st.position_ms,
            st.duration_ms,
            st.devices.len()
        );
    })
    .await
    .expect("could not join Spotify Connect");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let st = remote.state();
    for d in &st.devices {
        println!("device: {:<30} {:<9} vol={:>3}% active={}", d.name, d.kind, d.volume, d.active);
    }
    println!("shuffle={} repeat={}/{} next={} context={}", st.shuffle, st.repeat_context, st.repeat_track, st.next_uris.len(), st.context_uri);
}
