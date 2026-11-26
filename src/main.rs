//! ♡ femboy spotify ♡ — the desktop app (Tauri window, pink web UI in `ui/`).
//!
//! Music plays in the official Spotify app on any of the user's devices; this app
//! is a pretty remote for it (Spotify Connect), see `remote.rs`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use femboy_spotify::{
    api::{Api, LyricLine, Me, Playlist, Track, encode_query},
    auth::{self, Login},
    remote::{Remote, RemoteState},
};
use librespot::core::session::Session;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;

/// Everything that exists only after a successful login.
struct Core {
    api: Api,
    remote: Remote,
    session: Session,
}

#[derive(Default)]
struct AppState {
    core: RwLock<Option<Arc<Core>>>,
}

type CmdResult<T> = Result<T, String>;

/// Turns an error into a message for the UI and appends it to the log file.
fn err(e: impl std::fmt::Display) -> String {
    let msg = format!("{e:#}");
    log_line(&format!("error: {msg}"));
    msg
}

fn log_line(line: &str) {
    auth::log(line);
}

#[tauri::command]
fn log_file() -> String {
    auth::log_path().display().to_string()
}

impl AppState {
    async fn core(&self) -> CmdResult<Arc<Core>> {
        self.core
            .read()
            .await
            .clone()
            .ok_or_else(|| "not logged in".to_string())
    }

    async fn start(&self, app: &AppHandle, login: Login) -> CmdResult<()> {
        let app = app.clone();
        let remote = Remote::start(login.session.clone(), move |st: RemoteState| {
            let _ = app.emit("remote", st);
        })
        .await
        .map_err(err)?;
        *self.core.write().await = Some(Arc::new(Core {
            api: Api::new(&login),
            remote,
            session: login.session,
        }));
        Ok(())
    }
}

// ---------- login ----------

/// Tries the saved login. `true` when we're in.
#[tauri::command]
async fn init(app: AppHandle, state: State<'_, AppState>) -> CmdResult<bool> {
    if state.core.read().await.is_some() {
        return Ok(true);
    }
    match auth::connect_cached().await.map_err(err)? {
        Some(login) => {
            state.start(&app, login).await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Opens the browser for Spotify sign-in.
#[tauri::command]
async fn login(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    log_line("login: opening browser for Spotify sign-in");
    let login = auth::login_interactive().await.map_err(err)?;
    log_line("login: signed in, joining Spotify Connect");
    state.start(&app, login).await
}

#[tauri::command]
async fn logout(state: State<'_, AppState>) -> CmdResult<()> {
    state.core.write().await.take();
    auth::logout().map_err(err)
}

// ---------- library ----------

#[tauri::command]
async fn me(state: State<'_, AppState>) -> CmdResult<Me> {
    state.core().await?.api.me().await.map_err(err)
}

#[tauri::command]
async fn liked(state: State<'_, AppState>) -> CmdResult<Vec<Track>> {
    state.core().await?.api.liked().await.map_err(err)
}

#[tauri::command]
async fn playlists(state: State<'_, AppState>) -> CmdResult<Vec<Playlist>> {
    state.core().await?.api.playlists().await.map_err(err)
}

#[tauri::command]
async fn playlist_tracks(state: State<'_, AppState>, id: String) -> CmdResult<Vec<Track>> {
    state.core().await?.api.playlist_tracks(&id).await.map_err(err)
}

#[tauri::command]
async fn search(state: State<'_, AppState>, q: String) -> CmdResult<Vec<Track>> {
    state.core().await?.api.search(&q).await.map_err(err)
}

#[tauri::command]
async fn track_info(state: State<'_, AppState>, uri: String) -> CmdResult<Option<Track>> {
    state.core().await?.api.track(&uri).await.map_err(err)
}

#[tauri::command]
async fn save_track(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.core().await?.api.save_track(&id).await.map_err(err)
}

#[tauri::command]
async fn lyrics(state: State<'_, AppState>, id: String) -> CmdResult<Option<Vec<LyricLine>>> {
    state.core().await?.api.lyrics(&id).await.map_err(err)
}

// ---------- remote playback (Spotify Connect) ----------

#[tauri::command]
async fn remote_state(state: State<'_, AppState>) -> CmdResult<RemoteState> {
    Ok(state.core().await?.remote.state())
}

/// `context`: "liked", "search:<query>", a spotify: URI, or null to send `uris` as the queue.
#[tauri::command]
async fn play(
    state: State<'_, AppState>,
    context: Option<String>,
    uris: Vec<String>,
    index: usize,
    device: Option<String>,
) -> CmdResult<()> {
    let core = state.core().await?;
    let context = context.map(|c| {
        if c == "liked" {
            format!("spotify:user:{}:collection", core.session.username())
        } else if let Some(q) = c.strip_prefix("search:") {
            format!("spotify:search:{}", encode_query(q))
        } else {
            c
        }
    });
    core.remote
        .play(context, &uris, index, device.as_deref())
        .await
        .map_err(err)
}

/// pause, resume, skip_next, skip_prev
#[tauri::command]
async fn control(state: State<'_, AppState>, action: String) -> CmdResult<()> {
    let endpoint = match action.as_str() {
        "pause" | "resume" | "skip_next" | "skip_prev" => action.as_str(),
        _ => return Err(format!("unknown action {action}")),
    };
    state.core().await?.remote.simple(endpoint).await.map_err(err)
}

#[tauri::command]
async fn seek(state: State<'_, AppState>, ms: u64) -> CmdResult<()> {
    state.core().await?.remote.seek(ms).await.map_err(err)
}

/// shuffle, repeat_context, repeat_track
#[tauri::command]
async fn set_option(state: State<'_, AppState>, name: String, value: bool) -> CmdResult<()> {
    let endpoint = match name.as_str() {
        "shuffle" => "set_shuffling_context",
        "repeat_context" => "set_repeating_context",
        "repeat_track" => "set_repeating_track",
        _ => return Err(format!("unknown option {name}")),
    };
    state.core().await?.remote.set_option(endpoint, value).await.map_err(err)
}

#[tauri::command]
async fn set_volume(state: State<'_, AppState>, volume: u16) -> CmdResult<()> {
    state.core().await?.remote.set_volume(volume).await.map_err(err)
}

#[tauri::command]
async fn transfer(state: State<'_, AppState>, device: String) -> CmdResult<()> {
    state.core().await?.remote.transfer(&device).await.map_err(err)
}

fn main() {
    auth::init_runtime();
    auth::init_file_log();
    log_line(concat!("femboy spotify ", env!("CARGO_PKG_VERSION"), " started"));
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            init,
            login,
            log_file,
            logout,
            me,
            liked,
            playlists,
            playlist_tracks,
            search,
            track_info,
            save_track,
            lyrics,
            remote_state,
            play,
            control,
            seek,
            set_option,
            set_volume,
            transfer,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start femboy spotify");
}
