//! Windows и Linux: окна выбора файлов системы, настройки в папке пользователя.

use super::SaveOutcome;
use crate::i18n::t;
use crate::{AppWindow, Prefs};
use slint::ComponentHandle;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static SETTINGS_SEQ: AtomicUsize = AtomicUsize::new(0);

const OPEN_EXT: [&str; 10] = [
    "jpg", "jpeg", "png", "webp", "bmp", "gif", "tif", "tiff", "perfo", "zip",
];

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into())
}

fn open_path(path: &Path) {
    match std::fs::read(path) {
        Ok(bytes) => {
            let name = file_name(path);
            if name.ends_with(".perfo") || name.ends_with(".zip") {
                crate::app::remember_order(&path.display().to_string());
            }
            crate::app::open_bytes(name, bytes)
        }
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
    // Файлы из командной строки: Windows может передать несколько .perfo сразу
    for path in std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        slint::Timer::single_shot(std::time::Duration::from_millis(30), move || {
            open_path(&path)
        });
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

/// Выбрать JSON-файл с пользовательским профилем G-code.
pub fn load_profile_text() -> Option<String> {
    let path = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .pick_file()?;
    std::fs::read_to_string(path).ok()
}

/// Выбрать папку с картинками для пакетной обработки.
pub fn pick_folder() -> Option<String> {
    rfd::FileDialog::new()
        .pick_folder()
        .map(|p| p.display().to_string())
}

/// Пакетно обработать все картинки в папке: для каждой — генерация и экспорт DXF+CSV рядом.
pub fn batch_process(dir: &str, params: &perfo_core::params::Params) -> Result<usize, String> {
    use perfo_core::{export, gen};
    let root = std::path::Path::new(dir);
    let mut done = 0usize;
    for entry in std::fs::read_dir(root).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !["jpg", "jpeg", "png", "webp", "bmp", "gif", "tiff"].contains(&ext.as_str()) {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let decoded = match crate::files::decode(&bytes) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let r = gen::generate(Some(&decoded.gray), params);
        if r.stats.count == 0 {
            continue;
        }
        let base = path.with_extension("");
        std::fs::write(base.with_extension("dxf"), export::dxf(&r)).map_err(|e| e.to_string())?;
        std::fs::write(base.with_extension("csv"), export::csv(&r)).map_err(|e| e.to_string())?;
        done += 1;
    }
    Ok(done)
}

pub fn save_file(name: &str, _mime: &str, bytes: Vec<u8>) -> SaveOutcome {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_owned())
        .unwrap_or_default();
    let mut dlg = rfd::FileDialog::new().set_file_name(name);
    if !ext.is_empty() {
        dlg = dlg.add_filter(ext.to_uppercase(), &[ext.as_str()]);
    }
    let Some(mut path) = dlg.save_file() else {
        return SaveOutcome::Cancelled;
    };
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
    let r = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
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
    // Уникальное имя: два экземпляра программы не перетирают друг друга
    let tmp = path.with_file_name(format!(
        "settings.{}.{}.tmp",
        std::process::id(),
        SETTINGS_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    if std::fs::write(&tmp, s).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub fn system_locale() -> String {
    sys_locale::get_locale().unwrap_or_default()
}

pub fn set_bars(_bg: slint::Color, _light: bool) {}

/// Папка загрузок: обновление кладём туда, рядом с остальными файлами.
fn downloads_dir() -> PathBuf {
    #[cfg(windows)]
    if let Some(p) = std::env::var_os("USERPROFILE") {
        return PathBuf::from(p).join("Downloads");
    }
    #[cfg(not(windows))]
    if let Some(h) = std::env::var_os("HOME") {
        return PathBuf::from(h).join("Downloads");
    }
    std::env::temp_dir()
}

/// Скачивает файл релиза напрямую в «Загрузки» через системный curl.
pub fn download_update(_release_url: &str, version: &str) {
    let dir = downloads_dir();
    let version = version.to_owned();
    std::thread::spawn(move || {
        let path = dir.join(format!("PerfoCut-{version}.exe"));
        let url = crate::update::asset_url(&version, "PerfoCut.exe");
        let mut cmd = std::process::Command::new("curl");
        cmd.args([
            "-fL",
            "--max-time",
            "300",
            "-H",
            "User-Agent: PerfoCut",
            "-o",
        ]);
        cmd.arg(&path);
        cmd.arg(&url);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Оконная программа: не показываем мелькающую консоль
            cmd.creation_flags(0x0800_0000);
        }
        match cmd.output() {
            Ok(o) if o.status.success() => {
                let shown = path.display().to_string();
                let _ =
                    slint::invoke_from_event_loop(move || crate::app::update_downloaded(&shown));
            }
            Ok(o) => {
                let err = format!("curl: {}", o.status);
                let _ =
                    slint::invoke_from_event_loop(move || crate::app::update_download_failed(&err));
            }
            Err(e) => {
                let err = e.to_string();
                let _ =
                    slint::invoke_from_event_loop(move || crate::app::update_download_failed(&err));
            }
        }
    });
}

/// Проверка обновлений через системный `curl`: свой TLS-стек в сборку не тянем.
pub fn check_update() {
    std::thread::spawn(|| {
        let mut cmd = std::process::Command::new("curl");
        cmd.args([
            "-fsSL",
            "--max-time",
            "12",
            "-H",
            "User-Agent: PerfoCut",
            "-H",
            "Accept: application/vnd.github+json",
            crate::update::API,
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Оконная программа: не показываем мелькающую консоль
            cmd.creation_flags(0x0800_0000);
        }
        match cmd.output() {
            Ok(o) if o.status.success() => {
                let json = String::from_utf8_lossy(&o.stdout).into_owned();
                let _ = slint::invoke_from_event_loop(move || crate::app::update_json(&json));
            }
            Ok(o) => {
                // curl -f: код 22 — ответ HTTP 4xx/5xx, обычно 404 (нет стабильных релизов)
                let err = if o.status.code() == Some(22) {
                    "HTTP 4xx/5xx".to_owned()
                } else {
                    format!("curl: {}", o.status)
                };
                let _ = slint::invoke_from_event_loop(move || crate::app::update_failed(&err));
            }
            Err(e) => {
                let err = e.to_string();
                let _ = slint::invoke_from_event_loop(move || crate::app::update_failed(&err));
            }
        }
    });
}
