//! ♡ femboy spotify ♡ — the desktop app (Tauri window, pink web UI in `ui/`).
//!
//! Music plays in the official Spotify app on any of the user's devices; this app
//! is a pretty remote for it (Spotify Connect), see `remote.rs`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use femboy_spotify::{
    api::{AlbumPage, Api, ArtistPage, Card, LyricLine, Me, Playlist, SearchResults, Track, encode_query},
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
    /// Spotify's own home shelves (Daily Mixes, Release Radar...), read from the built-in player.
    home: std::sync::Mutex<Option<Vec<Shelf>>>,
    /// pieces of the shelves message while it arrives through the page title
    home_parts: std::sync::Mutex<Vec<Option<String>>>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Shelf {
    title: String,
    items: Vec<ShelfItem>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ShelfItem {
    id: String,
    name: String,
    #[serde(default)]
    sub: String,
    #[serde(default)]
    image: String,
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

/// Adds to (`saved`) or removes from Liked Songs.
#[tauri::command]
async fn set_saved(state: State<'_, AppState>, id: String, saved: bool) -> CmdResult<()> {
    state.core().await?.api.set_saved(&id, saved).await.map_err(err)
}

#[tauri::command]
async fn artist_page(state: State<'_, AppState>, uri: String) -> CmdResult<ArtistPage> {
    state.core().await?.api.artist(&uri).await.map_err(err)
}

#[tauri::command]
async fn album_page(state: State<'_, AppState>, uri: String) -> CmdResult<AlbumPage> {
    state.core().await?.api.album(&uri).await.map_err(err)
}

/// Song radio: tracks of the station seeded by one song.
#[tauri::command]
async fn song_radio(state: State<'_, AppState>, id: String) -> CmdResult<Vec<Track>> {
    state
        .core()
        .await?
        .api
        .context_tracks(&format!("spotify:station:track:{id}"), 100)
        .await
        .map_err(err)
}

#[tauri::command]
async fn playlist_add(state: State<'_, AppState>, playlist: String, uri: String) -> CmdResult<()> {
    state.core().await?.api.playlist_add(&playlist, &uri).await.map_err(err)
}

#[tauri::command]
async fn playlist_remove(state: State<'_, AppState>, playlist: String, uri: String) -> CmdResult<()> {
    state.core().await?.api.playlist_remove(&playlist, &uri).await.map_err(err)
}

/// Where you listened lately, from Spotify (playlists, albums, artists).
#[tauri::command]
async fn recent_spotify(state: State<'_, AppState>) -> CmdResult<Vec<Card>> {
    state.core().await?.api.recent().await.map_err(err)
}

#[tauri::command]
async fn search_all(state: State<'_, AppState>, q: String) -> CmdResult<SearchResults> {
    state.core().await?.api.search_all(&q).await.map_err(err)
}

/// "artist" = followed artists, "collection" = Liked Songs + saved albums.
#[tauri::command]
async fn collection_set(state: State<'_, AppState>, set: String) -> CmdResult<Vec<String>> {
    if set != "artist" && set != "collection" {
        return Err(format!("unknown set {set}"));
    }
    state.core().await?.api.collection_set(&set).await.map_err(err)
}

/// Follow / unfollow an artist, save / remove an album.
#[tauri::command]
async fn set_followed(state: State<'_, AppState>, uri: String, on: bool) -> CmdResult<()> {
    let set = if uri.starts_with("spotify:artist:") {
        "artist"
    } else if uri.starts_with("spotify:album:") {
        "collection"
    } else {
        return Err(format!("can't save {uri}"));
    };
    state.core().await?.api.collection_write(set, &uri, on).await.map_err(err)
}

#[tauri::command]
async fn playlist_create(state: State<'_, AppState>, name: String) -> CmdResult<String> {
    state.core().await?.api.playlist_create(name.trim()).await.map_err(err)
}

#[tauri::command]
async fn playlist_rename(state: State<'_, AppState>, playlist: String, name: String) -> CmdResult<()> {
    state.core().await?.api.playlist_rename(&playlist, name.trim()).await.map_err(err)
}

#[tauri::command]
async fn playlist_unfollow(state: State<'_, AppState>, playlist: String) -> CmdResult<()> {
    state.core().await?.api.playlist_unfollow(&playlist).await.map_err(err)
}

#[tauri::command]
async fn tracks_info(state: State<'_, AppState>, uris: Vec<String>) -> CmdResult<Vec<Track>> {
    state.core().await?.api.tracks_by_uri(&uris).await.map_err(err)
}

#[tauri::command]
async fn clear_queue(state: State<'_, AppState>) -> CmdResult<()> {
    state.core().await?.remote.clear_queue().await.map_err(err)
}

#[tauri::command]
async fn add_to_queue(state: State<'_, AppState>, uri: String) -> CmdResult<()> {
    state.core().await?.remote.add_to_queue(&uri).await.map_err(err)
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

/// Fetches the device list from Spotify now instead of waiting for a push.
#[tauri::command]
async fn refresh_devices(state: State<'_, AppState>) -> CmdResult<RemoteState> {
    state.core().await?.remote.refresh().await.map_err(err)
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

/// Runs in the player window. On Spotify's home page it reads the playlist shelves
/// ("Made For you", "Your top mixes", ...) from the page and hands them to the app
/// through the page title in small pieces ("femboy-home:<i>/<n>:<piece>"), so the
/// web page never needs access to the app's commands.
const HOME_SCRAPER: &str = r#"
(() => {
  if (location.hostname !== 'open.spotify.com' || window.__femboyHome) return;
  window.__femboyHome = true;
  const clean = (s) => (s || '').replace(/\s+/g, ' ').trim();
  const scrape = () => {
    const shelves = [];
    for (const s of document.querySelectorAll('section[data-testid=component-shelf]')) {
      const h2 = clean(s.querySelector('h2') && s.querySelector('h2').textContent);
      const aria = clean(s.getAttribute('aria-label'));
      const title = aria && aria.length <= h2.length + 12 ? aria : h2;
      const items = [];
      for (const c of s.querySelectorAll('[data-encore-id=card]')) {
        const a = c.querySelector('a[href^="/playlist/"]');
        if (!a) continue;
        const t = c.querySelector('[data-encore-id=cardTitle]');
        const st = c.querySelector('[data-encore-id=cardSubtitle]');
        const img = c.querySelector('img');
        items.push({ id: a.getAttribute('href').split('/')[2] || '', name: clean(t && t.textContent), sub: clean(st && st.textContent), image: (img && img.src) || '' });
      }
      if (title && items.length) shelves.push({ title, items: items.slice(0, 16) });
    }
    return shelves.slice(0, 10);
  };
  let last = '', sent = '';
  const send = (json) => {
    const size = 2500, n = Math.ceil(json.length / size), orig = document.title;
    for (let i = 0; i < n; i++) setTimeout(() => { document.title = `femboy-home:${i}/${n}:` + json.slice(i * size, (i + 1) * size); }, i * 150);
    setTimeout(() => { if (document.title.startsWith('femboy-home:')) document.title = orig; }, n * 150 + 50);
  };
  const tick = () => {
    const shelves = scrape();
    const json = JSON.stringify(shelves);
    // send once the page has settled (same result twice in a row)
    if (shelves.length && json === last && json !== sent) { sent = json; send(json); }
    last = json;
  };
  setInterval(tick, 2500);
})();
"#;

/// Takes one piece of the shelves message; stores and announces the shelves once complete.
fn home_piece(app: &AppHandle, title: &str) {
    let Some(rest) = title.strip_prefix("femboy-home:") else { return };
    let Some((pos, piece)) = rest.split_once(':') else { return };
    let Some((i, n)) = pos.split_once('/') else { return };
    let (Ok(i), Ok(n)) = (i.parse::<usize>(), n.parse::<usize>()) else { return };
    if n == 0 || n > 64 || i >= n {
        return;
    }
    let state = app.state::<AppState>();
    let json = {
        let mut parts = state.home_parts.lock().unwrap();
        if parts.len() != n {
            *parts = vec![None; n];
        }
        parts[i] = Some(piece.to_string());
        if parts.iter().any(Option::is_none) {
            return;
        }
        std::mem::take(&mut *parts).into_iter().flatten().collect::<String>()
    };
    let Ok(shelves) = serde_json::from_str::<Vec<Shelf>>(&json) else {
        log_line("home: could not read Spotify's shelves");
        return;
    };
    let shelves = clean_shelves(shelves);
    log_line(&format!("home: {} shelves from Spotify", shelves.len()));
    *state.home.lock().unwrap() = Some(shelves.clone());
    let _ = app.emit("home", shelves);
}

/// The page is a web page, so only keep well-formed playlist ids and Spotify CDN images.
fn clean_shelves(shelves: Vec<Shelf>) -> Vec<Shelf> {
    let cut = |s: String, n: usize| s.chars().take(n).collect::<String>();
    let spotify_image = |u: &str| {
        tauri::Url::parse(u).is_ok_and(|u| {
            u.scheme() == "https"
                && u.host_str().is_some_and(|h| h.ends_with(".spotifycdn.com") || h.ends_with(".scdn.co"))
        })
    };
    shelves
        .into_iter()
        .take(10)
        .map(|s| Shelf {
            title: cut(s.title, 80),
            items: s
                .items
                .into_iter()
                .filter(|it| it.id.len() == 22 && it.id.chars().all(|c| c.is_ascii_alphanumeric()))
                .take(16)
                .map(|it| ShelfItem {
                    image: if spotify_image(&it.image) { it.image } else { String::new() },
                    id: it.id,
                    name: cut(it.name, 100),
                    sub: cut(it.sub, 140),
                })
                .collect(),
        })
        .filter(|s| !s.title.is_empty() && !s.items.is_empty())
        .collect()
}

/// Spotify's home shelves, if the built-in player has shown them yet.
#[tauri::command]
fn spotify_home(state: State<'_, AppState>) -> Option<Vec<Shelf>> {
    state.home.lock().unwrap().clone()
}

/// Sites the sign-in window may visit (Spotify and its "continue with" providers).
fn player_may_visit(url: &tauri::Url) -> bool {
    if url.as_str() == "about:blank" {
        return true; // used to unload the player on exit
    }
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
        // big while hidden so Spotify's home shows full shelves; sign-in shrinks it
        .inner_size(1280.0, 900.0)
        .center()
        .visible(false)
        .skip_taskbar(true)
        .additional_browser_args(BROWSER_ARGS)
        .initialization_script(HOME_SCRAPER)
        .on_document_title_changed(|w, title| home_piece(w.app_handle(), &title))
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

/// Sets the built-in player's volume (0..=100) right on its page. Instant, unlike a
/// Spotify Connect volume command, which goes through Spotify's servers and which the
/// web player often undoes when the volume goes up.
#[tauri::command]
fn player_volume(app: AppHandle, volume: u16) -> CmdResult<()> {
    let w = app.get_webview_window(PLAYER).ok_or("no built-in player")?;
    let v = f64::from(volume.min(100)) / 100.0;
    // the slider snaps to 10 % steps unless its step is relaxed while we set it
    w.eval(format!(
        "(() => {{ const i = [...document.querySelectorAll('input[type=range]')].find((x) => x.max === '1'); \
         if (!i) return; const step = i.getAttribute('step'); i.setAttribute('step', 'any'); \
         Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, '{v}'); \
         i.dispatchEvent(new Event('input', {{ bubbles: true }})); i.dispatchEvent(new Event('change', {{ bubbles: true }})); \
         if (step) i.setAttribute('step', step); }})()"
    ))
    .map_err(err)
}

/// Shows the player window on Spotify's sign-in page; it hides itself once signed in.
#[tauri::command]
async fn player_login(app: AppHandle) -> CmdResult<()> {
    let w = player_window(&app).map_err(err)?;
    log_line("player: showing sign-in");
    w.navigate(PLAYER_LOGIN.parse().unwrap()).map_err(err)?;
    w.set_size(tauri::LogicalSize::new(520.0, 760.0)).map_err(err)?;
    w.center().map_err(err)?;
    w.set_skip_taskbar(false).map_err(err)?;
    w.show().map_err(err)?;
    w.set_focus().map_err(err)
}

/// The pink UI is laid out for a 1480×880 window. Zoom the page (like the browser's
/// Ctrl+wheel) so that whole layout fits the current window: bigger on big screens,
/// smaller when the window is made small. Everything scales together, icons and
/// text included, so nothing gets cut off or squashed.
fn fit_zoom(window: &tauri::Window) {
    const DESIGN: (f64, f64) = (1480.0, 880.0);
    let (Ok(size), Ok(scale)) = (window.inner_size(), window.scale_factor()) else { return };
    if size.width == 0 || size.height == 0 {
        return; // minimized
    }
    let logical = size.to_logical::<f64>(scale);
    let zoom = (logical.width / DESIGN.0).min(logical.height / DESIGN.1).clamp(0.5, 1.75);
    let zoom = (zoom * 100.0).round() / 100.0;
    if let Some(w) = window.app_handle().get_webview_window("main") {
        let _ = w.set_zoom(zoom);
    }
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

/// Tells Windows which app this process is. The installer's Start-menu shortcut carries
/// the same AppUserModelID, so the taskbar ties our window to it and shows its icon
/// (otherwise Explorer may keep a stale blank icon for the exe path after an update).
#[cfg(windows)]
fn set_app_id() {
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SetCurrentProcessExplicitAppUserModelID(id: *const u16) -> i32;
    }
    let id: Vec<u16> = "dev.scarakazuha.femboyspotify".encode_utf16().chain([0]).collect();
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(id.as_ptr());
    }
}

#[cfg(not(windows))]
fn set_app_id() {}

fn main() {
    set_app_id();
    auth::init_runtime();
    auth::init_file_log();
    log_line(concat!("femboy spotify ", env!("CARGO_PKG_VERSION"), " started"));
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .setup(|app| {
            if let Some(main) = app.get_webview_window("main") {
                set_big_icon(&main);
                fit_zoom(&main.as_ref().window());
            }
            // load the built-in player in the background right away
            if let Err(e) = player_window(app.handle()) {
                log_line(&format!("player: couldn't create window: {e}"));
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && matches!(event, WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. })
            {
                fit_zoom(window);
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == PLAYER {
                    // closing the sign-in window only hides it; the player lives on
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.set_skip_taskbar(true);
                } else {
                    // the hidden player would keep the app alive otherwise. Unload it
                    // first so it leaves Spotify Connect instead of lingering as a dead
                    // "active" device that swallows the next play command.
                    api.prevent_close();
                    let _ = window.hide();
                    let app = window.app_handle().clone();
                    if let Some(player) = app.get_webview_window(PLAYER) {
                        let _ = player.navigate("about:blank".parse().unwrap());
                    }
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(700));
                        app.exit(0);
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            spotify_home,
            player_volume,
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
            set_saved,
            artist_page,
            album_page,
            song_radio,
            playlist_add,
            playlist_remove,
            add_to_queue,
            recent_spotify,
            search_all,
            collection_set,
            set_followed,
            playlist_create,
            playlist_rename,
            playlist_unfollow,
            clear_queue,
            tracks_info,
            lyrics,
            remote_state,
            refresh_devices,
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
