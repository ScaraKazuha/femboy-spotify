//! App state and key handling. No I/O here: handlers return `Action`s.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::{ListState, TableState};

use crate::api::{Playlist, Track};
use crate::player::Event as PlayerEvent;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Search,
    Liked,
    Playlists,
}

impl Section {
    pub const ALL: [Section; 3] = [Section::Search, Section::Liked, Section::Playlists];

    pub fn label(self) -> &'static str {
        match self {
            Section::Search => "✿ search",
            Section::Liked => "♥ liked songs",
            Section::Playlists => "♫ playlists",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Main,
    Input,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Content {
    Tracks,
    Playlists,
}

/// Side effects the main loop performs for us.
pub enum Action {
    Search(String),
    LoadLiked,
    LoadPlaylists,
    LoadPlaylist { id: String, name: String },
    Play(String),
    Pause,
    Resume,
    Seek(u32),
    Volume(u16),
}

/// Results of background API requests.
pub enum Msg {
    User(String),
    Tracks { title: String, tracks: Vec<Track> },
    Playlists(Vec<Playlist>),
    Error(String),
}

pub struct App {
    pub section: Section,
    pub focus: Focus,
    pub content: Content,
    pub input: String,
    pub title: String,
    pub tracks: Vec<Track>,
    pub track_state: TableState,
    pub playlists: Vec<Playlist>,
    pub playlist_state: ListState,
    pub queue: Vec<Track>,
    pub queue_pos: Option<usize>,
    pub playing: bool,
    pub buffering: bool,
    position_ms: u64,
    position_at: Instant,
    pub volume: u16,
    pub shuffle: bool,
    pub status: String,
    pub loading: bool,
    pub user: String,
    pub tick: u64,
    pub show_help: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(volume: u16) -> Self {
        Self {
            section: Section::Liked,
            focus: Focus::Main,
            content: Content::Tracks,
            input: String::new(),
            title: Section::Liked.label().to_string(),
            tracks: Vec::new(),
            track_state: TableState::default(),
            playlists: Vec::new(),
            playlist_state: ListState::default(),
            queue: Vec::new(),
            queue_pos: None,
            playing: false,
            buffering: false,
            position_ms: 0,
            position_at: Instant::now(),
            volume,
            shuffle: false,
            status: "hiii~ loading your liked songs ♡".into(),
            loading: true,
            user: String::new(),
            tick: 0,
            show_help: false,
            should_quit: false,
        }
    }

    pub fn section_index(&self) -> usize {
        self.section.index()
    }

    pub fn now_playing(&self) -> Option<&Track> {
        self.queue_pos.and_then(|i| self.queue.get(i))
    }

    pub fn position_ms(&self) -> u64 {
        let mut pos = self.position_ms;
        if self.playing && !self.buffering {
            pos += self.position_at.elapsed().as_millis() as u64;
        }
        match self.now_playing() {
            Some(t) if t.duration_ms > 0 => pos.min(t.duration_ms),
            _ => pos,
        }
    }

    fn set_position(&mut self, ms: u64) {
        self.position_ms = ms;
        self.position_at = Instant::now();
    }

    // ---------- background results ----------

    pub fn on_msg(&mut self, msg: Msg) {
        self.loading = false;
        match msg {
            Msg::User(name) => {
                self.user = name;
                self.loading = self.tracks.is_empty() && self.content == Content::Tracks;
            }
            Msg::Tracks { title, tracks } => {
                self.status = if tracks.is_empty() {
                    "nothing found... (｡•́︿•̀｡)".into()
                } else {
                    format!("{} tracks ♡", tracks.len())
                };
                self.title = title;
                self.tracks = tracks;
                self.content = Content::Tracks;
                self.track_state.select(if self.tracks.is_empty() { None } else { Some(0) });
            }
            Msg::Playlists(pls) => {
                self.status = format!("{} playlists ♡", pls.len());
                self.title = Section::Playlists.label().into();
                self.playlists = pls;
                self.content = Content::Playlists;
                self.playlist_state
                    .select(if self.playlists.is_empty() { None } else { Some(0) });
            }
            Msg::Error(e) => self.status = format!("oopsie: {e}"),
        }
    }

    pub fn on_player_event(&mut self, ev: PlayerEvent) -> Option<Action> {
        match ev {
            PlayerEvent::Loading => self.buffering = true,
            PlayerEvent::Playing(ms) => {
                self.buffering = false;
                self.playing = true;
                self.set_position(ms as u64);
            }
            PlayerEvent::Paused(ms) => {
                self.buffering = false;
                self.playing = false;
                self.set_position(ms as u64);
            }
            PlayerEvent::Position(ms) => self.set_position(ms as u64),
            PlayerEvent::Stopped => {
                self.playing = false;
                self.buffering = false;
            }
            PlayerEvent::EndOfTrack => return self.next(),
            PlayerEvent::Unavailable => {
                self.status = "that track is unavailable, skipping~".into();
                return self.next();
            }
        }
        None
    }

    // ---------- playback ----------

    fn play_index(&mut self, i: usize) -> Option<Action> {
        let track = self.queue.get(i)?.clone();
        self.queue_pos = Some(i);
        self.playing = true;
        self.buffering = true;
        self.set_position(0);
        self.status = format!("♪ {} — {}", track.name, track.artists);
        Some(Action::Play(track.uri))
    }

    pub fn next(&mut self) -> Option<Action> {
        let pos = self.queue_pos?;
        if self.queue.is_empty() {
            return None;
        }
        let next = if self.shuffle && self.queue.len() > 1 {
            let mut n = fastrand::usize(..self.queue.len());
            while n == pos {
                n = fastrand::usize(..self.queue.len());
            }
            n
        } else if pos + 1 < self.queue.len() {
            pos + 1
        } else {
            self.playing = false;
            self.status = "end of the queue~ pick something else ♡".into();
            return None;
        };
        self.play_index(next)
    }

    fn prev(&mut self) -> Option<Action> {
        let pos = self.queue_pos?;
        // like every player: restart the song if we're more than 3s in
        if self.position_ms() > 3000 || pos == 0 {
            self.set_position(0);
            return Some(Action::Seek(0));
        }
        self.play_index(pos - 1)
    }

    fn seek_by(&mut self, delta_ms: i64) -> Option<Action> {
        let track = self.now_playing()?;
        let max = track.duration_ms.saturating_sub(1000) as i64;
        let target = (self.position_ms() as i64 + delta_ms).clamp(0, max.max(0)) as u64;
        self.set_position(target);
        Some(Action::Seek(target as u32))
    }

    fn toggle_pause(&mut self) -> Option<Action> {
        self.now_playing()?;
        if self.playing {
            let pos = self.position_ms();
            self.playing = false;
            self.set_position(pos);
            Some(Action::Pause)
        } else {
            self.playing = true;
            self.set_position(self.position_ms);
            Some(Action::Resume)
        }
    }

    fn change_volume(&mut self, delta: i16) -> Option<Action> {
        self.volume = (self.volume as i16 + delta).clamp(0, 100) as u16;
        Some(Action::Volume(self.volume))
    }

    // ---------- keys ----------

    pub fn on_key(&mut self, key: KeyEvent) -> Option<Action> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return None;
        }

        if self.show_help {
            self.show_help = false;
            return None;
        }

        if self.focus == Focus::Input {
            return self.on_input_key(key);
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('/') => {
                self.section = Section::Search;
                self.focus = Focus::Input;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Main,
                    _ => Focus::Sidebar,
                };
            }
            KeyCode::Char(' ') => return self.toggle_pause(),
            KeyCode::Char('n') => return self.next(),
            KeyCode::Char('p') => return self.prev(),
            KeyCode::Right => return self.seek_by(10_000),
            KeyCode::Left => return self.seek_by(-10_000),
            KeyCode::Char('+') | KeyCode::Char('=') => return self.change_volume(5),
            KeyCode::Char('-') | KeyCode::Char('_') => return self.change_volume(-5),
            KeyCode::Char('s') => {
                self.shuffle = !self.shuffle;
                self.status = if self.shuffle { "shuffle on ✧" } else { "shuffle off" }.into();
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::PageUp => self.move_cursor(-10),
            KeyCode::PageDown => self.move_cursor(10),
            KeyCode::Home | KeyCode::Char('g') => self.move_cursor(-100_000),
            KeyCode::End | KeyCode::Char('G') => self.move_cursor(100_000),
            KeyCode::Enter => return self.on_enter(),
            KeyCode::Esc | KeyCode::Backspace => {
                // back from a playlist's tracks to the playlist list
                if self.section == Section::Playlists
                    && self.content == Content::Tracks
                    && !self.playlists.is_empty()
                {
                    self.content = Content::Playlists;
                    self.title = Section::Playlists.label().into();
                }
            }
            _ => {}
        }
        None
    }

