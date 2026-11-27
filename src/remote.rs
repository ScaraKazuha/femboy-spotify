//! Spotify Connect remote control.
//!
//! Spotify no longer hands out audio decryption keys to librespot, so instead of
//! playing audio ourselves we act like the official app's "devices" menu: we join
//! the user's Connect cluster as a hidden observer, mirror what the active device
//! is playing, and send it commands (play, pause, skip, seek, volume, transfer).
//! The music itself plays in the official Spotify app (phone, computer, speaker…).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use futures_util::StreamExt;
use librespot::core::{dealer::protocol::Message, session::Session};
use librespot::protocol::{
    connect::{
        Capabilities, Cluster, ClusterUpdate, Device, DeviceInfo, MemberType, PutStateReason,
        PutStateRequest,
    },
    devices::DeviceType,
};
use protobuf::{EnumOrUnknown, Message as _, MessageField};
use reqwest::Method;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Serialize;
use serde_json::{Value, json};

use crate::auth::log;

/// Error text the UI recognises to show "open Spotify somewhere".
pub const NO_DEVICE: &str = "no-spotify-device";

/// Spotify's AI DJ ("X") is a special playlist with this id.
pub const DJ_URI: &str = "spotify:playlist:37i9dQZF1EYkqdzj48dyYq";

#[derive(Clone, Debug, Serialize)]
pub struct RemoteDevice {
    pub id: String,
    pub name: String,
    pub kind: String,
    /// 0..=100
    pub volume: u16,
    pub active: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RemoteState {
    pub devices: Vec<RemoteDevice>,
    pub active_device: Option<String>,
    pub active_device_name: Option<String>,
    pub track_uri: Option<String>,
    pub context_uri: String,
    pub playing: bool,
    pub buffering: bool,
    /// position at the moment this state was sent
    pub position_ms: u64,
    pub duration_ms: u64,
    pub shuffle: bool,
    pub repeat_context: bool,
    pub repeat_track: bool,
    /// active device volume, 0..=100
    pub volume: Option<u16>,
    /// what plays next (queued songs first, then the rest of the context)
    pub next_uris: Vec<String>,
    /// songs you put in the queue yourself ("Next in queue")
    pub queue_uris: Vec<String>,
    pub queue_revision: String,
}

type Listener = Box<dyn Fn(RemoteState) + Send + Sync>;

#[derive(Clone)]
pub struct Remote {
    session: Session,
    cluster: Arc<Mutex<Option<Cluster>>>,
    on_update: Arc<Listener>,
}

fn device_kind(t: DeviceType) -> &'static str {
    match t {
        DeviceType::COMPUTER => "computer",
        DeviceType::TABLET => "tablet",
        DeviceType::SMARTPHONE => "phone",
        DeviceType::SPEAKER => "speaker",
        DeviceType::TV => "tv",
        DeviceType::AVR | DeviceType::STB | DeviceType::AUDIO_DONGLE => "speaker",
        DeviceType::GAME_CONSOLE => "console",
        DeviceType::CAST_VIDEO | DeviceType::CAST_AUDIO => "cast",
        DeviceType::AUTOMOBILE => "car",
        DeviceType::SMARTWATCH => "watch",
        _ => "device",
    }
}

fn to_percent(v: u32) -> u16 {
    ((v.min(65535) as u64 * 100 + 32767) / 65535) as u16
}

