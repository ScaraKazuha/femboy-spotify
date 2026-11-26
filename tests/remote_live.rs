//! Live check of the Spotify Connect observer (does not change playback). Run by hand:
//! `cargo test --release --test remote_live -- --ignored --nocapture`

use femboy_spotify::{auth, remote::Remote};

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
