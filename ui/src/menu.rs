//! The pink pop-up menus: a song's "•••" / right click, a list's "•••", sorting,
//! the theme and app menus.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Element, HtmlElement};

use crate::app;
use crate::bridge;
use crate::icons::{hydrate, icon, icon_cls};
use crate::state::st;
use crate::types::Track;
use crate::util::*;
use crate::views::SORTS;

type Run = Rc<dyn Fn()>;
type Sub = Rc<dyn Fn() -> Vec<Item>>;

#[derive(Clone)]
pub enum Item {
    Sep,
    Entry(Entry),
}

#[derive(Clone, Default)]
pub struct Entry {
    pub icon: String,
    /// a drawn picture from art/icons (instead of a line icon)
    pub art: String,
    pub label: String,
    pub on: bool,
    pub disabled: bool,
    pub run: Option<Run>,
    pub sub: Option<Sub>,
}

/// a menu entry that runs `f`
pub fn item(icon: &str, label: impl Into<String>, f: impl Fn() + 'static) -> Item {
    Item::Entry(Entry { icon: icon.into(), label: label.into(), run: Some(Rc::new(f)), ..Default::default() })
}

/// an entry that opens a sub-menu
pub fn sub(icon: &str, label: impl Into<String>, f: impl Fn() -> Vec<Item> + 'static) -> Item {
    Item::Entry(Entry { icon: icon.into(), label: label.into(), sub: Some(Rc::new(f)), ..Default::default() })
}

pub fn disabled(icon: &str, label: impl Into<String>) -> Item {
    Item::Entry(Entry { icon: icon.into(), label: label.into(), disabled: true, ..Default::default() })
}

fn with_on(it: Item, on: bool) -> Item {
    match it {
        Item::Entry(mut e) => {
            e.on = on;
            Item::Entry(e)
        }
        s => s,
    }
}

thread_local! {
    static MENU: RefCell<Option<(Element, Element)>> = const { RefCell::new(None) };
    static ITEMS: RefCell<Vec<Item>> = const { RefCell::new(Vec::new()) };
    static SUB_ITEMS: RefCell<Vec<Item>> = const { RefCell::new(Vec::new()) };
    static OWNER: RefCell<Option<Element>> = const { RefCell::new(None) };
}

fn menu_el() -> Element {
    MENU.with(|m| m.borrow().as_ref().map(|x| x.0.clone())).expect("menu not set up")
}

fn sub_el() -> Element {
    MENU.with(|m| m.borrow().as_ref().map(|x| x.1.clone())).expect("menu not set up")
}

pub fn close() {
    let Some((m, s)) = MENU.with(|m| m.borrow().clone()) else { return };
    add_class(&m, "hidden");
    add_class(&s, "hidden");
    if let Some(o) = OWNER.with(|o| o.borrow_mut().take()) {
        remove_class(&o, "menu-open");
    }
}

