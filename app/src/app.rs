//! Связка интерфейса и ядра: состояние программы, обработчики кнопок, расчёт и предпросмотр.

mod draw;

use crate::convert;
use crate::files::{self, Format, FORMATS};
use crate::i18n::{self, fmt_n, fmt_time, t, tf};
use crate::platform::{self, SaveOutcome};
use crate::render::{self, Look, Rgb, Tf, View};
use crate::saved::{self, Saved};
use crate::{AppWindow, Bridge, Fmt, Prefs, SizeRow, StatsUI, Store, Theme, KV};
use perfo_core::gen::{self, GenResult};
use perfo_core::image::Gray;
use perfo_core::params::Params;
use perfo_core::sketch::Sketch;
use slint::{ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, Timer, TimerMode, VecModel, Weak};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const SHEET_DEFAULT: Rgb = Rgb(0x9a, 0x9e, 0xa3);
const BACK_DEFAULT: Rgb = Rgb(0xf2, 0xc4, 0x8d);

/// Загруженная картинка: исходные байты нужны для файла заказа.
struct Pic {
    name: String,
    /// Исходный файл; у чистого холста пусто
    bytes: Vec<u8>,
    w0: u32,
    h0: u32,
    /// Что видно и по чему считается узор: картинка вместе со штрихами
    pix: tiny_skia::Pixmap,
    gray: Arc<Gray>,
    /// Рисунок от руки поверх картинки или на чистом холсте
    sk: Option<draw::Sk>,
}

#[cfg(not(target_arch = "wasm32"))]
struct Job {
    ver: u64,
    gray: Arc<Gray>,
    p: Params,
}

struct State {
    ui: Weak<AppWindow>,
    p: Params,
    saved: Saved,
    pic: Option<Pic>,
    res: Option<Arc<GenResult>>,
    // Номер версии настроек и картинки; результат подходит, только если номера совпадают
    p_ver: u64,
    res_ver: u64,
    view: View,
    pinch_z: f64,
    gen_timer: Timer,
    draw_timer: Timer,
    toast_timer: Timer,
    ink_timer: Timer,
    width_timer: Timer,
    live: Option<draw::Live>,
    /// Был ли рисунок открыт до полного экрана: чтобы вернуть как было
    full_had_drawing: bool,
    /// Страница последнего стабильного релиза, если проверка нашла новую версию
    update_url: String,
    last_haptic: f64,
    formats: Rc<VecModel<Fmt>>,
    #[cfg(not(target_arch = "wasm32"))]
    jobs: std::sync::mpsc::Sender<Job>,
}

thread_local! {
    static ST: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Доступ к состоянию из обработчиков. Повторный вход (например, из окна выбора файла) просто пропускается.
fn with<R>(f: impl FnOnce(&mut State, &AppWindow) -> R) -> Option<R> {
    ST.with(|cell| {
        let mut guard = cell.try_borrow_mut().ok()?;
        let st = guard.as_mut()?;
        let ui = st.ui.upgrade()?;
        Some(f(st, &ui))
    })
}

fn r1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn differs(a: f64, b: f64) -> bool {
    (a - b).abs() > 1e-9
}

fn fix_default_name(p: &mut Params, en: bool) {
    if p.name == "Панель" || p.name == "Panel" {
        p.name = if en { "Panel" } else { "Панель" }.into();
    }
}

fn to_slint(c: Rgb) -> slint::Color {
    slint::Color::from_rgb_u8(c.0, c.1, c.2)
}

fn sk(c: slint::Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.red(), c.green(), c.blue(), c.alpha())
}

fn format_rows(saved: &Saved) -> Vec<Fmt> {
    FORMATS
        .iter()
        .map(|f| Fmt { title: t(f.title).into(), note: t(f.note).into(), on: saved.formats.get(f.id).copied().unwrap_or(f.on) })
        .collect()
}

