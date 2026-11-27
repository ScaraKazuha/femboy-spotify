//! What the interface does: pages, playback (Spotify Connect), DJ X, devices,
//! volume, Liked Songs, playlists, queue, lyrics, sign-in and updates.

use serde_json::json;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::Element;

use crate::bridge::{invoke, invoke_unit, invoke_value};
use crate::icons::{hydrate, set_icon};
use crate::state::{State, View, st};
use crate::types::*;
use crate::util::*;
use crate::views::{self, DJ_ART, DJ_URI, NO_ARTIST};

pub const NO_DEVICE: &str = "no-spotify-device";

// ---------- pages ----------

pub fn render() {
    let html = st(views::page_html);
    let main = main_el();
    main.set_inner_html(&html);
    hydrate(&main);
    fit_hero_title();
    update_sidebar();
    let (back, fwd) = st(|s| (!s.back.is_empty(), !s.forward.is_empty()));
    set_style(&el("btn-back"), "opacity", if back { "1" } else { ".4" });
    set_style(&el("btn-forward"), "opacity", if fwd { "1" } else { ".4" });
}

pub fn go(view: View, push: bool) {
    st(|s| {
        if push {
            let cur = std::mem::take(&mut s.view);
            s.back.push(cur);
            s.forward.clear();
        }
        s.view = view;
    });
    render();
    main_el().set_scroll_top(0);
}

pub fn go_home() {
    let v = st(|s| s.new_view("home"));
    go(v, true);
}

pub fn go_back() {
    let moved = st(|s| match s.back.pop() {
        Some(prev) => {
            let cur = std::mem::replace(&mut s.view, prev);
            s.forward.push(cur);
            true
        }
        None => false,
    });
    if moved {
        render();
    }
}

pub fn go_forward() {
    let moved = st(|s| match s.forward.pop() {
        Some(next) => {
            let cur = std::mem::replace(&mut s.view, next);
            s.back.push(cur);
            true
        }
        None => false,
    });
    if moved {
        render();
    }
}

/// Re-draws the page if `seq` is the one on screen.
fn render_if(seq: u64) {
    if st(|s| s.view.seq == seq) {
        render();
    }
}

pub fn fail(e: &str) {
    toast(format!("oopsie: {e}"));
    let redraw = st(|s| {
        if s.view.loading {
            s.view.loading = false;
            if s.view.tracks.is_none() {
                s.view.tracks = Some(vec![]);
            }
            true
        } else {
            false
        }
    });
    if redraw {
        render();
    }
}

pub fn open_liked() {
    let (view, have) = st(|s| {
        let mut v = s.new_view("list");
        v.kind = "liked".into();
        v.title = "Liked Songs".into();
        v.sub = "your favorite songs ♡".into();
        v.tracks = s.liked.clone();
        v.loading = s.liked.is_none();
        (v, s.liked.is_some())
    });
    let seq = view.seq;
    go(view, true);
    if have {
        return;
    }
    spawn(async move {
        match invoke::<Vec<Track>>("liked", json!({})).await {
            Ok(list) => {
                st(|s| {
                    for t in &list {
                        s.saved.insert(t.id.clone());
                    }
                    s.cache_tracks(&list);
                    s.liked = Some(list.clone());
                    s.update_view(seq, |v| {
                        v.tracks = Some(list.clone());
                        v.loading = false;
                    });
                });
                render_if(seq);
            }
            Err(e) => fail(&e),
        }
    });
}

/// history of what you played in femboy spotify (kept on this computer)
fn remember(t: &Track) {
    let mut h: Vec<Track> = store_get("history", vec![]);
    h.retain(|x| x.uri != t.uri);
    h.insert(0, t.clone());
    h.truncate(100);
    store_set("history", &h);
}

pub fn open_playlist(p: Playlist) {
    let view = st(|s| {
        let mut v = s.new_view("list");
        v.kind = "playlist".into();
        v.id = p.id.clone();
        v.title = p.name.clone();
        v.sub = if p.owner.is_empty() { "playlist".into() } else { format!("by {}", p.owner) };
        v.image = p.image.clone();
        v.loading = true;
        v
    });
    let seq = view.seq;
    go(view, true);
    spawn(async move {
        match invoke::<Vec<Track>>("playlist_tracks", json!({ "id": p.id })).await {
            Ok(tracks) => {
                st(|s| {
                    s.cache_tracks(&tracks);
                    s.update_view(seq, |v| {
                        v.tracks = Some(tracks.clone());
                        v.loading = false;
                    });
                });
                render_if(seq);
            }
            Err(e) => fail(&e),
        }
    });
}

pub fn open_playlist_by_id(id: &str) {
    let p = st(|s| s.playlists.iter().find(|x| x.id == id).cloned().or_else(|| s.home_items.get(id).cloned()));
    if let Some(p) = p {
        open_playlist(p);
    }
}

pub fn do_search(q: String) {
    let q = q.trim().to_string();
    if q.chars().count() < 2 {
        return;
    }
    let (view, seq, push) = st(|s| {
        s.search_seq += 1;
        let mut v = s.new_view("search");
        v.kind = "search".into();
        v.q = q.clone();
        v.title = format!("\"{q}\"");
        v.tab = if s.view.name == "search" { s.view.tab.clone() } else { "all".into() };
        v.tracks = Some(vec![]);
        v.loading = true;
        (v, s.search_seq, s.view.name != "search")
    });
    let vseq = view.seq;
    go(view, push);
    spawn(async move {
        match invoke::<SearchResults>("search_all", json!({ "q": q })).await {
            Ok(r) => {
                let current = st(|s| {
                    s.cache_tracks(&r.tracks);
                    for c in r.artists.iter().chain(r.albums.iter()) {
                        s.cards.insert(c.uri.clone(), c.clone());
                    }
                    if s.search_seq != seq {
                        return false;
                    }
                    s.update_view(vseq, |v| {
                        v.tracks = Some(r.tracks.clone());
                        v.results = Some(r.clone());
                        v.loading = false;
                    });
                    true
                });
                if current {
                    render_if(vseq);
                }
            }
            Err(e) => fail(&e),
        }
    });
}

// Biggest font size at which the hero title fits without breaking words.
pub fn fit_hero_title() {
    let Some(title) = query(&main_el(), ".hero-title") else { return };
    let Some(boxed) = title.parent_element() else { return };
    let t: web_sys::HtmlElement = title.unchecked_into();
    let b: web_sys::HtmlElement = boxed.unchecked_into();
    let max_h = b.client_height() - 110;
    let mut size = 150;
    let _ = t.style().set_property("font-size", &format!("{size}px"));
    while size > 40 && (t.scroll_width() > b.client_width() || t.offset_height() > max_h) {
        size -= 4;
        let _ = t.style().set_property("font-size", &format!("{size}px"));
    }
}

/// Spotify's home shelves arrive from the built-in player a little after start.
pub fn set_home(shelves: Vec<Shelf>) {
    let redraw = st(|s| {
        s.home_items.clear();
        for sh in &shelves {
            for it in &sh.items {
                s.home_items.insert(
                    it.id.clone(),
                    Playlist { id: it.id.clone(), name: it.name.clone(), owner: "Spotify".into(), image: Some(it.image.clone()).filter(|i| !i.is_empty()), ..Default::default() },
                );
            }
        }
        s.home = shelves;
        s.view.name == "home"
    });
    if redraw {
        render();
    }
}

pub fn update_sidebar() {
    let (name, kind) = st(|s| (s.view.name.clone(), s.view.kind.clone()));
    let active = match name.as_str() {
        "library" => "library",
        "queue" => "queue",
        "recent" => "recent",
        _ if kind == "liked" => "liked",
        _ => "",
    };
    for b in doc_query_all(".side-btn[data-nav]") {
        toggle(&b, "active", attr(&b, "data-nav").as_deref() == Some(active));
    }
    toggle(&el("btn-home"), "on", name == "home");
    toggle(&el("btn-queue"), "active", name == "queue");
    toggle(&el("btn-lyrics"), "active", name == "lyrics");
}