    fn on_input_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc => self.focus = Focus::Main,
            KeyCode::Enter => {
                let q = self.input.trim().to_string();
                self.focus = Focus::Main;
                if !q.is_empty() {
                    self.loading = true;
                    self.status = format!("searching for \"{q}\"...");
                    return Some(Action::Search(q));
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
        None
    }

    fn move_cursor(&mut self, delta: i32) {
        let step = |sel: Option<usize>, len: usize| -> Option<usize> {
            if len == 0 {
                return None;
            }
            let cur = sel.unwrap_or(0) as i64;
            Some((cur + delta as i64).clamp(0, len as i64 - 1) as usize)
        };
        match self.focus {
            Focus::Sidebar => {
                let i = step(Some(self.section_index()), Section::ALL.len()).unwrap_or(0);
                self.section = Section::ALL[i];
            }
            _ => match self.content {
                Content::Tracks => {
                    let s = step(self.track_state.selected(), self.tracks.len());
                    self.track_state.select(s);
                }
                Content::Playlists => {
                    let s = step(self.playlist_state.selected(), self.playlists.len());
                    self.playlist_state.select(s);
                }
            },
        }
    }

    fn on_enter(&mut self) -> Option<Action> {
        if self.focus == Focus::Sidebar {
            self.focus = Focus::Main;
            return match self.section {
                Section::Search => {
                    self.focus = Focus::Input;
                    None
                }
                Section::Liked => {
                    self.loading = true;
                    self.status = "loading liked songs...".into();
                    Some(Action::LoadLiked)
                }
                Section::Playlists => {
                    self.loading = true;
                    self.status = "loading playlists...".into();
                    Some(Action::LoadPlaylists)
                }
            };
        }

        match self.content {
            Content::Tracks => {
                let i = self.track_state.selected()?;
                self.queue = self.tracks.clone();
                self.play_index(i)
            }
            Content::Playlists => {
                let pl = self.playlists.get(self.playlist_state.selected()?)?.clone();
                self.loading = true;
                self.status = format!("loading {}...", pl.name);
                Some(Action::LoadPlaylist {
                    id: pl.id,
                    name: pl.name,
                })
            }
        }
    }
}

pub fn fmt_ms(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}
