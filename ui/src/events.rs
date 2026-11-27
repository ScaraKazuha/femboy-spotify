//! Clicks, keys and drags: what every button on the page does.

use std::cell::RefCell;

use gloo_timers::callback::Timeout;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Element, Event, HtmlElement, KeyboardEvent, MouseEvent, PointerEvent};

use crate::app::{self, *};
use crate::bridge::{self, invoke_unit, listen};
use crate::menu::{self, At};
use crate::state::st;
use crate::types::RemoteState;
use crate::util::*;

fn on(target: &web_sys::EventTarget, event: &str, f: impl FnMut(Event) + 'static) {
    let c = Closure::<dyn FnMut(Event)>::new(f);
    let _ = target.add_event_listener_with_callback(event, c.as_ref().unchecked_ref());
    c.forget();
}

fn click(id: &str, f: impl FnMut(Event) + 'static) {
    on(&el(id), "click", f);
}

fn num(e: &Element, name: &str) -> Option<usize> {
    attr(e, name)?.parse().ok()
}

fn mouse(e: &Event) -> (f64, f64) {
    let m: &MouseEvent = e.unchecked_ref();
    (f64::from(m.client_x()), f64::from(m.client_y()))
}

// ---------- drag bars (seek / volume) ----------

fn bind_bar(id: &str, on_move: impl Fn(f64) + 'static, on_done: impl Fn(f64) + 'static) {
    let bar = el(id);
    let on_move = std::rc::Rc::new(on_move);
    let on_done = std::rc::Rc::new(on_done);
    let ratio = |bar: &Element, e: &Event| {
        let r = bar.get_bounding_client_rect();
        let x = f64::from(e.unchecked_ref::<MouseEvent>().client_x());
        ((x - r.left()) / r.width()).clamp(0.0, 1.0)
    };
    let dragging = std::rc::Rc::new(std::cell::Cell::new(false));
    {
        let (bar2, m, d) = (bar.clone(), on_move.clone(), dragging.clone());
        on(&bar, "pointerdown", move |e| {
            let p: &PointerEvent = e.unchecked_ref();
            let _ = bar2.set_pointer_capture(p.pointer_id());
            add_class(&bar2, "drag");
            d.set(true);
            m(ratio(&bar2, &e));
        });
    }
    {
        let (bar2, m, d) = (bar.clone(), on_move.clone(), dragging.clone());
        on(&bar, "pointermove", move |e| {
            if d.get() {
                m(ratio(&bar2, &e));
            }
        });
    }
    {
        let (bar2, d) = (bar.clone(), dragging.clone());
        on(&bar, "pointerup", move |e| {
            if !d.replace(false) {
                return;
            }
            remove_class(&bar2, "drag");
            on_done(ratio(&bar2, &e));
        });
    }
}

fn bind_bars() {
    bind_bar(
        "seek-bar",
        |r| {
            let Some(dur) = st(|s| s.now.as_ref().map(|t| t.duration_ms)) else { return };
            st(|s| s.seek_drag = true);
            set_style(&el("seek-fill"), "width", &format!("{}%", r * 100.0));
            set_style(&el("seek-knob"), "left", &format!("{}%", r * 100.0));
            set_text(&el("time-pos"), &fmt(r * dur));
        },
        |r| {
            st(|s| s.seek_drag = false);
            if let Some(dur) = st(|s| s.now.as_ref().map(|t| t.duration_ms)) {
                seek_to(r * dur);
            }
        },
    );
    bind_bar("vol-bar", |r| set_volume(r * 100.0, true), |r| set_volume(r * 100.0, false));
}

// ---------- the main panel (event delegation) ----------

const CLICKABLE: &str = "[data-artist],[data-album],[data-like],[data-more],[data-play],[data-card],[data-tab],[data-filter],[data-open],[data-playlist],[data-index],[data-line],#hero-play,#hero-edit,#list-play,#list-shuffle,#list-more,#list-sort,#album-save,#artist-follow,#queue-clear,#create-playlist,.list-find";

