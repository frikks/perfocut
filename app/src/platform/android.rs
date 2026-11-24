//! Android: окно на Slint внутри NativeActivity, системные функции через Java-класс MainActivity.

use super::SaveOutcome;
use crate::{AppWindow, Prefs, Theme};
use jni::objects::{JByteArray, JClass, JObject, JObjectArray, JString, JValue};
use jni::{JNIEnv, JavaVM};
use slint::ComponentHandle;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

static VM: AtomicUsize = AtomicUsize::new(0);
static ACTIVITY: AtomicUsize = AtomicUsize::new(0);
static DATA_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
static SETTINGS_SEQ: AtomicUsize = AtomicUsize::new(0);

type ProfileCb = Box<dyn FnOnce(Option<Vec<u8>>) + Send + 'static>;
type BatchCb = Box<dyn FnOnce(Vec<(String, Vec<u8>)>) + Send + 'static>;
static PROFILE_CB: OnceLock<Mutex<Option<ProfileCb>>> = OnceLock::new();
static BATCH_CB: OnceLock<Mutex<Option<BatchCb>>> = OnceLock::new();

fn slot<T>(s: &'static OnceLock<Mutex<Option<T>>>) -> &'static Mutex<Option<T>> {
    s.get_or_init(|| Mutex::new(None))
}

/// Запоминает Java-машину и активность до запуска интерфейса.
pub fn set_app(app: &slint::android::AndroidApp) {
    VM.store(app.vm_as_ptr() as usize, Ordering::Relaxed);
    ACTIVITY.store(app.activity_as_ptr() as usize, Ordering::Relaxed);
    let _ = DATA_DIR.set(app.internal_data_path());
}

/// Вызов метода активности. Ошибки Java гасим: приложение работает дальше без функции.
fn with_activity<R>(
    f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> jni::errors::Result<R>,
) -> Option<R> {
    let vm_ptr = VM.load(Ordering::Relaxed) as *mut jni::sys::JavaVM;
    let act_ptr = ACTIVITY.load(Ordering::Relaxed) as jni::sys::jobject;
    if vm_ptr.is_null() || act_ptr.is_null() {
        return None;
    }
    // Указатели получены от android-activity и живут всё время работы приложения
    let vm = unsafe { JavaVM::from_raw(vm_ptr) }.ok()?;
    let mut env = vm.attach_current_thread_permanently().ok()?;
    let activity = unsafe { JObject::from_raw(act_ptr) };
    let r = env.with_local_frame(16, |env| f(env, &activity));
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    r.ok()
}

fn call_void(name: &str, sig: &str, args: &[JValue<'_, '_>]) {
    let _ = with_activity(|env, act| env.call_method(act, name, sig, args).map(|_| ()));
}

fn call_str(env: &mut JNIEnv<'_>, act: &JObject<'_>, name: &str) -> jni::errors::Result<String> {
    let o = env
        .call_method(act, name, "()Ljava/lang/String;", &[])?
        .l()?;
    if o.is_null() {
        return Ok(String::new());
    }
    Ok(env.get_string(&JString::from(o))?.into())
}

pub fn init(ui: &AppWindow) {
    let prefs = ui.global::<Prefs>();
    prefs.set_is_phone(true);
    prefs.set_can_minimize(true);
    prefs.set_can_drop(false);
    let theme = ui.global::<Theme>();
    theme.set_touch(true);
    if let Some(k) = with_activity(|env, act| env.call_method(act, "fontScale", "()F", &[])?.f()) {
        theme.set_font_scale((k.clamp(0.8, 1.5) * 0.92).clamp(0.72, 1.35));
    }
}

pub fn pick_file() {
    call_void("pickFile", "()V", &[]);
}

/// Выбрать JSON-профиль: Java вернёт байты через onProfilePicked.
pub fn pick_profile_file(cb: impl FnOnce(Option<Vec<u8>>) + Send + 'static) {
    *slot(&PROFILE_CB).lock().unwrap() = Some(Box::new(cb));
    call_void("pickProfile", "()V", &[]);
}

/// Выбрать несколько картинок: Java вернёт имена и байты через onBatchPicked.
pub fn pick_batch_files(cb: impl FnOnce(Vec<(String, Vec<u8>)>) + Send + 'static) {
    *slot(&BATCH_CB).lock().unwrap() = Some(Box::new(cb));
    call_void("pickBatch", "()V", &[]);
}

pub fn save_file(name: &str, mime: &str, bytes: Vec<u8>) -> SaveOutcome {
    let r = with_activity(|env, act| {
        let jn = env.new_string(name)?;
        let jm = env.new_string(mime)?;
        let jb = env.byte_array_from_slice(&bytes)?;
        let o = env
            .call_method(
                act,
                "saveFile",
                "(Ljava/lang/String;Ljava/lang/String;[B)Ljava/lang/String;",
                &[
                    JValue::Object(&jn),
                    JValue::Object(&jm),
                    JValue::Object(&jb),
                ],
            )?
            .l()?;
        if o.is_null() {
            return Ok(String::from("!"));
        }
        Ok(env.get_string(&JString::from(o))?.into())
    });
    match r {
        Some(s) if !s.starts_with('!') => SaveOutcome::Saved(s),
        Some(s) => SaveOutcome::Failed(s.trim_start_matches('!').to_owned()),
        None => SaveOutcome::Failed("Android".into()),
    }
}

pub fn vibrate(kind: &str) {
    let _ = with_activity(|env, act| {
        let k = env.new_string(kind)?;
        env.call_method(
            act,
            "vibrate",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&k)],
        )
        .map(|_| ())
    });
}