pub fn run() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let mut saved = Saved::load();
    if saved.lang != "ru" && saved.lang != "en" {
        saved.lang = saved::lang_from_locale(&platform::system_locale()).into();
    }
    let en = saved.lang == "en";
    i18n::set_en(en);
    let _ = slint::select_bundled_translation(if en { "en" } else { "" });
    let mut p = saved.params.clone().unwrap_or_default();
    p.lang = saved.lang.clone();
    fix_default_name(&mut p, en);
    saved.theme = saved.theme.clamp(0, 3);
    saved.view_mode = saved.view_mode.clamp(0, 2);
    let sheet = Rgb::parse(&saved.sheet_color).unwrap_or(SHEET_DEFAULT);
    let back = Rgb::parse(&saved.back_color).unwrap_or(BACK_DEFAULT);

    let prefs = ui.global::<Prefs>();
    prefs.set_theme(saved.theme);
    prefs.set_lang(i32::from(en));
    prefs.set_vibro(saved.vibro);
    prefs.set_version(env!("CARGO_PKG_VERSION").into());
    ui.global::<Theme>().set_mode(saved.theme);
    platform::init(&ui);

    let store = ui.global::<Store>();
    store.set_p(convert::to_par(&p));
    store.set_lock_ratio(saved.lock_ratio);
    store.set_sheet_color(to_slint(sheet));
    store.set_back_color(to_slint(back));
    store.set_sheet_hex(sheet.hex().into());
    store.set_back_hex(back.hex().into());
    store.set_view_mode(saved.view_mode);
    store.set_brush_size(saved.brush_size as f32);
    store.set_brush_soft(saved.brush_soft as f32);
    store.set_brush_tone(saved.brush_tone as f32);
    let formats = Rc::new(VecModel::from(format_rows(&saved)));
    store.set_formats(ModelRc::from(formats.clone()));

    bind(&ui);
    let st = State {
        ui: ui.as_weak(),
        p,
        saved,
        pic: None,
        res: None,
        p_ver: 1,
        res_ver: 0,
        view: View::default(),
        pinch_z: 1.0,
        gen_timer: Timer::default(),
        draw_timer: Timer::default(),
        toast_timer: Timer::default(),
        ink_timer: Timer::default(),
        width_timer: Timer::default(),
        live: None,
        full_had_drawing: false,
        update_url: String::new(),
        last_haptic: 0.0,
        formats,
        #[cfg(not(target_arch = "wasm32"))]
        jobs: start_worker(),
    };
    ST.with(|cell| *cell.borrow_mut() = Some(st));
    with(|st, ui| {
        show_meta(st, ui);
        show_stats(st, ui);
        bars(ui);
    });
    ui.run()
}

/// Расчёт в отдельном потоке: интерфейс не замирает на больших листах.
#[cfg(not(target_arch = "wasm32"))]
fn start_worker() -> std::sync::mpsc::Sender<Job> {
    let (tx, rx) = std::sync::mpsc::channel::<Job>();
    std::thread::spawn(move || {
        while let Ok(mut job) = rx.recv() {
            // Пока считали, могли прийти новые настройки: берём только последние
            while let Ok(next) = rx.try_recv() {
                job = next;
            }
            let r = gen::generate(Some(&job.gray), &job.p);
            let ver = job.ver;
            let sent = slint::invoke_from_event_loop(move || {
                with(|st, ui| apply_result(st, ui, ver, r));
            });
            if sent.is_err() {
                break;
            }
        }
    });
    tx
}

