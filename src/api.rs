//! Tiny Spotify Web API client (search, liked songs, playlists, lyrics).
//!
//! Access tokens come from the librespot session, so no separate
//! developer app registration is needed.

use anyhow::{Result, anyhow};
use librespot::core::{SpotifyId, session::Session};
use librespot::metadata::Lyrics;
use reqwest::{Method, Url};
use serde::Serialize;
use serde_json::Value;

const BASE: &str = "https://api.spotify.com/v1";
const SCOPES: &str = "user-read-private,playlist-read-private,playlist-read-collaborative,\
user-library-read,user-library-modify,user-read-recently-played";

/// Upper bound on how many items we pull from one list.
const MAX_ITEMS: usize = 1000;

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
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub total: u64,
    pub image: Option<String>,
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
    http: reqwest::Client,
    session: Session,
}

impl Api {
    pub fn new(session: Session) -> Self {
        Self {
            http: reqwest::Client::new(),
            session,
        }
    }

    async fn token(&self) -> Result<String> {
        let token = self
            .session
            .token_provider()
            .get_token(SCOPES)
            .await
            .map_err(|e| anyhow!("could not get access token: {e}"))?;
        Ok(token.access_token)
    }

    async fn send(&self, method: Method, url: Url) -> Result<Value> {
        for _ in 0..3 {
            let resp = self
                .http
                .request(method.clone(), url.clone())
                .bearer_auth(self.token().await?)
                .header("content-length", "0")
                .send()
                .await?;
            let status = resp.status();
            if status.as_u16() == 429 {
                let wait = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(2)
                    .min(10);
                tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                continue;
            }
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(anyhow!("Spotify API {status}: {}", body.trim()));
            }
            let body = resp.text().await?;
            if body.trim().is_empty() {
                return Ok(Value::Null);
            }
            return Ok(serde_json::from_str(&body)?);
        }
        Err(anyhow!("Spotify API is rate limiting us, try again later"))
    }

    async fn get(&self, url: Url) -> Result<Value> {
        self.send(Method::GET, url).await
    }

    /// Follows `next` links of a paging object and collects every item.
    async fn paged(&self, first: Url) -> Result<Vec<Value>> {
        let mut items = Vec::new();
        let mut next = Some(first);
        while let Some(url) = next.take() {
            let page = self.get(url).await?;
            if let Some(arr) = page["items"].as_array() {
                items.extend(arr.iter().cloned());
            }
            if items.len() >= MAX_ITEMS {
                break;
            }
            next = page["next"].as_str().and_then(|s| Url::parse(s).ok());
        }
        Ok(items)
    }

    pub async fn me(&self) -> Result<Me> {
        let me = self.get(Url::parse(&format!("{BASE}/me"))?).await?;
        Ok(Me {
            name: me["display_name"]
                .as_str()
                .or(me["id"].as_str())
                .unwrap_or("cutie")
                .to_string(),
            avatar: pick_image(&me["images"], 160),
        })
    }

    pub async fn display_name(&self) -> Result<String> {
        Ok(self.me().await?.name)
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Track>> {
        let url = Url::parse_with_params(
            &format!("{BASE}/search"),
            &[("q", query), ("type", "track"), ("limit", "50")],
        )?;
        let page = self.get(url).await?;
        Ok(page["tracks"]["items"]
            .as_array()
            .map(|a| a.iter().filter_map(parse_track).collect())
            .unwrap_or_default())
    }

    pub async fn liked(&self) -> Result<Vec<Track>> {
        let url = Url::parse(&format!("{BASE}/me/tracks?limit=50"))?;
        Ok(self
            .paged(url)
            .await?
            .iter()
            .filter_map(|item| parse_track(&item["track"]))
            .collect())
    }

    pub async fn recent(&self) -> Result<Vec<Track>> {
        let url = Url::parse(&format!("{BASE}/me/player/recently-played?limit=50"))?;
        let page = self.get(url).await?;
        let mut seen = std::collections::HashSet::new();
        Ok(page["items"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|item| parse_track(&item["track"]))
                    .filter(|t| seen.insert(t.uri.clone()))
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn save_track(&self, id: &str) -> Result<()> {
        let url = Url::parse_with_params(&format!("{BASE}/me/tracks"), &[("ids", id)])?;
        self.send(Method::PUT, url).await?;
        Ok(())
    }

    pub async fn playlists(&self) -> Result<Vec<Playlist>> {
        let url = Url::parse(&format!("{BASE}/me/playlists?limit=50"))?;
        Ok(self
            .paged(url)
            .await?
            .iter()
            .filter_map(|p| {
                let total = p["tracks"]["total"]
                    .as_u64()
                    .or(p["items"]["total"].as_u64())
                    .unwrap_or(0);
                Some(Playlist {
                    id: p["id"].as_str()?.to_string(),
                    name: p["name"].as_str().unwrap_or("untitled").to_string(),
                    owner: p["owner"]["display_name"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    total,
                    image: pick_image(&p["images"], 300),
                })
            })
            .collect())
    }

    pub async fn playlist_tracks(&self, id: &str) -> Result<Vec<Track>> {
        // Newer API versions renamed `/tracks` to `/items`; try both.
        let mut last_err = None;
        for endpoint in ["items", "tracks"] {
            let url = Url::parse(&format!("{BASE}/playlists/{id}/{endpoint}?limit=100"))?;
            match self.paged(url).await {
                Ok(items) => {
                    return Ok(items
                        .iter()
                        .filter_map(|it| {
                            let t = it
                                .get("track")
                                .filter(|t| !t.is_null())
                                .unwrap_or(&it["item"]);
                            parse_track(t)
                        })
                        .collect());
                }
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("could not load playlist")))
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

/// Picks the smallest image at least `target` px wide (or the biggest one available).
fn pick_image(images: &Value, target: u64) -> Option<String> {
    let arr = images.as_array()?;
    let mut best: Option<(u64, &str)> = None;
    for img in arr {
        let Some(url) = img["url"].as_str() else { continue };
        let w = img["width"].as_u64().unwrap_or(0);
        best = match best {
            None => Some((w, url)),
            Some((bw, _)) if w >= target && (bw < target || w < bw) => Some((w, url)),
            Some((bw, _)) if bw < target && w > bw => Some((w, url)),
            keep => keep,
        };
    }
    best.map(|(_, u)| u.to_string())
}

fn parse_track(t: &Value) -> Option<Track> {
    let uri = t["uri"].as_str()?;
    if !uri.starts_with("spotify:track:") {
        return None; // local files, podcast episodes, etc.
    }
    let artists = t["artists"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x["name"].as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    Some(Track {
        id: uri.trim_start_matches("spotify:track:").to_string(),
        uri: uri.to_string(),
        name: t["name"].as_str().unwrap_or("?").to_string(),
        artists,
        album: t["album"]["name"].as_str().unwrap_or("").to_string(),
        duration_ms: t["duration_ms"].as_u64().unwrap_or(0),
        image: pick_image(&t["album"]["images"], 300),
        image_large: pick_image(&t["album"]["images"], 640),
    })
}
