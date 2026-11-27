//! ♡ Femboy Spotify Lite ♡ — the version for slower computers.
//!
//! A small desktop window around Spotify's own web player (open.spotify.com), using
//! the WebView2 runtime that comes with Windows: Spotify's menus exactly as they are,
//! one web page instead of the main app's interface plus a hidden player. A splash
//! while it loads, a tray icon (closing the window keeps the music playing), one
//! instance only, and self-updates from this repository's releases. It draws without
//! the graphics card, which roughly halves its memory (see `BROWSER_ARGS`).
//!
//! The web page gets no access to the app: no commands, no capabilities.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod policy;

use policy::{Destination, destination};
use tauri::{
    AppHandle, Manager, Theme, WebviewUrl, WebviewWindowBuilder, WindowEvent,
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::NewWindowResponse,
    window::Color,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_updater::UpdaterExt;

const HOME: &str = "https://open.spotify.com";
const TITLE: &str = "Femboy Spotify Lite";
const SPLASH_TIME: std::time::Duration = std::time::Duration::from_millis(2500);
/// WebView2 settings for both windows (they share one WebView2, so they must match).
/// Tauri's defaults, plus drawing without the graphics card: the GPU process WebView2
/// otherwise keeps costs ~350 MB, while Spotify's page draws fine on the processor
/// (measured: 857 MB -> 495 MB for the whole app, idle CPU under 1 %).
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu";

fn restore(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn open_browser(app: &AppHandle, url: &url::Url) {
    // Never hand file:, javascript:, custom protocols or addresses with credentials
    // to the Windows shell. No shell command is built here.
    if destination(url) != Destination::Blocked {
        if let Err(error) = webbrowser::open(url.as_str()) {
            app.dialog()
                .message(format!("Could not open your browser: {error}\nOpen {HOME} manually."))
                .title(TITLE)
                .show(|_| {});
        }
    }
}

fn sign_in_help(app: &AppHandle) {
    let handle = app.clone();
    app.dialog()
        .message(
            "Femboy Spotify Lite is an unofficial app, not affiliated with Spotify.\n\n\
             It shows Spotify's own web player. Sign in with your Spotify email or username; \
             Google may refuse \"Continue with Google\" inside apps (\"this browser or app may not be secure\"), \
             which this app cannot get around: use your Spotify password, or Spotify in your browser.\n\n\
             The app keeps its own sign-in, separate from your browser and from the main Femboy Spotify.\n\n\
             Closing the window hides it to the tray and keeps the music playing. Choose Quit to stop.",
        )
        .title("About and sign-in help — Femboy Spotify Lite")
        .buttons(MessageDialogButtons::OkCancelCustom("Open in browser".into(), "Stay in app".into()))
        .show(move |open| {
            if open {
                open_browser(&handle, &HOME.parse().expect("static HTTPS URL"));
            }
        });
}

fn notify(app: &AppHandle, message: impl Into<String>) {
    app.dialog().message(message).title(TITLE).show(|_| {});
}

/// Checks the signed release feed (native code only: the web page gets no updater
/// permission). At startup it only speaks up when there is an update; a manual
/// check also says "up to date" and reports errors.
fn check_for_updates(app: AppHandle, manual: bool) {
    tauri::async_runtime::spawn(async move {
        let checked = match app.updater() {
            Ok(updater) => updater.check().await,
            Err(error) => Err(error),
        };
        match checked {
            Ok(Some(update)) => {
                let message = format!(
                    "Version {} is available (installed: {}).\n\nInstall it now? The music stops and the app restarts.",
                    update.version, update.current_version
                );
                let handle = app.clone();
                app.dialog()
                    .message(message)
                    .title(TITLE)
                    .buttons(MessageDialogButtons::OkCancelCustom("Update now".into(), "Later".into()))
                    .show(move |install| {
                        if !install {
                            return;
                        }
                        tauri::async_runtime::spawn(async move {
                            match update.download_and_install(|_, _| {}, || {}).await {
                                Ok(()) => handle.restart(),
                                Err(error) => notify(&handle, format!("Update failed: {error}")),
                            }
                        });
                    });
            }
            Ok(None) if manual => notify(&app, "You have the latest version ♡"),
            Err(error) if manual => notify(&app, format!("Could not check for updates: {error}")),
            _ => {}
        }
    });
}

fn main() {
    tauri::Builder::default()
        // registered first, so a second launch never makes another window or profile
        .plugin(tauri_plugin_single_instance::init(|app, _, _| restore(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // no invoke_handler, no capabilities: the page can't reach the app
        .setup(|app| {
            let navigation_app = app.handle().clone();
            let popup_app = app.handle().clone();
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(HOME.parse()?))
                .title(TITLE)
                // hidden behind the splash for a moment
                .additional_browser_args(BROWSER_ARGS)
                .visible(false)
                .inner_size(1200.0, 800.0)
                .min_inner_size(640.0, 480.0)
                .devtools(cfg!(debug_assertions))
                // dark frame and backdrop like Spotify's; no menu bar
                .theme(Some(Theme::Dark))
                .background_color(Color(18, 18, 18, 255))
                .on_navigation(move |url| {
                    if policy::sign_in_rejected(url) {
                        sign_in_help(&navigation_app);
                    }
                    match destination(url) {
                        Destination::Embedded => true,
                        Destination::Browser => {
                            open_browser(&navigation_app, url);
                            false
                        }
                        Destination::Blocked => false,
                    }
                })
                .on_new_window(move |url, _| {
                    match destination(&url) {
                        // sign-in popups stay in the same window (same profile)
                        Destination::Embedded => {
                            if let Some(window) = popup_app.get_webview_window("main") {
                                let _ = window.navigate(url);
                            }
                        }
                        Destination::Browser => open_browser(&popup_app, &url),
                        Destination::Blocked => {}
                    }
                    NewWindowResponse::Deny
                })
                .on_download(|_, _| false)
                .build()?;
            // a local splash page while Spotify loads
            let splash = WebviewWindowBuilder::new(app, "splash", WebviewUrl::App("splash.html".into()))
                .title(TITLE)
                .additional_browser_args(BROWSER_ARGS)
                .inner_size(400.0, 470.0)
                .center()
                .resizable(false)
                .decorations(false)
                .skip_taskbar(true)
                .theme(Some(Theme::Dark))
                .background_color(Color(18, 18, 18, 255))
                .build()?;
            std::thread::spawn({
                let window = window.clone();
                move || {
                    std::thread::sleep(SPLASH_TIME);
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = splash.close();
                }
            });
            window.on_window_event({
                let window = window.clone();
                move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        // hide only: the music keeps playing
                        let _ = window.hide();
                    }
                }
            });
            let tray_menu = MenuBuilder::new(app)
                .text("show", "Show")
                .text("home", "Spotify home")
                .text("browser", "Open in browser")
                .separator()
                .text("update", "Check for updates")
                .text("help", "About / sign-in help")
                .separator()
                .text("quit", "Quit")
                .build()?;
            TrayIconBuilder::with_id("player")
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?)
                .tooltip("Femboy Spotify Lite — right-click for Show / Quit")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }
                    ) {
                        restore(tray.app_handle());
                    }
                })
                .build(app)?;
            check_for_updates(app.handle().clone(), false);
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => restore(app),
            "quit" => app.exit(0),
            "help" => sign_in_help(app),
            "update" => check_for_updates(app.clone(), true),
            "browser" => open_browser(app, &HOME.parse().expect("static HTTPS URL")),
            "home" => {
                restore(app);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.navigate(HOME.parse().expect("static HTTPS URL"));
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("Unable to start Femboy Spotify Lite");
}
