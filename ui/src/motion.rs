//! The little endless motions: the buddy swaying (and bopping while music plays,
//! with a floating note), the DJ X badge glowing, the panels' ears twitching and the
//! banner's stars twinkling.
//!
//! They used to be endless CSS animations. Those redraw at the screen's refresh rate
//! (240 times a second on a 240 Hz screen) for as long as they run, which kept
//! WebView2's GPU process busy at ~30-45 % of a core and its memory high. Here they
//! are moved by hand instead: 30 frames a second while something lively plays, 15
//! for the slow idle sway, none while the window is minimized or hidden. They look
//! the same.

use std::cell::Cell;
use std::f64::consts::PI;

use wasm_bindgen::JsCast;
use web_sys::Element;

use crate::util::*;

thread_local! {
    static FRAME: Cell<u64> = const { Cell::new(0) };
}

/// 0..1..0 smoothly, over `period` seconds (like `ease-in-out alternate`).
fn swing(t: f64, period: f64) -> f64 {
    (1.0 - (2.0 * PI * t / period).cos()) / 2.0
}

fn set(e: &Element, prop: &str, value: &str) {
    if let Some(h) = e.dyn_ref::<web_sys::HtmlElement>() {
        let _ = h.style().set_property(prop, value);
    }
}

fn reduced_motion() -> bool {
    window().match_media("(prefers-reduced-motion: reduce)").ok().flatten().is_some_and(|q| q.matches())
}

/// Seconds into an ear twitch for a group that twitches every 7 s at `offset`
/// (main panel, then the right one, then the sidebar), if it is twitching now.
fn twitch_at(t: f64, offset: f64) -> Option<f64> {
    let p = (t - offset).rem_euclid(7.0);
    (p < 1.1).then_some(p)
}

/// The twitch shape (as the old keyframes): lean +16° and squash at 30 %, -5° at 60 %.
fn ear_transform(right: bool, p: f64) -> String {
    let (skew, sy) = if !(0.0..=0.7).contains(&p) {
        (0.0, 1.0)
    } else {
        let k = p / 0.7;
        if k < 0.3 {
            let s = k / 0.3;
            (16.0 * s, 1.0 - 0.1 * s)
        } else if k < 0.6 {
            let s = (k - 0.3) / 0.3;
            (16.0 - 21.0 * s, 0.9 + 0.13 * s)
        } else {
            let s = (k - 0.6) / 0.4;
            (-5.0 + 5.0 * s, 1.03 - 0.03 * s)
        }
    };
    let flip = if right { "scaleX(-1) " } else { "" };
    format!("{flip}skewX(calc(var(--lean) + {skew:.2}deg)) scaleY({sy:.3})")
}

