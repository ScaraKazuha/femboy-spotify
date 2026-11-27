//! Talking to the app's Rust side through Tauri (commands and events), or to the
//! demo backend when the page runs in a plain browser.

use std::rc::Rc;

use serde::de::DeserializeOwned;
use serde_json::Value;
use wasm_bindgen::{JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;

use crate::demo;
use crate::util::{js_call, js_err, js_get, window};

fn tauri() -> JsValue {
    js_get(&window(), "__TAURI__")
}

/// No Tauri around: the page is the demo (`trunk serve`, screenshots).
pub fn is_demo() -> bool {
    let t = tauri();
    t.is_undefined() || t.is_null()
}

fn to_js(v: &Value) -> JsValue {
    let ser = serde_wasm_bindgen::Serializer::json_compatible();
    serde::Serialize::serialize(v, &ser).unwrap_or(JsValue::NULL)
}

fn from_js(v: JsValue) -> Value {
    serde_wasm_bindgen::from_value(v).unwrap_or(Value::Null)
}

/// Calls a Tauri command. Errors come back as text (as the commands send them).
pub async fn invoke_value(cmd: &str, args: Value) -> Result<Value, String> {
    if is_demo() {
        return demo::invoke(cmd, args).await;
    }
    let core = js_get(&tauri(), "core");
    let promise = js_call(&core, "invoke", &[cmd.into(), to_js(&args)]).map_err(js_err)?;
    let out = JsFuture::from(js_sys::Promise::from(promise)).await.map_err(js_err)?;
    Ok(from_js(out))
}

pub async fn invoke<T: DeserializeOwned>(cmd: &str, args: Value) -> Result<T, String> {
    let v = invoke_value(cmd, args).await?;
    serde_json::from_value(v).map_err(|e| format!("odd answer to {cmd}: {e}"))
}

/// For commands whose answer doesn't matter.
pub async fn invoke_unit(cmd: &str, args: Value) -> Result<(), String> {
    invoke_value(cmd, args).await.map(|_| ())
}

pub type Listener = Rc<dyn Fn(Value)>;

/// Listens to an app event ("remote", "home", "player", "update-progress").
pub fn listen(event: &str, cb: impl Fn(Value) + 'static) {
    let cb: Listener = Rc::new(cb);
    if is_demo() {
        demo::listen(event, cb);
        return;
    }
    let handler = Closure::<dyn Fn(JsValue)>::new(move |e: JsValue| cb(from_js(js_get(&e, "payload"))));
    let ev = js_get(&tauri(), "event");
    let _ = js_call(&ev, "listen", &[event.into(), handler.as_ref().clone()]);
    handler.forget();
}

// ---------- the window (frameless, so the page has the buttons) ----------

fn app_window() -> Option<JsValue> {
    if is_demo() {
        return None;
    }
    js_call(&js_get(&tauri(), "window"), "getCurrentWindow", &[]).ok()
}

pub fn window_call(method: &str) {
    if let Some(w) = app_window() {
        let _ = js_call(&w, method, &[]);
    }
}

pub async fn toggle_fullscreen() -> bool {
    let Some(w) = app_window() else { return false };
    let is = match js_call(&w, "isFullscreen", &[]) {
        Ok(p) => JsFuture::from(js_sys::Promise::from(p)).await.ok().and_then(|v| v.as_bool()).unwrap_or(false),
        Err(_) => false,
    };
    if let Ok(p) = js_call(&w, "setFullscreen", &[JsValue::from_bool(!is)]) {
        let _ = JsFuture::from(js_sys::Promise::from(p)).await;
    }
    true
}

pub fn has_window() -> bool {
    app_window().is_some()
}

pub async fn app_version() -> String {
    if is_demo() {
        return "demo".into();
    }
    let app = js_get(&tauri(), "app");
    match js_call(&app, "getVersion", &[]) {
        Ok(p) => JsFuture::from(js_sys::Promise::from(p))
            .await
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| "?".into()),
        Err(_) => "?".into(),
    }
}

/// Copies text to the clipboard.
pub async fn copy_text(text: &str) -> bool {
    let clipboard = js_get(&js_get(&window(), "navigator"), "clipboard");
    match js_call(&clipboard, "writeText", &[text.into()]) {
        Ok(p) => JsFuture::from(js_sys::Promise::from(p)).await.is_ok(),
        Err(_) => false,
    }
}
