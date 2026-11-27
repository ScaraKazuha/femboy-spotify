//! Pink tooltips. The browser's own `title` tooltip is a black system box, so every
//! `title` (in the HTML or set later) moves to data-tip + aria-label and we draw our own.

use std::cell::RefCell;

use gloo_timers::callback::Timeout;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Element, HtmlElement, MutationObserver, MutationObserverInit, MutationRecord, Node};

use crate::util::*;

thread_local! {
    static TIP: RefCell<Option<Element>> = const { RefCell::new(None) };
    static TIP_FOR: RefCell<Option<Element>> = const { RefCell::new(None) };
    static TIMER: RefCell<Option<Timeout>> = const { RefCell::new(None) };
}

fn tip_el() -> Option<Element> {
    TIP.with(|t| t.borrow().clone())
}

fn adopt_title(e: &Element) {
    let Some(t) = attr(e, "title") else { return };
    let _ = e.remove_attribute("title");
    let _ = e.set_attribute("data-tip", &t);
    if !t.is_empty() {
        let _ = e.set_attribute("aria-label", &t);
    }
    if TIP_FOR.with(|f| f.borrow().as_ref() == Some(e)) {
        show(e);
    }
}

fn adopt_titles(root: &Node) {
    let Some(root) = root.dyn_ref::<Element>() else { return };
    if root.has_attribute("title") {
        adopt_title(root);
    }
    for e in query_all(root, "[title]") {
        adopt_title(&e);
    }
}

fn show(e: &Element) {
    let Some(tip) = tip_el() else { return };
    let text = attr(e, "data-tip").unwrap_or_default();
    if text.is_empty() || !e.is_connected() {
        return hide();
    }
    set_text(&tip, &text);
    let r = e.get_bounding_client_rect();
    let th: &HtmlElement = tip.unchecked_ref();
    let w = f64::from(th.offset_width());
    let h = f64::from(th.offset_height());
    let below = r.top() - h - 10.0 < 4.0;
    let x = (r.left() + r.width() / 2.0 - w / 2.0).max(6.0).min(inner_width() - w - 6.0);
    let y = if below { r.bottom() + 10.0 } else { r.top() - h - 10.0 };
    set_style(&tip, "left", &format!("{}px", x.round()));
    set_style(&tip, "top", &format!("{}px", y.round()));
    toggle(&tip, "below", below);
    add_class(&tip, "show");
}

fn hide() {
    TIMER.with(|t| t.borrow_mut().take());
    TIP_FOR.with(|f| *f.borrow_mut() = None);
    if let Some(tip) = tip_el() {
        remove_class(&tip, "show");
    }
}

fn on(target: &web_sys::EventTarget, event: &str, f: impl FnMut(web_sys::Event) + 'static) {
    let c = Closure::<dyn FnMut(web_sys::Event)>::new(f);
    let _ = target.add_event_listener_with_callback(event, c.as_ref().unchecked_ref());
    c.forget();
}

pub fn setup() {
    let doc = document();
    let body = doc.body().expect("no body");
    let tip = doc.create_element("div").unwrap();
    tip.set_class_name("tip");
    let _ = tip.set_attribute("role", "tooltip");
    let _ = body.append_child(&tip);
    TIP.with(|t| *t.borrow_mut() = Some(tip));

    adopt_titles(&body);
    let cb = Closure::<dyn FnMut(js_sys::Array)>::new(|muts: js_sys::Array| {
        for m in muts.iter() {
            let m: MutationRecord = m.unchecked_into();
            if m.type_() == "attributes" {
                if let Some(t) = m.target().and_then(|t| t.dyn_into::<Element>().ok()) {
                    adopt_title(&t);
                }
            } else {
                let added = m.added_nodes();
                for i in 0..added.length() {
                    if let Some(n) = added.get(i) {
                        adopt_titles(&n);
                    }
                }
            }
        }
    });
    if let Ok(obs) = MutationObserver::new(cb.as_ref().unchecked_ref()) {
        let opts = MutationObserverInit::new();
        opts.set_subtree(true);
        opts.set_child_list(true);
        opts.set_attributes(true);
        opts.set_attribute_filter(&js_sys::Array::of1(&"title".into()));
        let _ = obs.observe_with_options(&body, &opts);
    }
    cb.forget();

    on(&doc, "mouseover", |e| {
        let el = event_element(&e).and_then(|t| closest(&t, "[data-tip]"));
        if TIP_FOR.with(|f| *f.borrow() == el) {
            return;
        }
        hide();
        let Some(el) = el else { return };
        TIP_FOR.with(|f| *f.borrow_mut() = Some(el.clone()));
        let timer = Timeout::new(380, move || show(&el));
        TIMER.with(|t| *t.borrow_mut() = Some(timer));
    });
    on(&doc, "mouseout", |e| {
        let related = e.unchecked_ref::<web_sys::MouseEvent>().related_target().and_then(|t| t.dyn_into::<Node>().ok());
        let leave = TIP_FOR.with(|f| f.borrow().as_ref().is_some_and(|el| !el.contains(related.as_ref())));
        if leave {
            hide();
        }
    });
    for ev in ["mousedown", "wheel", "keydown"] {
        on(&doc, ev, |_| hide());
    }
    on(&window(), "blur", |_| hide());
}
