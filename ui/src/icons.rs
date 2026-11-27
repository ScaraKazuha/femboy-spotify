//! Icons: the line icons (SVG), the hand-drawn pictures that replace them
//! (ui/art/icons/*.png), and the cross-fade when a button's picture changes.

use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{AddEventListenerOptions, Element};

use crate::util::{add_class, attr, document, later, query_all, remove_class};

/// Line icons (24×24, stroke = currentColor).
fn svg(name: &str) -> &'static str {
    match name {
        "dots" => r##"<circle cx="5" cy="12" r="1.8" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1.8" fill="currentColor" stroke="none"/><circle cx="19" cy="12" r="1.8" fill="currentColor" stroke="none"/>"##,
        "chevron-left" => r##"<path d="M15 4.5 7.5 12l7.5 7.5"/>"##,
        "chevron-right" => r##"<path d="M9 4.5 16.5 12 9 19.5"/>"##,
        "home" => r##"<path fill="currentColor" stroke="none" d="M2.6 11.2 11.2 3.9a1.3 1.3 0 0 1 1.6 0l8.6 7.3c.5.4.2 1.3-.5 1.3H20v7a1.5 1.5 0 0 1-1.5 1.5h-4v-6h-5v6h-4A1.5 1.5 0 0 1 4 19.5v-7h-.9c-.7 0-1-.9-.5-1.3z"/>"##,
        "search" => r##"<circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 5 5"/>"##,
        "browse" => r##"<rect x="3" y="4" width="18" height="5.5" rx="1.5"/><path d="M4.5 9.5v8.5a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V9.5M10 13.5h4"/>"##,
        "bell" => r##"<path d="M6 16.5V11a6 6 0 1 1 12 0v5.5l1.8 1.8H4.2z"/><path d="M10 20.5a2 2 0 0 0 4 0"/>"##,
        "users" => r##"<circle cx="9" cy="8" r="3.4"/><path d="M2.8 20c.4-3.6 3-5.6 6.2-5.6s5.8 2 6.2 5.6"/><circle cx="17" cy="8.8" r="2.6"/><path d="M17.2 14.3c2.4.3 3.9 2 4.2 4.6"/>"##,
        "minimize" => r##"<path d="M5 12.5h14"/>"##,
        "maximize" => r##"<rect x="5.5" y="5.5" width="13" height="13" rx="1.5"/>"##,
        "close" => r##"<path d="M6 6l12 12M18 6 6 18"/>"##,
        "library" => r##"<path d="M5 4v16M10 4v16M14.2 4.6 19 19.4"/>"##,
        "plus-circle" => r##"<circle cx="12" cy="12" r="9"/><path d="M12 7.5v9M7.5 12h9"/>"##,
        "check-circle" => r##"<circle cx="12" cy="12" r="9" fill="currentColor"/><path d="m7.8 12.3 2.8 2.8 5.6-5.8" stroke="#fff"/>"##,
        "heart" => r##"<path d="M12 20.5s-7.6-4.6-9.5-9.3C1.1 7.7 3.3 4 7 4c2.2 0 3.9 1.3 5 3 1.1-1.7 2.8-3 5-3 3.7 0 5.9 3.7 4.5 7.2-1.9 4.7-9.5 9.3-9.5 9.3z"/>"##,
        "bookmark" => r##"<path d="M6.5 3.5h11v17L12 16.6l-5.5 3.9z"/>"##,
        "pie" => r##"<circle cx="12" cy="12" r="9"/><path d="M12 3v9h9"/>"##,
        "cat" => r##"<path d="M4.2 9.4 5 3.2l4.4 3.4h5.2L19 3.2l.8 6.2c1.1 1.3 1.7 2.8 1.7 4.4 0 4.3-4.3 7-9.5 7s-9.5-2.7-9.5-7c0-1.6.6-3.1 1.7-4.4z"/><circle cx="8.8" cy="12.6" r=".9" fill="currentColor"/><circle cx="15.2" cy="12.6" r=".9" fill="currentColor"/><path d="M10.6 15.6c.5.5.9.6 1.4.2.5.4.9.3 1.4-.2"/>"##,
        "ribbon" => r##"<path fill="currentColor" d="M12 11.5 3.5 6.5v10zM12 11.5l8.5-5v10z"/><circle cx="12" cy="11.5" r="2.3" fill="currentColor"/><path d="m11 13.5-3 7.5M13 13.5l3 7.5"/>"##,
        "shuffle" => r##"<path d="M3 6.5h3.5c4.5 0 6.5 11 11 11H21M3 17.5h3.5c1.6 0 2.8-1.4 3.8-3.2M13.7 9.7c1-1.8 2.2-3.2 3.8-3.2H21M18 3.5l3 3-3 3M18 14.5l3 3-3 3"/>"##,
        "prev" => r##"<path fill="currentColor" d="M6 5h2.6v14H6zM19.5 5.6v12.8a.8.8 0 0 1-1.2.7L9.4 12.7a.8.8 0 0 1 0-1.4l8.9-6.4a.8.8 0 0 1 1.2.7z"/>"##,
        "next" => r##"<path fill="currentColor" d="M15.4 5H18v14h-2.6zM4.5 5.6v12.8a.8.8 0 0 0 1.2.7l8.9-6.4a.8.8 0 0 0 0-1.4L5.7 4.9a.8.8 0 0 0-1.2.7z"/>"##,
        // optically centred: the box sits 1px right of centre (a triangle looks left-heavy)
        "play" => r##"<path fill="currentColor" d="M6.7 5.3v13.4a1 1 0 0 0 1.5.9l10.7-6.7a1 1 0 0 0 0-1.8L8.2 4.4A1 1 0 0 0 6.7 5.3z"/>"##,
        "pause" => r##"<rect x="6.5" y="5" width="3.8" height="14" rx="1.2" fill="currentColor"/><rect x="13.7" y="5" width="3.8" height="14" rx="1.2" fill="currentColor"/>"##,
        "repeat" => r##"<path d="M4 11.5v-1.5a3.5 3.5 0 0 1 3.5-3.5H19M16 3.5l3 3-3 3M20 12.5V14a3.5 3.5 0 0 1-3.5 3.5H5M8 20.5l-3-3 3-3"/>"##,
        "repeat-one" => r##"<path d="M4 11.5v-1.5a3.5 3.5 0 0 1 3.5-3.5H19M16 3.5l3 3-3 3M20 12.5V14a3.5 3.5 0 0 1-3.5 3.5H5M8 20.5l-3-3 3-3"/><path d="M11.3 10.3l1.2-.8v5" stroke-width="1.8"/>"##,
        "disc" => r##"<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="2.6"/>"##,
        "pen" => r##"<path d="M14.5 4.5l5 5L9 20H4v-5z"/><path d="M12.5 6.5l5 5"/>"##,
        "moon" => r##"<path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5z"/>"##,
        "sun" => r##"<circle cx="12" cy="12" r="4.2"/><path d="M12 2.5v2.2M12 19.3v2.2M4.6 4.6l1.6 1.6M17.8 17.8l1.6 1.6M2.5 12h2.2M19.3 12h2.2M4.6 19.4l1.6-1.6M17.8 6.2l1.6-1.6"/>"##,
        "plus" => r##"<path d="M12 5v14M5 12h14"/>"##,
        "minus" => r##"<path d="M5 12h14"/>"##,
        "timer" => r##"<circle cx="12" cy="13.5" r="7.5"/><path d="M12 13.5V9.5M9.5 2.5h5M18.5 6l1.5-1.5"/>"##,
        "radio" => r##"<circle cx="12" cy="12" r="2.2" fill="currentColor"/><path d="M7.8 7.8a6 6 0 0 0 0 8.4M16.2 7.8a6 6 0 0 1 0 8.4M4.9 4.9a10 10 0 0 0 0 14.2M19.1 4.9a10 10 0 0 1 0 14.2"/>"##,
        "user" => r##"<circle cx="12" cy="8" r="4"/><path d="M4.5 20.5c1.2-4 4-6 7.5-6s6.3 2 7.5 6"/>"##,
        "share" => r##"<path d="M12 3.5v11M7.5 8 12 3.5 16.5 8"/><path d="M5 12.5V19a1.5 1.5 0 0 0 1.5 1.5h11A1.5 1.5 0 0 0 19 19v-6.5"/>"##,
        "link" => r##"<path d="M10 14a4 4 0 0 0 5.7 0l3.6-3.6a4 4 0 0 0-5.7-5.7L12 6.3"/><path d="M14 10a4 4 0 0 0-5.7 0l-3.6 3.6a4 4 0 0 0 5.7 5.7L12 17.7"/>"##,
        "np" => r##"<rect x="3" y="4" width="18" height="16" rx="3"/><path fill="currentColor" stroke="none" d="M9.6 8.7v6.6a.7.7 0 0 0 1.1.6l5-3.3a.7.7 0 0 0 0-1.2l-5-3.3a.7.7 0 0 0-1.1.6z"/>"##,
        "mic" => r##"<circle cx="15" cy="9" r="5.5"/><path d="M11.3 5.3l7.4 7.4"/><path fill="currentColor" stroke="none" d="M10.4 11.2l2.4 2.4-7.4 7a1.6 1.6 0 0 1-2.3-2.3z"/>"##,
        "queue" => r##"<path d="M4 6.5h16M4 12h16M4 17.5h10"/>"##,
        "devices" => r##"<rect x="3" y="4" width="11" height="16" rx="2"/><rect x="16.5" y="8" width="4.5" height="12" rx="1.2"/><path d="M7.5 16.5h2"/>"##,
        "volume" => r##"<path d="M4 9.5v5h3.5l5 4v-13l-5 4z" fill="currentColor"/><path d="M16 8.8a4.6 4.6 0 0 1 0 6.4M18.7 6.2a8.3 8.3 0 0 1 0 11.6"/>"##,
        "volume-off" => r##"<path d="M4 9.5v5h3.5l5 4v-13l-5 4z" fill="currentColor"/><path d="m16 9.5 5 5M21 9.5l-5 5"/>"##,
        "mini" => r##"<rect x="3" y="5" width="18" height="14" rx="2"/><rect x="12" y="11.5" width="6.5" height="5" rx="1" fill="currentColor"/>"##,
        "fullscreen" => r##"<path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5"/>"##,
        "sparkle" => r##"<path fill="currentColor" stroke="none" d="M12 1.5c.6 5.2 2.3 7.6 9.5 10.5-7.2 2.9-8.9 5.3-9.5 10.5-.6-5.2-2.3-7.6-9.5-10.5C9.7 9.1 11.4 6.7 12 1.5z"/>"##,
        // plain versions of the ones that only had a drawing (Spotify theme)
        "menu" => r##"<path d="M4 7h16M4 12h16M4 17h16"/>"##,
        "clock" => r##"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"##,
        "sort" => r##"<path d="M4 7h16M7 12h10M10 17h4"/>"##,
        "search-in-list" => r##"<circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 5 5"/>"##,
        "delete" => r##"<path d="M4 7h16M9 7V4.5h6V7M6.5 7l1 13h9l1-13"/>"##,
        "rename" => r##"<path d="M4 20l1-4.5L15.5 5a2.1 2.1 0 0 1 3 3L8 18.5z"/>"##,
        "clear-queue" => r##"<path d="M4 7h11M4 12h11M4 17h7M15 15l5 5M20 15l-5 5"/>"##,
        "follow" => r##"<circle cx="9" cy="8" r="3.4"/><path d="M2.8 20c.4-3.6 3-5.6 6.2-5.6s5.8 2 6.2 5.6M19 8v6M16 11h6"/>"##,
        "following" => r##"<circle cx="9" cy="8" r="3.4"/><path d="M2.8 20c.4-3.6 3-5.6 6.2-5.6s5.8 2 6.2 5.6M16 11l2 2 4-4"/>"##,
        "save-album" => r##"<circle cx="12" cy="12" r="9"/><path d="M12 7.5v9M7.5 12h9"/>"##,
        "album-saved" => r##"<circle cx="12" cy="12" r="9" fill="currentColor"/><path d="m7.8 12.3 2.8 2.8 5.6-5.8" stroke="#000"/>"##,
        _ => "",
    }
}

