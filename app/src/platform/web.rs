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
        let Ok(buf) = JsFuture::from(f.array_buffer()).await else {
            return;
        };
        let bytes = js_sys::Uint8Array::new(&buf).to_vec();
        crate::app::open_bytes(name, bytes);
    });
}

/// Доставить результат асинхронного выбора файла в цикл событий Slint.
fn deliver<T: Send + 'static>(cb: impl FnOnce(T) + Send + 'static, value: T) {
    let _ = slint::invoke_from_event_loop(move || cb(value));
}

/// Общий выбор файлов через скрытый input. Результат — имя и байты каждого файла.
fn pick_files(
    accept: &str,
    multiple: bool,
    cb: impl FnOnce(Vec<(String, Vec<u8>)>) + Send + 'static,
) {
    let Some(doc) = window().and_then(|w| w.document()) else {
        deliver(cb, Vec::new());
        return;
    };
    let Ok(input) = doc
        .create_element("input")
        .map(|e| e.unchecked_into::<web_sys::HtmlInputElement>())
    else {
        deliver(cb, Vec::new());
        return;
    };
    input.set_type("file");
    input.set_accept(accept);
    input.set_multiple(multiple);
    let inp = input.clone();
    let on = Closure::once(move || {
        let mut files: Vec<web_sys::File> = Vec::new();
        if let Some(list) = inp.files() {
            for i in 0..list.length() {
                if let Some(f) = list.get(i) {
                    files.push(f);
                }
            }
        }
        wasm_bindgen_futures::spawn_local(async move {
            let mut out: Vec<(String, Vec<u8>)> = Vec::with_capacity(files.len());
            for f in files {
                if let Ok(buf) = JsFuture::from(f.array_buffer()).await {
                    let bytes = js_sys::Uint8Array::new(&buf).to_vec();
                    out.push((f.name(), bytes));
                }
            }
            deliver(cb, out);
        });
    });
    input.set_onchange(Some(on.as_ref().unchecked_ref()));
    on.forget();
    input.click();
}

/// Выбрать JSON-профиль станка. Байты уходят в колбэк (браузер читает файл асинхронно).
pub fn pick_profile_file(cb: impl FnOnce(Option<Vec<u8>>) + Send + 'static) {
    pick_files(".json,application/json", false, move |mut files| {
        cb(if files.is_empty() {
            None
        } else {
            Some(files.remove(0).1)
        });
    });
}

/// Выбрать несколько картинок для пакетной обработки.
pub fn pick_batch_files(cb: impl FnOnce(Vec<(String, Vec<u8>)>) + Send + 'static) {
    pick_files(
        "image/*,.jpg,.jpeg,.png,.webp,.bmp,.gif,.tif,.tiff",
        true,
        cb,
    );
}

fn fit_to_viewport(ui: &AppWindow) {
    let Some(w) = window() else { return };
    let size = |v: Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>| {
        v.ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
    };
    let (width, height) = (size(w.inner_width()), size(w.inner_height()));
    if width > 0.0 && height > 0.0 {
        ui.window()
            .set_size(slint::LogicalSize::new(width as f32, height as f32));
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
    let Some(doc) = window().and_then(|w| w.document()) else {
        return;
    };
    let over =
        Closure::<dyn FnMut(web_sys::DragEvent)>::new(|e: web_sys::DragEvent| e.prevent_default());
    let _ = doc.add_event_listener_with_callback("dragover", over.as_ref().unchecked_ref());
    over.forget();
    let drop = Closure::<dyn FnMut(web_sys::DragEvent)>::new(|e: web_sys::DragEvent| {
        e.prevent_default();
        // Перетаскивание нескольких картинок должно открывать все, а не только первую
        if let Some(list) = e.data_transfer().and_then(|d| d.files()) {
            for i in 0..list.length() {
                if let Some(f) = list.get(i) {
                    read_file(f);
                }
            }
        }
    });
    let _ = doc.add_event_listener_with_callback("drop", drop.as_ref().unchecked_ref());
    drop.forget();
}

pub fn pick_file() {
    let Some(doc) = window().and_then(|w| w.document()) else {
        return;
    };
    let Ok(input) = doc
        .create_element("input")
        .map(|e| e.unchecked_into::<web_sys::HtmlInputElement>())
    else {
        return;
    };
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
        let a = doc
            .create_element("a")
            .ok()?
            .unchecked_into::<web_sys::HtmlAnchorElement>();
        a.set_href(&url);
        a.set_download(name);
        doc.body()?.append_child(&a).ok()?;
        a.click();
        a.remove();
        // Увеличенный таймаут (30 сек), чтобы мобильные браузеры успели скачать тяжелый STEP/DXF
        let cb = Closure::once(move || {
            let _ = web_sys::Url::revoke_object_url(&url);
        });
        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            30_000,
        );
        cb.forget();
        Some(())
    };
    match run() {
        Some(()) => SaveOutcome::Downloaded(name.to_owned()),
        None => SaveOutcome::Failed("не удалось создать Blob или ссылку для скачивания".into()),
    }
}

pub fn vibrate(kind: &str) {
    if let Some(w) = window() {
        let _ = w
            .navigator()
            .vibrate_with_duration(if kind == "tick" { 4 } else { 10 });
    }
}

pub fn open_url(url: &str) {
    if let Some(w) = window() {
        let _ = w.open_with_url_and_target(url, "_blank");
    }
}

pub fn download_update(url: &str, _version: &str) {
    open_url(url);
    crate::app::update_page_opened();
}

pub fn minimize(_ui: &AppWindow) {}

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
    window()
        .and_then(|w| w.navigator().language())
        .unwrap_or_default()
}

pub fn set_bars(_bg: slint::Color, _light: bool) {}

pub fn check_update() {
    let Some(w) = window() else { return };
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let d = done.clone();
    let late = Closure::once(move || {
        if !d.replace(true) {
            crate::app::update_failed("");
        }
    });
    let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(
        late.as_ref().unchecked_ref(),
        15_000,
    );
    late.forget();
    wasm_bindgen_futures::spawn_local(async move {
        let text = match JsFuture::from(w.fetch_with_str(crate::update::API)).await {
            Ok(v) => match v.dyn_into::<web_sys::Response>() {
                Ok(r) if r.status() == 200 => match r.text() {
                    Ok(p) => JsFuture::from(p).await.ok().and_then(|t| t.as_string()),
                    Err(_) => None,
                },
                Ok(r) => {
                    if !done.replace(true) {
                        crate::app::update_failed(&format!("HTTP {}", r.status()));
                    }
                    return;
                }
                Err(_) => None,
            },
            Err(_) => None,
        };
        if done.replace(true) {
            return;
        }
        match text {
            Some(s) => {
                let _ = slint::invoke_from_event_loop(move || crate::app::update_json(&s));
            }
            None => {
                let _ = slint::invoke_from_event_loop(|| crate::app::update_failed(""));
            }
        }
    });
}

pub fn now_ms() -> f64 {
    js_sys::Date::now()
}

pub fn now_iso() -> String {
    // Формат должен совпадать с desktop: без миллисекунд, с Z на конце
    let s = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default();
    match s.split_once('.') {
        Some((head, _)) => format!("{}Z", head),
        None => s.trim_end_matches('Z').to_owned() + "Z",
    }
}
