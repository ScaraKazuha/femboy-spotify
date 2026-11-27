//! A pretend backend for the page in a plain browser (`trunk serve`, the README
//! screenshot): demo playlists and songs, and a pretend Spotify Connect device.
//! `?shot` starts a song, `?update` shows an update, `?noplayer` the player setup.

use std::cell::RefCell;

use serde_json::{Value, json};

use crate::bridge::Listener;
use crate::util::{later, now_ms, sleep, window};
use crate::views::DJ_URI;

fn search() -> String {
    window().location().search().unwrap_or_default()
}

pub fn has_flag(flag: &str) -> bool {
    search().contains(flag)
}

pub fn cover(seed: usize, label: &str) -> String {
    let pal = [
        ("#ffc6de", "#c9b2f5"),
        ("#bfe3ff", "#ffc2dc"),
        ("#ffe0c2", "#ff9fc6"),
        ("#d6c5ff", "#9fc9ff"),
        ("#ffd6ea", "#fff0b8"),
        ("#c9f1e4", "#d8c4ff"),
    ][seed % 6];
    let shapes = [
        r##"<path d="M50 74S24 58 20 44c-3-10 3-20 13-20 7 0 12 4 17 10 5-6 10-10 17-10 10 0 16 10 13 20-4 14-30 30-30 30z" fill="#fff" opacity=".9"/>"##,
        r##"<path d="M50 14c3 22 9 30 36 36-27 6-33 14-36 36-3-22-9-30-36-36 27-6 33-14 36-36z" fill="#fff" opacity=".9"/>"##,
        r##"<circle cx="50" cy="52" r="24" fill="#fff" opacity=".85"/><path d="M34 34 30 16l14 10M66 34l4-18-14 10" fill="#fff" opacity=".85"/>"##,
    ][seed % 3];
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/>{shapes}<text x="50" y="94" font-family="sans-serif" font-size="9" text-anchor="middle" fill="#6d3462" opacity=".7">{label}</text></svg>"##,
        pal.0, pal.1
    );
    format!("data:image/svg+xml,{}", String::from(js_sys::encode_uri_component(&svg)))
}

const PLAYLISTS: [&str; 14] = [
    "Boykisser Mix", "DJ", "Meme Mix", "onokami Mix", "Spit It Out", "LEGACY Radio", "Shluzov Mix", "Daily Mix 1",
    "pink noise", "sleepy girl hours", "gym but cute", "night drive ✧", "hyperpop", "study ♡",
];

const TRACKS: [(&str, &str, u32); 8] = [
    ("MURDER EVERY 1 U KNOW!", "femtanyl, takihasdied", 101000),
    ("strawberry milk", "pastel kid", 184000),
    ("heart.exe", "glitchbunny", 152000),
    ("pink sky", "lavender", 201000),
    ("catboy summer", "meowmix", 176000),
    ("sugar rush", "candycore", 143000),
    ("blåhaj", "shark gang", 165000),
    ("skirt season", "thigh highs", 199000),
];

fn playlists() -> Value {
    PLAYLISTS
        .iter()
        .enumerate()
        .map(|(i, name)| json!({ "id": format!("pl{i}"), "name": name, "owner": "cutie", "total": 20 + i * 7, "image": cover(i, "") }))
        .collect()
}

pub fn tracks() -> Value {
    TRACKS
        .iter()
        .enumerate()
        .map(|(i, (name, artists, d))| {
            let list: Vec<Value> = artists
                .split(", ")
                .map(|a| json!({ "name": a, "uri": format!("spotify:artist:{}", a.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect::<String>()) }))
                .collect();
            json!({
                "id": format!("t{i}"), "uri": format!("spotify:track:t{i}"), "name": name, "artists": artists,
                "artist_list": list, "album": format!("cute album vol. {}", i % 3 + 1), "album_uri": format!("spotify:album:a{}", i % 3 + 1),
                "duration_ms": d, "image": cover(i + 2, ""), "image_large": cover(i + 2, ""),
            })
        })
        .collect()
}

fn track_list() -> Vec<Value> {
    tracks().as_array().cloned().unwrap_or_default()
}

fn matching(q: &str) -> Value {
    let q = q.to_lowercase();
    track_list()
        .into_iter()
        .filter(|t| format!("{}{}", t["name"].as_str().unwrap_or(""), t["artists"].as_str().unwrap_or("")).to_lowercase().contains(&q))
        .collect()
}

#[derive(Default)]
struct Remote {
    active: Option<String>,
    uris: Vec<String>,
    uri: Option<String>,
    context: String,
    playing: bool,
    pos: f64,
    at: f64,
    volume: f64,
    shuffle: bool,
    repeat_context: bool,
    repeat_track: bool,
}

