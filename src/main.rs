//! ♡ femboy spotify ♡ — the desktop app (Tauri window, pink web UI in `ui/`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use femboy_spotify::{
    api::{Api, LyricLine, Me, Playlist, Track},
    auth,
    player::Player,
};
use librespot::core::session::Session;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;

/// Everything that exists only after a successful login.
struct Core {
    api: Api,
    player: Player,
}

#[derive(Default)]
struct AppState {
    core: RwLock<Option<Arc<Core>>>,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

impl AppState {
    async fn core(&self) -> CmdResult<Arc<Core>> {
        self.core
            .read()
            .await
            .clone()
            .ok_or_else(|| "not logged in".to_string())
    }

    async fn start(&self, app: &AppHandle, session: Session, volume: u16) -> CmdResult<()> {
        let (player, mut events) = Player::new(session.clone(), volume).map_err(err)?;
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(ev) = events.recv().await {
                let _ = app.emit("player", ev);
            }
        });
        *self.core.write().await = Some(Arc::new(Core {
            api: Api::new(session),
            player,
        }));
        Ok(())
    }
}

// ---------- login ----------

/// Tries the saved login. `true` when we're in.
#[tauri::command]
async fn init(app: AppHandle, state: State<'_, AppState>, volume: u16) -> CmdResult<bool> {
    if state.core.read().await.is_some() {
        return Ok(true);
    }
    match auth::connect_cached().await.map_err(err)? {
        Some(session) => {
            state.start(&app, session, volume).await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Opens the browser for Spotify sign-in.
#[tauri::command]
async fn login(app: AppHandle, state: State<'_, AppState>, volume: u16) -> CmdResult<()> {
    let session = auth::login_interactive().await.map_err(err)?;
    state.start(&app, session, volume).await
}

#[tauri::command]
async fn logout(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(core) = state.core.write().await.take() {
        core.player.pause();
    }
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
async fn recent(state: State<'_, AppState>) -> CmdResult<Vec<Track>> {
    state.core().await?.api.recent().await.map_err(err)
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
async fn save_track(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.core().await?.api.save_track(&id).await.map_err(err)
}

#[tauri::command]
async fn lyrics(state: State<'_, AppState>, id: String) -> CmdResult<Option<Vec<LyricLine>>> {
    state.core().await?.api.lyrics(&id).await.map_err(err)
}

// ---------- playback ----------

#[tauri::command]
async fn play(state: State<'_, AppState>, uri: String) -> CmdResult<()> {
    state.core().await?.player.load(&uri).map_err(err)
}

#[tauri::command]
async fn pause(state: State<'_, AppState>) -> CmdResult<()> {
    state.core().await?.player.pause();
    Ok(())
}

#[tauri::command]
async fn resume(state: State<'_, AppState>) -> CmdResult<()> {
    state.core().await?.player.play();
    Ok(())
}

#[tauri::command]
async fn seek(state: State<'_, AppState>, ms: u32) -> CmdResult<()> {
    state.core().await?.player.seek(ms);
    Ok(())
}

#[tauri::command]
async fn set_volume(state: State<'_, AppState>, volume: u16) -> CmdResult<()> {
    state.core().await?.player.set_volume(volume);
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            init,
            login,
            logout,
            me,
            liked,
            recent,
            playlists,
            playlist_tracks,
            search,
            save_track,
            lyrics,
            play,
            pause,
            resume,
            seek,
            set_volume,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start femboy spotify");
}