fn main_click(e: Event) {
    let Some(el) = event_element(&e).and_then(|t| closest(&t, CLICKABLE)) else { return };
    let id = el.id();
    let has = |n: &str| el.has_attribute(n);
    let val = |n: &str| attr(&el, n).unwrap_or_default();
    if has_class(&el, "list-find") {
        add_class(&el, "open");
        if let Some(i) = query(&el, "input") {
            let _ = i.unchecked_ref::<HtmlElement>().focus();
        }
        return;
    }
    if !val("data-artist").is_empty() {
        return open_artist(val("data-artist"));
    }
    if !val("data-album").is_empty() {
        return open_album(val("data-album"));
    }
    if has("data-play") {
        if let Some(i) = num(&el, "data-play") {
            play_row(i);
        }
        return;
    }
    if !val("data-card").is_empty() {
        return open_card(val("data-card"));
    }
    if !val("data-tab").is_empty() {
        st(|s| s.view.tab = val("data-tab"));
        return render();
    }
    match id.as_str() {
        "list-more" => return menu::open_list_menu(el),
        "list-sort" => return menu::open_sort_menu(el),
        "album-save" | "artist-follow" => return toggle_follow(st(|s| s.view.uri.clone())),
        "queue-clear" => return clear_queue(),
        "create-playlist" => return create_playlist(),
        _ => {}
    }
    if has("data-like") {
        if let Some(t) = num(&el, "data-like").and_then(|i| st(|s| s.view_track(i))) {
            toggle_saved(t, None);
        }
        return;
    }
    if has("data-more") {
        let i = num(&el, "data-more").unwrap_or(usize::MAX);
        let t = st(|s| s.view_track(i));
        return menu::open_track_menu(t, At::Under(el), i as i64, None);
    }
    if !val("data-filter").is_empty() {
        let f = val("data-filter");
        match f.as_str() {
            "liked" => return open_liked(),
            "recent" => return open_recent(),
            _ => {
                st(|s| s.filter = f);
                return render();
            }
        }
    }
    match val("data-open").as_str() {
        "liked" => return open_liked(),
        "dj" => return play_dj(),
        _ => {}
    }
    if !val("data-playlist").is_empty() {
        return open_playlist_by_id(&val("data-playlist"));
    }
    if has("data-line") {
        let ms = num(&el, "data-line").and_then(|i| st(|s| s.lyrics.as_ref().and_then(|l| l.get(i).map(|x| x.start_ms))));
        if let Some(ms) = ms.filter(|m| *m > 0.0) {
            seek_to(ms);
        }
        return;
    }
    // like Spotify: one click selects a song, a double click (or its ▶) plays it
    if has("data-index") {
        if let Some(i) = num(&el, "data-index") {
            select_row(i);
        }
        return;
    }
    match id.as_str() {
        "hero-edit" => open_customize(),
        "hero-play" => play_featured(),
        "list-play" | "list-shuffle" => {
            let (n, shuffle_now) = st(|s| (s.view_tracks().len(), s.shuffle));
            if n == 0 {
                return;
            }
            let shuffle = id == "list-shuffle";
            if shuffle != shuffle_now {
                set_option("shuffle", shuffle);
            }
            let start = if shuffle { (js_sys::Math::random() * n as f64) as usize } else { 0 };
            play_row(start);
        }
        _ => {}
    }
}

