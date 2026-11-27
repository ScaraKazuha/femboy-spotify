//! Themes: light / dark (or the same as Windows) with the drawn icons, and "Spotify"
//! (for normies): Spotify's own dark look with plain icons, no ears. The custom banner
//! and everything else stay. Spotify is the dark theme plus `data-look="spotify"`.

use wasm_bindgen::{JsCast, closure::Closure};

use crate::bridge::is_demo;
use crate::icons::set_icon;
use crate::util::*;

/// light | dark | auto | spotify
pub fn choice() -> String {
    // demo / screenshots: ?theme=spotify
    if is_demo() && !store_get("themeTouched", false) && window().location().search().unwrap_or_default().contains("theme=spotify") {
        return "spotify".into();
    }
    store_get("theme", "light".to_string())
}

fn dark_query() -> Option<web_sys::MediaQueryList> {
    window().match_media("(prefers-color-scheme: dark)").ok().flatten()
}

fn now() -> String {
    // demo / screenshots: ?theme=dark
    if is_demo() && !store_get("themeTouched", false) {
        let q = window().location().search().unwrap_or_default();
        for t in ["light", "dark"] {
            if q.contains(&format!("theme={t}")) {
                return t.into();
            }
        }
    }
    match choice().as_str() {
        "spotify" => "dark".into(),
        "auto" => if dark_query().is_some_and(|q| q.matches()) { "dark" } else { "light" }.into(),
        c => c.into(),
    }
}

/// Sets the theme on the page. Called first thing, before anything is drawn
/// (the page stays hidden until it has a theme, so it never flashes light).
pub fn apply(animate: bool) {
    let Some(root) = document().document_element() else { return };
    let next = now();
    if animate && attr(&root, "data-theme").as_deref() != Some(next.as_str()) {
        add_class(&root, "theme-switching");
        let r = root.clone();
        later(450, move || remove_class(&r, "theme-switching"));
    }
    let _ = root.set_attribute("data-theme", &next);
    let spotify = choice() == "spotify";
    let was_spotify = attr(&root, "data-look").as_deref() == Some("spotify");
    if spotify {
        let _ = root.set_attribute("data-look", "spotify");
    } else {
        let _ = root.remove_attribute("data-look");
    }
    let drawn = try_el("btn-theme").is_some_and(|b| query(&b, ".ico").is_some());
    if drawn && spotify != was_spotify {
        // drawings <-> plain icons: draw the page again
        crate::icons::redraw_all();
        crate::app::render();
        crate::app::render_now();
        crate::app::render_play_state();
    }
    if let Some(btn) = try_el("btn-theme") {
        if drawn {
            set_icon(&btn, if next == "dark" { "sun" } else { "moon" });
        }
        let title = match (spotify, next.as_str()) {
            (true, _) => "theme: Spotify (click to change)",
            (_, "dark") => "theme: dark (click to change)",
            _ => "theme: light (click to change)",
        };
        let _ = btn.set_attribute("title", title);
    }
}

pub fn set(c: &str) {
    store_set("theme", &c);
    store_set("themeTouched", &true);
    apply(true);
}

/// follows Windows when the choice is "Same as Windows"
pub fn watch_system() {
    let Some(q) = dark_query() else { return };
    let cb = Closure::<dyn FnMut()>::new(|| {
        if choice() == "auto" {
            apply(true);
        }
    });
    let _ = q.add_event_listener_with_callback("change", cb.as_ref().unchecked_ref());
    cb.forget();
}