fn bind(ui: &AppWindow) {
    let b = ui.global::<Bridge>();
    b.on_fmt(|v, d| i18n::fmt_field(f64::from(v), d.clamp(0, 6) as usize).into());
    b.on_fmt_n(|v, d| i18n::fmt_short(f64::from(v), d.clamp(0, 6) as usize).into());
    b.on_num_ok(|s| i18n::parse_num(&s).is_some());
    b.on_num(|s| i18n::parse_num(&s).unwrap_or(0.0) as f32);
    b.on_haptic(|kind| {
        with(|st, ui| haptic(st, ui, &kind));
    });

    let s = ui.global::<Store>();
    s.on_changed(|| {
        with(params_changed);
    });
    s.on_set_holes(|v| {
        with(|st, ui| set_holes(st, ui, f64::from(v)));
    });
    s.on_open(platform::pick_file);
    s.on_sample(|| match files::sample_png() {
        Ok(bytes) => {
            set_picture(format!("{}.png", t("пример")), bytes);
        }
        Err(e) => show_error(&t("Не удалось открыть картинку: "), &e),
    });
    s.on_aspect(|| {
        with(aspect);
    });
    s.on_fit_dmax(|| {
        with(|st, ui| {
            let mut p = st.p.clone();
            p.d_max = r2(p.pitch - p.bridge).max(0.0);
            set_params(st, ui, p, true);
        });
    });
    s.on_trace(|| {
        with(trace);
    });
    s.on_reset(|| {
        with(reset);
    });
    s.on_image_clear(|| {
        with(clear_image);
    });
    s.on_full_canvas_changed(|on| {
        with(|st, ui| set_full_canvas(st, ui, on));
    });
    s.on_check_update(|| {
        let started = with(|st, ui| {
            let store = ui.global::<Store>();
            if store.get_update_state() == 1 {
                return false;
            }
            store.set_update_state(1);
            store.set_update_note(t("Проверяю…").into());
            st.update_url.clear();
            true
        });
        if started == Some(true) {
            platform::check_update();
        }
    });
    s.on_update_open(|| {
        with(|st, _| {
            if !st.update_url.is_empty() {
                platform::open_url(&st.update_url);
            }
        });
    });
    s.on_view_changed(|| {
        with(|st, ui| {
            st.saved.view_mode = ui.global::<Store>().get_view_mode();
            st.saved.store();
            request_draw(st);
        });
    });
    s.on_canvas_size(|_, _| {
        with(|st, _| request_draw(st));
    });
    s.on_pan(|dx, dy| {
        with(|st, _| {
            st.view.x += f64::from(dx);
            st.view.y += f64::from(dy);
            request_draw(st);
        });
    });
    s.on_wheel(|x, y, dy| {
        with(|st, ui| {
            // Колесо мыши: шаг около 12 % за щелчок
            zoom(st, ui, f64::from(x), f64::from(y), (f64::from(dy) * 0.002).exp());
        });
    });
    s.on_pinch_start(|| {
        with(|st, _| st.pinch_z = st.view.z);
    });
    s.on_pinch(|scale, x, y| {
        with(|st, ui| {
            let f = st.pinch_z * f64::from(scale) / st.view.z;
            zoom(st, ui, f64::from(x), f64::from(y), f);
        });
    });
    s.on_fit_view(|| {
        with(|st, _| {
            st.view = View::default();
            request_draw(st);
        });
    });
    s.on_set_color(|i, hex| {
        with(|st, ui| set_color(st, ui, i, &hex));
    });
    s.on_format_toggled(|i, on| {
        with(|st, _| {
            let Ok(idx) = usize::try_from(i) else { return };
            let (Some(mut row), Some(f)) = (st.formats.row_data(idx), FORMATS.get(idx)) else { return };
            row.on = on;
            st.formats.set_row_data(idx, row);
            st.saved.formats.insert(f.id.to_owned(), on);
            st.saved.store();
        });
    });
    s.on_export_one(export_one);
    s.on_export_zip(|| {
        let ok = with(|st, ui| {
            if ready(st, ui).is_none() {
                return false;
            }
            if selected(st).is_empty() {
                toast(st, ui, &t("Отметьте хотя бы один формат"), 2500);
                return false;
            }
            toast(st, ui, &t("Собираю архив…"), 1500);
            true
        });
        if ok == Some(true) {
            // Даём окну показать сообщение до долгой сборки архива
            Timer::single_shot(Duration::from_millis(40), export_zip);
        }
    });
    s.on_save_order(save_order);
    draw::bind(ui);

    let pr = ui.global::<Prefs>();
    pr.on_changed(|| {
        with(prefs_changed);
    });
    pr.on_open_url(|url| platform::open_url(&url));
    pr.on_minimize(|| {
        with(|_, ui| platform::minimize(ui));
    });
}

fn haptic(st: &mut State, ui: &AppWindow, kind: &str) {
    if !st.saved.vibro || !ui.global::<Prefs>().get_is_phone() {
        return;
    }
    let now = platform::now_ms();
    let gap = if kind == "tick" { 28.0 } else { 60.0 };
    if now - st.last_haptic < gap {
        return;
    }
    st.last_haptic = now;
    platform::vibrate(kind);
}

fn toast(st: &mut State, ui: &AppWindow, text: &str, ms: u64) {
    let store = ui.global::<Store>();
    store.set_toast(text.into());
    store.set_toast_shown(true);
    let w = st.ui.clone();
    st.toast_timer.start(TimerMode::SingleShot, Duration::from_millis(ms), move || {
        if let Some(ui) = w.upgrade() {
            ui.global::<Store>().set_toast_shown(false);
        }
    });
}

/// Сообщение об ошибке из любого места программы.
pub fn show_error(prefix: &str, err: &str) {
    let text = format!("{prefix}{err}");
    with(|st, ui| toast(st, ui, &text, 5000));
}

// ---------- Обновления ----------

/// Ответ GitHub с последним стабильным релизом. Вызывается из потока или из веб-асинхронщины.
pub(crate) fn update_json(json: &str) {
    let current = env!("CARGO_PKG_VERSION");
    match crate::update::parse(json) {
        Ok(rel) => {
            let newer = crate::update::is_newer(&rel.version, current);
            let note = if newer {
                tf("Доступна версия {v}", &[("v", &rel.version)])
            } else {
                tf("Установлена последняя версия {v}", &[("v", current)])
            };
            with(|st, ui| {
                let store = ui.global::<Store>();
                store.set_update_state(if newer { 3 } else { 2 });
                store.set_update_note(note.as_str().into());
                st.update_url = if newer { rel.url } else { String::new() };
                if newer {
                    toast(st, ui, &note, 4000);
                }
            });
        }
        Err(e) => update_failed(&e),
    }
}