pub fn render_side_thumbs() {
    let html: String = st(|s| {
        s.playlists
            .iter()
            .take(12)
            .map(|p| format!(r#"<button data-playlist="{}" title="{}"><img src="{}" alt=""></button>"#, esc(&p.id), esc(&p.name), views::img(&p.image)))
            .collect()
    });
    el("side-thumbs").set_inner_html(&html);
}

// ---------- now playing ----------

pub fn render_now() {
    let (t, saved, list) = st(|s| (s.now.clone(), s.now.as_ref().is_some_and(|t| s.saved.contains(&t.id)), s.view_tracks()));
    let placeholder = r#"<img class="cover-placeholder" src="art/icons/no-cover.png" alt="">"#;
    let big = t.as_ref().and_then(|t| t.image_large.clone().or_else(|| t.image.clone()));
    el("right-cover").set_inner_html(&match &big {
        Some(c) if !c.is_empty() => format!(r#"<img src="{}" alt="">"#, esc(c)),
        _ => placeholder.into(),
    });
    el("np-cover").set_inner_html(&match t.as_ref().and_then(|t| t.image.clone()) {
        Some(c) if !c.is_empty() => format!(r#"<img src="{}" alt="">"#, esc(&c)),
        _ => placeholder.into(),
    });
    set_text(&el("right-name"), t.as_ref().map(|t| t.name.as_str()).unwrap_or("nothing yet~"));
    el("right-artist").set_inner_html(&t.as_ref().map(views::artist_links).unwrap_or_else(|| "pick a song ♡".into()));
    set_text(&el("np-name"), t.as_ref().map(|t| t.name.as_str()).unwrap_or("—"));
    el("np-artist").set_inner_html(&t.as_ref().map(views::artist_links).unwrap_or_default());
    let has_album = t.as_ref().is_some_and(|t| t.album_uri.is_some());
    for id in ["right-name", "np-name"] {
        toggle(&el(id), "alink", has_album);
    }
    render_device_info();
    document().set_title(&match &t {
        Some(t) => format!("{} · {} ♡", t.name, t.artists),
        None => "Femboy Spotify".into(),
    });
    for id in ["right-add", "np-add"] {
        let b = el(id);
        set_icon(&b, if saved { "check-circle" } else { "plus-circle" });
        toggle(&b, "saved", saved);
    }
    render_play_state();
    render_lyrics_card();
    // highlight the playing row
    let uri = t.map(|t| t.uri);
    for row in query_all(&main_el(), ".trow[data-index]") {
        let i: usize = attr(&row, "data-index").and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
        let playing = uri.is_some() && list.get(i).map(|x| Some(&x.uri) == uri.as_ref()).unwrap_or(false);
        toggle(&row, "playing", playing);
        if let Some(n) = query(&row, ".tnum .n") {
            let num = attr(&row, "data-n").unwrap_or_default();
            set_text(&n, if playing { "♪" } else { &num });
        }
    }
}

pub fn render_play_state() {
    let (playing, buffering, dj, shuffle, repeat, muted, volume) =
        st(|s| (s.playing, s.buffering, s.dj_on, s.shuffle, s.repeat.clone(), s.muted, s.volume));
    set_icon(&el("btn-play"), if playing { "pause" } else { "play" });
    let panel = el("btn-panel");
    toggle(&panel, "vibing", playing && !buffering);
    toggle(&panel, "dj", dj);
    toggle(&el("btn-shuffle"), "active", shuffle);
    set_icon(&el("btn-repeat"), if repeat == "one" { "repeat-one" } else { "repeat" });
    toggle(&el("btn-repeat"), "active", repeat != "off");
    set_icon(&el("btn-mute"), if muted || volume == 0.0 { "volume-off" } else { "volume" });
}

pub fn tick() {
    let (pos, dur, drag) = st(|s| (s.position(), s.duration(), s.seek_drag));
    let pct = if dur > 0.0 { pos / dur * 100.0 } else { 0.0 };
    if !drag {
        set_style(&el("seek-fill"), "width", &format!("{pct}%"));
        set_style(&el("seek-knob"), "left", &format!("{pct}%"));
    }
    set_text(&el("time-pos"), &fmt(pos));
    set_text(&el("time-left"), &format!("-{}", fmt(dur - pos)));
    sync_lyrics(pos);
}

// ---------- playback (Spotify Connect) ----------
// Music plays in the built-in player or the Spotify app on a device; we send
// commands and mirror the state Spotify pushes back ("remote" events).

/// which Spotify context a list page corresponds to
pub fn context_of(v: &View) -> Option<String> {
    match v.kind.as_str() {
        "liked" => Some("liked".into()),
        "playlist" => Some(format!("spotify:playlist:{}", v.id)),
        "album" => Some(v.uri.clone()),
        // search results, history, queue: send the list itself
        _ => None,
    }
}

/// the app's own built-in player (Spotify's web player in a hidden window)
pub fn is_builtin(d: &RemoteDevice) -> bool {
    d.name.to_lowercase().starts_with("web player")
}

pub fn device_name(d: &RemoteDevice) -> String {
    if is_builtin(d) { "femboy spotify (this app) ♡".into() } else { d.name.clone() }
}

// Where to start music. Keep going on a device that is playing right now; otherwise
// use our own player. Every app start makes a new web-player device and Spotify can
// keep the previous (dead) one marked active for a while, so an idle "active"
// device is not trusted and a live built-in one wins over it.
fn pick_device(s: &State) -> Option<String> {
    let active = s.devices.iter().find(|d| Some(&d.id) == s.active_device.as_ref());
    if active.is_some() && s.playing {
        return None;
    }
    let builtins: Vec<&RemoteDevice> = s.devices.iter().filter(|d| is_builtin(d)).collect();
    if let Some(b) = builtins.iter().find(|d| Some(&d.id) != s.active_device.as_ref()).or_else(|| builtins.first()) {
        return Some(b.id.clone());
    }
    if active.is_some() {
        return None;
    }
    s.devices.iter().find(|d| d.kind == "computer").or_else(|| s.devices.first()).map(|d| d.id.clone())
}

/// pick_device, but first makes sure our own player is in the device list: Spotify
/// only pushes the list when playback changes, so right after start it can be missing.
async fn choose_device() -> Option<String> {
    for i in 0..4 {
        let need = st(|s| s.player_ready && !s.devices.iter().any(is_builtin));
        if !need {
            break;
        }
        if i > 0 {
            sleep(1500).await;
        }
        match invoke::<RemoteState>("refresh_devices", json!({})).await {
            Ok(r) => st(|s| {
                s.devices = r.devices;
                s.active_device = r.active_device;
            }),
            Err(_) => break,
        }
    }
    st(|s| pick_device(s))
}

pub fn handle_remote_error(e: &str) {
    if e.contains(NO_DEVICE) {
        show_no_device();
    } else {
        toast(format!("oopsie: {e}"));
    }
}

pub fn play_from(tracks: Vec<Track>, i: usize, label: &str, ctx: Option<String>) {
    st(|s| {
        s.cache_tracks(&tracks);
        s.queue = tracks;
        s.context = label.to_string();
        s.ctx = ctx;
    });
    spawn(play_index(i));
}

pub async fn play_index(i: usize) {
    let Some(t) = st(|s| s.queue.get(i).cloned()) else { return };
    // optimistic: show it right away, Spotify confirms a moment later
    st(|s| {
        s.index = i as i64;
        s.playing = true;
        s.buffering = true;
        s.duration_ms = t.duration_ms;
        s.set_pos(0.0);
    });
    set_now(t);
    let device = choose_device().await;
    let (ctx, uris) = st(|s| (s.ctx.clone(), s.queue.iter().map(|x| x.uri.clone()).collect::<Vec<_>>()));
    if let Err(e) = invoke_unit("play", json!({ "context": ctx, "uris": uris, "index": i, "device": device })).await {
        st(|s| {
            s.playing = false;
            s.buffering = false;
        });
        render_play_state();
        handle_remote_error(&e);
    }
}

pub fn control(action: &'static str) {
    spawn(async move {
        if let Err(e) = invoke_unit("control", json!({ "action": action })).await {
            handle_remote_error(&e);
        }
    });
}

pub fn next() {
    if st(|s| s.now.is_some()) {
        control("skip_next");
    }
}

pub fn prev() {
    let (has, pos) = st(|s| (s.now.is_some(), s.position()));
    if !has {
        return;
    }
    if pos > 3000.0 {
        seek_to(0.0);
    } else {
        control("skip_prev");
    }
}

pub fn toggle_play() {
    let (has_now, tracks, title, ctx, liked) = st(|s| (s.now.is_some(), s.view.tracks.clone(), s.view.title.clone(), context_of(&s.view), s.liked.clone()));
    if !has_now {
        if let Some(t) = tracks.filter(|t| !t.is_empty()) {
            play_from(t, 0, &title, ctx);
        } else if let Some(l) = liked.filter(|l| !l.is_empty()) {
            play_from(l, 0, "Liked Songs", Some("liked".into()));
        }
        return;
    }
    // the active device is a web player from an earlier app start (ours is the other one)
    let (stale, active, playing, index, dj) = st(|s| {
        let stale = s.devices.iter().any(|d| Some(&d.id) == s.active_device.as_ref() && is_builtin(d))
            && s.devices.iter().filter(|d| is_builtin(d)).count() > 1;
        (stale, s.active_device.is_some(), s.playing, s.index, s.dj_on)
    });
    if !active || (stale && !playing) {
        // nothing (alive) active any more: start the song again on a device
        if index >= 0 {
            spawn(play_index(index as usize));
        } else if dj {
            play_dj();
        } else {
            show_no_device();
        }
        return;
    }
    if playing {
        st(|s| {
            let p = s.position();
            s.set_pos(p);
            s.playing = false;
        });
        render_play_state();
        control("pause");
    } else {
        st(|s| {
            s.playing = true;
            let p = s.pos;
            s.set_pos(p);
        });
        render_play_state();
        control("resume");
    }
}

pub fn seek_to(ms: f64) {
    st(|s| s.set_pos(ms));
    spawn(async move {
        if let Err(e) = invoke_unit("seek", json!({ "ms": ms.round() as u64 })).await {
            handle_remote_error(&e);
        }
    });
    tick();
}

pub fn draw_volume(v: f64) {
    set_style(&el("vol-fill"), "width", &format!("{v}%"));
    set_style(&el("vol-knob"), "left", &format!("{v}%"));
}

// Our own player is turned up/down right on its page (instant); other devices get a
// Spotify Connect command (goes through Spotify, so it is sent less often).
fn on_builtin(s: &State) -> bool {
    let d = s.devices.iter().find(|x| Some(&x.id) == s.active_device.as_ref()).or_else(|| {
        if s.active_device.is_none() { s.devices.iter().find(|d| is_builtin(d)) } else { None }
    });
    d.is_some_and(is_builtin)
}

pub fn send_volume(v: f64) {
    let builtin = st(|s| {
        s.vol_sent_at = now_ms();
        on_builtin(s)
    });
    let v = v.round() as u16;
    spawn(async move {
        // (the player reports its new level to Spotify by itself, so other apps stay in step)
        let cmd = if builtin { "player_volume" } else { "set_volume" };
        let _ = invoke_unit(cmd, json!({ "volume": v })).await;
    });
}

/// `live`: called while dragging; sends at most every 40 ms (own player) / 200 ms (others)
pub fn set_volume(v: f64, live: bool) {
    let (vol, send) = st(|s| {
        s.volume = v.round().clamp(0.0, 100.0);
        s.muted = false;
        s.vol_touched = now_ms();
        let gap = if on_builtin(s) { 40.0 } else { 200.0 };
        (s.volume, !live || now_ms() - s.vol_sent_at >= gap)
    });
    store_set("volume", &vol);
    draw_volume(vol);
    render_play_state();
    if send {
        send_volume(vol);
    }
}

pub fn set_option(name: &'static str, value: bool) {
    spawn(async move {
        if let Err(e) = invoke_unit("set_option", json!({ "name": name, "value": value })).await {
            handle_remote_error(&e);
        }
    });
}

/// the currently playing track changed (by us or on another device)
pub fn set_now(t: Track) {
    let changed = st(|s| {
        let changed = s.now.as_ref().map(|n| n.uri != t.uri).unwrap_or(true);
        s.now = Some(t.clone());
        s.index = s.queue.iter().position(|x| x.uri == t.uri).map(|i| i as i64).unwrap_or(-1);
        if changed {
            s.lyrics = None;
            s.lyrics_for = None;
        }
        changed
    });
    if changed {
        check_sleep_on_track_change(&t.uri);
        if t.uri.starts_with("spotify:track:") {
            remember(&t);
        }
        if !t.id.is_empty() {
            spawn(load_lyrics(t.clone()));
        } else {
            st(|s| s.lyrics_for = Some(t.id.clone()));
            render_lyrics_card();
        }
    }
    render_now();
    let name = st(|s| s.view.name.clone());
    if changed && (name == "lyrics" || name == "queue") {
        render();
    }
}

// ---------- DJ X (Spotify DJ) ----------

fn dj_card(uri: &str, name: &str, artists: &str, duration: f64) -> Track {
    Track {
        uri: uri.into(),
        name: name.into(),
        artists: artists.into(),
        duration_ms: duration,
        image: Some(DJ_ART.into()),
        image_large: Some(DJ_ART.into()),
        ..Default::default()
    }
}

// DJ X's voice only plays in the official Spotify app, so DJ X goes there when that
// app is open on this computer (Femboy Spotify still drives it); otherwise our player.
async fn dj_device() -> (Option<String>, bool) {
    if let Ok(here) = invoke::<LocalSpotify>("local_spotify", json!({})).await {
        if here.running && !here.name.is_empty() {
            if let Ok(r) = invoke::<RemoteState>("refresh_devices", json!({})).await {
                st(|s| {
                    s.devices = r.devices;
                    s.active_device = r.active_device;
                });
            }
            let app = st(|s| {
                s.devices.iter().find(|d| !is_builtin(d) && d.name.to_lowercase() == here.name.to_lowercase()).map(|d| d.id.clone())
            });
            if let Some(id) = app {
                return (Some(id), true);
            }
        }
    }
    (choose_device().await, false)
}

pub fn play_dj() {
    st(|s| {
        s.queue.clear();
        s.index = -1;
        s.ctx = None;
        s.context = "DJ X".into();
        s.dj_on = true;
        s.playing = true;
        s.buffering = true;
        s.set_pos(0.0);
    });
    set_now(dj_card("dj:start", "DJ X is warming up ♪", "Spotify DJ", 0.0));
    spawn(async {
        let (id, voice) = dj_device().await;
        toast(if voice { "DJ X is on in your Spotify app, with his voice ♪" } else { "DJ X is on the way ♪" });
        let hint = st(|s| {
            let first = !voice && !s.dj_voice_hint;
            if first {
                s.dj_voice_hint = true;
            }
            first
        });
        if hint {
            later(3600, || toast("DJ X only talks in the Spotify app: open it to hear his voice ♡"));
        }
        if let Err(e) = invoke_unit("play_dj", json!({ "device": id })).await {
            st(|s| {
                s.playing = false;
                s.dj_on = false;
            });
            render_play_state();
            handle_remote_error(&e);
        }
    });
}

// DJ X talking: "spotify:media:000…" = DJ speaking, "spotify:media:<track id>" = DJ introducing that track
fn on_dj_media(uri: String) {
    let id = uri.rsplit(':').next().unwrap_or("").to_string();
    let dur = st(|s| s.duration_ms);
    if id.chars().all(|c| c == '0') {
        set_now(dj_card(&uri, "DJ X is talking ♪", "Spotify DJ", dur));
        return;
    }
    set_now(dj_card(&uri, "DJ X ♪ introducing...", "Spotify DJ", dur));
    spawn(async move {
        let track_uri = format!("spotify:track:{id}");
        let info = match st(|s| s.cache.get(&track_uri).cloned()) {
            Some(t) => Some(t),
            None => invoke::<Option<Track>>("track_info", json!({ "uri": track_uri })).await.ok().flatten(),
        };
        if let Some(info) = info {
            let ok = st(|s| {
                if s.now.as_ref().map(|n| n.uri == uri).unwrap_or(false) {
                    s.now = Some(Track {
                        id: String::new(),
                        uri: uri.clone(),
                        name: format!("DJ X ♪ {}", info.name),
                        artists: format!("introducing · {}", info.artists),
                        ..info.clone()
                    });
                    true
                } else {
                    false
                }
            });
            if ok {
                render_now();
            }
        }
    });
}

fn on_remote_track(uri: String) {
    if uri.starts_with("spotify:media:") {
        return on_dj_media(uri);
    }
    let known = st(|s| s.cache.get(&uri).cloned().or_else(|| s.queue.iter().find(|x| x.uri == uri).cloned()));
    if let Some(t) = known {
        return set_now(t);
    }
    let dur = st(|s| s.duration_ms);
    set_now(Track {
        uri: uri.clone(),
        id: uri.rsplit(':').next().unwrap_or("").into(),
        name: "...".into(),
        duration_ms: dur,
        ..Default::default()
    });
    spawn(async move {
        if let Ok(Some(info)) = invoke::<Option<Track>>("track_info", json!({ "uri": uri })).await {
            let ok = st(|s| {
                s.cache.insert(uri.clone(), info.clone());
                if s.now.as_ref().map(|n| n.uri == uri).unwrap_or(false) {
                    s.now = Some(info.clone());
                    true
                } else {
                    false
                }
            });
            if ok {
                render_now();
            }
        }
    });
}

pub fn apply_remote(r: RemoteState) {
    let (queue_changed, on_queue, new_track) = st(|s| {
        s.devices = r.devices.clone();
        // the queue screen follows what Spotify says comes next
        let key = format!("{:?}|{:?}", r.queue_uris, r.next_uris);
        s.queue_uris = r.queue_uris.clone();
        s.next_uris = r.next_uris.clone();
        let queue_changed = key != s.queue_key;
        s.queue_key = key;
        s.dj_on = r.active_device.is_some() && r.context_uri == DJ_URI;
        s.active_device = r.active_device.clone();
        let active = s.devices.iter().find(|d| Some(&d.id) == r.active_device.as_ref());
        s.active_device_name = active.map(device_name).or_else(|| r.active_device_name.clone());
        s.playing = r.playing;
        s.buffering = r.playing && r.buffering;
        if r.duration_ms > 0.0 {
            s.duration_ms = r.duration_ms;
        }
        s.set_pos(r.position_ms);
        s.shuffle = r.shuffle;
        s.repeat = if r.repeat_track { "one" } else if r.repeat_context { "all" } else { "off" }.into();
        let new_track = r.track_uri.clone().filter(|u| s.now.as_ref().map(|n| &n.uri != u).unwrap_or(true));
        (queue_changed, s.view.name == "queue", new_track)
    });
    if queue_changed && on_queue {
        spawn(refresh_queue());
    }
    if let Some(v) = r.volume {
        let take = st(|s| !s.muted && now_ms() - s.vol_touched > 1500.0) && !has_class(&el("vol-bar"), "drag");
        if take {
            st(|s| s.volume = v);
            draw_volume(v);
        }
    }
    if let Some(u) = new_track {
        on_remote_track(u);
    }
    render_device_info();
    render_play_state();
    if !has_class(&el("devices-pop"), "hidden") {
        render_devices();
    }
}

// ---------- devices ----------

fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "speaker" | "car" => "volume",
        "tv" | "cast" | "console" => "mini",
        "watch" => "disc",
        _ => "devices",
    }
}

pub fn render_device_info() {
    let (name, dj, ctx) = st(|s| (s.active_device_name.clone(), s.dj_on, s.context.clone()));
    let what = if dj { "DJ X" } else { "Now playing" };
    let title = match &name {
        Some(n) => format!("{what} • on {n}"),
        None if !ctx.is_empty() => format!("{what} • {ctx}"),
        None => what.into(),
    };
    set_text(&el("right-title"), &title);
    let b = el("btn-devices");
    toggle(&b, "active", name.is_some());
    let _ = b.set_attribute("title", &name.map(|n| format!("playing on {n}")).unwrap_or_else(|| "devices".into()));
}

pub fn render_devices() {
    let html = st(|s| {
        if s.devices.is_empty() {
            return format!(
                r#"<p class="dev-empty">the built-in player is waking up (｡•́︿•̀｡)<br>{}</p>"#,
                if s.player_ready { "give it a few seconds" } else { r#"<button class="chip" data-player-login>sign in to it</button>"# }
            );
        }
        // our own player on top
        let mut list: Vec<&RemoteDevice> = s.devices.iter().filter(|d| is_builtin(d)).collect();
        list.extend(s.devices.iter().filter(|d| !is_builtin(d)));
        list.iter()
            .map(|d| {
                format!(
                    r#"<button class="dev {}" data-device="{}">{}<span class="dev-name">{}</span><span class="dev-meta">{}</span></button>"#,
                    if d.active { "active" } else { "" },
                    esc(&d.id),
                    crate::icons::icon(if is_builtin(d) { "heart" } else { kind_icon(&d.kind) }),
                    esc(&device_name(d)),
                    if d.active { (if s.playing { "playing ♪" } else { "connected" }).to_string() } else { esc(&d.kind) }
                )
            })
            .collect()
    });
    el("devices-list").set_inner_html(&html);
}

pub fn toggle_devices(force: Option<bool>) {
    let pop = el("devices-pop");
    let show = force.unwrap_or_else(|| has_class(&pop, "hidden"));
    toggle(&pop, "hidden", !show);
    if show {
        render_devices();
    }
}

pub fn pick_device_from_list(id: String) {
    let (active, index) = st(|s| (s.active_device.clone(), s.index));
    if active.as_deref() == Some(id.as_str()) {
        return toggle_devices(Some(false));
    }
    toggle_devices(Some(false));
    spawn(async move {
        let res = if active.is_some() {
            toast("moving the music ♡");
            invoke_unit("transfer", json!({ "device": id })).await
        } else if index >= 0 {
            let (ctx, uris) = st(|s| (s.ctx.clone(), s.queue.iter().map(|x| x.uri.clone()).collect::<Vec<_>>()));
            invoke_unit("play", json!({ "context": ctx, "uris": uris, "index": index, "device": id })).await
        } else {
            invoke_unit("transfer", json!({ "device": id })).await
        };
        if let Err(e) = res {
            handle_remote_error(&e);
        }
    });
}

// ---------- built-in player ----------
// Spotify's web player runs hidden inside the app and plays the music itself.

pub fn show_no_device() {
    if !st(|s| s.player_ready) {
        return show_player_setup();
    }
    // signed in, but the player hasn't shown up in Spotify Connect yet
    toast("the built-in player is waking up, try again in a few seconds ♡");
}

fn show_player_setup() {
    remove_class(&el("nodevice"), "hidden");
}

pub fn hide_player_setup() {
    add_class(&el("nodevice"), "hidden");
}

pub fn player_login() {
    hide_player_setup();
    spawn(async {
        match invoke_unit("player_login", json!({})).await {
            Ok(()) => toast("sign in to Spotify in the little window ♡"),
            Err(e) => toast(format!("oopsie: {e}")),
        }
    });
}

pub fn on_player_ready() {
    st(|s| s.player_ready = true);
    hide_player_setup();
    toast("the built-in player is ready, music plays right here now ♡");
}

async fn check_player() {
    match invoke::<bool>("player_status", json!({})).await {
        Ok(ready) => {
            st(|s| s.player_ready = ready);
            if !ready {
                show_player_setup();
            }
        }
        Err(e) => toast(format!("built-in player: {e}")),
    }
}

// ---------- Liked Songs ----------

pub fn save_current() {
    if let Some(t) = st(|s| s.now.clone()) {
        toggle_saved(t, None);
    }
}

/// Save to / remove from Liked Songs. The heart updates at once; Spotify confirms after.
pub fn toggle_saved(t: Track, want: Option<bool>) {
    if t.id.is_empty() || !t.uri.starts_with("spotify:track:") {
        return;
    }
    let save = want.unwrap_or_else(|| !st(|s| s.saved.contains(&t.id)));
    apply_saved(&t, save);
    spawn(async move {
        match invoke_unit("set_saved", json!({ "id": t.id, "saved": save })).await {
            Ok(()) => toast(if save { "added to Liked Songs ♡" } else { "removed from Liked Songs" }),
            Err(e) => {
                apply_saved(&t, !save);
                toast(format!("couldn't update Liked Songs: {e}"));
            }
        }
    });
}

fn apply_saved(t: &Track, on: bool) {
    let on_liked = st(|s| {
        if on {
            s.saved.insert(t.id.clone());
            if let Some(l) = s.liked.as_mut() {
                if !l.iter().any(|x| x.id == t.id) {
                    l.insert(0, t.clone());
                }
            }
        } else {
            s.saved.remove(&t.id);
            if let Some(l) = s.liked.as_mut() {
                l.retain(|x| x.id != t.id);
            }
        }
        if s.view.kind == "liked" {
            s.view.tracks = s.liked.clone();
            true
        } else {
            false
        }
    });
    if on_liked {
        render();
    } else {
        refresh_saved_marks();
    }
    render_now();
}

/// swaps the hearts on the rows on screen (with a little pop)
fn refresh_saved_marks() {
    let (list, saved) = st(|s| (s.view_tracks(), s.saved.clone()));
    for b in query_all(&main_el(), ".tlike[data-like]") {
        let i: usize = attr(&b, "data-like").and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
        let Some(t) = list.get(i) else { continue };
        let on = saved.contains(&t.id);
        if has_class(&b, "on") == on {
            continue;
        }
        toggle(&b, "on", on);
        let _ = b.set_attribute("data-tip", if on { "remove from Liked Songs" } else { "save to Liked Songs" });
        if let Some(im) = query(&b, "img") {
            let _ = im.set_attribute("src", &format!("art/icons/{}.png", if on { "liked" } else { "add" }));
            remove_class(&im, "pop");
            let _ = im.unchecked_ref::<web_sys::HtmlElement>().offset_width();
            add_class(&im, "pop");
        }
    }
}

// ---------- rows ----------

/// play row i of what's on screen (sorted / filtered lists send the list itself)
pub fn play_row(i: usize) {
    let (v, list) = st(|s| (s.view.clone(), s.view_tracks()));
    let Some(t) = list.get(i).cloned() else { return };
    if v.name == "queue" {
        let label = st(|s| if s.context.is_empty() { "queue".to_string() } else { s.context.clone() });
        return play_from(list, i, &label, None);
    }
    // like Spotify: a song picked from search results or history is followed by
    // songs like it (Spotify's song radio), not by the other, unrelated results
    if v.kind == "search" || v.name == "recent" {
        return play_with_radio(t);
    }
    let ctx = if views::is_reordered(&v) { None } else { context_of(&v) };
    play_from(list, i, &v.title, ctx);
}

fn play_with_radio(t: Track) {
    let seq = st(|s| {
        s.radio_seq += 1;
        s.queue = vec![t.clone()];
        s.index = 0;
        s.radio_seq
    });
    // show it right away; the similar songs arrive a moment later
    set_now(t.clone());
    st(|s| {
        s.playing = true;
        s.buffering = true;
    });
    render_play_state();
    spawn(async move {
        let similar: Vec<Track> = invoke::<Vec<Track>>("song_radio", json!({ "id": t.id }))
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|x| x.uri != t.uri)
            .collect();
        if st(|s| s.radio_seq != seq) {
            return; // something else was picked meanwhile
        }
        let mut list = vec![t.clone()];
        list.extend(similar);
        play_from(list, 0, &format!("{} radio", t.name), None);
    });
}

pub fn select_row(i: usize) {
    let main = main_el();
    for r in query_all(&main, ".trow.selected") {
        remove_class(&r, "selected");
    }
    if let Some(r) = query(&main, &format!(".trow[data-index=\"{i}\"]")) {
        add_class(&r, "selected");
    }
}

/// re-draws only the rows (keeps the search box focused while typing)
pub fn refresh_rows() {
    let main = main_el();
    let Some(boxed) = query(&main, ".tracks") else { return render() };
    let html = st(|s| {
        if s.view.tracks.is_none() {
            return None;
        }
        let shown = views::shown_tracks(&s.view);
        let html = views::rows_html(s, &shown);
        s.view.shown = Some(shown);
        Some(html)
    });
    let Some(html) = html else { return render() };
    boxed.set_outer_html(&html);
    hydrate(&main);
}

// ---------- artist, album, radio, cards ----------

pub fn open_artist(uri: String) {
    if uri.is_empty() || st(|s| s.view.name == "artist" && s.view.uri == uri) {
        return;
    }
    let view = st(|s| {
        let mut v = s.new_view("artist");
        v.uri = uri.clone();
        v.loading = true;
        v.tracks = Some(vec![]);
        v
    });
    let seq = view.seq;
    go(view, true);
    spawn(async move {
        match invoke::<ArtistPage>("artist_page", json!({ "uri": uri })).await {
            Ok(a) => {
                st(|s| {
                    s.cache_tracks(&a.top);
                    s.update_view(seq, |v| {
                        v.image = a.image.clone();
                        v.top = a.top.clone();
                        v.albums = a.albums.clone();
                        v.singles = a.singles.clone();
                        v.loading = false;
                        v.title = a.name.clone();
                        v.tracks = Some(a.top.clone());
                    });
                });
                render_if(seq);
            }
            Err(e) => fail(&e),
        }
    });
}

pub fn open_album(uri: String) {
    if uri.is_empty() || st(|s| s.view.kind == "album" && s.view.uri == uri) {
        return;
    }
    let view = st(|s| {
        let mut v = s.new_view("list");
        v.kind = "album".into();
        v.uri = uri.clone();
        v.loading = true;
        v
    });
    let seq = view.seq;
    go(view, true);
    spawn(async move {
        match invoke::<AlbumPage>("album_page", json!({ "uri": uri })).await {
            Ok(al) => {
                let links = al
                    .artists
                    .iter()
                    .map(|a| format!(r#"<span class="alink" data-artist="{}">{}</span>"#, esc(&a.uri), esc(&a.name)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let sub_html = format!("{links}{}", if al.year > 0 { format!(" · {}", al.year) } else { String::new() });
                st(|s| {
                    s.cache_tracks(&al.tracks);
                    s.update_view(seq, |v| {
                        v.title = al.name.clone();
                        v.image = al.image.clone();
                        v.sub_html = Some(sub_html.clone());
                        v.sub = al.artists.iter().map(|a| a.name.clone()).collect::<Vec<_>>().join(", ");
                        v.tracks = Some(al.tracks.clone());
                        v.loading = false;
                    });
                });
                render_if(seq);
            }
            Err(e) => fail(&e),
        }
    });
}

pub fn open_radio(t: Track) {
    let view = st(|s| {
        let mut v = s.new_view("list");
        v.kind = "radio".into();
        v.title = format!("{} Radio", t.name);
        v.sub = format!("songs like {} by {}", t.name, t.artists);
        v.image = t.image_large.clone().or_else(|| t.image.clone());
        v.loading = true;
        v
    });
    let seq = view.seq;
    go(view, true);
    spawn(async move {
        match invoke::<Vec<Track>>("song_radio", json!({ "id": t.id })).await {
            Ok(tracks) => {
                st(|s| {
                    s.cache_tracks(&tracks);
                    s.update_view(seq, |v| {
                        v.tracks = Some(tracks.clone());
                        v.loading = false;
                    });
                });
                render_if(seq);
            }
            Err(e) => fail(&e),
        }
    });
}

pub fn open_card(uri: String) {
    if uri.starts_with("spotify:artist:") {
        return open_artist(uri);
    }
    if uri.starts_with("spotify:album:") {
        return open_album(uri);
    }
    if uri.contains(":collection") {
        return open_liked();
    }
    if let Some(id) = uri.strip_prefix("spotify:playlist:") {
        let p = st(|s| {
            s.playlists.iter().find(|p| p.id == id).cloned().or_else(|| s.home_items.get(id).cloned()).unwrap_or_else(|| {
                let c = s.cards.get(&uri);
                Playlist {
                    id: id.into(),
                    name: c.map(|c| c.name.clone()).unwrap_or_else(|| "playlist".into()),
                    owner: c.map(|c| c.sub.clone()).unwrap_or_default(),
                    image: c.and_then(|c| c.image.clone()),
                    ..Default::default()
                }
            })
        });
        open_playlist(p);
    }
}

// ---------- recently played, queue ----------

pub fn open_recent() {
    let history: Vec<Track> = store_get("history", vec![]);
    let view = st(|s| {
        let mut v = s.new_view("recent");
        v.title = "Recently played".into();
        v.cards = s.recent_cards.clone();
        v.tracks = Some(history);
        v
    });
    let seq = view.seq;
    go(view, true);
    spawn(async move {
        match invoke::<Vec<Card>>("recent_spotify", json!({})).await {
            Ok(cards) => st(|s| {
                for c in &cards {
                    s.cards.insert(c.uri.clone(), c.clone());
                }
                s.recent_cards = Some(cards.clone());
                s.update_view(seq, |v| v.cards = Some(cards.clone()));
            }),
            Err(e) => st(|s| {
                s.update_view(seq, |v| {
                    v.cards = Some(vec![]);
                    v.error = Some(e.clone());
                })
            }),
        }
        render_if(seq);
    });
}

pub fn open_queue() {
    let view = st(|s| {
        let mut v = s.new_view("queue");
        v.title = "Queue".into();
        v.tracks = Some(vec![]);
        v
    });
    go(view, true);
    spawn(refresh_queue());
}

/// fetches info for queued songs we haven't seen yet, then draws
async fn refresh_queue() {
    let (seq, missing) = st(|s| {
        let missing: Vec<String> = s.queue_uris.iter().chain(s.next_uris.iter()).filter(|u| !s.cache.contains_key(*u)).cloned().collect();
        (s.view.seq, missing)
    });
    if st(|s| s.view.name != "queue") {
        return;
    }
    if !missing.is_empty() {
        if let Ok(list) = invoke::<Vec<Track>>("tracks_info", json!({ "uris": missing })).await {
            st(|s| s.cache_tracks(&list));
        }
    }
    render_if(seq);
}

pub fn clear_queue() {
    spawn(async {
        match invoke_unit("clear_queue", json!({})).await {
            Ok(()) => {
                st(|s| s.queue_uris.clear());
                render();
                toast("queue cleared");
            }
            Err(e) => handle_remote_error(&e),
        }
    });
}

// ---------- follow artists, save albums ----------

async fn load_follows() {
    let artists = invoke::<Vec<String>>("collection_set", json!({ "set": "artist" })).await;
    let coll = invoke::<Vec<String>>("collection_set", json!({ "set": "collection" })).await;
    st(|s| {
        if let Ok(a) = artists {
            s.followed = a.into_iter().collect();
        }
        if let Ok(c) = coll {
            s.saved_albums = c.into_iter().filter(|u| u.starts_with("spotify:album:")).collect();
        }
    });
}

pub fn toggle_follow(uri: String) {
    if uri.is_empty() {
        return;
    }
    let artist = uri.starts_with("spotify:artist:");
    let on = !st(|s| if artist { s.followed.contains(&uri) } else { s.saved_albums.contains(&uri) });
    let apply = move |uri: &str, v: bool| {
        st(|s| {
            let set = if artist { &mut s.followed } else { &mut s.saved_albums };
            if v {
                set.insert(uri.to_string());
            } else {
                set.remove(uri);
            }
        });
        render();
    };
    apply(&uri, on);
    spawn(async move {
        match invoke_unit("set_followed", json!({ "uri": uri, "on": on })).await {
            Ok(()) => toast(match (artist, on) {
                (true, true) => "following ♡",
                (true, false) => "unfollowed",
                (false, true) => "saved to Your Library ♡",
                (false, false) => "removed from Your Library",
            }),
            Err(e) => {
                apply(&uri, !on);
                toast(format!("couldn't update: {e}"));
            }
        }
    });
}

// ---------- your own playlists ----------

/// small pink dialog asking for a name (or a yes/no when `confirm_only`)
pub async fn ask_name(title: &str, value: &str, ok: &str, confirm_only: bool, text: &str) -> Option<String> {
    let (tx, rx) = futures_channel::oneshot::channel::<Option<String>>();
    let tx = std::rc::Rc::new(std::cell::RefCell::new(Some(tx)));
    let boxed = el("askbox");
    set_text(&el("ask-title"), title);
    set_text(&el("ask-text"), text);
    toggle(&el("ask-text"), "hidden", text.is_empty());
    let inp = input("ask-input");
    toggle(&inp, "hidden", confirm_only);
    inp.set_value(value);
    set_text(&el("ask-ok"), ok);
    remove_class(&boxed, "hidden");
    if !confirm_only {
        let i = inp.clone();
        later(30, move || {
            let _ = i.focus();
            i.select();
        });
    }
    let done = {
        let tx = tx.clone();
        move |result: Option<String>| {
            add_class(&el("askbox"), "hidden");
            html_el("ask-ok").set_onclick(None);
            html_el("ask-cancel").set_onclick(None);
            input("ask-input").set_onkeydown(None);
            if let Some(tx) = tx.borrow_mut().take() {
                let _ = tx.send(result);
            }
        }
    };
    let ok_cb = {
        let done = done.clone();
        Closure::<dyn Fn()>::new(move || {
            let v = input("ask-input").value().trim().to_string();
            done(if confirm_only { Some(String::new()) } else if v.is_empty() { None } else { Some(v) });
        })
    };
    let cancel_cb = {
        let done = done.clone();
        Closure::<dyn Fn()>::new(move || done(None))
    };
    let key_cb = {
        let done = done.clone();
        Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            if e.key() == "Enter" {
                html_el("ask-ok").click();
            }
            if e.key() == "Escape" {
                done(None);
            }
        })
    };
    html_el("ask-ok").set_onclick(Some(ok_cb.as_ref().unchecked_ref()));
    html_el("ask-cancel").set_onclick(Some(cancel_cb.as_ref().unchecked_ref()));
    inp.set_onkeydown(Some(key_cb.as_ref().unchecked_ref()));
    let result = rx.await.unwrap_or(None);
    drop((ok_cb, cancel_cb, key_cb));
    result
}

async fn reload_playlists() {
    if let Ok(p) = invoke::<Vec<Playlist>>("playlists", json!({})).await {
        st(|s| s.playlists = p);
        render_side_thumbs();
    }
}

pub fn create_playlist() {
    spawn(async {
        let n = st(|s| s.playlists.iter().filter(|p| p.owner == "you").count() + 1);
        let Some(name) = ask_name("new playlist ♡", &format!("My Playlist #{n}"), "create", false, "").await else { return };
        match invoke::<String>("playlist_create", json!({ "name": name })).await {
            Ok(id) => {
                toast(format!("made {name} ♡"));
                reload_playlists().await;
                let p = st(|s| s.playlists.iter().find(|p| p.id == id).cloned())
                    .unwrap_or(Playlist { id, name, owner: "you".into(), editable: true, ..Default::default() });
                open_playlist(p);
            }
            Err(e) => toast(format!("couldn't make the playlist: {e}")),
        }
    });
}

pub fn rename_playlist(p: Playlist) {
    spawn(async move {
        let Some(name) = ask_name("rename playlist", &p.name, "rename", false, "").await else { return };
        if name == p.name {
            return;
        }
        match invoke_unit("playlist_rename", json!({ "playlist": p.id, "name": name })).await {
            Ok(()) => {
                st(|s| {
                    if s.view.kind == "playlist" && s.view.id == p.id {
                        s.view.title = name.clone();
                    }
                });
                reload_playlists().await;
                render();
                toast("renamed ♡");
            }
            Err(e) => toast(format!("couldn't rename: {e}")),
        }
    });
}

pub fn remove_playlist(p: Playlist) {
    spawn(async move {
        let own = p.owner == "you";
        let ok = ask_name(
            &if own { format!("delete {}?", p.name) } else { format!("remove {}?", p.name) },
            "",
            if own { "delete" } else { "remove" },
            true,
            if own { "It disappears from Your Library." } else { "It leaves Your Library; you can follow it again later." },
        )
        .await;
        if ok.is_none() {
            return;
        }
        match invoke_unit("playlist_unfollow", json!({ "playlist": p.id })).await {
            Ok(()) => {
                reload_playlists().await;
                toast(if own { "deleted" } else { "removed from Your Library" });
                go_home();
            }
            Err(e) => toast(format!("couldn't remove it: {e}")),
        }
    });
}

pub fn add_all_to_queue(tracks: Vec<Track>) {
    spawn(async move {
        let list: Vec<Track> = tracks.into_iter().take(50).collect();
        toast(format!("adding {} songs to the queue...", list.len()));
        for t in &list {
            if let Err(e) = invoke_unit("add_to_queue", json!({ "uri": t.uri })).await {
                return handle_remote_error(&e);
            }
        }
        toast("added to queue ♪");
    });
}

pub fn add_to_playlist(p: Playlist, t: Track) {
    spawn(async move {
        match invoke_unit("playlist_add", json!({ "playlist": p.id, "uri": t.uri })).await {
            Ok(()) => {
                let redraw = st(|s| {
                    if let Some(x) = s.playlists.iter_mut().find(|x| x.id == p.id) {
                        x.total += 1.0;
                    }
                    if s.view.kind == "playlist" && s.view.id == p.id {
                        if let Some(list) = s.view.tracks.as_mut() {
                            list.push(t.clone());
                            return true;
                        }
                    }
                    false
                });
                if redraw {
                    render();
                }
                toast(format!("added to {} ♡", p.name));
            }
            Err(e) => toast(format!("couldn't add it: {e}")),
        }
    });
}

pub fn remove_from_playlist(t: Track) {
    let (id, title, seq) = st(|s| (s.view.id.clone(), s.view.title.clone(), s.view.seq));
    spawn(async move {
        match invoke_unit("playlist_remove", json!({ "playlist": id, "uri": t.uri })).await {
            Ok(()) => {
                st(|s| {
                    let mut removed = 0.0;
                    s.update_view(seq, |v| {
                        if let Some(list) = v.tracks.as_mut() {
                            list.retain(|x| x.uri != t.uri);
                        }
                    });
                    if let Some(p) = s.playlists.iter_mut().find(|x| x.id == id) {
                        removed += 1.0;
                        p.total = (p.total - removed).max(0.0);
                    }
                });
                render_if(seq);
                toast(format!("removed from {title}"));
            }
            Err(e) => toast(format!("couldn't remove it: {e}")),
        }
    });
}

pub fn add_to_queue(t: Track) {
    spawn(async move {
        match invoke_unit("add_to_queue", json!({ "uri": t.uri })).await {
            Ok(()) => {
                st(|s| s.cache_tracks(std::slice::from_ref(&t)));
                toast("added to queue ♪");
            }
            Err(e) => handle_remote_error(&e),
        }
    });
}

// ---------- sleep timer: pause after some minutes, or when this song ends ----------

pub fn sleep_label() -> String {
    st(|s| {
        if s.sleep.end_of_track.is_some() {
            return "end of song".into();
        }
        let left = ((s.sleep.until - js_sys::Date::now()) / 60000.0).round().max(0.0) as i64;
        if left > 0 { format!("{left} min left") } else { "less than a minute".into() }
    })
}

pub fn sleep_active() -> bool {
    st(|s| s.sleep.until > 0.0 || s.sleep.end_of_track.is_some())
}

pub fn clear_sleep() {
    st(|s| s.sleep = Default::default());
}

pub fn set_sleep(min: Option<u32>) {
    clear_sleep();
    let Some(min) = min else {
        st(|s| s.sleep.end_of_track = s.now.as_ref().map(|t| t.uri.clone()));
        toast("music stops when this song ends ☾");
        return;
    };
    let timer = gloo_timers::callback::Timeout::new(min * 60000, || {
        clear_sleep();
        if st(|s| s.playing) {
            control("pause");
        }
        toast("sleep timer: good night ☾");
    });
    st(|s| {
        s.sleep.until = js_sys::Date::now() + f64::from(min) * 60000.0;
        s.sleep.timer = Some(timer);
    });
    toast(if min >= 60 { format!("music stops in {} hour ☾", min / 60) } else { format!("music stops in {min} minutes ☾") });
}

// "end of song": pause as soon as the next one starts
fn check_sleep_on_track_change(uri: &str) {
    let stop = st(|s| s.sleep.end_of_track.as_deref().is_some_and(|u| u != uri));
    if stop {
        clear_sleep();
        control("pause");
        toast("sleep timer: good night ☾");
    }
}

// ---------- lyrics ----------

async fn load_lyrics(t: Track) {
    let lines = invoke::<Option<Vec<LyricLine>>>("lyrics", json!({ "id": t.id })).await.ok().flatten();
    let still = st(|s| s.now.as_ref().map(|n| n.id == t.id).unwrap_or(false));
    if !still {
        return;
    }
    let on_page = st(|s| {
        s.lyrics = lines.filter(|l| !l.is_empty());
        s.lyrics_for = Some(t.id.clone());
        s.view.name == "lyrics"
    });
    render_lyrics_card();
    if on_page {
        render();
    }
}

pub fn render_lyrics_card() {
    let html = st(|s| {
        let Some(t) = &s.now else { return String::new() };
        s.last_line = -1;
        match &s.lyrics {
            None => format!(
                r#"<p class="none">{}</p>"#,
                if s.lyrics_for.as_deref() == Some(t.id.as_str()) { "no lyrics for this one~" } else { "loading lyrics..." }
            ),
            Some(lines) => lines.iter().enumerate().map(|(i, l)| format!(r#"<p data-line="{i}">{}</p>"#, esc(&l.words))).collect(),
        }
    });
    el("lyrics-lines").set_inner_html(&html);
}

fn sync_lyrics(pos: f64) {
    let idx = st(|s| {
        let lines = s.lyrics.as_ref()?;
        if !lines.iter().any(|l| l.start_ms > 0.0) {
            return None; // not time-synced
        }
        let mut idx = -1i64;
        for (i, l) in lines.iter().enumerate() {
            if l.start_ms <= pos + 150.0 {
                idx = i as i64;
            } else {
                break;
            }
        }
        if idx == s.last_line {
            return None;
        }
        s.last_line = idx;
        Some(idx)
    });
    let Some(idx) = idx else { return };
    let main = main_el();
    for (root, is_view) in [(try_el("lyrics-lines"), false), (try_el("lyrics-view"), true)] {
        let Some(root) = root else { continue };
        for p in query_all(&root, "p[data-line]") {
            let i: i64 = attr(&p, "data-line").and_then(|v| v.parse().ok()).unwrap_or(-2);
            toggle(&p, "now", i == idx);
            toggle(&p, "past", i < idx);
        }
        if let Some(line) = query(&root, &format!("p[data-line=\"{idx}\"]")) {
            let top = line.unchecked_ref::<web_sys::HtmlElement>().offset_top() as f64
                - if is_view { main.client_height() as f64 * 0.35 } else { root.client_height() as f64 * 0.3 };
            let opts = web_sys::ScrollToOptions::new();
            opts.set_top(top);
            opts.set_behavior(web_sys::ScrollBehavior::Smooth);
            let scroller: &Element = if is_view { &main } else { &root };
            scroller.scroll_to_with_scroll_to_options(&opts);
        }
    }
}

// ---------- featured banner ----------

pub fn play_featured() {
    let pl = st(|s| views::featured_playlist(s));
    spawn(async move {
        let Some(pl) = pl else {
            let liked = match st(|s| s.liked.clone()) {
                Some(l) => l,
                None => match invoke::<Vec<Track>>("liked", json!({})).await {
                    Ok(l) => {
                        st(|s| s.liked = Some(l.clone()));
                        l
                    }
                    Err(e) => return toast(e),
                },
            };
            return play_from(liked, 0, "Liked Songs", Some("liked".into()));
        };
        toast(format!("playing {} ♡", pl.name));
        match invoke::<Vec<Track>>("playlist_tracks", json!({ "id": pl.id })).await {
            Ok(tracks) if !tracks.is_empty() => {
                let start = if st(|s| s.shuffle) { (js_sys::Math::random() * tracks.len() as f64) as usize } else { 0 };
                play_from(tracks, start, &pl.name, Some(format!("spotify:playlist:{}", pl.id)));
            }
            Ok(_) => {}
            Err(e) => toast(e),
        }
    });
}

// ---------- auto-update (GitHub Releases) ----------

pub async fn check_for_update() {
    // offline or no release yet: stay quiet
    let Ok(Some(info)) = invoke::<Option<UpdateInfo>>("check_update", json!({})).await else { return };
    let is_new = st(|s| {
        let is_new = s.update.as_ref().map(|u| u.version != info.version).unwrap_or(true);
        s.update = Some(info.clone());
        is_new
    });
    remove_class(&el("bell-dot"), "hidden");
    if is_new {
        toast(format!("femboy spotify {} is out ♡ click the bell", info.version));
    }
}

pub fn show_update() {
    let Some(u) = st(|s| s.update.clone()) else { return };
    set_text(&el("update-text"), &format!("version {} is ready (you have {})", u.version, u.current));
    set_text(&el("update-notes"), u.notes.as_deref().unwrap_or(""));
    toggle(&el("update-notes"), "hidden", u.notes.as_deref().unwrap_or("").is_empty());
    remove_class(&el("update"), "hidden");
}

pub fn install_update() {
    let b = el("update-go");
    let _ = b.set_attribute("disabled", "");
    set_text(&b, "downloading...");
    spawn(async {
        // the app restarts when this succeeds
        if let Err(e) = invoke_unit("install_update", json!({})).await {
            toast(format!("update failed: {e}"));
            let b = el("update-go");
            let _ = b.remove_attribute("disabled");
            set_text(&b, "update & restart");
        }
    });
}

// ---------- customize banner ----------

pub fn open_customize() {
    st(|s| s.pending_banner = None);
    input("banner-title").set_value(&store_get("bannerTitle", String::new()));
    input("banner-file").set_value("");
    remove_class(&el("customize"), "hidden");
}

pub fn reset_banner() {
    store_remove("banner");
    store_remove("bannerTitle");
    add_class(&el("customize"), "hidden");
    if st(|s| s.view.name == "home") {
        render();
    }
    toast("banner reset ♡");
}

pub fn save_banner() {
    store_set("bannerTitle", &input("banner-title").value().trim().to_string());
    if let Some(b) = st(|s| s.pending_banner.clone()) {
        if !store_set("banner", &b) {
            toast("that picture is too big, try a smaller one~");
        }
    }
    add_class(&el("customize"), "hidden");
    if st(|s| s.view.name == "home") { render() } else { go_home() }
}

/// Shrinks a picked picture to at most `max_w` wide, as a JPEG data URL.
pub async fn downscale(file: web_sys::File, max_w: f64) -> Option<String> {
    let reader = web_sys::FileReader::new().ok()?;
    let p = js_sys::Promise::new(&mut |resolve, reject| {
        reader.set_onload(Some(&resolve));
        reader.set_onerror(Some(&reject));
    });
    reader.read_as_data_url(&file).ok()?;
    wasm_bindgen_futures::JsFuture::from(p).await.ok()?;
    let data = reader.result().ok()?.as_string()?;
    let im = web_sys::HtmlImageElement::new().ok()?;
    let p = js_sys::Promise::new(&mut |resolve, reject| {
        im.set_onload(Some(&resolve));
        im.set_onerror(Some(&reject));
    });
    im.set_src(&data);
    wasm_bindgen_futures::JsFuture::from(p).await.ok()?;
    let scale = (max_w / f64::from(im.natural_width())).min(1.0);
    let w = (f64::from(im.natural_width()) * scale).round() as u32;
    let h = (f64::from(im.natural_height()) * scale).round() as u32;
    let canvas: web_sys::HtmlCanvasElement = document().create_element("canvas").ok()?.unchecked_into();
    canvas.set_width(w);
    canvas.set_height(h);
    let ctx: web_sys::CanvasRenderingContext2d = canvas.get_context("2d").ok()??.unchecked_into();
    ctx.draw_image_with_html_image_element_and_dw_and_dh(&im, 0.0, 0.0, f64::from(w), f64::from(h)).ok()?;
    canvas.to_data_url_with_type_and_encoder_options("image/jpeg", &wasm_bindgen::JsValue::from_f64(0.86)).ok()
}

// ---------- sign-in / startup ----------

pub fn logout() {
    if !window().confirm_with_message("log out of femboy spotify? ♡").unwrap_or(false) {
        return;
    }
    spawn(async {
        if let Err(e) = invoke_unit("logout", json!({})).await {
            toast(e);
        }
        let _ = window().location().reload();
    });
}

pub fn login() {
    let b = el("btn-login");
    let _ = b.set_attribute("disabled", "");
    set_text(&b, "waiting for Spotify...");
    add_class(&el("login-error"), "hidden");
    set_text(&el("login-text"), "sign in in the Spotify window that just opened ♡");
    spawn(async {
        match invoke_unit("login", json!({})).await {
            Ok(()) => {
                add_class(&el("login"), "hidden");
                after_login().await;
            }
            Err(e) => {
                let log = invoke::<String>("log_file", json!({})).await.unwrap_or_default();
                let boxed = el("login-error");
                let text = if log.is_empty() { format!("sign-in failed: {e}") } else { format!("sign-in failed: {e}\n\nlog: {log}") };
                set_text(&boxed, &text);
                set_style(&boxed, "white-space", "pre-wrap");
                remove_class(&boxed, "hidden");
                set_text(&el("login-text"), "something went wrong (｡•́︿•̀｡) you can try again:");
            }
        }
        let b = el("btn-login");
        let _ = b.remove_attribute("disabled");
        set_text(&b, "sign in with Spotify");
    });
}

async fn after_login() {
    spawn(check_player());
    spawn(load_follows());
    load_library().await;
}

async fn load_library() {
    spawn(async {
        match invoke::<Me>("me", json!({})).await {
            Ok(me) => {
                let src = me.avatar.clone().filter(|a| !a.is_empty()).unwrap_or_else(|| NO_ARTIST.into());
                let _ = el("avatar-img").set_attribute("src", &src);
                st(|s| s.me = Some(me));
            }
            Err(_) => {}
        }
    });
    match invoke::<Vec<Playlist>>("playlists", json!({})).await {
        Ok(p) => st(|s| s.playlists = p),
        Err(e) => toast(format!("couldn't load playlists: {e}")),
    }
    render_side_thumbs();
    if st(|s| s.view.name == "home") {
        render();
    }
    spawn(async {
        if let Ok(list) = invoke::<Vec<Track>>("liked", json!({})).await {
            st(|s| {
                s.cache_tracks(&list);
                for t in &list {
                    s.saved.insert(t.id.clone());
                }
                s.liked = Some(list);
            });
        }
    });
    spawn(async {
        if let Ok(r) = invoke::<RemoteState>("remote_state", json!({})).await {
            apply_remote(r);
        }
    });
    spawn(async {
        if let Ok(Some(h)) = invoke::<Option<Vec<Shelf>>>("spotify_home", json!({})).await {
            set_home(h);
        }
    });
}

/// Logs in with the saved login. If Spotify can't be reached yet (e.g. right after
/// Windows starts, before the network is up) keeps trying instead of asking to sign
/// in again: the saved login is fine.
pub async fn connect_saved() {
    let mut attempt = 0u32;
    loop {
        match invoke_value("init", json!({})).await {
            Ok(v) => {
                if v.as_bool().unwrap_or(false) {
                    if attempt > 0 {
                        toast("connected to Spotify ♡");
                    }
                    after_login().await;
                } else {
                    remove_class(&el("login"), "hidden");
                }
                return;
            }
            Err(_) => {
                if attempt == 0 {
                    toast("waiting for the internet to reach Spotify...");
                }
                attempt += 1;
                sleep((5000 * attempt).min(30000)).await;
            }
        }
    }
}
