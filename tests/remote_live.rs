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

/// What Spotify says about the DJ's voice segments in the play queue (read-only).
#[tokio::test]
#[ignore]
async fn dj_voice_metadata() {
    femboy_spotify::auth::init_runtime();
    let login = femboy_spotify::auth::connect_cached().await.unwrap().expect("no saved login");
    let remote = femboy_spotify::remote::Remote::start(login.session.clone(), |_| {}).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    for (uri, provider, md) in remote.debug_tracks() {
        println!("{uri}  provider={provider}");
        for (k, v) in md {
            println!("    {k} = {}", v.chars().take(160).collect::<String>());
        }
    }
}

/// Try to find where a DJ voice segment's audio lives (read-only).
#[tokio::test]
#[ignore]
async fn dj_voice_audio_probe() {
    use librespot::protocol::{
        extended_metadata::{BatchedEntityRequest, EntityRequest, ExtensionQuery},
        extension_kind::ExtensionKind,
    };
    use protobuf::EnumOrUnknown;
    femboy_spotify::auth::init_runtime();
    let login = femboy_spotify::auth::connect_cached().await.unwrap().expect("no saved login");
    let remote = femboy_spotify::remote::Remote::start(login.session.clone(), |_| {}).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let Some((uri, _, _)) = remote.debug_tracks().into_iter().find(|(u, _, _)| u.starts_with("spotify:media:")) else {
        println!("no DJ voice segment in the queue right now");
        return;
    };
    println!("segment {uri}");
    for kind in [ExtensionKind::AUDIO_FILES, ExtensionKind::PLAYABILITY, ExtensionKind::AUDIO_ATTRIBUTES, ExtensionKind::AUDIO_ASSOCIATIONS] {
        let req = BatchedEntityRequest {
            entity_request: vec![EntityRequest {
                entity_uri: uri.clone(),
                query: vec![ExtensionQuery { extension_kind: EnumOrUnknown::new(kind), ..Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        };
        match login.session.spclient().get_extended_metadata(req).await {
            Ok(res) => {
                for em in res.extended_metadata {
                    for d in em.extension_data {
                        let bytes = d.extension_data.as_ref().map(|a| a.value.clone()).unwrap_or_default();
                        let text: String = bytes.iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
                        println!("{kind:?}: {} bytes, header={:?}\n  {}", bytes.len(), d.header.as_ref().map(|h| h.status_code), text.chars().take(400).collect::<String>());
                    }
                }
            }
            Err(e) => println!("{kind:?}: ERR {e}"),
        }
    }
}
