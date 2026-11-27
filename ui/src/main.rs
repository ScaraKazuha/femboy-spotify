//! ♡ Femboy Spotify's interface ♡ — Rust compiled to WebAssembly, running in the
//! app's WebView. It draws the pages and talks to the app's Rust side (src/) through
//! Tauri commands and events.

mod app;
mod bridge;
mod demo;
mod events;
mod icons;
mod menu;
mod state;
mod theme;
mod tips;
mod types;
mod util;
mod views;

use wasm_bindgen::{JsCast, closure::Closure};

use crate::state::st;
use crate::util::*;

fn main() {
    console_error_panic_hook::set_once();
    // the theme first, so the page never flashes the wrong one
    theme::apply(false);
    theme::watch_system();

    tips::setup();
    icons::hydrate_page();
    icons::preload();
    theme::apply(false);
    menu::setup();
    events::setup();

    let resize = Closure::<dyn FnMut()>::new(app::fit_hero_title);
    let _ = window().add_event_listener_with_callback("resize", resize.as_ref().unchecked_ref());
    resize.forget();

    let _ = el("avatar-img").set_attribute("src", views::NO_ARTIST);
    app::draw_volume(st(|s| s.volume));
    app::render_play_state();
    app::render();
    app::render_now();
    gloo_timers::callback::Interval::new(250, app::tick).forget();
    spawn(app::check_for_update());
    gloo_timers::callback::Interval::new(6 * 60 * 60 * 1000, || spawn(app::check_for_update())).forget();
    spawn(app::connect_saved());

    // demo page with "?shot": a song is already playing (for the README screenshot)
    if bridge::is_demo() && demo::has_flag("shot") {
        later(400, || {
            let tracks: Vec<types::Track> = serde_json::from_value(demo::tracks()).unwrap_or_default();
            app::play_from(tracks, 1, "Boykisser Mix", None);
        });
    }
}
