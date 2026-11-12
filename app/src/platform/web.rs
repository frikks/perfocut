//! Браузер: та же программа на Rust, собранная в WebAssembly. Файлы через загрузку и скачивание.

use super::SaveOutcome;
use crate::{AppWindow, Prefs, Theme};
use slint::ComponentHandle;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

const KEY: &str = "perfocut.v3";

fn window() -> Option<web_sys::Window> {
    web_sys::window()
}

fn coarse_pointer() -> bool {
    window()
        .and_then(|w| w.match_media("(pointer:coarse)").ok().flatten())
        .map(|m| m.matches())
        .unwrap_or(false)
}

fn read_file(f: web_sys::File) {
    let name = f.name();
    wasm_bindgen_futures::spawn_local(async move {
        let Ok(buf) = JsFuture::from(f.array_buffer()).await else { return };
        let bytes = js_sys::Uint8Array::new(&buf).to_vec();
        crate::app::open_bytes(name, bytes);
    });
}

// Окно всегда по размеру вкладки. Без этого Slint берёт preferred-width из app.slint
fn fit_to_viewport(ui: &AppWindow) {
    let Some(w) = window() else { return };
    let size = |v: Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>| v.ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let (width, height) = (size(w.inner_width()), size(w.inner_height()));
    if width > 0.0 && height > 0.0 {
        ui.window().set_size(slint::LogicalSize::new(width as f32, height as f32));
    }
}

pub fn init(ui: &AppWindow) {
    fit_to_viewport(ui);
    let weak = ui.as_weak();
    let resize = Closure::<dyn FnMut()>::new(move || {
        if let Some(ui) = weak.upgrade() {
            fit_to_viewport(&ui);
        }
    });
    if let Some(w) = window() {
        let _ = w.add_event_listener_with_callback("resize", resize.as_ref().unchecked_ref());
    }
    resize.forget();
    let touch = coarse_pointer();
    let prefs = ui.global::<Prefs>();
    prefs.set_is_phone(touch);
    prefs.set_can_minimize(false);
    prefs.set_can_drop(!touch);
    ui.global::<Theme>().set_touch(touch);
    let Some(doc) = window().and_then(|w| w.document()) else { return };
    let over = Closure::<dyn FnMut(web_sys::DragEvent)>::new(|e: web_sys::DragEvent| e.prevent_default());
    let _ = doc.add_event_listener_with_callback("dragover", over.as_ref().unchecked_ref());
    over.forget();
    let drop = Closure::<dyn FnMut(web_sys::DragEvent)>::new(|e: web_sys::DragEvent| {
        e.prevent_default();
        if let Some(f) = e.data_transfer().and_then(|d| d.files()).and_then(|l| l.get(0)) {
            read_file(f);
        }
    });
    let _ = doc.add_event_listener_with_callback("drop", drop.as_ref().unchecked_ref());
    drop.forget();
}

pub fn pick_file() {
    let Some(doc) = window().and_then(|w| w.document()) else { return };
    let Ok(input) = doc.create_element("input").map(|e| e.unchecked_into::<web_sys::HtmlInputElement>()) else { return };
    input.set_type("file");
    input.set_accept(".jpg,.jpeg,.png,.webp,.bmp,.gif,.tif,.tiff,.perfo,.zip,image/*");
    let inp = input.clone();
    let cb = Closure::once(move || {
        if let Some(f) = inp.files().and_then(|l| l.get(0)) {
            read_file(f);
        }
    });
    input.set_onchange(Some(cb.as_ref().unchecked_ref()));
    cb.forget();
    input.click();
}

pub fn save_file(name: &str, mime: &str, bytes: Vec<u8>) -> SaveOutcome {
    let run = || -> Option<()> {
        let w = window()?;
        let doc = w.document()?;
        let arr = js_sys::Uint8Array::from(bytes.as_slice());
        let parts = js_sys::Array::new();
        parts.push(&arr);
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type(mime);
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).ok()?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).ok()?;
        let a = doc.create_element("a").ok()?.unchecked_into::<web_sys::HtmlAnchorElement>();
        a.set_href(&url);
        a.set_download(name);
        doc.body()?.append_child(&a).ok()?;
        a.click();
        a.remove();
        // Ссылку освобождаем позже: браузеру нужно время, чтобы начать скачивание
        let cb = Closure::once(move || {
            let _ = web_sys::Url::revoke_object_url(&url);
        });
        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), 5000);
        cb.forget();
        Some(())
    };
    match run() {
        Some(()) => SaveOutcome::Downloaded(name.to_owned()),
        None => SaveOutcome::Failed(name.to_owned()),
    }
}

pub fn vibrate(kind: &str) {
    if let Some(w) = window() {
        let _ = w.navigator().vibrate_with_duration(if kind == "tick" { 4 } else { 10 });
    }
}

pub fn open_url(url: &str) {
    if let Some(w) = window() {
        let _ = w.open_with_url_and_target(url, "_blank");
    }
}

pub fn minimize(_ui: &AppWindow) {}

/// В браузере холст занимает всю вкладку, отдельного полноэкранного окна нет.
pub fn set_full_canvas(_ui: &AppWindow, _on: bool) {}

fn storage() -> Option<web_sys::Storage> {
    window()?.local_storage().ok().flatten()
}

pub fn load_settings() -> Option<String> {
    storage()?.get_item(KEY).ok().flatten()
}

pub fn store_settings(s: &str) {
    if let Some(st) = storage() {
        let _ = st.set_item(KEY, s);
    }
}

pub fn system_locale() -> String {
    window().and_then(|w| w.navigator().language()).unwrap_or_default()
}

pub fn set_bars(_bg: slint::Color, _light: bool) {}

pub fn now_ms() -> f64 {
    js_sys::Date::now()
}

pub fn now_iso() -> String {
    js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default()
}