fn menu_html(items: &[Item]) -> String {
    items
        .iter()
        .enumerate()
        .map(|(i, it)| match it {
            Item::Sep => r#"<div class="menu-sep"></div>"#.to_string(),
            Item::Entry(e) => {
                let pic = if !e.art.is_empty() && crate::icons::plain() {
                    // Spotify theme: its own marks for liked / not liked
                    icon(if e.art == "unlike" { "check-circle" } else { "plus-circle" })
                } else if !e.art.is_empty() {
                    format!(r#"<img class="ico art" src="art/icons/{}.png" alt="" draggable="false">"#, e.art)
                } else if !e.icon.is_empty() {
                    icon(&e.icon)
                } else {
                    r#"<span class="ico"></span>"#.into()
                };
                format!(
                    r#"<button class="menu-item {} {}" data-mi="{i}" {}>
      {pic}<span class="mi-label">{}</span>{}</button>"#,
                    if e.sub.is_some() { "has-sub" } else { "" },
                    if e.on { "on" } else { "" },
                    if e.disabled { "disabled" } else { "" },
                    esc(&e.label),
                    if e.sub.is_some() { icon_cls("chevron-right", "mi-arrow") } else { String::new() },
                )
            }
        })
        .collect()
}

/// place a menu at a point (or next to an element), kept inside the window
fn place(m: &Element, x: f64, y: f64, flip_x: Option<f64>) {
    set_style(m, "left", "0px");
    set_style(m, "top", "0px");
    remove_class(m, "hidden");
    let h: &HtmlElement = m.unchecked_ref();
    let w = f64::from(h.offset_width());
    let hh = f64::from(h.offset_height());
    let mut left = x;
    if left + w > inner_width() - 8.0 {
        left = match flip_x {
            Some(f) => f - w,
            None => inner_width() - w - 8.0,
        };
    }
    let top = y.min(inner_height() - hh - 8.0).max(8.0);
    set_style(m, "left", &format!("{}px", left.max(8.0)));
    set_style(m, "top", &format!("{top}px"));
}

fn pop_in(m: &Element) {
    remove_class(m, "open");
    let _ = m.unchecked_ref::<HtmlElement>().offset_width();
    add_class(m, "open");
}

pub enum At {
    Point(f64, f64),
    /// below an element, right-aligned with it
    Under(Element),
}

pub fn show(items: Vec<Item>, at: At, owner: Option<Element>) {
    close();
    let m = menu_el();
    m.set_inner_html(&menu_html(&items));
    hydrate(&m);
    ITEMS.with(|i| *i.borrow_mut() = items);
    if let Some(o) = &owner {
        add_class(o, "menu-open");
    }
    OWNER.with(|o| *o.borrow_mut() = owner);
    match at {
        At::Under(e) => {
            let r = e.get_bounding_client_rect();
            place(&m, r.right() - 240.0, r.bottom() + 6.0, None);
        }
        At::Point(x, y) => place(&m, x, y, None),
    }
    pop_in(&m);
}

fn open_sub(btn: &Element, items: Vec<Item>) {
    let s = sub_el();
    s.set_inner_html(&menu_html(&items));
    hydrate(&s);
    SUB_ITEMS.with(|i| *i.borrow_mut() = items);
    let r = btn.get_bounding_client_rect();
    place(&s, r.right() + 4.0, r.top() - 6.0, Some(r.left() - 4.0));
    pop_in(&s);
}

fn run_item(it: Option<Item>, btn: &Element) {
    let Some(Item::Entry(e)) = it else { return };
    if e.disabled {
        return;
    }
    if let Some(s) = e.sub {
        return open_sub(btn, s());
    }
    close();
    if let Some(r) = e.run {
        r();
    }
}

fn item_at(list: &'static std::thread::LocalKey<RefCell<Vec<Item>>>, btn: &Element) -> Option<Item> {
    let i: usize = attr(btn, "data-mi")?.parse().ok()?;
    list.with(|l| l.borrow().get(i).cloned())
}

fn on(target: &web_sys::EventTarget, event: &str, f: impl FnMut(web_sys::Event) + 'static) {
    let c = Closure::<dyn FnMut(web_sys::Event)>::new(f);
    let _ = target.add_event_listener_with_callback(event, c.as_ref().unchecked_ref());
    c.forget();
}

pub fn setup() {
    let doc = document();
    let body = doc.body().expect("no body");
    let m = doc.create_element("div").unwrap();
    m.set_class_name("menu hidden");
    let s = doc.create_element("div").unwrap();
    s.set_class_name("menu sub hidden");
    let _ = body.append_child(&m);
    let _ = body.append_child(&s);
    MENU.with(|x| *x.borrow_mut() = Some((m.clone(), s.clone())));

    on(&m, "click", |e| {
        if let Some(b) = event_element(&e).and_then(|t| closest(&t, "[data-mi]")) {
            run_item(item_at(&ITEMS, &b), &b);
        }
    });
    on(&m, "mouseover", |e| {
        let Some(b) = event_element(&e).and_then(|t| closest(&t, "[data-mi]")) else { return };
        match item_at(&ITEMS, &b) {
            Some(Item::Entry(Entry { sub: Some(s), .. })) => open_sub(&b, s()),
            _ => add_class(&sub_el(), "hidden"),
        }
    });
    on(&s, "click", |e| {
        if let Some(b) = event_element(&e).and_then(|t| closest(&t, "[data-mi]")) {
            run_item(item_at(&SUB_ITEMS, &b), &b);
        }
    });
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_capture(true);
    let down = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
        if event_element(&e).and_then(|t| closest(&t, ".menu")).is_none() {
            close();
        }
    });
    let _ = doc.add_event_listener_with_callback_and_add_event_listener_options("mousedown", down.as_ref().unchecked_ref(), &opts);
    down.forget();
    on(&doc, "keydown", |e| {
        if e.unchecked_ref::<web_sys::KeyboardEvent>().key() == "Escape" {
            close();
        }
    });
    on(&window(), "blur", |_| close());
    on(&window(), "resize", |_| close());
    on(&main_el(), "scroll", |_| close());
}

// ---------- the menus ----------

pub fn copy(text: String, what: &'static str) {
    spawn(async move {
        if bridge::copy_text(&text).await {
            toast(format!("{what} copied ♡"));
        } else {
            toast(format!("couldn't copy the {what}"));
        }
    });
}

