//! Small helpers: the page's elements, text escaping, time formats, saved settings,
//! toasts and timers.

use std::cell::RefCell;

use gloo_timers::callback::Timeout;
use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, Window};

pub fn window() -> Window {
    web_sys::window().expect("no window")
}

pub fn document() -> Document {
    window().document().expect("no document")
}

/// An element by id (the page always has the fixed ones).
pub fn el(id: &str) -> Element {
    document().get_element_by_id(id).unwrap_or_else(|| panic!("missing #{id}"))
}

pub fn try_el(id: &str) -> Option<Element> {
    document().get_element_by_id(id)
}

pub fn html_el(id: &str) -> HtmlElement {
    el(id).unchecked_into()
}

pub fn input(id: &str) -> HtmlInputElement {
    el(id).unchecked_into()
}

pub fn main_el() -> Element {
    el("main")
}

pub fn query(root: &Element, sel: &str) -> Option<Element> {
    root.query_selector(sel).ok().flatten()
}

pub fn query_all(root: &Element, sel: &str) -> Vec<Element> {
    let mut out = Vec::new();
    if let Ok(list) = root.query_selector_all(sel) {
        for i in 0..list.length() {
            if let Some(n) = list.get(i) {
                if let Ok(e) = n.dyn_into::<Element>() {
                    out.push(e);
                }
            }
        }
    }
    out
}

pub fn doc_query_all(sel: &str) -> Vec<Element> {
    let mut out = Vec::new();
    if let Ok(list) = document().query_selector_all(sel) {
        for i in 0..list.length() {
            if let Some(n) = list.get(i) {
                if let Ok(e) = n.dyn_into::<Element>() {
                    out.push(e);
                }
            }
        }
    }
    out
}

pub fn toggle(e: &Element, class: &str, on: bool) {
    let _ = e.class_list().toggle_with_force(class, on);
}

pub fn add_class(e: &Element, class: &str) {
    let _ = e.class_list().add_1(class);
}

pub fn remove_class(e: &Element, class: &str) {
    let _ = e.class_list().remove_1(class);
}

pub fn has_class(e: &Element, class: &str) -> bool {
    e.class_list().contains(class)
}

pub fn set_text(e: &Element, text: &str) {
    e.set_text_content(Some(text));
}

pub fn set_style(e: &Element, prop: &str, value: &str) {
    if let Some(h) = e.dyn_ref::<HtmlElement>() {
        let _ = h.style().set_property(prop, value);
    }
}

pub fn attr(e: &Element, name: &str) -> Option<String> {
    e.get_attribute(name)
}

/// The element an event happened on (or its parent, for a text node).
pub fn event_element(ev: &web_sys::Event) -> Option<Element> {
    let t = ev.target()?;
    match t.dyn_into::<Element>() {
        Ok(e) => Some(e),
        Err(t) => t.dyn_into::<web_sys::Node>().ok()?.parent_element(),
    }
}

pub fn closest(e: &Element, sel: &str) -> Option<Element> {
    e.closest(sel).ok().flatten()
}

pub fn now_ms() -> f64 {
    window().performance().map(|p| p.now()).unwrap_or_else(js_sys::Date::now)
}

pub fn inner_width() -> f64 {
    window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1480.0)
}

pub fn inner_height() -> f64 {
    window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(880.0)
}

/// HTML-escapes text placed into the page.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// 3:07
pub fn fmt(ms: f64) -> String {
    let s = (ms / 1000.0).floor().max(0.0) as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// "26 hr 32 min" / "3 min 20 sec"
pub fn fmt_long(ms: f64) -> String {
    let m = (ms / 60000.0).round() as i64;
    if m >= 60 {
        return format!("{} hr {} min", m / 60, m % 60);
    }
    let s = (ms / 1000.0).round() as i64;
    if m >= 1 { format!("{} min {} sec", s / 60, s % 60) } else { format!("{s} sec") }
}

/// "7 minutes ago", "2 days ago", "3 weeks ago", then a date — like Spotify's "Date added"
pub fn ago(ms: Option<f64>) -> String {
    let Some(ms) = ms.filter(|m| *m > 0.0) else { return String::new() };
    let s = (js_sys::Date::now() - ms) / 1000.0;
    let n = |v: f64, unit: &str| {
        let v = v.floor() as i64;
        format!("{v} {unit}{} ago", if v == 1 { "" } else { "s" })
    };
    if s < 60.0 {
        return "just now".into();
    }
    if s < 3600.0 {
        return n(s / 60.0, "minute");
    }
    if s < 86400.0 {
        return n(s / 3600.0, "hour");
    }
    if s < 7.0 * 86400.0 {
        return n(s / 86400.0, "day");
    }
    if s < 30.0 * 86400.0 {
        return n(s / (7.0 * 86400.0), "week");
    }
    let d = js_sys::Date::new(&JsValue::from_f64(ms));
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&opts, &"month".into(), &"short".into());
    let _ = js_sys::Reflect::set(&opts, &"day".into(), &"numeric".into());
    let _ = js_sys::Reflect::set(&opts, &"year".into(), &"numeric".into());
    d.to_locale_date_string("en-US", &opts).into()
}

// ---------- saved settings (localStorage, JSON values) ----------

fn storage() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

pub fn store_get<T: DeserializeOwned>(key: &str, default: T) -> T {
    storage()
        .and_then(|s| s.get_item(key).ok().flatten())
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or(default)
}

/// false when it didn't fit (e.g. a huge banner picture)
pub fn store_set<T: Serialize>(key: &str, value: &T) -> bool {
    let Ok(json) = serde_json::to_string(value) else { return false };
    storage().is_some_and(|s| s.set_item(key, &json).is_ok())
}

pub fn store_remove(key: &str) {
    if let Some(s) = storage() {
        let _ = s.remove_item(key);
    }
}

// ---------- toasts ----------

thread_local! {
    static TOAST_TIMER: RefCell<Option<Timeout>> = const { RefCell::new(None) };
}

pub fn toast(msg: impl AsRef<str>) {
    let t = el("toast");
    set_text(&t, msg.as_ref());
    add_class(&t, "show");
    let timer = Timeout::new(3200, move || remove_class(&el("toast"), "show"));
    TOAST_TIMER.with(|c| *c.borrow_mut() = Some(timer));
}

/// Runs `f` once after `ms` (not cancellable).
pub fn later(ms: u32, f: impl FnOnce() + 'static) {
    Timeout::new(ms, f).forget();
}

pub async fn sleep(ms: u32) {
    gloo_timers::future::TimeoutFuture::new(ms).await;
}

pub fn spawn(f: impl std::future::Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(f);
}

/// A JS error / rejection as text.
pub fn js_err(v: JsValue) -> String {
    if let Some(s) = v.as_string() {
        return s;
    }
    if let Some(e) = v.dyn_ref::<js_sys::Error>() {
        return String::from(e.message());
    }
    format!("{v:?}")
}

/// Calls `obj.method(args...)` on a plain JS object.
pub fn js_call(obj: &JsValue, method: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
    let f: js_sys::Function = js_sys::Reflect::get(obj, &method.into())?.dyn_into()?;
    let arr = js_sys::Array::new();
    for a in args {
        arr.push(a);
    }
    f.apply(obj, &arr)
}

pub fn js_get(obj: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(obj, &key.into()).unwrap_or(JsValue::UNDEFINED)
}