impl Remote {
    /// Joins the Connect cluster. `on_update` is called whenever the state changes.
    pub async fn start(
        session: Session,
        on_update: impl Fn(RemoteState) + Send + Sync + 'static,
    ) -> Result<Self> {
        fn connection_id(msg: Message) -> Result<String, librespot::core::Error> {
            msg.headers
                .get("Spotify-Connection-Id")
                .cloned()
                .ok_or_else(|| librespot::core::Error::invalid_argument("no connection id"))
        }

        let dealer = session.dealer();
        let mut conn_ids = dealer
            .listen_for("hm://pusher/v1/connections/", connection_id)
            .map_err(|e| anyhow!("connect: {e}"))?;
        let mut updates = dealer
            .listen_for("hm://connect-state/v1/cluster", Message::from_raw::<ClusterUpdate>)
            .map_err(|e| anyhow!("connect: {e}"))?;
        dealer
            .start()
            .await
            .map_err(|e| anyhow!("could not reach Spotify Connect: {e}"))?;

        let remote = Self {
            session,
            cluster: Arc::new(Mutex::new(None)),
            on_update: Arc::new(Box::new(on_update)),
        };

        let first = tokio::time::timeout(Duration::from_secs(20), conn_ids.next())
            .await
            .map_err(|_| anyhow!("Spotify Connect did not answer (timeout)"))?
            .ok_or_else(|| anyhow!("Spotify Connect closed the connection"))?
            .map_err(|e| anyhow!("connect: {e}"))?;
        remote.register(&first).await?;

        // keep listening: new connection ids (reconnects) and cluster updates
        let r = remote.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    id = conn_ids.next() => match id {
                        Some(Ok(id)) => {
                            if let Err(e) = r.register(&id).await {
                                log(&format!("connect: re-register failed: {e:#}"));
                            }
                        }
                        Some(Err(e)) => log(&format!("connect: bad connection id: {e}")),
                        None => break,
                    },
                    up = updates.next() => match up {
                        Some(Ok(mut up)) => {
                            if let Some(c) = up.cluster.take() {
                                r.set_cluster(c);
                            }
                        }
                        Some(Err(e)) => log(&format!("connect: bad cluster update: {e}")),
                        None => break,
                    },
                }
            }
            log("connect: listener stopped");
        });
        Ok(remote)
    }

    /// Announces us as a hidden, non-playing observer device and stores the cluster.
    async fn register(&self, connection_id: &str) -> Result<()> {
        self.session.set_connection_id(connection_id);
        let info = DeviceInfo {
            can_play: false,
            volume: 0,
            name: "femboy spotify".into(),
            device_id: self.session.device_id().to_string(),
            device_type: EnumOrUnknown::new(DeviceType::COMPUTER),
            device_software_version: env!("CARGO_PKG_VERSION").into(),
            client_id: self.session.client_id(),
            capabilities: MessageField::some(Capabilities {
                can_be_player: false,
                is_observable: true,
                is_controllable: false,
                hidden: true,
                needs_full_player_state: true,
                supports_gzip_pushes: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let req = PutStateRequest {
            member_type: EnumOrUnknown::new(MemberType::CONNECT_STATE),
            put_state_reason: EnumOrUnknown::new(PutStateReason::NEW_DEVICE),
            device: MessageField::some(Device {
                device_info: MessageField::some(info),
                ..Default::default()
            }),
            ..Default::default()
        };
        let bytes = self
            .session
            .spclient()
            .put_connect_state_request(&req)
            .await
            .map_err(|e| anyhow!("could not join Spotify Connect: {e}"))?;
        let cluster = Cluster::parse_from_bytes(&bytes).context("bad cluster from Spotify")?;
        log(&format!(
            "connect: joined, {} device(s), active: '{}'",
            cluster.device.len(),
            cluster.active_device_id
        ));
        self.set_cluster(cluster);
        Ok(())
    }

    /// Asks Spotify for the current cluster again. Spotify only pushes updates when
    /// playback changes, so a device that just came online (our built-in player
    /// right after start) may be missing until we ask.
    pub async fn refresh(&self) -> Result<RemoteState> {
        let id = self.session.connection_id();
        if !id.is_empty() {
            self.register(&id).await?;
        }
        Ok(self.state())
    }

    fn set_cluster(&self, c: Cluster) {
        let state = self.state_of(&c);
        *self.cluster.lock().unwrap() = Some(c);
        (self.on_update)(state);
    }

    pub fn state(&self) -> RemoteState {
        match &*self.cluster.lock().unwrap() {
            Some(c) => self.state_of(c),
            None => RemoteState::default(),
        }
    }

    fn state_of(&self, c: &Cluster) -> RemoteState {
        let me = self.session.device_id().to_string();
        let mut devices: Vec<RemoteDevice> = c
            .device
            .values()
            .filter(|d| d.device_id != me)
            .filter(|d| !d.capabilities.hidden && d.can_play)
            .map(|d| RemoteDevice {
                id: d.device_id.clone(),
                name: d.name.clone(),
                kind: device_kind(d.device_type.enum_value_or_default()).into(),
                volume: to_percent(d.volume),
                active: d.device_id == c.active_device_id,
            })
            .collect();
        devices.sort_by(|a, b| b.active.cmp(&a.active).then(a.name.cmp(&b.name)));

        let active = (!c.active_device_id.is_empty()).then(|| c.active_device_id.clone());
        let active_dev = active.as_ref().and_then(|id| c.device.get(id));
        let p = c.player_state.as_ref();
        let (mut st, uri) = (RemoteState::default(), p.and_then(|p| p.track.as_ref()).map(|t| t.uri.clone()));
        if let Some(p) = p {
            let playing = p.is_playing && !p.is_paused;
            let mut pos = p.position_as_of_timestamp.max(0);
            if playing && c.server_timestamp_ms > 0 && p.timestamp > 0 {
                pos += (c.server_timestamp_ms - p.timestamp).max(0);
            }
            if p.duration > 0 {
                pos = pos.min(p.duration);
            }
            st.playing = playing && active.is_some();
            st.buffering = p.is_buffering;
            st.position_ms = pos as u64;
            st.duration_ms = p.duration.max(0) as u64;
            st.context_uri = p.context_uri.clone();
            if let Some(o) = p.options.as_ref() {
                st.shuffle = o.shuffling_context;
                st.repeat_context = o.repeating_context;
                st.repeat_track = o.repeating_track;
            }
            st.queue_uris = p
                .next_tracks
                .iter()
                .filter(|t| t.provider == "queue" && t.uri.starts_with("spotify:track:"))
                .map(|t| t.uri.clone())
                .collect();
            st.queue_revision = p.queue_revision.clone();
            st.next_uris = p
                .next_tracks
                .iter()
                .map(|t| t.uri.clone())
                .filter(|u| u.starts_with("spotify:track:"))
                .take(50)
                .collect();
        }
        st.track_uri = uri.filter(|u| !u.is_empty() && active.is_some());
        st.devices = devices;
        st.active_device_name = active_dev.map(|d| d.name.clone());
        st.volume = active_dev.map(|d| to_percent(d.volume));
        st.active_device = active;
        st
    }

    fn active_device(&self) -> Option<String> {
        self.cluster
            .lock()
            .unwrap()
            .as_ref()
            .map(|c| c.active_device_id.clone())
            .filter(|id| !id.is_empty())
    }

    fn target(&self, device: Option<&str>) -> Result<String> {
        if let Some(d) = device.filter(|d| !d.is_empty()) {
            return Ok(d.to_string());
        }
        if let Some(d) = self.active_device() {
            return Ok(d);
        }
        bail!(NO_DEVICE)
    }

    async fn post_json(&self, method: Method, endpoint: &str, body: Value) -> Result<()> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let body = body.to_string();
        self.session
            .spclient()
            .request(&method, endpoint, Some(headers), Some(body.as_bytes()))
            .await
            .map_err(|e| anyhow!("Spotify did not accept the command: {e}"))?;
        Ok(())
    }

    async fn command(&self, device: Option<&str>, command: Value) -> Result<()> {
        let to = self.target(device)?;
        let me = self.session.device_id();
        let endpoint = command["endpoint"].as_str().unwrap_or("?").to_string();
        let ctx = command["context"]["uri"].as_str().unwrap_or("").to_string();
        let res = self
            .post_json(
                Method::POST,
                &format!("/connect-state/v1/player/command/from/{me}/to/{to}"),
                json!({ "command": command }),
            )
            .await;
        log(&format!(
            "connect: {endpoint} {ctx} -> {to}: {}",
            match &res {
                Ok(()) => "ok".to_string(),
                Err(e) => format!("{e:#}"),
            }
        ));
        res
    }

    /// Plays `uris[index]`. With `context` (a playlist, liked songs or search) the
    /// device keeps playing that context; otherwise the given list is sent as-is.
    pub async fn play(
        &self,
        context: Option<String>,
        uris: &[String],
        index: usize,
        device: Option<&str>,
    ) -> Result<()> {
        let track = uris.get(index).cloned().ok_or_else(|| anyhow!("nothing to play"))?;
        let ctx = match context {
            Some(uri) => json!({ "uri": uri, "url": format!("context://{uri}"), "metadata": {} }),
            None => json!({
                "uri": format!("spotify:femboy:queue:{}", track.trim_start_matches("spotify:track:")),
                "metadata": {},
                "pages": [{ "tracks": uris.iter().take(500).map(|u| json!({ "uri": u })).collect::<Vec<_>>() }],
            }),
        };
        let cmd = json!({
            "endpoint": "play",
            "context": ctx,
            "play_origin": { "feature_identifier": "femboy_spotify", "feature_version": env!("CARGO_PKG_VERSION") },
            "options": {
                "license": "premium",
                "skip_to": { "track_uri": track, "track_index": index },
                "player_options_override": {},
            },
        });
        self.command(device, cmd).await
    }

    /// Starts a whole context from its beginning (used for Spotify DJ).
    pub async fn play_context(&self, uri: &str, device: Option<&str>) -> Result<()> {
        let cmd = json!({
            "endpoint": "play",
            "context": { "uri": uri, "url": format!("context://{uri}"), "metadata": {} },
            "play_origin": { "feature_identifier": "femboy_spotify", "feature_version": env!("CARGO_PKG_VERSION") },
            "options": { "license": "premium", "player_options_override": {} },
        });
        self.command(device, cmd).await
    }

    /// Puts a track in the play queue (after the current song).
    pub async fn add_to_queue(&self, track_uri: &str) -> Result<()> {
        self.command(
            None,
            json!({
                "endpoint": "add_to_queue",
                "track": { "uri": track_uri, "metadata": { "is_queued": "true" }, "provider": "queue" },
            }),
        )
        .await
    }

    /// The current and upcoming tracks as Spotify describes them (uri, provider,
    /// metadata), for looking into special items such as DJ voice segments.
    pub fn debug_tracks(&self) -> Vec<(String, String, Vec<(String, String)>)> {
        let guard = self.cluster.lock().unwrap();
        let Some(p) = guard.as_ref().and_then(|c| c.player_state.as_ref()) else { return vec![] };
        p.track
            .iter()
            .chain(p.next_tracks.iter())
            .take(12)
            .map(|t| {
                let mut md: Vec<(String, String)> = t.metadata.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                md.sort();
                (t.uri.clone(), t.provider.clone(), md)
            })
            .collect()
    }

    /// DJ "switch it up": jump to the start of the DJ's next block (a new vibe).
    /// Spotify marks those starts with `jump_track` in the queue; without one we
    /// just go to the next song.
    pub async fn dj_switch(&self) -> Result<()> {
        let target = {
            let guard = self.cluster.lock().unwrap();
            guard
                .as_ref()
                .and_then(|c| c.player_state.as_ref())
                .and_then(|p| {
                    p.next_tracks
                        .iter()
                        .find(|t| t.metadata.get("jump_track").is_some_and(|v| v == "true"))
                        .map(|t| json!({ "uri": t.uri, "uid": t.uid }))
                })
        };
        match target {
            Some(track) => self.command(None, json!({ "endpoint": "skip_next", "track": track })).await,
            None => self.simple("skip_next").await,
        }
    }

    /// Drops the songs you queued yourself; the rest of the context keeps playing.
    pub async fn clear_queue(&self) -> Result<()> {
        let (next, prev, revision) = {
            let guard = self.cluster.lock().unwrap();
            let p = guard
                .as_ref()
                .and_then(|c| c.player_state.as_ref())
                .ok_or_else(|| anyhow!("nothing is playing"))?;
            let keep = |t: &librespot::protocol::player::ProvidedTrack| {
                json!({ "uri": t.uri, "uid": t.uid, "provider": t.provider, "metadata": t.metadata })
            };
            let next: Vec<Value> = p.next_tracks.iter().filter(|t| t.provider != "queue").map(keep).collect();
            let prev: Vec<Value> = p.prev_tracks.iter().map(keep).collect();
            (next, prev, p.queue_revision.clone())
        };
        self.command(
            None,
            json!({ "endpoint": "set_queue", "next_tracks": next, "prev_tracks": prev, "queue_revision": revision }),
        )
        .await
    }

    pub async fn simple(&self, endpoint: &str) -> Result<()> {
        self.command(None, json!({ "endpoint": endpoint })).await
    }

    pub async fn seek(&self, ms: u64) -> Result<()> {
        self.command(None, json!({ "endpoint": "seek_to", "value": ms })).await
    }

    pub async fn set_option(&self, endpoint: &str, value: bool) -> Result<()> {
        self.command(None, json!({ "endpoint": endpoint, "value": value })).await
    }

    /// `percent` is 0..=100.
    pub async fn set_volume(&self, percent: u16) -> Result<()> {
        let to = self.target(None)?;
        let me = self.session.device_id();
        let v = (percent.min(100) as u32 * 65535 / 100) as u64;
        self.post_json(
            Method::PUT,
            &format!("/connect-state/v1/connect/volume/from/{me}/to/{to}"),
            json!({ "volume": v }),
        )
        .await
    }

    /// Moves playback to another device.
    pub async fn transfer(&self, to: &str) -> Result<()> {
        let from = self.active_device().unwrap_or_else(|| to.to_string());
        self.post_json(
            Method::POST,
            &format!("/connect-state/v1/connect/transfer/from/{from}/to/{to}"),
            json!({ "transfer_options": { "restore_paused": "restore" } }),
        )
        .await
    }
}