thread_local! {
    static REMOTE: RefCell<Remote> = RefCell::new(Remote { volume: 66.0, ..Default::default() });
    static LISTENERS: RefCell<Vec<(String, Listener)>> = const { RefCell::new(Vec::new()) };
}

pub fn listen(event: &str, cb: Listener) {
    LISTENERS.with(|l| l.borrow_mut().push((event.to_string(), cb)));
}

fn emit(event: &'static str, payload: Value) {
    later(250, move || {
        let cbs: Vec<Listener> = LISTENERS.with(|l| l.borrow().iter().filter(|(e, _)| e == event).map(|(_, c)| c.clone()).collect());
        for cb in cbs {
            cb(payload.clone());
        }
    });
}

fn devices(r: &Remote) -> Vec<Value> {
    [("laptop", "Demo Laptop", "computer"), ("phone", "Demo Phone", "phone")]
        .iter()
        .map(|(id, name, kind)| json!({ "id": id, "name": name, "kind": kind, "volume": r.volume, "active": r.active.as_deref() == Some(*id) }))
        .collect()
}

fn pos(r: &Remote) -> f64 {
    r.pos + if r.playing { now_ms() - r.at } else { 0.0 }
}

fn state() -> Value {
    REMOTE.with(|r| {
        let r = r.borrow();
        let devs = devices(&r);
        let dev_name = devs.iter().find(|d| d["active"] == true).map(|d| d["name"].clone()).unwrap_or(Value::Null);
        let dur = track_list().into_iter().find(|t| Some(t["uri"].as_str().unwrap_or("")) == r.uri.as_deref()).map(|t| t["duration_ms"].clone()).unwrap_or(json!(0));
        json!({
            "devices": devs, "active_device": r.active, "active_device_name": dev_name,
            "track_uri": if r.active.is_some() { json!(r.uri) } else { Value::Null },
            "context_uri": r.context, "playing": r.playing && r.active.is_some(), "buffering": false,
            "position_ms": pos(&r).round(), "duration_ms": dur,
            "shuffle": r.shuffle, "repeat_context": r.repeat_context, "repeat_track": r.repeat_track,
            "volume": if r.active.is_some() { json!(r.volume) } else { Value::Null }, "next_uris": [],
        })
    })
}

fn emit_state() {
    emit("remote", state());
}

fn with_remote<R>(f: impl FnOnce(&mut Remote) -> R) -> R {
    REMOTE.with(|r| f(&mut r.borrow_mut()))
}

