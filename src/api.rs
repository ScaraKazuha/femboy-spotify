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
    playlist4_external::{
        Add, ChangeInfo, CreateListReply, Delta, Item, ListAttributes, ListAttributesPartialState, ListChanges,
        ListUpdateRequest, Op, Rem, SelectedListContent, UpdateListAttributes, op,
    },
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
    /// when it was added to Liked Songs / the playlist (ms since 1970)
    pub added_at: Option<i64>,
}

/// A playlist, album or artist shown as a card (history, search results).
#[derive(Clone, Debug, Default, Serialize)]
pub struct Card {
    pub uri: String,
    /// "playlist", "album" or "artist"
    pub kind: String,
    pub name: String,
    pub sub: String,
    pub image: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub artists: Vec<Card>,
    pub albums: Vec<Card>,
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
        Ok(self.context_items(context, limit).await?.into_iter().map(|(u, _)| u).collect())
    }

    /// Like `context_uris`, with each track's "added at" time when the context has one.
    async fn context_items(&self, context: &str, limit: usize) -> Result<Vec<(String, Option<i64>)>> {
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

    /// Info for many tracks by uri, in order (e.g. the play queue).
    pub async fn tracks_by_uri(&self, uris: &[String]) -> Result<Vec<Track>> {
        self.tracks(uris).await
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
        let items = self
            .context_items(&format!("spotify:user:{user}:collection"), MAX_ITEMS)
            .await?;
        self.tracks_with_dates(items).await
    }

    async fn tracks_with_dates(&self, items: Vec<(String, Option<i64>)>) -> Result<Vec<Track>> {
        let uris: Vec<String> = items.iter().map(|(u, _)| u.clone()).collect();
        let dates: HashMap<String, i64> = items.into_iter().filter_map(|(u, d)| Some((u, d?))).collect();
        let mut tracks = self.tracks(&uris).await?;
        for t in &mut tracks {
            t.added_at = dates.get(&t.uri).copied();
        }
        Ok(tracks)
    }

    /// Where you listened lately (playlists, albums, artists), newest first, from Spotify.
    pub async fn recent(&self) -> Result<Vec<Card>> {
        let user = self.session.username();
        let v = self
            .json(&format!("/recently-played/v3/user/{user}/recently-played?format=json&limit=30&filter=default"))
            .await?;
        let uris: Vec<String> = v["playContexts"]
            .as_array()
            .map(|a| a.iter().filter_map(|c| c["uri"].as_str().map(String::from)).collect())
            .unwrap_or_default();
        let api = self.clone();
        Ok(stream::iter(uris)
            .map(move |u| {
                let api = api.clone();
                async move { api.card(&u).await }
            })
            .buffered(8)
            .filter_map(|c| async move { c })
            .collect()
            .await)
    }

    /// Name and picture of a playlist, album or artist.
    pub async fn card(&self, uri: &str) -> Option<Card> {
        let id = SpotifyUri::from_uri(uri).ok()?;
        if let Some(pid) = uri.strip_prefix("spotify:playlist:") {
            let pl = self.playlist_raw(pid).await.ok()?;
            if pl.length() <= 0 {
                return None;
            }
            let picture = pl.attributes.picture();
            let mut image = (!picture.is_empty()).then(|| image_url(&hex(picture)));
            if image.is_none() {
                let first = pl.contents.items.iter().find(|i| i.uri().starts_with("spotify:track:"))?;
                image = self.track(first.uri()).await.ok().flatten().and_then(|t| t.image);
            }
            let owner = pl.owner_username();
            return Some(Card {
                uri: uri.into(),
                kind: "playlist".into(),
                name: pl.attributes.name().to_string(),
                sub: if owner == "spotify" { "Spotify".into() } else { "playlist".into() },
                image,
            });
        }
        if uri.starts_with("spotify:album:") {
            let al = LsAlbum::get(&self.session, &id).await.ok()?;
            let by = al.artists.0.iter().map(|a| a.name.clone()).collect::<Vec<_>>().join(", ");
            return Some(Card { uri: uri.into(), kind: "album".into(), name: al.name, sub: by, image: best_image(&al.covers) });
        }
        if uri.starts_with("spotify:artist:") {
            let a = LsArtist::get(&self.session, &id).await.ok()?;
            let image = best_image(&a.portraits).or_else(|| best_image(&a.portrait_group));
            return Some(Card { uri: uri.into(), kind: "artist".into(), name: a.name, sub: "artist".into(), image });
        }
        None
    }

    /// Songs plus the artists and albums they come from (Spotify's full search
    /// service isn't reachable for third-party clients, so those come from the songs).
    pub async fn search_all(&self, query: &str) -> Result<SearchResults> {
        let tracks = self.search(query).await?;
        // artists with the most songs in the results first (the top result)
        let mut counts: Vec<(String, usize)> = Vec::new();
        let mut album_uris: Vec<String> = Vec::new();
        for t in &tracks {
            for a in &t.artist_list {
                match counts.iter_mut().find(|(u, _)| *u == a.uri) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((a.uri.clone(), 1)),
                }
            }
            if let Some(al) = &t.album_uri {
                if !album_uris.contains(al) {
                    album_uris.push(al.clone());
                }
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1)); // stable: ties keep result order
        let artist_uris: Vec<String> = counts.into_iter().map(|(u, _)| u).collect();
        let api = self.clone();
        let cards = |uris: Vec<String>, n: usize| {
            let api = api.clone();
            async move {
                stream::iter(uris.into_iter().take(n))
                    .map(move |u| {
                        let api = api.clone();
                        async move { api.card(&u).await }
                    })
                    .buffered(8)
                    .filter_map(|c| async move { c })
                    .collect::<Vec<Card>>()
                    .await
            }
        };
        let (artists, albums) = futures_util::join!(cards(artist_uris, 12), cards(album_uris, 16));
        Ok(SearchResults { tracks, artists, albums })
    }

    /// Every uri in one of your collection sets: "artist" (followed artists),
    /// "collection" (Liked Songs and saved albums).
    pub async fn collection_set(&self, set: &str) -> Result<Vec<String>> {
        let user = self.session.username();
        let mut out = Vec::new();
        let mut token = String::new();
        for _ in 0..50 {
            let mut req = Vec::new();
            pb_string(&mut req, 1, &user);
            pb_string(&mut req, 2, set);
            if !token.is_empty() {
                pb_string(&mut req, 3, &token);
            }
            pb_varint_field(&mut req, 4, 300);
            let bytes = self
                .session
                .spclient()
                .request(&Method::POST, "/collection/v2/paging", Some(collection_headers()), Some(&req))
                .await
                .map_err(|e| anyhow!("could not read your library: {e}"))?;
            token.clear();
            for (field, value) in pb_fields(&bytes) {
                match (field, value) {
                    (1, PbValue::Bytes(item)) => {
                        let mut uri = None;
                        let mut removed = false;
                        for (f, v) in pb_fields(&item) {
                            match (f, v) {
                                (1, PbValue::Bytes(u)) => uri = String::from_utf8(u).ok(),
                                (3, PbValue::Varint(r)) => removed = r != 0,
                                _ => {}
                            }
                        }
                        if let (Some(u), false) = (uri, removed) {
                            out.push(u);
                        }
                    }
                    (2, PbValue::Bytes(t)) => token = String::from_utf8(t).unwrap_or_default(),
                    _ => {}
                }
            }
            if token.is_empty() {
                break;
            }
        }
        Ok(out)
    }

    /// Adds `uri` to a collection set, or removes it (`on == false`).
    pub async fn collection_write(&self, set: &str, uri: &str, on: bool) -> Result<()> {
        let added_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut item = Vec::new();
        pb_string(&mut item, 1, uri);
        pb_varint_field(&mut item, 2, added_at);
        if !on {
            pb_varint_field(&mut item, 3, 1); // is_removed
        }
        let mut body = Vec::new();
        pb_string(&mut body, 1, &self.session.username());
        pb_string(&mut body, 2, set);
        pb_bytes(&mut body, 3, &item);
        self.session
            .spclient()
            .request(&Method::POST, "/collection/v2/write", Some(collection_headers()), Some(&body))
            .await
            .map_err(|e| anyhow!("Spotify did not accept the library change: {e}"))?;
        Ok(())
    }

    /// Makes a new empty playlist and puts it at the top of your library. Returns its id.
    pub async fn playlist_create(&self, name: &str) -> Result<String> {
        let req = ListUpdateRequest {
            attributes: MessageField::some(ListAttributes { name: Some(name.to_string()), ..Default::default() }),
            info: MessageField::some(self.change_info()),
            ..Default::default()
        };
        let bytes = self
            .session
            .spclient()
            .request(&Method::POST, "/playlist/v2/playlist", Some(protobuf_headers()), Some(&req.write_to_bytes()?))
            .await
            .map_err(|e| anyhow!("Spotify did not create the playlist: {e}"))?;
        let reply = CreateListReply::parse_from_bytes(&bytes)?;
        let uri = reply.uri().to_string();
        let op = Op {
            kind: Some(EnumOrUnknown::new(op::Kind::ADD)),
            add: MessageField::some(Add {
                from_index: Some(0),
                items: vec![Item { uri: Some(uri.clone()), ..Default::default() }],
                add_first: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        self.rootlist_change(op).await?;
        Ok(uri.trim_start_matches("spotify:playlist:").to_string())
    }

    pub async fn playlist_rename(&self, playlist_id: &str, name: &str) -> Result<()> {
        let op = Op {
            kind: Some(EnumOrUnknown::new(op::Kind::UPDATE_LIST_ATTRIBUTES)),
            update_list_attributes: MessageField::some(UpdateListAttributes {
                new_attributes: MessageField::some(ListAttributesPartialState {
                    values: MessageField::some(ListAttributes { name: Some(name.to_string()), ..Default::default() }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        self.playlist_change(playlist_id, op).await
    }

    /// Takes a playlist out of your library (what "Delete" does for your own ones).
    pub async fn playlist_unfollow(&self, playlist_id: &str) -> Result<()> {
        let uri = format!("spotify:playlist:{playlist_id}");
        let root = self.rootlist_raw().await?;
        let Some(index) = root.contents.items.iter().position(|i| i.uri() == uri) else {
            return Ok(());
        };
        let op = Op {
            kind: Some(EnumOrUnknown::new(op::Kind::REM)),
            rem: MessageField::some(Rem {
                from_index: Some(index as i32),
                length: Some(1),
                items: vec![Item { uri: Some(uri), ..Default::default() }],
                ..Default::default()
            }),
            ..Default::default()
        };
        self.rootlist_change(op).await
    }

    async fn rootlist_raw(&self) -> Result<SelectedListContent> {
        let bytes = self
            .session
            .spclient()
            .get_rootlist(0, Some(1000))
            .await
            .map_err(|e| anyhow!("could not load your playlists: {e}"))?;
        Ok(SelectedListContent::parse_from_bytes(&bytes)?)
    }

    async fn rootlist_change(&self, op: Op) -> Result<()> {
        let revision = self.rootlist_raw().await?.revision().to_vec();
        let user = self.session.username();
        self.send_changes(&format!("/playlist/v2/user/{user}/rootlist/changes"), revision, op).await
    }

    fn change_info(&self) -> ChangeInfo {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        ChangeInfo { user: Some(self.session.username()), timestamp: Some(now_ms), ..Default::default() }
    }

    async fn send_changes(&self, endpoint: &str, revision: Vec<u8>, op: Op) -> Result<()> {
        let changes = ListChanges {
            base_revision: Some(revision.clone()),
            deltas: vec![Delta {
                base_version: Some(revision),
                ops: vec![op],
                info: MessageField::some(self.change_info()),
                ..Default::default()
            }],
            want_resulting_revisions: Some(true),
            ..Default::default()
        };
        self.session
            .spclient()
            .request(&Method::POST, endpoint, Some(protobuf_headers()), Some(&changes.write_to_bytes()?))
            .await
            .map_err(|e| anyhow!("Spotify did not accept the playlist change: {e}"))?;
        Ok(())
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
            // empty Spotify pseudo-playlists (the "DJ" entry) are hidden; your own
            // new, still-empty playlists are not
            if pl.length() <= 0 && pl.owner_username() != me {
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
        let items = pl
            .contents
            .items
            .iter()
            .map(|i| (i.uri().to_string(), i.attributes.timestamp))
            .collect();
        self.tracks_with_dates(items).await
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
        let revision = self.playlist_raw(playlist_id).await?.revision().to_vec();
        self.send_changes(&format!("/playlist/v2/playlist/{playlist_id}/changes"), revision, op).await
    }

    /// Adds a track to Liked Songs.
    pub async fn save_track(&self, id: &str) -> Result<()> {
        self.set_saved(id, true).await
    }

    /// Adds a track to Liked Songs, or removes it (`saved == false`).
    pub async fn set_saved(&self, id: &str, saved: bool) -> Result<()> {
        self.collection_write("collection", &format!("spotify:track:{id}"), saved)
            .await
            .context("Liked Songs")
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

fn collect_page(page: &Value, uris: &mut Vec<(String, Option<i64>)>, pending: &mut Vec<String>) {
    if let Some(tracks) = page["tracks"].as_array() {
        uris.extend(tracks.iter().filter_map(|t| {
            let added = t["metadata"]["added_at"].as_str().and_then(|s| s.parse().ok());
            t["uri"].as_str().map(|u| (u.to_string(), added))
        }));
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
        added_at: None,
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

enum PbValue {
    Varint(u64),
    Bytes(Vec<u8>),
}

fn pb_read_varint(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *data.get(*pos)?;
        *pos += 1;
        v |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// The top-level fields of a protobuf message (varints and length-delimited only;
/// stops at anything else, which the collection replies don't use).
fn pb_fields(data: &[u8]) -> Vec<(u32, PbValue)> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let Some(key) = pb_read_varint(data, &mut pos) else { break };
        let field = (key >> 3) as u32;
        match key & 7 {
            0 => match pb_read_varint(data, &mut pos) {
                Some(v) => out.push((field, PbValue::Varint(v))),
                None => break,
            },
            2 => {
                let Some(len) = pb_read_varint(data, &mut pos) else { break };
                let end = pos + len as usize;
                let Some(bytes) = data.get(pos..end) else { break };
                out.push((field, PbValue::Bytes(bytes.to_vec())));
                pos = end;
            }
            _ => break,
        }
    }
    out
}

fn collection_headers() -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/vnd.collection-v2.spotify.proto"),
    );
    h
}

fn protobuf_headers() -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(reqwest::header::CONTENT_TYPE, reqwest::header::HeaderValue::from_static("application/x-protobuf"));
    h
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