fn bind_main() {
    let main = main_el();
    on(&main, "click", main_click);
    on(&main, "dblclick", |e| {
        let Some(t) = event_element(&e) else { return };
        if let Some(row) = closest(&t, ".trow[data-index]") {
            if closest(&t, "button,.alink").is_none() {
                if let Some(i) = num(&row, "data-index") {
                    play_row(i);
                }
            }
        }
    });
    on(&main, "input", |e| {
        let Some(t) = event_element(&e) else { return };
        if t.id() != "list-find" {
            return;
        }
        let v = t.unchecked_ref::<web_sys::HtmlInputElement>().value();
        st(|s| s.view.filter_text = v);
        refresh_rows();
    });
    on(&main, "keydown", |e| {
        let Some(t) = event_element(&e) else { return };
        if t.id() == "list-find" && e.unchecked_ref::<KeyboardEvent>().key() == "Escape" {
            st(|s| s.view.filter_text.clear());
            render();
        }
    });
    // long lists draw the rows near the screen as it scrolls
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_passive(true);
    let scroll = Closure::<dyn FnMut()>::new(fix_list_window);
    let _ = main.add_event_listener_with_callback_and_add_event_listener_options("scroll", scroll.as_ref().unchecked_ref(), &opts);
    scroll.forget();
    on(&window(), "resize", |_| fix_list_window());
    on(&main, "contextmenu", |e| {
        let Some(row) = event_element(&e).and_then(|t| closest(&t, ".trow[data-index]")) else { return };
        e.prevent_default();
        let i = num(&row, "data-index").unwrap_or(usize::MAX);
        let (x, y) = mouse(&e);
        menu::open_track_menu(st(|s| s.view_track(i)), At::Point(x, y), i as i64, None);
    });
}

// ---------- sidebar, top bar, player bar ----------

thread_local! {
    static SEARCH_TIMER: RefCell<Option<Timeout>> = const { RefCell::new(None) };
}