pub fn open_url(url: &str) {
    let _ = with_activity(|env, act| {
        let u = env.new_string(url)?;
        env.call_method(
            act,
            "openUrl",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&u)],
        )
        .map(|_| ())
    });
}

/// «Назад» на главном экране: сворачиваем, как делают системные приложения.
pub fn minimize(_ui: &AppWindow) {
    call_void("goBack", "()V", &[]);
}

/// Полный экран рисования: горизонтальная ориентация и скрытые системные панели.
pub fn set_full_canvas(_ui: &AppWindow, on: bool) {
    call_void("fullCanvas", "(Z)V", &[JValue::Bool(u8::from(on))]);
}

fn settings_path() -> Option<PathBuf> {
    DATA_DIR
        .get()
        .cloned()
        .flatten()
        .map(|d| d.join("settings.json"))
}

pub fn load_settings() -> Option<String> {
    std::fs::read_to_string(settings_path()?).ok()
}

pub fn store_settings(s: &str) {
    let Some(path) = settings_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Уникальное имя на случай параллельных записей
    let tmp = path.with_file_name(format!(
        "settings.{}.tmp",
        SETTINGS_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    if std::fs::write(&tmp, s).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub fn system_locale() -> String {
    with_activity(|env, act| call_str(env, act, "locale")).unwrap_or_default()
}

/// Цвет строки состояния и панели навигации под тему.
pub fn set_bars(bg: slint::Color, light: bool) {
    let argb = (u32::from(bg.alpha()) << 24)
        | (u32::from(bg.red()) << 16)
        | (u32::from(bg.green()) << 8)
        | u32::from(bg.blue());
    call_void(
        "setBars",
        "(IZ)V",
        &[JValue::Int(argb as i32), JValue::Bool(u8::from(light))],
    );
}

/// Проверка обновлений: запрос делает Java (HttpURLConnection), ответ вернёт `onUpdateInfo`.
pub fn check_update() {
    call_void("checkUpdate", "()V", &[]);
}

/// Скачивание обновления: Java кладёт файл в «Загрузки» и сообщает имя через `onDownloadDone`.
pub fn download_update(_release_url: &str, version: &str) {
    let file = format!("PerfoCut-{version}.apk");
    let url = crate::update::asset_url(version, "PerfoCut.apk");
    let _ = with_activity(|env, act| {
        let ju = env.new_string(&url)?;
        let jn = env.new_string(&file)?;
        env.call_method(
            act,
            "downloadUpdate",
            "(Ljava/lang/String;Ljava/lang/String;)V",
            &[JValue::Object(&ju), JValue::Object(&jn)],
        )
        .map(|_| ())
    });
}

/// Java сообщает имя скачанного файла или ошибку после `!`.
#[no_mangle]
pub extern "system" fn Java_com_perfocut_app_MainActivity_onDownloadDone<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    data: JString<'l>,
) {
    let s: String = env.get_string(&data).map(Into::into).unwrap_or_default();
    let _ = slint::invoke_from_event_loop(move || match s.strip_prefix('!') {
        Some(err) => crate::app::update_download_failed(err),
        None => crate::app::update_downloaded(&s),
    });
}

/// Java передаёт JSON последнего стабильного релиза или строку с ошибкой после `!`.
#[no_mangle]
pub extern "system" fn Java_com_perfocut_app_MainActivity_onUpdateInfo<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    data: JString<'l>,
) {
    let s: String = env.get_string(&data).map(Into::into).unwrap_or_default();
    let _ = slint::invoke_from_event_loop(move || match s.strip_prefix('!') {
        Some(err) => crate::app::update_failed(err),
        None => crate::app::update_json(&s),
    });
}

/// Java передаёт выбранный файл. Вызывается не из потока интерфейса.
#[no_mangle]
pub extern "system" fn Java_com_perfocut_app_MainActivity_onFilePicked<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    name: JString<'l>,
    data: JByteArray<'l>,
) {
    let name: String = env
        .get_string(&name)
        .map(Into::into)
        .unwrap_or_else(|_| "file".into());
    let Ok(bytes) = env.convert_byte_array(&data) else {
        return;
    };
    let _ = slint::invoke_from_event_loop(move || crate::app::open_bytes(name, bytes));
}

/// Java передал байты выбранного JSON-профиля (пустой массив = отмена).
#[no_mangle]
pub extern "system" fn Java_com_perfocut_app_MainActivity_onProfilePicked<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
    data: JByteArray<'l>,
) {
    let bytes = env.convert_byte_array(&data).unwrap_or_default();
    let cb = slot(&PROFILE_CB).lock().unwrap().take();
    if let Some(cb) = cb {
        let _ = slint::invoke_from_event_loop(move || {
            cb(if bytes.is_empty() { None } else { Some(bytes) })
        });
    }
}

/// Java передал имена и байты картинок для пакетной обработки.
#[no_mangle]
pub extern "system" fn Java_com_perfocut_app_MainActivity_onBatchPicked<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    names: JObjectArray<'l>,
    datas: JObjectArray<'l>,
) {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    if let Ok(n) = env.get_array_length(&names) {
        for i in 0..n {
            let Ok(name_obj) = env.get_object_array_element(&names, i) else {
                continue;
            };
            let name = env
                .get_string(&JString::from(name_obj))
                .map(Into::into)
                .unwrap_or_else(|_| "file".to_owned());
            let Ok(data_obj) = env.get_object_array_element(&datas, i) else {
                continue;
            };
            if let Ok(bytes) = env.convert_byte_array(&JByteArray::from(data_obj)) {
                files.push((name, bytes));
            }
        }
    }
    let cb = slot(&BATCH_CB).lock().unwrap().take();
    if let Some(cb) = cb {
        let _ = slint::invoke_from_event_loop(move || cb(files));
    }
}
