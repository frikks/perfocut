//! PerfoCut: перфорация листового металла под лазерную резку.
//! Одна программа на Rust для Windows, Android и браузера; интерфейс на Slint.

slint::include_modules!();

mod app;
mod convert;
mod files;
mod i18n;
mod platform;
mod render;
mod saved;
mod update;

pub use app::run;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: slint::android::AndroidApp) {
    platform::set_app(&app);
    if slint::android::init(app).is_err() {
        return;
    }
    let _ = run();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    let _ = run();
}
