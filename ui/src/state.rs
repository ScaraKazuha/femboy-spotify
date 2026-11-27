//! Everything the interface remembers while it runs.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::types::*;
use crate::util::{now_ms, store_get};

/// What the main panel shows. `seq` identifies one opened page, so an answer that
/// arrives late only updates the page it was asked for.
#[derive(Clone, Default)]
pub struct View {
    pub seq: u64,
    /// home | library | list | artist | search | recent | queue | lyrics
    pub name: String,
    /// list kinds: liked | playlist | album | radio | search
    pub kind: String,
    pub id: String,
    pub uri: String,
    pub title: String,
    pub sub: String,
    pub sub_html: Option<String>,
    pub image: Option<String>,
    /// None until loaded
    pub tracks: Option<Vec<Track>>,
    /// what the rows show (filtered / sorted); rows' data-index points into this
    pub shown: Option<Vec<Track>>,
    pub loading: bool,
    pub filter_text: String,
    pub sort_key: String,
    // search
    pub q: String,
    pub tab: String,
    pub results: Option<SearchResults>,
    // artist
    pub top: Vec<Track>,
    pub albums: Vec<AlbumCard>,
    pub singles: Vec<AlbumCard>,
    // recent
    pub cards: Option<Vec<Card>>,
    pub error: Option<String>,
}

impl View {
    pub fn new(name: &str) -> Self {
        View { name: name.into(), ..Default::default() }
    }
}

#[derive(Default)]
pub struct Sleep {
    pub until: f64,
    pub end_of_track: Option<String>,
    pub timer: Option<gloo_timers::callback::Timeout>,
}

pub struct State {
    pub me: Option<Me>,
    pub playlists: Vec<Playlist>,
    pub liked: Option<Vec<Track>>,
    pub view: View,
    pub back: Vec<View>,
    pub forward: Vec<View>,
    pub next_seq: u64,
    /// home chips: all | playlists
    pub filter: String,
    // what plays
    pub queue: Vec<Track>,
    pub index: i64,
    /// label of what plays ("Liked Songs", "<song> radio"...)
    pub context: String,
    /// Spotify context sent with play
    pub ctx: Option<String>,
    pub playing: bool,
    pub buffering: bool,
    pub pos: f64,
    pub pos_at: f64,
    pub duration_ms: f64,
    pub volume: f64,
    pub muted: bool,
    pub shuffle: bool,
    /// off | all | one
    pub repeat: String,
    pub now: Option<Track>,
    pub lyrics: Option<Vec<LyricLine>>,
    pub lyrics_for: Option<String>,
    pub last_line: i64,
    pub saved: HashSet<String>,
    pub cache: HashMap<String, Track>,
    // Spotify Connect
    pub devices: Vec<RemoteDevice>,
    pub active_device: Option<String>,
    pub active_device_name: Option<String>,
    pub queue_uris: Vec<String>,
    pub next_uris: Vec<String>,
    pub queue_key: String,
    pub player_ready: bool,
    pub dj_on: bool,
    pub dj_voice_hint: bool,
    // library extras
    pub home: Vec<Shelf>,
    pub home_items: HashMap<String, Playlist>,
    pub cards: HashMap<String, Card>,
    pub recent_cards: Option<Vec<Card>>,
    pub followed: HashSet<String>,
    pub saved_albums: HashSet<String>,
    // misc
    pub update: Option<UpdateInfo>,
    pub seek_drag: bool,
    pub vol_touched: f64,
    pub vol_sent_at: f64,
    pub sleep: Sleep,
    pub pending_banner: Option<String>,
    pub search_seq: u64,
    pub radio_seq: u64,
}

impl State {
    fn new() -> Self {
        State {
            me: None,
            playlists: vec![],
            liked: None,
            view: View { seq: 1, ..View::new("home") },
            back: vec![],
            forward: vec![],
            next_seq: 2,
            filter: "all".into(),
            queue: vec![],
            index: -1,
            context: String::new(),
            ctx: None,
            playing: false,
            buffering: false,
            pos: 0.0,
            pos_at: now_ms(),
            duration_ms: 0.0,
            volume: store_get("volume", 70.0),
            muted: false,
            shuffle: store_get("shuffle", false),
            repeat: store_get("repeat", "off".to_string()),
            now: None,
            lyrics: None,
            lyrics_for: None,
            last_line: -1,
            saved: HashSet::new(),
            cache: HashMap::new(),
            devices: vec![],
            active_device: None,
            active_device_name: None,
            queue_uris: vec![],
            next_uris: vec![],
            queue_key: String::new(),
            player_ready: false,
            dj_on: false,
            dj_voice_hint: false,
            home: vec![],
            home_items: HashMap::new(),
            cards: HashMap::new(),
            recent_cards: None,
            followed: HashSet::new(),
            saved_albums: HashSet::new(),
            update: None,
            seek_drag: false,
            vol_touched: 0.0,
            vol_sent_at: 0.0,
            sleep: Sleep::default(),
            pending_banner: None,
            search_seq: 0,
            radio_seq: 0,
        }
    }

    pub fn new_view(&mut self, name: &str) -> View {
        let seq = self.next_seq;
        self.next_seq += 1;
        View { seq, ..View::new(name) }
    }

    /// The tracks behind the rows on screen.
    pub fn view_tracks(&self) -> Vec<Track> {
        self.view.shown.clone().or_else(|| self.view.tracks.clone()).unwrap_or_default()
    }

    pub fn view_track(&self, i: usize) -> Option<Track> {
        match &self.view.shown {
            Some(s) => s.get(i).cloned(),
            None => self.view.tracks.as_ref().and_then(|t| t.get(i).cloned()),
        }
    }

    pub fn cache_tracks(&mut self, list: &[Track]) {
        for t in list {
            self.cache.insert(t.uri.clone(), t.clone());
        }
    }

    /// Updates the page `seq` wherever it is (on screen, or in back / forward).
    pub fn update_view(&mut self, seq: u64, f: impl Fn(&mut View)) {
        if self.view.seq == seq {
            f(&mut self.view);
        }
        for v in self.back.iter_mut().chain(self.forward.iter_mut()) {
            if v.seq == seq {
                f(v);
            }
        }
    }

    pub fn duration(&self) -> f64 {
        if self.duration_ms > 0.0 {
            self.duration_ms
        } else {
            self.now.as_ref().map(|t| t.duration_ms).unwrap_or(0.0)
        }
    }

    pub fn position(&self) -> f64 {
        let mut p = self.pos;
        if self.playing && !self.buffering {
            p += now_ms() - self.pos_at;
        }
        if self.now.is_some() { p.min(self.duration()) } else { 0.0 }
    }

    pub fn set_pos(&mut self, ms: f64) {
        self.pos = ms;
        self.pos_at = now_ms();
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::new());
}

/// Reads or changes the state. Never call it again from inside `f`.
pub fn st<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}