/// Проверка не удалась: сеть, нет стабильных релизов, непонятный ответ.
pub(crate) fn update_failed(detail: &str) {
    let base = t("Не удалось проверить обновления");
    let note = if detail.trim().is_empty() { base } else { format!("{base}: {detail}") };
    with(|st, ui| {
        let store = ui.global::<Store>();
        store.set_update_state(4);
        store.set_update_note(note.as_str().into());
        st.update_url.clear();
        toast(st, ui, &note, 5000);
    });
}

// ---------- Настройки ----------

fn params_changed(st: &mut State, ui: &AppWindow) {
    let store = ui.global::<Store>();
    let par = store.get_p();
    let lock = store.get_lock_ratio();
    let mut np = convert::to_params(&par, &st.p);
    let mut push = par.gc_decimals != np.gc_decimals as i32;
    if lock && (!st.saved.lock_ratio || st.saved.ratio <= 0.0) {
        st.saved.ratio = np.sheet_w / np.sheet_h;
    } else if lock {
        if differs(np.sheet_w, st.p.sheet_w) && np.sheet_w > 0.0 {
            np.sheet_h = r1(np.sheet_w / st.saved.ratio);
            push = true;
        } else if differs(np.sheet_h, st.p.sheet_h) && np.sheet_h > 0.0 {
            np.sheet_w = r1(np.sheet_h * st.saved.ratio);
            push = true;
        }
    }
    st.saved.lock_ratio = lock;
    set_params(st, ui, np, push);
}

fn set_params(st: &mut State, ui: &AppWindow, p: Params, push: bool) {
    st.p = p;
    st.p_ver += 1;
    if push {
        ui.global::<Store>().set_p(convert::to_par(&st.p));
    }
    show_meta(st, ui);
    schedule_gen(st);
}

fn long_side(p: &Params) -> f64 {
    p.sheet_w.max(p.sheet_h) - 2.0 * p.border
}

fn show_meta(st: &State, ui: &AppWindow) {
    let p = &st.p;
    let store = ui.global::<Store>();
    store.set_sheet_meta(format!("{} × {}", fmt_n(p.sheet_w, 0), fmt_n(p.sheet_h, 0)).into());
    store.set_metal_meta(format!("{} {}", fmt_n(p.thickness, 1), t("мм")).into());
    let long = long_side(p);
    let n = if p.pitch > 0.0 && long > 0.0 { (long / p.pitch).round() } else { 0.0 };
    store.set_holes_n(n.min(1e6) as i32);
    // Самая широкая кисть — треть длинной стороны листа
    store.set_brush_max((p.sheet_w.max(p.sheet_h) / 3.0).clamp(20.0, 5000.0) as f32);
}

fn set_holes(st: &mut State, ui: &AppWindow, v: f64) {
    let n = v.round();
    let long = long_side(&st.p);
    if n < 1.0 || long <= 0.0 {
        return;
    }
    let mut p = st.p.clone();
    p.pitch = r2(long / n).max(0.5);
    set_params(st, ui, p, true);
}

fn aspect(st: &mut State, ui: &AppWindow) {
    let Some(pic) = &st.pic else { return };
    let mut p = st.p.clone();
    p.sheet_h = (p.sheet_w * f64::from(pic.h0) / f64::from(pic.w0)).round().max(1.0);
    st.saved.ratio = p.sheet_w / p.sheet_h;
    set_params(st, ui, p, true);
}

fn trace(st: &mut State, ui: &AppWindow) {
    let mut p = st.p.clone();
    p.grid = "square".into();
    p.shape = "circle".into();
    p.size_mode = "prop".into();
    p.fixed = false;
    p.tone_cut = 0.05;
    p.d_max = r2((p.pitch * 1.3 / 1.5).min(p.pitch - p.bridge)).max(0.0);
    set_params(st, ui, p, true);
    toast(st, ui, &t("Как в SVG tracing: квадратная сетка, круги, диаметр пропорционален тону"), 4000);
}

fn reset(st: &mut State, ui: &AppWindow) {
    let old = &st.p;
    let mut p = Params {
        name: old.name.clone(),
        order_no: old.order_no.clone(),
        client: old.client.clone(),
        phone: old.phone.clone(),
        email: old.email.clone(),
        comment: old.comment.clone(),
        lang: old.lang.clone(),
        ..Params::default()
    };
    fix_default_name(&mut p, i18n::is_en());
    st.saved.ratio = p.sheet_w / p.sheet_h;
    set_params(st, ui, p, true);
    toast(st, ui, &t("Настройки сброшены"), 2500);
}

