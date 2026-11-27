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
use tauri::{
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent, webview::PageLoadEvent,
};
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

/// Starts Spotify DJ ("DJ X") on the active (or given) device.
#[tauri::command]
async fn play_dj(state: State<'_, AppState>, device: Option<String>) -> CmdResult<()> {
    state
        .core()
        .await?
        .remote
        .play_context(femboy_spotify::remote::DJ_URI, device.as_deref())
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

// ---------- built-in player ----------
//
// Spotify refuses audio keys to librespot, so the app carries Spotify's own web
// player in a hidden window instead. WebView2 has Edge's DRM, so it plays fine, and
// it shows up as a Spotify Connect device ("Web Player (Microsoft Edge)") that the
// remote above drives like any other. The page gets no IPC: capabilities only cover
// the "main" window.

const PLAYER: &str = "player";
const PLAYER_HOME: &str = "https://open.spotify.com/";
const PLAYER_LOGIN: &str =
    "https://accounts.spotify.com/login?continue=https%3A%2F%2Fopen.spotify.com%2F";
/// Every webview of the app shares one WebView2 environment, so `tauri.conf.json`
/// gives the main window exactly the same arguments. The first part is Tauri's
/// default; the rest lets the hidden player start music without a click and keeps
/// it from being throttled while hidden.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
     --autoplay-policy=no-user-gesture-required --disable-background-timer-throttling \
     --disable-renderer-backgrounding --disable-backgrounding-occluded-windows";

/// Sites the sign-in window may visit (Spotify and its "continue with" providers).
fn player_may_visit(url: &tauri::Url) -> bool {
    let host = url.host_str().unwrap_or_default();
    ["spotify.com", "google.com", "apple.com", "facebook.com", "gstatic.com"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

fn player_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(PLAYER) {
        return Ok(w);
    }
    let w = WebviewWindowBuilder::new(app, PLAYER, WebviewUrl::External(PLAYER_HOME.parse().unwrap()))
        .title("Femboy Spotify ♡ sign in to the built-in player")
        .inner_size(520.0, 760.0)
        .center()
        .visible(false)
        .skip_taskbar(true)
        .additional_browser_args(BROWSER_ARGS)
        .on_navigation(player_may_visit)
        .on_page_load(|w, p| {
            if p.event() != PageLoadEvent::Finished
                || p.url().host_str() != Some("open.spotify.com")
            {
                return;
            }
            // cookies can't be read on the UI thread (it would deadlock on Windows)
            tauri::async_runtime::spawn(async move {
                if player_signed_in(&w) && w.is_visible().unwrap_or(false) {
                    log_line("player: signed in");
                    let _ = w.hide();
                    let _ = w.set_skip_taskbar(true);
                    let _ = w.app_handle().emit("player", true);
                    if let Some(main) = w.app_handle().get_webview_window("main") {
                        let _ = main.set_focus();
                    }
                }
            });
        })
        .build()?;
    set_big_icon(&w);
    Ok(w)
}

/// Spotify's web login cookie.
fn player_signed_in(w: &WebviewWindow) -> bool {
    w.cookies_for_url("https://open.spotify.com/".parse().unwrap())
        .map(|c| c.iter().any(|c| c.name() == "sp_dc"))
        .unwrap_or(false)
}

/// `true` when the built-in player is signed in (async: reading cookies must not block the UI thread).
#[tauri::command]
async fn player_status(app: AppHandle) -> CmdResult<bool> {
    let w = player_window(&app).map_err(err)?;
    Ok(player_signed_in(&w))
}

/// Shows the player window on Spotify's sign-in page; it hides itself once signed in.
#[tauri::command]
async fn player_login(app: AppHandle) -> CmdResult<()> {
    let w = player_window(&app).map_err(err)?;
    log_line("player: showing sign-in");
    w.navigate(PLAYER_LOGIN.parse().unwrap()).map_err(err)?;
    w.set_skip_taskbar(false).map_err(err)?;
    w.show().map_err(err)?;
    w.set_focus().map_err(err)
}

/// Tauri only sets the small window icon, so the taskbar shows Windows' blank one.
/// Load the big icon from the exe's own icon resource (id 32512, set by tauri-build).
#[cfg(windows)]
fn set_big_icon(window: &WebviewWindow) {
    use std::ffi::c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn LoadImageW(inst: *mut c_void, name: *const u16, kind: u32, cx: i32, cy: i32, flags: u32) -> *mut c_void;
        fn SendMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn GetSystemMetrics(index: i32) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }
    const WM_SETICON: u32 = 0x0080;
    const IMAGE_ICON: u32 = 1;
    let Ok(hwnd) = window.hwnd() else { return };
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        // (ICON_BIG, SM_CXICON, SM_CYICON), (ICON_SMALL, SM_CXSMICON, SM_CYSMICON)
        for (kind, cx, cy) in [(1usize, 11, 12), (0, 49, 50)] {
            let icon = LoadImageW(module, 32512 as *const u16, IMAGE_ICON, GetSystemMetrics(cx), GetSystemMetrics(cy), 0);
            if !icon.is_null() {
                SendMessageW(hwnd.0, WM_SETICON, kind, icon as isize);
            }
        }
    }
}

#[cfg(not(windows))]
fn set_big_icon(_window: &WebviewWindow) {}

// ---------- auto-update from GitHub Releases ----------

#[derive(serde::Serialize)]
struct UpdateInfo {
    current: String,
    version: String,
    notes: Option<String>,
}

/// Looks at the latest GitHub release; `Some` when a newer signed version exists.
#[tauri::command]
async fn check_update(app: AppHandle) -> CmdResult<Option<UpdateInfo>> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app.updater().map_err(err)?.check().await.map_err(err)?;
    if let Some(u) = &update {
        log_line(&format!("update: {} -> {} available", u.current_version, u.version));
    }
    Ok(update.map(|u| UpdateInfo {
        current: u.current_version.clone(),
        version: u.version.clone(),
        notes: u.body.clone(),
    }))
}

/// Downloads the update (signature checked), installs it and restarts the app.
#[tauri::command]
async fn install_update(app: AppHandle) -> CmdResult<()> {
    use tauri_plugin_updater::UpdaterExt;
    let Some(update) = app.updater().map_err(err)?.check().await.map_err(err)? else {
        return Err("you already have the newest version ♡".into());
    };
    log_line(&format!("update: installing {}", update.version));
    let progress = app.clone();
    let mut done: u64 = 0;
    update
        .download_and_install(
            move |chunk, total| {
                done += chunk as u64;
                let _ = progress.emit("update-progress", (done, total));
            },
            || {},
        )
        .await
        .map_err(err)?;
    app.restart();
}

fn main() {
    auth::init_runtime();
    auth::init_file_log();
    log_line(concat!("femboy spotify ", env!("CARGO_PKG_VERSION"), " started"));
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .setup(|app| {
            if let Some(main) = app.get_webview_window("main") {
                set_big_icon(&main);
            }
            // load the built-in player in the background right away
            if let Err(e) = player_window(app.handle()) {
                log_line(&format!("player: couldn't create window: {e}"));
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == PLAYER {
                    // closing the sign-in window only hides it; the player lives on
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.set_skip_taskbar(true);
                } else {
                    // the hidden player would keep the app alive otherwise
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            player_status,
            player_login,
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
            play_dj,
            control,
            seek,
            set_option,
            set_volume,
            transfer,
            check_update,
            install_update,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start femboy spotify");
}