pub async fn invoke(cmd: &str, a: Value) -> Result<Value, String> {
    sleep(120).await;
    let s = |k: &str| a[k].as_str().unwrap_or("").to_string();
    Ok(match cmd {
        "init" => json!(true),
        "player_status" => json!(!has_flag("noplayer")),
        "player_login" => {
            emit("player", json!(true));
            Value::Null
        }
        "me" => json!({ "name": "cutie", "avatar": cover(4, "") }),
        "playlists" => playlists(),
        "liked" | "recent" | "playlist_tracks" => tracks(),
        "search" => matching(&s("q")),
        "lyrics" => json!([
            { "start_ms": 0, "words": "this is a demo, no real lyrics here" },
            { "start_ms": 4000, "words": "log in with Spotify to hear the music" },
            { "start_ms": 8000, "words": "pink pink pink ♡" },
            { "start_ms": 12000, "words": "la la la~" },
        ]),
        "remote_state" | "refresh_devices" => state(),
        "artist_page" => {
            let t = track_list();
            json!({
                "uri": s("uri"), "name": "pastel kid", "image": cover(5, ""), "top": (t[..5]),
                "albums": ([1, 2].iter().map(|n| json!({ "uri": format!("spotify:album:a{n}"), "name": format!("cute album vol. {n}"), "year": 2026 - n, "image": cover(*n, "") })).collect::<Vec<_>>()),
                "singles": [{ "uri": "spotify:album:s1", "name": "strawberry milk", "year": 2026, "image": cover(3, "") }],
            })
        }
        "album_page" => json!({
            "uri": s("uri"), "name": "cute album vol. 1", "artists": [{ "name": "pastel kid", "uri": "spotify:artist:demo" }],
            "year": 2025, "image": cover(1, ""), "tracks": (track_list()[..4]),
        }),
        "song_radio" => {
            let mut t = track_list();
            t.reverse();
            json!(t)
        }
        "local_spotify" => json!({ "name": "DEMO-PC", "running": false }),
        "autostart_get" => json!(true),
        "playlist_create" => json!("pl0"),
        "collection_set" => json!([]),
        "tracks_info" => {
            let want: Vec<String> = a["uris"].as_array().map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            track_list().into_iter().filter(|t| want.iter().any(|u| t["uri"] == u.as_str())).collect()
        }
        "recent_spotify" => playlists()
            .as_array()
            .map(|p| p.iter().take(6).map(|p| json!({ "uri": format!("spotify:playlist:{}", p["id"].as_str().unwrap_or("")), "kind": "playlist", "name": p["name"], "sub": "playlist", "image": p["image"] })).collect())
            .unwrap_or(json!([])),
        "search_all" => json!({
            "tracks": matching(&s("q")),
            "artists": [{ "uri": "spotify:artist:pastelkid", "kind": "artist", "name": "pastel kid", "sub": "artist", "image": cover(5, "") }],
            "albums": [{ "uri": "spotify:album:a1", "kind": "album", "name": "cute album vol. 1", "sub": "pastel kid", "image": cover(1, "") }],
        }),
        "spotify_home" => {
            let shelf = |title: &str, names: &[&str], sub: &str, id: &str, seed: usize| {
                json!({ "title": title, "items": (names.iter().enumerate().map(|(i, n)| json!({ "id": format!("{id}{i}"), "name": n, "sub": sub, "image": cover(i + seed, n) })).collect::<Vec<_>>()) })
            };
            json!([
                shelf("Made For cutie", &["Daily Mix 1", "Daily Mix 2", "Release Radar", "Discover Weekly", "Daily Mix 3"], "cute songs, mixed for you", "mix", 3),
                shelf("Your top mixes", &["Hyperpop Mix", "Pastel Mix", "Night Mix"], "Spotify", "top", 1),
            ])
        }
        "check_update" => {
            if has_flag("update") {
                json!({ "current": "0.3.0", "version": "0.4.0", "notes": "- demo update notes\n- even more pink ♡" })
            } else {
                Value::Null
            }
        }
        "play_dj" => {
            with_remote(|r| {
                r.active = a["device"].as_str().map(String::from).or(r.active.clone()).or(Some("laptop".into()));
                r.context = DJ_URI.into();
                r.uris = track_list().iter().map(|t| t["uri"].as_str().unwrap_or("").to_string()).collect();
                r.uri = Some("spotify:media:0000000000000000000000".into());
                r.playing = true;
                r.pos = 0.0;
                r.at = now_ms();
            });
            emit_state();
            later(2500, || {
                with_remote(|r| r.uri = Some("spotify:media:t3".into()));
                emit_state();
            });
            later(5000, || {
                with_remote(|r| r.uri = Some("spotify:track:t3".into()));
                emit_state();
            });
            Value::Null
        }
        "track_info" => {
            let uri = s("uri");
            track_list().into_iter().find(|t| t["uri"] == uri.as_str()).unwrap_or(Value::Null)
        }
        "play" => {
            with_remote(|r| {
                r.active = a["device"].as_str().map(String::from).or(r.active.clone()).or(Some("laptop".into()));
                r.uris = a["uris"].as_array().map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                r.context = String::new();
                r.uri = r.uris.get(a["index"].as_u64().unwrap_or(0) as usize).cloned();
                r.playing = true;
                r.pos = 0.0;
                r.at = now_ms();
            });
            emit_state();
            Value::Null
        }
        "control" => {
            let action = s("action");
            let ok = with_remote(|r| {
                if r.active.is_none() {
                    return false;
                }
                r.pos = pos(r);
                r.at = now_ms();
                match action.as_str() {
                    "pause" => r.playing = false,
                    "resume" => r.playing = true,
                    "skip_next" | "skip_prev" if !r.uris.is_empty() => {
                        let n = r.uris.len() as i64;
                        let cur = r.uri.as_ref().and_then(|u| r.uris.iter().position(|x| x == u)).map(|i| i as i64).unwrap_or(-1);
                        let i = cur + if action == "skip_next" { 1 } else { -1 };
                        r.uri = Some(r.uris[(((i % n) + n) % n) as usize].clone());
                        r.pos = 0.0;
                    }
                    _ => {}
                }
                true
            });
            if !ok {
                return Err(crate::app::NO_DEVICE.into());
            }
            emit_state();
            Value::Null
        }
        "seek" => {
            with_remote(|r| {
                r.pos = a["ms"].as_f64().unwrap_or(0.0);
                r.at = now_ms();
            });
            emit_state();
            Value::Null
        }
        "set_option" => {
            let v = a["value"].as_bool().unwrap_or(false);
            with_remote(|r| match s("name").as_str() {
                "shuffle" => r.shuffle = v,
                "repeat_context" => r.repeat_context = v,
                "repeat_track" => r.repeat_track = v,
                _ => {}
            });
            emit_state();
            Value::Null
        }
        "set_volume" | "player_volume" => {
            with_remote(|r| r.volume = a["volume"].as_f64().unwrap_or(r.volume));
            Value::Null
        }
        "transfer" => {
            with_remote(|r| r.active = a["device"].as_str().map(String::from));
            emit_state();
            Value::Null
        }
        _ => Value::Null,
    })
}