fn prefs_changed(st: &mut State, ui: &AppWindow) {
    let pr = ui.global::<Prefs>();
    st.saved.theme = pr.get_theme().clamp(0, 3);
    st.saved.vibro = pr.get_vibro();
    let lang = if pr.get_lang() == 1 { "en" } else { "ru" };
    if lang != st.saved.lang {
        set_lang(st, ui, lang);
    }
    st.saved.store();
    bars(ui);
    request_draw(st);
}

fn set_lang(st: &mut State, ui: &AppWindow, lang: &str) {
    let en = lang == "en";
    st.saved.lang = lang.into();
    i18n::set_en(en);
    let _ = slint::select_bundled_translation(if en { "en" } else { "" });
    for (i, row) in format_rows(&st.saved).into_iter().enumerate() {
        st.formats.set_row_data(i, row);
    }
    let mut p = st.p.clone();
    p.lang = lang.into();
    fix_default_name(&mut p, en);
    set_params(st, ui, p, true);
    show_stats(st, ui);
}

fn bars(ui: &AppWindow) {
    let th = ui.global::<Theme>();
    platform::set_bars(th.get_bg(), !th.get_dark());
}

fn set_color(st: &mut State, ui: &AppWindow, which: i32, hex: &str) {
    let Some(c) = Rgb::parse(hex) else { return };
    let store = ui.global::<Store>();
    if which == 0 {
        st.saved.sheet_color = c.hex();
        store.set_sheet_color(to_slint(c));
        store.set_sheet_hex(c.hex().into());
    } else {
        st.saved.back_color = c.hex();
        store.set_back_color(to_slint(c));
        store.set_back_hex(c.hex().into());
    }
    st.saved.store();
    request_draw(st);
}

/// Холст на весь экран. На телефоне — горизонтальная ориентация и скрытые панели,
/// на компьютере — окно без рамки. Заодно открываем рисование, если его ещё не было.
fn set_full_canvas(st: &mut State, ui: &AppWindow, on: bool) {
    let store = ui.global::<Store>();
    if on {
        st.full_had_drawing = store.get_drawing();
        platform::set_full_canvas(ui, true);
        if !st.full_had_drawing {
            draw::open(st, ui);
        }
    } else {
        platform::set_full_canvas(ui, false);
        if !st.full_had_drawing {
            draw::close(st, ui);
        }
    }
    // Холст стал другого размера: лист снова целиком в окне
    st.view = View::default();
    request_draw(st);
}

// ---------- Картинка и заказ ----------

/// Открыть файл: картинку или заказ .perfo. Вызывается в потоке интерфейса.
pub fn open_bytes(name: String, bytes: Vec<u8>) {
    let lower = name.to_lowercase();
    if lower.ends_with(".perfo") || lower.ends_with(".zip") {
        open_order(&name, &bytes);
    } else {
        set_picture(name, bytes);
    }
}

fn set_picture(name: String, bytes: Vec<u8>) -> bool {
    set_picture_with(name, bytes, None)
}

/// Картинка из файла; `sketch` — рисунок поверх неё из файла заказа.
fn set_picture_with(name: String, bytes: Vec<u8>, sketch: Option<Sketch>) -> bool {
    let d = match files::decode(&bytes) {
        Ok(d) => d,
        Err(e) => {
            show_error(&t("Не удалось открыть картинку: "), &e);
            return false;
        }
    };
    with(move |st, ui| {
        let mut pic = Pic { name, bytes, w0: d.w0, h0: d.h0, pix: d.pix, gray: Arc::new(d.gray), sk: None };
        if let Some(sk) = sketch {
            draw::attach(&mut pic, sk);
        }
        install(st, ui, pic);
    })
    .is_some()
}

/// Новая картинка или холст вместо прежних: сброс вида и пересчёт узора.
fn install(st: &mut State, ui: &AppWindow, pic: Pic) {
    let store = ui.global::<Store>();
    store.set_drawing(false);
    store.set_ink(Image::default());
    st.live = None;
    if st.saved.lock_ratio {
        st.saved.ratio = st.p.sheet_w / st.p.sheet_h;
    }
    st.pic = Some(pic);
    st.view = View::default();
    st.res = None;
    st.p_ver += 1;
    draw::show_pic(st, ui);
    show_stats(st, ui);
    st.gen_timer.stop();
    run_gen(st, ui);
}

/// Убрать картинку вместе со штрихами и сразу рисовать на чистом холсте.
fn clear_image(st: &mut State, ui: &AppWindow) {
    if st.pic.is_none() {
        return;
    }
    // Старую картинку выбрасываем; open() сделает пустой холст по размеру листа
    st.pic = None;
    draw::open(st, ui);
    toast(st, ui, &t("Холст очищен"), 2200);
}