fn open_link(uri: &str) -> String {
    format!("https://open.spotify.com/{}", uri.trim_start_matches("spotify:").replacen(':', "/", 1))
}

fn sleep_items() -> Vec<Item> {
    let mut items: Vec<Item> = [5u32, 10, 15, 30, 45, 60]
        .into_iter()
        .map(|m| item("", if m == 60 { "1 hour".to_string() } else { format!("{m} minutes") }, move || app::set_sleep(Some(m))))
        .collect();
    items.push(item("", "End of song", || app::set_sleep(None)));
    if app::sleep_active() {
        items.push(Item::Sep);
        items.push(item("", "Turn off timer", || {
            app::clear_sleep();
            toast("sleep timer off");
        }));
    }
    items
}

/// A song's menu. `index` is its row on screen (-1 for the now-playing song).
pub fn open_track_menu(t: Option<Track>, at: At, index: i64, owner: Option<Element>) {
    let Some(t) = t.filter(|t| t.uri.starts_with("spotify:track:")) else { return };
    let (mine, in_mine, saved) = st(|s| {
        let mine: Vec<_> = s.playlists.iter().filter(|p| p.editable).cloned().collect();
        let in_mine = s.view.kind == "playlist" && mine.iter().any(|p| p.id == s.view.id);
        (mine, in_mine, s.saved.contains(&t.id))
    });
    let artists = t.artist_list.clone();
    let link = format!("https://open.spotify.com/track/{}", t.id);
    let mut items = vec![];
    {
        let t = t.clone();
        items.push(sub("plus", "Add to playlist", move || {
            if mine.is_empty() {
                return vec![disabled("", "no playlists you can edit yet")];
            }
            mine.iter()
                .map(|p| {
                    let (p, t) = (p.clone(), t.clone());
                    item("", p.name.clone(), move || app::add_to_playlist(p.clone(), t.clone()))
                })
                .collect()
        }));
    }
    if in_mine {
        let t = t.clone();
        items.push(item("minus", "Remove from this playlist", move || app::remove_from_playlist(t.clone())));
    }
    {
        let t = t.clone();
        items.push(Item::Entry(Entry {
            art: if saved { "unlike" } else { "add" }.into(),
            label: if saved { "Remove from Liked Songs" } else { "Save to Liked Songs" }.into(),
            run: Some(Rc::new(move || app::toggle_saved(t.clone(), Some(!saved)))),
            ..Default::default()
        }));
    }
    {
        let t = t.clone();
        items.push(item("queue", "Add to queue", move || app::add_to_queue(t.clone())));
    }
    let timer = if app::sleep_active() { format!("Sleep timer ({})", app::sleep_label()) } else { "Sleep timer".into() };
    items.push(sub("timer", timer, sleep_items));
    items.push(Item::Sep);
    {
        let t = t.clone();
        items.push(item("radio", "Go to song radio", move || app::open_radio(t.clone())));
    }
    if artists.len() > 1 {
        items.push(sub("user", "Go to artist", move || {
            artists
                .iter()
                .map(|a| {
                    let uri = a.uri.clone();
                    item("", a.name.clone(), move || app::open_artist(uri.clone()))
                })
                .collect()
        }));
    } else if let Some(a) = artists.first() {
        let uri = a.uri.clone();
        items.push(item("user", "Go to artist", move || app::open_artist(uri.clone())));
    } else {
        items.push(disabled("user", "Go to artist"));
    }
    match t.album_uri.clone() {
        Some(u) => items.push(item("disc", "Go to album", move || app::open_album(u.clone()))),
        None => items.push(disabled("disc", "Go to album")),
    }
    items.push(Item::Sep);
    let uri = t.uri.clone();
    items.push(sub("share", "Share", move || {
        let (link, uri) = (link.clone(), uri.clone());
        vec![
            item("link", "Copy song link", move || copy(link.clone(), "song link")),
            item("link", "Copy Spotify URI", move || copy(uri.clone(), "Spotify URI")),
        ]
    }));
    let row = owner.or_else(|| if index >= 0 { query(&main_el(), &format!(".trow[data-index=\"{index}\"]")) } else { None });
    show(items, at, row);
}