/// Player buttons marked `data-art` use these pictures for their icon name.
fn player_art(name: &str) -> Option<&'static str> {
    Some(match name {
        "play" => "play",
        "pause" => "pause",
        "prev" => "previous",
        "next" => "next",
        "shuffle" => "shuffle",
        "repeat" => "repeat",
        "repeat-one" => "repeat-one",
        "queue" => "queue",
        "volume" => "volume",
        "volume-off" => "mute",
        "plus-circle" => "add",
        "check-circle" => "liked",
        "mic" => "lyrics",
        "devices" => "devices",
        "mini" => "mini-player",
        "fullscreen" => "fullscreen",
        _ => return None,
    })
}

/// The hand-drawn set used everywhere in place of a line icon of the same meaning.
fn icon_art(name: &str) -> Option<&'static str> {
    Some(match name {
        "menu" => "menu",
        "dots" => "more",
        "chevron-left" => "back",
        "chevron-right" => "forward",
        "home" => "home",
        "search" => "search",
        "browse" => "browse",
        "bell" => "bell",
        "users" => "friends",
        "minimize" => "minimize",
        "maximize" => "maximize",
        "close" => "close",
        "library" => "library",
        "plus-circle" | "create" => "create",
        "heart" => "liked-songs",
        "bookmark" => "queue-nav",
        "pie" => "recent",
        "cat" => "customize",
        "ribbon" => "logout",
        "np" => "now-playing",
        "pen" => "edit",
        "play" => "play-big",
        "shuffle" => "shuffle-play",
        "plus" => "add-to-playlist",
        "minus" => "remove",
        "queue" => "add-to-queue",
        "clear-queue" => "clear-queue",
        "timer" => "sleep-timer",
        "radio" => "song-radio",
        "user" => "artist",
        "disc" => "album",
        "share" => "share",
        "link" => "copy-link",
        "sparkle" => "loading",
        "sort" => "sort",
        "search-in-list" => "search-in-list",
        "clock" => "clock",
        "rename" => "rename",
        "delete" => "delete",
        "follow" => "follow",
        "following" => "following",
        "save-album" => "save-album",
        "album-saved" => "album-saved",
        "moon" => "theme-dark",
        "sun" => "theme-light",
        _ => return None,
    })
}