fn open_order(name: &str, bytes: &[u8]) {
    let order = match files::read_order(bytes) {
        Ok(o) => o,
        Err(e) => {
            show_error("", &e);
            return;
        }
    };
    with(|st, ui| {
        let mut p = order.params;
        p.lang = st.saved.lang.clone();
        if st.saved.lock_ratio {
            st.saved.ratio = p.sheet_w / p.sheet_h;
        }
        set_params(st, ui, p, true);
    });
    let (source, sketch) = (order.source, order.sketch);
    if order.blank && sketch.is_some() {
        // Рисунок на чистом холсте: картинка в заказе — только его копия для старых версий
        let Some(pic) = sketch.and_then(draw::blank_pic) else { return };
        with(|st, ui| install(st, ui, pic));
    } else if let Some((src_name, data)) = source {
        if !set_picture_with(src_name, data, sketch) {
            return;
        }
    }
    let text = format!("{}{name}", t("Заказ открыт: "));
    with(|st, ui| toast(st, ui, &text, 2500));
}

// ---------- Расчёт ----------

fn schedule_gen(st: &mut State) {
    st.gen_timer.start(TimerMode::SingleShot, Duration::from_millis(90), || {
        with(run_gen);
    });
}

fn run_gen(st: &mut State, ui: &AppWindow) {
    st.saved.params = Some(st.p.clone());
    st.saved.store();
    let Some(pic) = &st.pic else {
        st.res = None;
        st.res_ver = st.p_ver;
        show_stats(st, ui);
        request_draw(st);
        return;
    };
    let ver = st.p_ver;
    #[cfg(target_arch = "wasm32")]
    {
        let r = gen::generate(Some(&pic.gray), &st.p);
        apply_result(st, ui, ver, r);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let job = Job { ver, gray: pic.gray.clone(), p: st.p.clone() };
        if st.jobs.send(job).is_err() {
            let r = gen::generate(Some(&pic.gray), &st.p);
            apply_result(st, ui, ver, r);
        }
    }
}

fn apply_result(st: &mut State, ui: &AppWindow, ver: u64, r: GenResult) {
    // Устаревший результат: уже посчитан или считается более новый
    if ver != st.p_ver || st.pic.is_none() {
        return;
    }
    st.res = Some(Arc::new(r));
    st.res_ver = ver;
    show_stats(st, ui);
    request_draw(st);
}

/// Результат для текущих настроек; если расчёт ещё идёт, считаем сразу.
fn fresh(st: &mut State, ui: &AppWindow) -> Option<Arc<GenResult>> {
    let pic = st.pic.as_ref()?;
    if st.res_ver != st.p_ver || st.res.is_none() {
        let r = gen::generate(Some(&pic.gray), &st.p);
        st.res = Some(Arc::new(r));
        st.res_ver = st.p_ver;
        show_stats(st, ui);
        request_draw(st);
    }
    st.res.clone()
}

fn stats_ui(st: &State) -> StatsUI {
    let mut s = StatsUI::default();
    if st.pic.is_none() {
        s.ok_line = t("Загрузите картинку").into();
        return s;
    }
    let Some(r) = &st.res else { return s };
    let x = &r.stats;
    let p = &st.p;
    let cur = p.currency.as_str();
    let lines = p.pattern == "lines";
    s.has = true;
    s.cost = fmt_n(x.cost, 0).into();
    s.currency = cur.into();
    let mut rows = vec![(t("Резка"), x.cost_cut), (t("Врезки"), x.cost_pierce)];
    if x.cost_material > 0.0 {
        rows.push((t("Металл"), x.cost_material));
    }
    if x.cost_fixed > 0.0 {
        rows.push((t("Подготовка"), x.cost_fixed));
    }
    let rows: Vec<KV> = rows.into_iter().map(|(k, v)| KV { k: k.into(), v: format!("{} {cur}", fmt_n(v, 0)).into() }).collect();
    s.rows = ModelRc::new(VecModel::from(rows));
    s.count = fmt_n(x.count as f64, 0).into();
    s.pierces = fmt_n(x.pierces as f64, 0).into();
    s.cut = format!("{} {}", fmt_n(x.cut_len_m, 1), t("м")).into();
    s.time = format!("≈ {}", fmt_time(x.time_min)).into();
    s.open = format!("{} %", fmt_n(x.open_area_pct, 1)).into();
    s.mass = format!("{} {}", fmt_n(x.weight_kg, 1), t("кг")).into();

    let bs = &x.by_size;
    let mx = bs.iter().map(|b| b.count).max().unwrap_or(1).max(1) as f64;
    let mut list: Vec<_> = bs.iter().collect();
    if bs.len() > 14 {
        list.sort_by_key(|b| std::cmp::Reverse(b.count));
        list.truncate(14);
        list.sort_by(|a, b| a.size.total_cmp(&b.size));
        s.sizes_note = tf("Показаны 14 из {n}. Увеличьте шаг размеров", &[("n", &bs.len().to_string())]).into();
    }
    let sizes: Vec<SizeRow> = list
        .iter()
        .map(|b| {
            let d = if (b.size - b.size.round()).abs() > 1e-9 { 1 } else { 0 };
            let label = if lines { format!("{} {}", fmt_n(b.size, d), t("мм")) } else { format!("Ø {}", fmt_n(b.size, d)) };
            SizeRow { label: label.into(), frac: (b.count as f64 / mx).max(0.02) as f32, n: fmt_n(b.count as f64, 0).into() }
        })
        .collect();
    s.sizes = ModelRc::new(VecModel::from(sizes));

    let mut warns: Vec<SharedString> = x.warnings.iter().map(|w| w.as_str().into()).collect();
    if x.removed_edge > 0 {
        warns.push(tf("{n} отв. у края не поместились в поле и пропущены", &[("n", &fmt_n(x.removed_edge as f64, 0))]).into());
    }
    s.warnings = ModelRc::new(VecModel::from(warns));
    let key = if lines {
        "Ширина от {a} до {b} мм, перемычка не меньше {c} мм"
    } else {
        "Размеры от {a} до {b} мм, перемычка не меньше {c} мм"
    };
    s.ok_line = tf(key, &[("a", &fmt_n(x.size_min, 1)), ("b", &fmt_n(x.size_max, 1)), ("c", &fmt_n(p.bridge, 1))]).into();
    s.hud = tf("{n} контуров · {p} %", &[("n", &fmt_n(x.count as f64, 0)), ("p", &fmt_n(x.open_area_pct, 1))]).into();
    s
}