/// the "•••" next to a list's play button
pub fn open_list_menu(btn: Element) {
    let (v, playlist, saved_album) = st(|s| {
        let p = s.playlists.iter().find(|x| x.id == s.view.id).cloned();
        (s.view.clone(), p, s.saved_albums.contains(&s.view.uri))
    });
    let mut items = vec![];
    if let Some(tracks) = v.tracks.clone().filter(|t| !t.is_empty()) {
        items.push(item("queue", "Add to queue", move || app::add_all_to_queue(tracks.clone())));
        items.push(item("shuffle", "Shuffle play", || {
            if let Some(b) = try_el("list-shuffle") {
                b.unchecked_into::<HtmlElement>().click();
            }
        }));
    }
    if v.kind == "playlist" {
        match playlist {
            Some(p) if p.owner == "you" => {
                let p2 = p.clone();
                items.push(Item::Sep);
                items.push(item("rename", "Rename", move || app::rename_playlist(p.clone())));
                items.push(item("delete", "Delete", move || app::remove_playlist(p2.clone())));
            }
            Some(p) => {
                items.push(Item::Sep);
                items.push(item("minus", "Remove from Your Library", move || app::remove_playlist(p.clone())));
            }
            None => {}
        }
        let link = format!("https://open.spotify.com/playlist/{}", v.id);
        items.push(Item::Sep);
        items.push(item("link", "Copy link to playlist", move || copy(link.clone(), "playlist link")));
    }
    if v.kind == "album" {
        let uri = v.uri.clone();
        items.push(Item::Sep);
        items.push(item(
            if saved_album { "album-saved" } else { "save-album" },
            if saved_album { "Remove from Your Library" } else { "Save to Your Library" },
            move || app::toggle_follow(uri.clone()),
        ));
        if let Some(a) = v.tracks.as_ref().and_then(|t| t.first()).and_then(|t| t.artist_list.first()) {
            let u = a.uri.clone();
            items.push(item("user", "Go to artist", move || app::open_artist(u.clone())));
        }
        let link = open_link(&v.uri);
        items.push(Item::Sep);
        items.push(item("link", "Copy link to album", move || copy(link.clone(), "album link")));
    }
    if !items.is_empty() {
        show(items, At::Under(btn), None);
    }
}

pub fn open_sort_menu(btn: Element) {
    let cur = st(|s| if s.view.sort_key.is_empty() { "custom".to_string() } else { s.view.sort_key.clone() });
    let items = SORTS
        .iter()
        .map(|(k, label)| {
            let k = k.to_string();
            item(if cur == k { "check-circle" } else { "" }, *label, move || {
                st(|s| s.view.sort_key = k.clone());
                app::render();
            })
        })
        .collect();
    show(items, At::Under(btn), None);
}

pub fn open_theme_menu(btn: Element) {
    let c = crate::theme::choice();
    show(
        vec![
            with_on(item("sun", "Light", || crate::theme::set("light")), c == "light"),
            with_on(item("moon", "Dark", || crate::theme::set("dark")), c == "dark"),
            with_on(item("devices", "Same as Windows", || crate::theme::set("auto")), c == "auto"),
            Item::Sep,
            with_on(item("sparkle", "Spotify (for normies)", || crate::theme::set("spotify")), c == "spotify"),
        ],
        At::Under(btn),
        None,
    );
}

/// the sidebar's "+" button
pub fn open_create_menu(btn: Element) {
    let r = btn.get_bounding_client_rect();
    show(
        vec![
            item("create", "Create playlist", app::create_playlist),
            item("search", "Search", || {
                let _ = html_el("search").focus();
            }),
        ],
        At::Point(r.right() + 8.0, r.top()),
        None,
    );
}

/// the "•••" in the top bar: version, start with Windows, updates
pub fn open_app_menu(btn: Element) {
    spawn(async move {
        let v = bridge::app_version().await;
        let auto = bridge::invoke::<bool>("autostart_get", serde_json::json!({})).await.unwrap_or(false);
        let r = btn.get_bounding_client_rect();
        show(
            vec![
                disabled("sparkle", format!("femboy spotify v{v} ♡")),
                Item::Sep,
                with_on(
                    item("devices", "Start with Windows", move || {
                        spawn(async move {
                            match bridge::invoke_unit("autostart_set", serde_json::json!({ "on": !auto })).await {
                                Ok(()) => toast(if !auto { "starts with Windows now ♡" } else { "won't start with Windows" }),
                                Err(e) => toast(format!("couldn't change it: {e}")),
                            }
                        })
                    }),
                    auto,
                ),
                item("bell", "Check for updates", || {
                    spawn(async {
                        app::check_for_update().await;
                        if st(|s| s.update.is_none()) {
                            toast("you have the newest version ♡");
                        }
                    })
                }),
            ],
            At::Point(r.left(), r.bottom() + 6.0),
            None,
        );
    });
}
