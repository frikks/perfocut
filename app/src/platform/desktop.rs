//! Windows и Linux: окна выбора файлов системы, настройки в папке пользователя.

use super::SaveOutcome;
use crate::i18n::t;
use crate::{AppWindow, Prefs};
use slint::ComponentHandle;
use std::path::{Path, PathBuf};

const OPEN_EXT: [&str; 10] = ["jpg", "jpeg", "png", "webp", "bmp", "gif", "tif", "tiff", "perfo", "zip"];

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into())
}

fn open_path(path: &Path) {
    match std::fs::read(path) {
        Ok(bytes) => crate::app::open_bytes(file_name(path), bytes),
        Err(e) => crate::app::show_error(&t("Не удалось открыть файл: "), &e.to_string()),
    }
}

pub fn init(ui: &AppWindow) {
    use slint::winit_030::winit::event::WindowEvent;
    use slint::winit_030::{EventResult, WinitWindowAccessor};
    let prefs = ui.global::<Prefs>();
    prefs.set_can_drop(true);
    prefs.set_can_minimize(true);
    ui.window().on_winit_window_event(|_w, ev| {
        if let WindowEvent::DroppedFile(path) = ev {
            // Открываем после обработки события, чтобы не мешать циклу окна
            let path = path.clone();
            slint::Timer::single_shot(std::time::Duration::ZERO, move || open_path(&path));
            return EventResult::PreventDefault;
        }
        EventResult::Propagate
    });
    // Файл из командной строки: так Windows открывает .perfo двойным щелчком
    if let Some(path) = std::env::args_os().nth(1).map(PathBuf::from).filter(|p| p.is_file()) {
        slint::Timer::single_shot(std::time::Duration::from_millis(30), move || open_path(&path));
    }
}

pub fn pick_file() {
    let picked = rfd::FileDialog::new()
        .add_filter(t("Картинка или заказ"), &OPEN_EXT)
        .add_filter(t("Все файлы"), &["*"])
        .pick_file();
    if let Some(path) = picked {
        open_path(&path);
    }
}

pub fn save_file(name: &str, _mime: &str, bytes: Vec<u8>) -> SaveOutcome {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_owned()).unwrap_or_default();
    let mut dlg = rfd::FileDialog::new().set_file_name(name);
    if !ext.is_empty() {
        dlg = dlg.add_filter(ext.to_uppercase(), &[ext.as_str()]);
    }
    let Some(mut path) = dlg.save_file() else { return SaveOutcome::Cancelled };
    if path.extension().is_none() && !ext.is_empty() {
        path.set_extension(&ext);
    }
    match std::fs::write(&path, bytes) {
        Ok(()) => SaveOutcome::Saved(path.display().to_string()),
        Err(e) => SaveOutcome::Failed(e.to_string()),
    }
}

pub fn vibrate(_kind: &str) {}

pub fn open_url(url: &str) {
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(windows, target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = r {
        crate::app::show_error(&t("Не удалось открыть ссылку: "), &e.to_string());
    }
}

pub fn minimize(ui: &AppWindow) {
    ui.window().set_minimized(true);
}

/// Полный экран для рисования: окно без рамки во весь экран.
pub fn set_full_canvas(ui: &AppWindow, on: bool) {
    ui.window().set_fullscreen(on);
}

fn settings_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let dir = std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("PerfoCut"));
    #[cfg(not(windows))]
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|d| d.join("perfocut"));
    dir.map(|d| d.join("settings.json"))
}

pub fn load_settings() -> Option<String> {
    std::fs::read_to_string(settings_path()?).ok()
}

pub fn store_settings(s: &str) {
    let Some(path) = settings_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Через временный файл, чтобы при сбое не остаться с половиной настроек
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, s).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub fn system_locale() -> String {
    sys_locale::get_locale().unwrap_or_default()
}

pub fn set_bars(_bg: slint::Color, _light: bool) {}