fn show_stats(st: &State, ui: &AppWindow) {
    let store = ui.global::<Store>();
    store.set_stats(stats_ui(st));
    let ready = st.pic.is_some() && st.res.as_ref().is_some_and(|r| r.stats.count > 0);
    store.set_can_export(ready);
}

// ---------- Предпросмотр ----------

fn look(st: &State, ui: &AppWindow) -> Look {
    let th = ui.global::<Theme>();
    Look {
        sheet: Rgb::parse(&st.saved.sheet_color).unwrap_or(SHEET_DEFAULT),
        back: Rgb::parse(&st.saved.back_color).unwrap_or(BACK_DEFAULT),
        card: sk(th.get_card()),
        ink: sk(th.get_t1()),
        frame: sk(th.get_t3()),
        dim: sk(th.get_dim()),
    }
}

fn zoom(st: &mut State, ui: &AppWindow, x: f64, y: f64, f: f64) {
    let (cw, ch) = (f64::from(ui.get_cw()), f64::from(ui.get_ch()));
    st.view.zoom_at(cw, ch, x, y, f);
    request_draw(st);
}

// Частые события (перетаскивание, колесо) склеиваются в одну перерисовку
fn request_draw(st: &mut State) {
    st.draw_timer.start(TimerMode::SingleShot, Duration::ZERO, || {
        with(draw_now);
    });
}

fn draw_now(st: &mut State, ui: &AppWindow) {
    let store = ui.global::<Store>();
    let Some(pic) = &st.pic else {
        store.set_preview(Image::default());
        store.set_dim_w(SharedString::new());
        store.set_dim_h(SharedString::new());
        return;
    };
    let (cw, ch) = (f64::from(ui.get_cw()), f64::from(ui.get_ch()));
    if cw < 4.0 || ch < 4.0 {
        return;
    }
    // Чёткость по пикселям экрана, но не больше 2× в браузере, чтобы не тормозить
    let max_sf = if cfg!(target_arch = "wasm32") { 2.0 } else { 3.0 };
    let sf = f64::from(ui.window().scale_factor()).clamp(1.0, max_sf);
    let (pw, ph) = ((cw * sf).round() as u32, (ch * sf).round() as u32);
    let Some(mut pm) = tiny_skia::Pixmap::new(pw.max(1), ph.max(1)) else { return };
    let p = &st.p;
    let tf = Tf::fit(cw, ch, p, &st.view);
    let lk = look(st, ui);
    let ts = tiny_skia::Transform::from_scale(sf as f32, sf as f32);
    let shapes = st.res.as_ref().map(|r| r.shapes.as_slice()).unwrap_or(&[]);
    // Во время рисования виден сам рисунок и контуры отверстий поверх него
    let drawing = store.get_drawing();
    let mode = if drawing { 2 } else { store.get_view_mode() };
    render::paint_sheet(&mut pm, ts, &tf, (cw, ch), p, shapes, mode, &lk, Some(&pic.pix));
    render::paint_dims(&mut pm, ts, &tf, p, lk.dim);
    let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(pm.data(), pw.max(1), ph.max(1));
    store.set_preview(Image::from_rgba8_premultiplied(buf));
    let sh = p.sheet_h * tf.s;
    store.set_sx(tf.ox as f32);
    store.set_sy((tf.oy - sh) as f32);
    store.set_sw((p.sheet_w * tf.s) as f32);
    store.set_sh(sh as f32);
    let num = |v: f64| fmt_n(v, if (v - v.round()).abs() > 1e-9 { 1 } else { 0 });
    store.set_dim_w(format!("{} {}", num(p.sheet_w), t("мм")).into());
    store.set_dim_h(format!("{}\n{}", num(p.sheet_h), t("мм")).into());
    if drawing {
        if let Some(m) = draw::map(st, ui) {
            store.set_mm_px(m.px_per_mm() as f32);
        }
    }
}

