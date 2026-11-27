//! Data the app's Rust side sends over (same shapes as src/api.rs, src/remote.rs).
//! Every field has a default so older saved history and the demo stay readable.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ArtistRef {
    pub name: String,
    pub uri: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    pub id: String,
    pub uri: String,
    pub name: String,
    pub artists: String,
    pub album: String,
    pub duration_ms: f64,
    pub image: Option<String>,
    pub image_large: Option<String>,
    pub artist_list: Vec<ArtistRef>,
    pub album_uri: Option<String>,
    pub added_at: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub total: f64,
    pub image: Option<String>,
    pub editable: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Me {
    pub name: String,
    pub avatar: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LyricLine {
    pub start_ms: f64,
    pub words: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Card {
    pub uri: String,
    pub kind: String,
    pub name: String,
    pub sub: String,
    pub image: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub artists: Vec<Card>,
    pub albums: Vec<Card>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AlbumCard {
    pub uri: String,
    pub name: String,
    pub year: i32,
    pub image: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ArtistPage {
    pub uri: String,
    pub name: String,
    pub image: Option<String>,
    pub top: Vec<Track>,
    pub albums: Vec<AlbumCard>,
    pub singles: Vec<AlbumCard>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AlbumPage {
    pub uri: String,
    pub name: String,
    pub artists: Vec<ArtistRef>,
    pub year: i32,
    pub image: Option<String>,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ShelfItem {
    pub id: String,
    pub name: String,
    pub sub: String,
    pub image: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Shelf {
    pub title: String,
    pub items: Vec<ShelfItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteDevice {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub volume: f64,
    pub active: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteState {
    pub devices: Vec<RemoteDevice>,
    pub active_device: Option<String>,
    pub active_device_name: Option<String>,
    pub track_uri: Option<String>,
    pub context_uri: String,
    pub playing: bool,
    pub buffering: bool,
    pub position_ms: f64,
    pub duration_ms: f64,
    pub shuffle: bool,
    pub repeat_context: bool,
    pub repeat_track: bool,
    pub volume: Option<f64>,
    pub next_uris: Vec<String>,
    pub queue_uris: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateInfo {
    pub current: String,
    pub version: String,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalSpotify {
    pub name: String,
    pub running: bool,
}