fn art_img(file: &str, cls: &str) -> String {
    format!(r#"<img class="ico art {cls}" src="art/icons/{file}.png" alt="" draggable="false">"#)
}

/// An icon by name: the hand-drawn picture when there is one, else the line icon.
pub fn icon(name: &str) -> String {
    icon_cls(name, "")
}

/// The Spotify theme uses plain line icons instead of the drawings.
pub fn plain() -> bool {
    document().document_element().is_some_and(|r| attr(&r, "data-look").as_deref() == Some("spotify"))
}

pub fn icon_cls(name: &str, cls: &str) -> String {
    if !plain() {
        if let Some(file) = icon_art(name) {
            return art_img(file, cls);
        }
    } else if let Some(shape) = crate::spot_icons::spot(name) {
        return format!(r#"<svg class="ico spot {cls}" viewBox="0 0 24 24" fill="currentColor" stroke="none" aria-hidden="true">{shape}</svg>"#);
    }
    format!(
        r#"<svg class="ico {cls}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{}</svg>"#,
        svg(name)
    )
}

fn icon_for(el: &Element, name: &str) -> String {
    if el.has_attribute("data-art") && !plain() {
        if let Some(file) = player_art(name) {
            return art_img(file, "");
        }
    }
    icon(name)
}

/// Puts the icon into every `[data-icon]` element under `root` that has none yet.
pub fn hydrate(root: &Element) {
    for e in query_all(root, "[data-icon]") {
        if e.query_selector(":scope > .ico").ok().flatten().is_none() {
            let name = attr(&e, "data-icon").unwrap_or_default();
            let _ = e.insert_adjacent_html("afterbegin", &icon_for(&e, &name));
        }
    }
}

pub fn hydrate_page() {
    if let Some(body) = document().body() {
        hydrate(&body);
    }
}

/// Draws every icon on the page again (the theme switched between drawings and plain icons).
pub fn redraw_all() {
    let Some(body) = document().body() else { return };
    for e in query_all(&body, "[data-icon]") {
        for old in query_all(&e, ":scope > .ico") {
            old.remove();
        }
        let name = attr(&e, "data-icon").unwrap_or_default();
        let _ = e.insert_adjacent_html("afterbegin", &icon_for(&e, &name));
    }
}

fn once() -> AddEventListenerOptions {
    let o = AddEventListenerOptions::new();
    o.set_once(true);
    o
}

/// Changes a button's icon. Pictures cross-fade: the old one spins out while the
/// new one pops in, in the same spot.
pub fn set_icon(el: &Element, name: &str) {
    let old = el.query_selector(":scope > .ico:not(.leaving)").ok().flatten();
    if old.is_some() && attr(el, "data-icon").as_deref() == Some(name) {
        return; // nothing changes: no re-animation
    }
    let _ = el.set_attribute("data-icon", name);
    let Some(old) = old else {
        let _ = el.insert_adjacent_html("afterbegin", &icon_for(el, name));
        return;
    };
    if !el.has_attribute("data-art") {
        old.set_outer_html(&icon_for(el, name));
        return;
    }
    for x in query_all(el, ":scope > .ico.leaving") {
        x.remove();
    }
    add_class(&old, "leaving");
    {
        let o = old.clone();
        let cb = Closure::once_into_js(move || o.remove());
        let _ = old.add_event_listener_with_callback_and_add_event_listener_options("animationend", cb.unchecked_ref(), &once());
    }
    {
        let o = old.clone();
        later(600, move || o.remove());
    }
    let fresh_html = icon_for(el, name).replacen("class=\"ico art \"", "class=\"ico art entering\"", 1);
    let _ = old.insert_adjacent_html("afterend", &fresh_html);
    if let Some(fresh) = old.next_element_sibling() {
        let f = fresh.clone();
        let cb = Closure::once_into_js(move || remove_class(&f, "entering"));
        let _ = fresh.add_event_listener_with_callback_and_add_event_listener_options("animationend", cb.unchecked_ref(), &once());
    }
}

/// Loads every button picture up front so swaps never flash empty.
pub fn preload() {
    let names = [
        "play", "pause", "previous", "next", "shuffle", "repeat", "repeat-one", "queue", "volume", "mute", "add",
        "liked", "lyrics", "devices", "mini-player", "fullscreen", "unlike", "theme-dark", "theme-light", "play-big",
        "more", "loading", "empty",
    ];
    for n in names {
        if let Ok(img) = web_sys::HtmlImageElement::new() {
            img.set_src(&format!("art/icons/{n}.png"));
        }
    }
}