// ---------- Экспорт ----------

fn ready(st: &mut State, ui: &AppWindow) -> Option<Arc<GenResult>> {
    if st.pic.is_none() {
        toast(st, ui, &t("Сначала загрузите картинку"), 2500);
        return None;
    }
    let r = fresh(st, ui)?;
    if r.stats.count == 0 {
        toast(st, ui, &t("Узор пустой: проверьте настройки"), 2500);
        return None;
    }
    Some(r)
}

fn selected(st: &State) -> Vec<&'static Format> {
    FORMATS.iter().enumerate().filter(|(i, _)| st.formats.row_data(*i).is_some_and(|r| r.on)).map(|(_, f)| f).collect()
}

/// Сохранить готовые байты. Окно сохранения может быть модальным, поэтому вне `with`.
fn save_out(name: String, mime: &str, bytes: Vec<u8>) {
    let res = platform::save_file(&name, mime, bytes);
    with(|st, ui| match res {
        SaveOutcome::Saved(path) => toast(st, ui, &format!("{}{path}", t("Сохранено: ")), 4000),
        SaveOutcome::Downloaded(n) => toast(st, ui, &format!("{}{n}", t("Скачано: ")), 2500),
        SaveOutcome::Cancelled => {}
        SaveOutcome::Failed(e) => toast(st, ui, &format!("{}{e}", t("Не удалось сохранить: ")), 5000),
    });
}

fn export_one(i: i32) {
    let out = with(|st, ui| {
        let f = usize::try_from(i).ok().and_then(|i| FORMATS.get(i))?;
        let r = ready(st, ui)?;
        match files::format_bytes(f, &r, &look(st, ui)) {
            Ok(b) => Some((format!("{}.{}", files::base_name(&r.params), f.ext), files::mime_for(f.ext), b)),
            Err(e) => {
                toast(st, ui, &format!("{}{e}", t("Ошибка экспорта: ")), 5000);
                None
            }
        }
    })
    .flatten();
    if let Some((name, mime, bytes)) = out {
        save_out(name, mime, bytes);
    }
}

fn export_zip() {
    let out = with(|st, ui| {
        let r = ready(st, ui)?;
        let sel = selected(st);
        match files::zip_formats(&sel, &r, &look(st, ui)) {
            Ok(b) => Some((format!("{}.zip", files::base_name(&r.params)), b)),
            Err(e) => {
                toast(st, ui, &format!("{}{e}", t("Ошибка экспорта: ")), 5000);
                None
            }
        }
    })
    .flatten();
    if let Some((name, bytes)) = out {
        save_out(name, "application/zip", bytes);
    }
}

fn save_order() {
    let out = with(|st, ui| {
        if st.pic.is_none() {
            toast(st, ui, &t("Сначала загрузите картинку"), 2500);
            return None;
        }
        let r = fresh(st, ui)?;
        let pic = st.pic.as_ref()?;
        let sketch = pic.sk.as_ref().map(|s| (&s.sketch, s.blank));
        // У чистого холста исходного файла нет: кладём готовую картинку, её откроют и старые версии
        let flat;
        let (src_name, src) = if pic.sk.as_ref().is_some_and(|s| s.blank) {
            flat = files::encode_png(&pic.pix).unwrap_or_default();
            (format!("{}.png", t("Рисунок")), flat.as_slice())
        } else {
            (pic.name.clone(), pic.bytes.as_slice())
        };
        match files::order_bytes(&r, &src_name, src, sketch, &look(st, ui), &platform::now_iso()) {
            Ok(b) => Some((format!("{}.perfo", files::base_name(&r.params)), b)),
            Err(e) => {
                toast(st, ui, &format!("{}{e}", t("Не удалось сохранить заказ: ")), 5000);
                None
            }
        }
    })
    .flatten();
    if let Some((name, bytes)) = out {
        save_out(name, "application/octet-stream", bytes);
    }
}
