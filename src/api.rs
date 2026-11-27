//! Library data straight from Spotify's internal services (the same ones the
//! official app uses), over the librespot session.
//!
//! The public Web API (api.spotify.com) is not used: librespot's shared Client ID
//! is permanently rate limited there. Everything here works right after sign-in.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use futures_util::{StreamExt, stream};
use librespot::core::{SpotifyId, SpotifyUri, session::Session};
use librespot::metadata::{Album as LsAlbum, Artist as LsArtist, Lyrics, Metadata, Track as LsTrack};
use librespot::protocol::{
    extended_metadata::{BatchedEntityRequest, EntityRequest, ExtensionQuery},
    extension_kind::ExtensionKind,
    metadata::Track as TrackProto,
    playlist4_external::{Add, ChangeInfo, Delta, Item, ListChanges, Op, Rem, SelectedListContent, op},
};
use protobuf::{EnumOrUnknown, Message, MessageField};
use reqwest::Method;
use serde::Serialize;
use serde_json::Value;

use crate::auth::Login;

/// Upper bound on how many tracks we pull from one list.
const MAX_ITEMS: usize = 2000;
/// Tracks per extended-metadata request.
const BATCH: usize = 100;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Track {
    pub id: String,
    pub uri: String,
    pub name: String,
    pub artists: String,
    pub album: String,
    pub duration_ms: u64,
    /// ~300px cover
    pub image: Option<String>,
    /// largest cover
    pub image_large: Option<String>,
    /// every artist with their "spotify:artist:…" uri (each name links to an artist page)
    pub artist_list: Vec<ArtistRef>,
    /// e.g. "spotify:album:…" (for "Go to album")
    pub album_uri: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ArtistRef {
    pub name: String,
    pub uri: String,
}

/// An album or single on an artist page.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AlbumCard {
    pub uri: String,
    pub name: String,
    pub year: i32,
    pub image: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ArtistPage {
    pub uri: String,
    pub name: String,
    pub image: Option<String>,
    /// popular tracks
    pub top: Vec<Track>,
    pub albums: Vec<AlbumCard>,
    pub singles: Vec<AlbumCard>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AlbumPage {
    pub uri: String,
    pub name: String,
    pub artists: Vec<ArtistRef>,
    pub year: i32,
    pub image: Option<String>,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub total: u64,
    pub image: Option<String>,
    /// yours, or shared with you for editing (songs can be added / removed)
    pub editable: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Me {
    pub name: String,
    pub avatar: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LyricLine {
    pub start_ms: u64,
    pub words: String,
}

#[derive(Clone)]
pub struct Api {
    session: Session,
}

fn image_url(hex: &str) -> String {
    format!("https://i.scdn.co/image/{hex}")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Percent-encodes a search query for a `spotify:search:` URI (spaces become `+`).
pub fn encode_query(q: &str) -> String {
    let mut out = String::new();
    for b in q.trim().bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

impl Api {
    pub fn new(login: &Login) -> Self {
        Self {
            session: login.session.clone(),
        }
    }

    async fn json(&self, endpoint: &str) -> Result<Value> {
        let bytes = self
            .session
            .spclient()
            .request_as_json(&Method::GET, endpoint, None, None)
            .await
            .map_err(|e| anyhow!("Spotify request failed: {e}"))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn me(&self) -> Result<Me> {
        let user = self.session.username();
        let p = self.json(&format!("/user-profile-view/v3/profile/{user}?playlist_limit=0&artist_limit=0")).await?;
        Ok(Me {
            name: p["name"].as_str().unwrap_or(&user).to_string(),
            avatar: p["image_url"].as_str().filter(|s| !s.is_empty()).map(String::from),
        })
    }

    pub async fn display_name(&self) -> Result<String> {
        Ok(self.me().await?.name)
    }

    /// Metadata for many tracks at once, in the given order (unknown ones are skipped).
    async fn tracks(&self, uris: &[String]) -> Result<Vec<Track>> {
        let uris: Vec<String> = uris
            .iter()
            .filter(|u| u.starts_with("spotify:track:"))
            .take(MAX_ITEMS)
            .cloned()
            .collect();
        let chunks: Vec<Vec<String>> = uris.chunks(BATCH).map(|c| c.to_vec()).collect();
        let api = self.clone();
        let results: Vec<Result<HashMap<String, Track>>> = stream::iter(chunks)
            .map(move |chunk| {
                let api = api.clone();
                async move { api.track_batch(chunk).await }
            })
            .buffered(4)
            .collect()
            .await;
        let mut by_uri = HashMap::new();
        for r in results {
            by_uri.extend(r?);
        }
        Ok(uris.iter().filter_map(|u| by_uri.remove(u)).collect())
    }

    async fn track_batch(&self, uris: Vec<String>) -> Result<HashMap<String, Track>> {
        let req = BatchedEntityRequest {
            entity_request: uris
                .iter()
                .map(|u| EntityRequest {
                    entity_uri: u.to_string(),
                    query: vec![ExtensionQuery {
                        extension_kind: EnumOrUnknown::new(ExtensionKind::TRACK_V4),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let res = self
            .session
            .spclient()
            .get_extended_metadata(req)
            .await
            .map_err(|e| anyhow!("could not load track info: {e}"))?;

        let mut out = HashMap::new();
        for em in res.extended_metadata {
            for data in em.extension_data {
                let uri = data.entity_uri.clone();
                let Some(any) = data.extension_data.as_ref() else { continue };
                let Ok(msg) = TrackProto::parse_from_bytes(&any.value) else { continue };
                let Ok(id) = SpotifyUri::from_uri(&uri) else { continue };
                let Ok(t) = LsTrack::parse(&msg, &id) else { continue };
                out.insert(uri.clone(), convert_track(&uri, &t));
            }
        }
        Ok(out)
    }

    /// Track URIs of a context ("spotify:user:…:collection", "spotify:search:…"), following pages.
    async fn context_uris(&self, context: &str, limit: usize) -> Result<Vec<String>> {
        let ctx = self.json(&format!("/context-resolve/v1/{context}")).await?;
        let mut uris = Vec::new();
        let mut pending: Vec<String> = Vec::new();
        for page in ctx["pages"].as_array().cloned().unwrap_or_default() {
            collect_page(&page, &mut uris, &mut pending);
        }
        while let Some(url) = pending.pop() {
            if uris.len() >= limit {
                break;
            }
            let path = url.trim_start_matches("hm://");
            match self.json(&format!("/{path}")).await {
                Ok(page) => collect_page(&page, &mut uris, &mut pending),
                Err(e) => {
                    crate::auth::log(&format!("context page failed: {e:#}"));
                    break;
                }
            }
        }
        uris.truncate(limit);
        Ok(uris)
    }

    /// Info for one track (e.g. what another device is playing right now).
    pub async fn track(&self, uri: &str) -> Result<Option<Track>> {
        Ok(self.tracks(&[uri.to_string()]).await?.into_iter().next())
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Track>> {
        let uris = self
            .context_uris(&format!("spotify:search:{}", encode_query(query)), 50)
            .await?;
        self.tracks(&uris).await
    }

    pub async fn liked(&self) -> Result<Vec<Track>> {
        let user = self.session.username();
        let uris = self
            .context_uris(&format!("spotify:user:{user}:collection"), MAX_ITEMS)
            .await?;
        self.tracks(&uris).await
    }

    async fn playlist_raw(&self, id: &str) -> Result<SelectedListContent> {
        let sid = SpotifyId::from_base62(id).map_err(|e| anyhow!("bad playlist id: {e}"))?;
        let bytes = self
            .session
            .spclient()
            .get_playlist(&sid)
            .await
            .map_err(|e| anyhow!("could not load playlist: {e}"))?;
        Ok(SelectedListContent::parse_from_bytes(&bytes)?)
    }

    pub async fn playlists(&self) -> Result<Vec<Playlist>> {
        let bytes = self
            .session
            .spclient()
            .get_rootlist(0, Some(500))
            .await
            .map_err(|e| anyhow!("could not load your playlists: {e}"))?;
        let root = SelectedListContent::parse_from_bytes(&bytes)?;
        let ids: Vec<String> = root
            .contents
            .items
            .iter()
            .filter_map(|i| i.uri().strip_prefix("spotify:playlist:").map(String::from))
            .collect();

        let me = self.session.username();
        let api = self.clone();
        let raws: Vec<(String, Result<SelectedListContent>)> = stream::iter(ids)
            .map(move |id| {
                let api = api.clone();
                async move {
                    let r = api.playlist_raw(&id).await;
                    (id, r)
                }
            })
            .buffered(8)
            .collect()
            .await;

        // playlists without their own picture get the cover of their first track
        let mut need_cover = Vec::new();
        let mut out = Vec::new();
        for (id, raw) in raws {
            let Ok(pl) = raw else { continue };
            if pl.length() <= 0 {
                continue; // e.g. the "DJ" pseudo-playlist
            }
            let picture = pl.attributes.picture();
            let image = (!picture.is_empty()).then(|| image_url(&hex(picture)));
            if image.is_none() {
                if let Some(first) = pl.contents.items.iter().find(|i| i.uri().starts_with("spotify:track:")) {
                    need_cover.push(first.uri().to_string());
                }
            }
            let owner = pl.owner_username();
            out.push((
                pl.contents
                    .items
                    .iter()
                    .find(|i| i.uri().starts_with("spotify:track:"))
                    .map(|i| i.uri().to_string()),
                Playlist {
                    id,
                    name: pl.attributes.name().to_string(),
                    owner: if owner == me {
                        "you".into()
                    } else if owner == "spotify" {
                        "Spotify".into()
                    } else {
                        String::new() // other users only have opaque ids here
                    },
                    total: pl.length().max(0) as u64,
                    image,
                    editable: owner == me || pl.capabilities.can_edit_items(),
                },
            ));
        }
        let covers: HashMap<String, Option<String>> = self
            .tracks(&need_cover)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|t| (t.uri, t.image))
            .collect();
        Ok(out
            .into_iter()
            .map(|(first, mut p)| {
                if p.image.is_none() {
                    p.image = first.and_then(|u| covers.get(&u).cloned().flatten());
                }
                p
            })
            .collect())
    }

    pub async fn playlist_tracks(&self, id: &str) -> Result<Vec<Track>> {
        let pl = self.playlist_raw(id).await?;
        let uris: Vec<String> = pl.contents.items.iter().map(|i| i.uri().to_string()).collect();
        self.tracks(&uris).await
    }

    /// Tracks of any context Spotify can resolve: an album, an artist (their top
    /// tracks), a song radio ("spotify:station:track:…").
    pub async fn context_tracks(&self, uri: &str, limit: usize) -> Result<Vec<Track>> {
        let uris = self.context_uris(uri, limit.min(MAX_ITEMS)).await?;
        self.tracks(&uris).await
    }

    /// An artist page: popular tracks, albums and singles.
    pub async fn artist(&self, uri: &str) -> Result<ArtistPage> {
        let id = SpotifyUri::from_uri(uri).map_err(|e| anyhow!("bad artist: {e}"))?;
        let a = LsArtist::get(&self.session, &id)
            .await
            .map_err(|e| anyhow!("could not load the artist: {e}"))?;
        let top_uris: Vec<String> = a
            .top_tracks
            .for_country(&self.session.country())
            .0
            .iter()
            .filter_map(|u| u.to_uri().ok())
            .collect();
        let top = if top_uris.is_empty() {
            self.context_tracks(uri, 10).await.unwrap_or_default()
        } else {
            self.tracks(&top_uris).await?
        };
        // each group lists versions of one release; the first is the main one
        let firsts = |groups: &librespot::metadata::artist::AlbumGroups| -> Vec<SpotifyUri> {
            groups.0.iter().filter_map(|g| g.0.0.first().cloned()).take(40).collect()
        };
        let albums = self.album_cards(firsts(&a.albums)).await;
        let singles = self.album_cards(firsts(&a.singles)).await;
        let image = best_image(&a.portraits)
            .or_else(|| best_image(&a.portrait_group))
            .or_else(|| albums.first().and_then(|al| al.image.clone()))
            .or_else(|| top.first().and_then(|t| t.image_large.clone()));
        Ok(ArtistPage { uri: uri.to_string(), name: a.name, image, top, albums, singles })
    }

    async fn album_cards(&self, ids: Vec<SpotifyUri>) -> Vec<AlbumCard> {
        let session = self.session.clone();
        let mut cards: Vec<AlbumCard> = stream::iter(ids)
            .map(move |id| {
                let session = session.clone();
                async move {
                    let al = LsAlbum::get(&session, &id).await.ok()?;
                    Some(AlbumCard {
                        uri: id.to_uri().ok()?,
                        name: al.name,
                        year: al.date.year(),
                        image: best_image(&al.covers),
                    })
                }
            })
            .buffered(8)
            .filter_map(|c| async move { c })
            .collect()
            .await;
        cards.sort_by(|a, b| b.year.cmp(&a.year)); // newest first, like Spotify
        cards
    }

    /// An album with all its tracks in order.
    pub async fn album(&self, uri: &str) -> Result<AlbumPage> {
        let id = SpotifyUri::from_uri(uri).map_err(|e| anyhow!("bad album: {e}"))?;
        let al = LsAlbum::get(&self.session, &id)
            .await
            .map_err(|e| anyhow!("could not load the album: {e}"))?;
        let uris: Vec<String> = al
            .discs
            .0
            .iter()
            .flat_map(|d| d.tracks.0.iter())
            .filter_map(|u| u.to_uri().ok())
            .collect();
        let tracks = self.tracks(&uris).await?;
        Ok(AlbumPage {
            uri: uri.to_string(),
            name: al.name,
            artists: artist_refs(&al.artists),
            year: al.date.year(),
            image: best_image(&al.covers),
            tracks,
        })
    }

    /// Adds a track to the end of one of your playlists.
    pub async fn playlist_add(&self, playlist_id: &str, track_uri: &str) -> Result<()> {
        let op = Op {
            kind: Some(EnumOrUnknown::new(op::Kind::ADD)),
            add: MessageField::some(Add {
                items: vec![Item { uri: Some(track_uri.to_string()), ..Default::default() }],
                add_last: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        self.playlist_change(playlist_id, op).await
    }

    /// Removes every copy of a track from one of your playlists.
    pub async fn playlist_remove(&self, playlist_id: &str, track_uri: &str) -> Result<()> {
        let pl = self.playlist_raw(playlist_id).await?;
        // remove from the end so earlier indexes stay valid
        let positions: Vec<usize> = pl
            .contents
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.uri() == track_uri)
            .map(|(n, _)| n)
            .rev()
            .collect();
        if positions.is_empty() {
            return Ok(());
        }
        for n in positions {
            let op = Op {
                kind: Some(EnumOrUnknown::new(op::Kind::REM)),
                rem: MessageField::some(Rem {
                    from_index: Some(n as i32),
                    length: Some(1),
                    items: vec![Item { uri: Some(track_uri.to_string()), ..Default::default() }],
                    ..Default::default()
                }),
                ..Default::default()
            };
            self.playlist_change(playlist_id, op).await?;
        }
        Ok(())
    }

    /// Sends one change to a playlist, based on its current revision (like the official app).
    async fn playlist_change(&self, playlist_id: &str, op: Op) -> Result<()> {
        let pl = self.playlist_raw(playlist_id).await?;
        let revision = pl.revision().to_vec();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let changes = ListChanges {
            base_revision: Some(revision.clone()),
            deltas: vec![Delta {
                base_version: Some(revision),
                ops: vec![op],
                info: MessageField::some(ChangeInfo {
                    user: Some(self.session.username()),
                    timestamp: Some(now_ms),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            want_resulting_revisions: Some(true),
            ..Default::default()
        };
        let body = changes.write_to_bytes()?;
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/x-protobuf"),
        );
        self.session
            .spclient()
            .request(&Method::POST, &format!("/playlist/v2/playlist/{playlist_id}/changes"), Some(headers), Some(&body))
            .await
            .map_err(|e| anyhow!("Spotify did not accept the playlist change: {e}"))?;
        Ok(())
    }

    /// Adds a track to Liked Songs (collection v2 write, protobuf encoded by hand).
    pub async fn save_track(&self, id: &str) -> Result<()> {
        self.set_saved(id, true).await
    }

    /// Adds a track to Liked Songs, or removes it (`saved == false`).
    pub async fn set_saved(&self, id: &str, saved: bool) -> Result<()> {
        let user = self.session.username();
        let added_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut item = Vec::new();
        pb_string(&mut item, 1, &format!("spotify:track:{id}"));
        pb_varint_field(&mut item, 2, added_at);
        if !saved {
            pb_varint_field(&mut item, 3, 1); // is_removed
        }
        let mut body = Vec::new();
        pb_string(&mut body, 1, &user);
        pb_string(&mut body, 2, "collection");
        pb_bytes(&mut body, 3, &item);

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/vnd.collection-v2.spotify.proto"),
        );
        self.session
            .spclient()
            .request(&Method::POST, "/collection/v2/write", Some(headers), Some(&body))
            .await
            .map_err(|e| anyhow!("could not add to Liked Songs: {e}"))
            .context("save track")?;
        Ok(())
    }

    /// Time-synced (when available) lyrics of a track. `None` if Spotify has none.
    pub async fn lyrics(&self, track_id: &str) -> Result<Option<Vec<LyricLine>>> {
        let id = SpotifyId::from_base62(track_id).map_err(|e| anyhow!("bad track id: {e}"))?;
        match Lyrics::get(&self.session, &id).await {
            Ok(l) => Ok(Some(
                l.lyrics
                    .lines
                    .into_iter()
                    .map(|line| LyricLine {
                        start_ms: line.start_time_ms.parse().unwrap_or(0),
                        words: line.words,
                    })
                    .filter(|l| !l.words.trim().is_empty() && l.words.trim() != "♪")
                    .collect(),
            )),
            Err(_) => Ok(None), // most often: no lyrics for this track
        }
    }
}

fn collect_page(page: &Value, uris: &mut Vec<String>, pending: &mut Vec<String>) {
    if let Some(tracks) = page["tracks"].as_array() {
        uris.extend(tracks.iter().filter_map(|t| t["uri"].as_str().map(String::from)));
    } else if let Some(url) = page["page_url"].as_str() {
        pending.insert(0, url.to_string());
    }
    if let Some(next) = page["next_page_url"].as_str() {
        pending.insert(0, next.to_string());
    }
}

fn convert_track(uri: &str, t: &LsTrack) -> Track {
    let covers = &t.album.covers.0;
    let pick = |w: i32| {
        covers
            .iter()
            .find(|c| c.width == w)
            .or_else(|| covers.iter().max_by_key(|c| c.width))
            .and_then(|c| c.id.to_base16().ok())
            .map(|h| image_url(&h))
    };
    Track {
        id: uri.trim_start_matches("spotify:track:").to_string(),
        uri: uri.to_string(),
        name: t.name.clone(),
        artists: t
            .artists
            .0
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(", "),
        album: t.album.name.clone(),
        duration_ms: t.duration.max(0) as u64,
        image: pick(300),
        image_large: pick(640),
        artist_list: artist_refs(&t.artists),
        album_uri: t.album.id.to_uri().ok(),
    }
}

fn artist_refs(artists: &librespot::metadata::artist::Artists) -> Vec<ArtistRef> {
    artists
        .0
        .iter()
        .filter_map(|a| Some(ArtistRef { name: a.name.clone(), uri: a.id.to_uri().ok()? }))
        .collect()
}

/// Largest picture of a set, as an image URL.
fn best_image(images: &librespot::metadata::image::Images) -> Option<String> {
    images
        .0
        .iter()
        .max_by_key(|i| i.width)
        .and_then(|i| i.id.to_base16().ok())
        .map(|h| image_url(&h))
}

// ---- minimal protobuf encoding for the collection write ----

fn pb_varint(buf: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        buf.push((v as u8) | 0x80);
        v >>= 7;
    }
    buf.push(v as u8);
}

fn pb_varint_field(buf: &mut Vec<u8>, field: u32, v: u64) {
    pb_varint(buf, (field as u64) << 3);
    pb_varint(buf, v);
}

fn pb_bytes(buf: &mut Vec<u8>, field: u32, data: &[u8]) {
    pb_varint(buf, ((field as u64) << 3) | 2);
    pb_varint(buf, data.len() as u64);
    buf.extend_from_slice(data);
}

fn pb_string(buf: &mut Vec<u8>, field: u32, s: &str) {
    pb_bytes(buf, field, s.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_encoding() {
        assert_eq!(encode_query("pink sky"), "pink+sky");
        assert_eq!(encode_query("a&b/ü"), "a%26b%2F%C3%BC");
    }

    #[test]
    fn protobuf_varint() {
        let mut b = Vec::new();
        pb_varint(&mut b, 300);
        assert_eq!(b, [0xAC, 0x02]);
    }
}