fn frame() {
    if document().hidden() || reduced_motion() {
        return;
    }
    let n = FRAME.with(|f| {
        f.set(f.get() + 1);
        f.get()
    });
    let t = now_ms() / 1000.0;
    let buddy = try_el("btn-panel");
    let vibing = buddy.as_ref().is_some_and(|b| has_class(b, "vibing"));
    let dj = buddy.as_ref().is_some_and(|b| has_class(b, "dj"));
    let twitching = [0.0, 2.1, 4.3].iter().any(|o| twitch_at(t, *o).is_some());
    // what's on screen right now of: beating hearts (sign-in cards), the update dot,
    // spinners and the hopping star (while a page loads)
    let shown = |sel: &str| -> Vec<Element> {
        doc_query_all(sel).into_iter().filter(|e| e.get_bounding_client_rect().width() > 0.0).collect()
    };
    let beating = shown(".login-heart, .bell-dot");
    let spinning = shown(".spinner");
    let hopping = shown(".loading-art");
    let lively = vibing || dj || twitching || !beating.is_empty() || !spinning.is_empty() || !hopping.is_empty();
    // the slow idle sway needs only 15 frames a second
    if !lively && n % 2 == 1 {
        return;
    }
    for e in &beating {
        // a heartbeat every 1.4 s: up to 1.12, back, up to 1.08, back
        let k = (t % 1.4) / 1.4;
        let s = match k {
            k if k < 0.15 => 1.0 + 0.12 * (k / 0.15),
            k if k < 0.30 => 1.12 - 0.12 * ((k - 0.15) / 0.15),
            k if k < 0.45 => 1.0 + 0.08 * ((k - 0.30) / 0.15),
            k if k < 0.60 => 1.08 - 0.08 * ((k - 0.45) / 0.15),
            _ => 1.0,
        };
        set(e, "transform", &format!("scale({s:.3})"));
    }
    for e in &spinning {
        set(e, "transform", &format!("rotate({:.1}deg)", 360.0 * ((t % 1.2) / 1.2)));
    }
    for e in &hopping {
        // a little hop with a tilt, once a second
        let k = t % 1.0;
        let (y, r, s) = if k < 0.4 {
            let a = k / 0.4;
            (-14.0 * a, -12.0 * a, 1.0 + 0.06 * a)
        } else if k < 0.7 {
            let a = (k - 0.4) / 0.3;
            (-14.0 + 14.0 * a, -12.0 + 20.0 * a, 1.06 - 0.10 * a)
        } else {
            let a = (k - 0.7) / 0.3;
            (0.0, 8.0 - 8.0 * a, 0.96 + 0.04 * a)
        };
        set(e, "transform", &format!("translateY({y:.1}px) rotate({r:.1}deg) scale({s:.3})"));
    }

    if let Some(b) = &buddy {
        if let Some(img) = query(b, "img") {
            let pressed = b.matches(":active").unwrap_or(false);
            let tf = if vibing {
                let s = swing(t, if dj { 0.76 } else { 1.0 });
                format!("rotate({:.2}deg) translateY({:.2}px) scaleY({:.3})", -9.0 + 18.0 * s, -3.0 * s, 0.97 + 0.05 * s)
            } else {
                let s = swing(t, 3.4);
                format!("rotate({:.2}deg) translateY({:.2}px)", -3.0 + 6.0 * s, -1.5 * s)
            };
            set(&img, "transform", &if pressed { format!("{tf} scale(.9)") } else { tf });
        }
        // the floating ♪ (the ::after reads these)
        if vibing {
            let p = (t % 1.6) / 1.6;
            let e = 1.0 - (1.0 - p) * (1.0 - p);
            let o = if p < 0.25 { p / 0.25 } else { 1.0 - (p - 0.25) / 0.75 };
            set(b, "--note-o", &format!("{o:.2}"));
            set(b, "--note-x", &format!("{:.1}px", 10.0 * e));
            set(b, "--note-y", &format!("{:.1}px", 8.0 - 26.0 * e));
            set(b, "--note-r", &format!("{:.1}deg", 18.0 * e));
        }
        if dj {
            if let Some(badge) = query(b, ".onair") {
                let s = swing(t, 2.4);
                set(&badge, "--onair-blur", &format!("{:.1}px", 6.0 + 10.0 * s));
                set(&badge, "--onair-glow", &format!("rgba({:.0}, {:.0}, {:.0}, {:.2})", 139.0 + 116.0 * s, 92.0 + 35.0 * s, 246.0 - 68.0 * s, 0.4 + 0.5 * s));
            }
        }
    }

    for (group, offset) in [(".ears-main", 0.0), (".ears-right", 2.1), (".ears-side", 4.3)] {
        let p = twitch_at(t, offset).unwrap_or(9.0);
        for ear in doc_query_all(&format!("{group} .ear")) {
            let right = has_class(&ear, "r");
            // the right ear follows 0.35 s later
            set(&ear, "transform", &ear_transform(right, if right { p - 0.35 } else { p }));
        }
    }

    // the banner's stars, each a little after the one before
    if let Some(deco) = query(&main_el(), ".hero-deco") {
        for (i, s) in query_all(&deco, ":scope > span").into_iter().enumerate() {
            if !has_class(&s, "s") {
                continue;
            }
            let k = swing(t - i as f64 * 0.37, 3.0);
            set(&s, "opacity", &format!("{:.2}", 1.0 - 0.55 * k));
            set(&s, "transform", &format!("scale({:.3})", 1.0 - 0.25 * k));
        }
    }
}

pub fn start() {
    gloo_timers::callback::Interval::new(33, frame).forget();
}