fn bind_chrome() {
    on(&el("side-thumbs"), "click", |e| {
        if let Some(b) = event_element(&e).and_then(|t| closest(&t, "[data-playlist]")) {
            open_playlist_by_id(&attr(&b, "data-playlist").unwrap_or_default());
        }
    });
    for b in doc_query_all(".side-btn[data-nav]") {
        let btn = b.clone();
        on(&b, "click", move |_| match attr(&btn, "data-nav").unwrap_or_default().as_str() {
            "library" => {
                let v = st(|s| s.new_view("library"));
                go(v, true);
            }
            "search" => menu::open_create_menu(btn.clone()),
            "liked" => open_liked(),
            "queue" => open_queue(),
            "recent" => open_recent(),
            "customize" => open_customize(),
            "logout" => logout(),
            _ => {}
        });
    }

    click("btn-theme", |_| menu::open_theme_menu(el("btn-theme")));
    click("btn-home", |_| go_home());
    click("btn-browse", |_| {
        let v = st(|s| s.new_view("library"));
        go(v, true);
    });
    click("btn-back", |_| go_back());
    click("btn-forward", |_| go_forward());
    click("btn-menu", |_| menu::open_app_menu(el("btn-menu")));
    click("btn-bell", |_| if st(|s| s.update.is_some()) { show_update() } else { toast("no new notifications ♡") });
    click("update-later", |_| add_class(&el("update"), "hidden"));
    click("update-go", |_| install_update());
    click("btn-friends", |_| toast("friend activity is coming soon~"));
    click("btn-avatar", |_| {
        toast(match st(|s| s.me.as_ref().map(|m| m.name.clone())) {
            Some(n) => format!("logged in as {n} ♡ (ribbon in the sidebar = log out)"),
            None => "not logged in".into(),
        })
    });

    click("btn-play", |_| toggle_play());
    click("btn-next", |_| next());
    click("btn-prev", |_| prev());
    click("btn-shuffle", |_| {
        let on = st(|s| {
            s.shuffle = !s.shuffle;
            s.shuffle
        });
        render_play_state();
        set_option("shuffle", on);
        toast(if on { "shuffle on ✧" } else { "shuffle off" });
    });
    click("btn-repeat", |_| {
        let r = st(|s| {
            s.repeat = match s.repeat.as_str() {
                "off" => "all",
                "all" => "one",
                _ => "off",
            }
            .into();
            s.repeat.clone()
        });
        render_play_state();
        match r.as_str() {
            "all" => set_option("repeat_context", true),
            "one" => set_option("repeat_track", true),
            _ => {
                set_option("repeat_track", false);
                set_option("repeat_context", false);
            }
        }
        toast(match r.as_str() {
            "all" => "repeat all ↻",
            "one" => "repeat one ♡",
            _ => "repeat off",
        });
    });
    click("btn-mute", |_| {
        let (muted, vol) = st(|s| (s.muted, s.volume));
        if muted {
            set_volume(vol, false);
        } else {
            st(|s| s.muted = true);
            send_volume(0.0);
            draw_volume(0.0);
            render_play_state();
        }
    });
    click("right-add", |_| save_current());
    click("np-add", |_| save_current());
    // now playing: artist names open the artist, the song name opens its album,
    // right click gives the same song menu as in lists
    for id in ["np-artist", "right-artist"] {
        click(id, |e| {
            if let Some(a) = event_element(&e).and_then(|t| closest(&t, "[data-artist]")) {
                open_artist(attr(&a, "data-artist").unwrap_or_default());
            }
        });
    }
    for id in ["np-name", "right-name"] {
        click(id, |_| {
            if let Some(u) = st(|s| s.now.as_ref().and_then(|t| t.album_uri.clone())) {
                open_album(u);
            }
        });
    }
    for sel in [".np", ".right-meta"] {
        if let Some(target) = document().query_selector(sel).ok().flatten() {
            on(&target, "contextmenu", |e| {
                e.prevent_default();
                let (x, y) = mouse(&e);
                menu::open_track_menu(st(|s| s.now.clone()), At::Point(x, y), -1, None);
            });
        }
    }
    let lyrics_view = || {
        let v = st(|s| s.new_view("lyrics"));
        go(v, true);
    };
    click("btn-lyrics", move |_| if st(|s| s.view.name == "lyrics") { go_back() } else { lyrics_view() });
    click("lyrics-open", move |_| lyrics_view());
    click("btn-queue", |_| if st(|s| s.view.name == "queue") { go_back() } else { open_queue() });
    click("btn-devices", |_| toggle_devices(None));
    // the now-playing side panel (Spotify's "Now playing view" button)
    click("btn-np", |_| {
        let appel = el("app");
        let hidden = appel.class_list().toggle("no-right").unwrap_or(false);
        toggle(&el("btn-np"), "active", !hidden);
    });
    // the headphones buddy is the DJ X button: start DJ, or let the DJ switch it up
    click("btn-panel", |_| {
        let (dj, playing) = st(|s| (s.dj_on, s.playing));
        if !dj {
            return play_dj();
        }
        if !playing {
            return toggle_play();
        }
        toast("DJ X is switching up the vibe ♪");
        spawn(async {
            if let Err(e) = invoke_unit("dj_switch", serde_json::json!({})).await {
                handle_remote_error(&e);
            }
        });
    });
    click("btn-mini", |_| html_el("btn-np").click());
    click("btn-full", |_| {
        spawn(async {
            if !bridge::toggle_fullscreen().await {
                toast("fullscreen works in the app ♡");
            }
        })
    });
    click("win-min", |_| bridge::window_call("minimize"));
    click("win-max", |_| bridge::window_call("toggleMaximize"));
    click("win-close", |_| if bridge::has_window() { bridge::window_call("close") } else { toast("bye bye~ ♡") });

    // devices pop-up
    on(&el("devices-list"), "click", |e| {
        let Some(t) = event_element(&e) else { return };
        if closest(&t, "[data-player-login]").is_some() {
            toggle_devices(Some(false));
            return player_login();
        }
        if let Some(b) = closest(&t, "[data-device]") {
            pick_device_from_list(attr(&b, "data-device").unwrap_or_default());
        }
    });
    on(&document(), "click", |e| {
        let Some(t) = event_element(&e) else { return };
        if closest(&t, "#devices-pop").is_none() && closest(&t, "#btn-devices").is_none() {
            toggle_devices(Some(false));
        }
    });
    click("nodevice-ok", |_| player_login());
    click("nodevice-later", |_| hide_player_setup());

    // search: as you type (a little after), or right away on Enter
    on(&el("search"), "input", |_| {
        let q = input("search").value();
        let t = Timeout::new(450, move || do_search(q));
        SEARCH_TIMER.with(|s| *s.borrow_mut() = Some(t));
    });
    on(&el("search"), "keydown", |e| {
        let k = e.unchecked_ref::<KeyboardEvent>().key();
        if k == "Enter" {
            SEARCH_TIMER.with(|s| s.borrow_mut().take());
            do_search(input("search").value());
        }
        if k == "Escape" {
            let _ = html_el("search").blur();
        }
    });

    // customize banner
    on(&el("customize"), "click", |e| {
        if event_element(&e).is_some_and(|t| t.id() == "customize") {
            add_class(&el("customize"), "hidden");
        }
    });
    on(&el("banner-file"), "change", |_| {
        let Some(file) = input("banner-file").files().and_then(|f| f.get(0)) else { return };
        spawn(async move {
            let b = app::downscale(file, 1800.0).await;
            st(|s| s.pending_banner = b);
        });
    });
    click("banner-reset", |_| reset_banner());
    click("banner-save", |_| save_banner());
    click("btn-login", |_| login());
}

// ---------- keyboard and mouse side buttons ----------

fn bind_keys() {
    on(&document(), "keydown", |e| {
        let k: &KeyboardEvent = e.unchecked_ref();
        if event_element(&e).is_some_and(|t| t.tag_name() == "INPUT") {
            return;
        }
        let key = k.key();
        let now = st(|s| s.now.as_ref().map(|t| t.duration_ms));
        if k.code() == "Space" {
            e.prevent_default();
            toggle_play();
        } else if key == "ArrowRight" && now.is_some() && !k.alt_key() {
            let p = st(|s| s.position());
            seek_to((p + 10000.0).min(now.unwrap_or(0.0) - 1000.0));
        } else if key == "ArrowLeft" && now.is_some() && !k.alt_key() {
            let p = st(|s| s.position());
            seek_to((p - 10000.0).max(0.0));
        } else if key == "ArrowUp" {
            e.prevent_default();
            let v = st(|s| s.volume);
            set_volume(v + 5.0, false);
        } else if key == "ArrowDown" {
            e.prevent_default();
            let v = st(|s| s.volume);
            set_volume(v - 5.0, false);
        } else if key == "n" {
            next();
        } else if key == "p" {
            prev();
        } else if key == "/" || (k.ctrl_key() && key == "l") {
            e.prevent_default();
            let _ = html_el("search").focus();
        } else if k.alt_key() && key == "ArrowLeft" {
            go_back();
        } else if k.alt_key() && key == "ArrowRight" {
            go_forward();
        }
    });
    on(&document(), "mouseup", |e| match e.unchecked_ref::<MouseEvent>().button() {
        3 => go_back(),
        4 => go_forward(),
        _ => {}
    });
}

// ---------- app events ----------

fn bind_app_events() {
    listen("remote", |v| {
        if let Ok(r) = serde_json::from_value::<RemoteState>(v) {
            apply_remote(r);
        }
    });
    listen("home", |v| {
        if let Ok(h) = serde_json::from_value(v) {
            set_home(h);
        }
    });
    listen("player", |_| on_player_ready());
    listen("update-progress", |v| {
        let done = v[0].as_f64().unwrap_or(0.0);
        let total = v[1].as_f64().unwrap_or(0.0);
        if total > 0.0 {
            set_text(&el("update-go"), &format!("downloading {}%", (done / total * 100.0).round()));
        }
    });
}

pub fn setup() {
    bind_bars();
    bind_main();
    bind_chrome();
    bind_keys();
    bind_app_events();
}
