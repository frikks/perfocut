//! Рисование от руки: штрихи кистью на чистом холсте или поверх картинки.
//!
//! Штрихи хранятся вектором (`perfo_core::sketch`). После каждого штриха рисунок
//! растеризуется в картинку, и перфорация считается по ней, как по обычному фото.

use super::*;
use perfo_core::sketch::{self, Sketch, Stroke};
use tiny_skia::{FillRule, IntSize, LineCap, Mask, Paint, PathBuilder, Pixmap, Rect, Transform};

/// Длинная сторона растра чистого холста, пикселей: как у загруженных картинок
const CANVAS_SIDE: usize = 1600;

enum Act {
    Add(Stroke),
    Clear(Vec<Stroke>),
}

/// Рисунок картинки: вектор штрихов, подложка для ластика и история правок.
pub(super) struct Sk {
    pub sketch: Sketch,
    /// Белый холст или исходная картинка
    pub base: Pixmap,
    /// Чистый холст, без картинки под штрихами
    pub blank: bool,
    undo: Vec<Act>,
    redo: Vec<Act>,
}

impl Sk {
    pub fn new(sketch: Sketch, base: Pixmap, blank: bool) -> Sk {
        Sk { sketch, base, blank, undo: Vec::new(), redo: Vec::new() }
    }

    /// Все штрихи поверх подложки.
    pub fn render(&self) -> Pixmap {
        let (w, h) = (self.base.width(), self.base.height());
        let data = self.sketch.render(self.base.data(), w as usize, h as usize);
        IntSize::from_wh(w, h).and_then(|s| Pixmap::from_vec(data, s)).unwrap_or_else(|| self.base.clone())
    }

    fn has_any(&self) -> bool {
        self.blank || !self.sketch.strokes.is_empty()
    }
}

/// Штрих, который рисуется сейчас: точки в мм и быстрый слой на экране.
pub(super) struct Live {
    raw: Vec<[f64; 2]>,
    ink: Pixmap,
    mask: Option<Mask>,
    last: (f32, f32),
    width: f32,
    color: tiny_skia::Color,
    erase: bool,
    size: f64,
    soft: f64,
    tone: f64,
}

impl Live {
    fn paint(&self) -> Paint<'static> {
        let mut p = Paint::default();
        p.set_color(self.color);
        p.anti_alias = true;
        p
    }

    fn dot(&mut self, x: f32, y: f32) {
        if let Some(path) = PathBuilder::from_circle(x, y, (self.width / 2.0).max(0.5)) {
            let paint = self.paint();
            self.ink.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), self.mask.as_ref());
        }
    }

    fn seg(&mut self, x: f32, y: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(self.last.0, self.last.1);
        pb.line_to(x, y);
        if let Some(path) = pb.finish() {
            let st = tiny_skia::Stroke { width: self.width.max(1.0), line_cap: LineCap::Round, ..tiny_skia::Stroke::default() };
            let paint = self.paint();
            self.ink.stroke_path(&path, &paint, &st, Transform::identity(), self.mask.as_ref());
        }
        self.last = (x, y);
    }
}

/// Перевод точек экрана в миллиметры холста.
pub(super) struct Map {
    s: f64,
    ox: f64,
    top: f64,
    l: f64,
    t: f64,
    dw: f64,
    dh: f64,
    kx: f64,
    ky: f64,
}

impl Map {
    fn to_mm(&self, x: f64, y: f64) -> [f64; 2] {
        [((x - self.ox) / self.s - self.l) * self.kx, ((y - self.top) / self.s - self.t) * self.ky]
    }

    /// Точек экрана в миллиметре холста
    pub fn px_per_mm(&self) -> f64 {
        self.s / self.kx
    }

    /// Где на экране видно рисунок: пересечение листа и холста.
    fn clip(&self, p: &Params) -> Option<Rect> {
        let x0 = self.l.max(0.0);
        let y0 = self.t.max(0.0);
        let x1 = (self.l + self.dw).min(p.sheet_w);
        let y1 = (self.t + self.dh).min(p.sheet_h);
        Rect::from_ltrb(
            (self.ox + x0 * self.s) as f32,
            (self.top + y0 * self.s) as f32,
            (self.ox + x1 * self.s) as f32,
            (self.top + y1 * self.s) as f32,
        )
    }
}

pub(super) fn map(st: &State, ui: &AppWindow) -> Option<Map> {
    let pic = st.pic.as_ref()?;
    let sk = pic.sk.as_ref()?;
    let (cw, ch) = (f64::from(ui.get_cw()), f64::from(ui.get_ch()));
    let tf = Tf::fit(cw, ch, &st.p, &st.view);
    let (l, t, dw, dh) = render::image_rect(&st.p, f64::from(pic.pix.width()), f64::from(pic.pix.height()));
    if dw <= 0.0 || dh <= 0.0 {
        return None;
    }
    Some(Map { s: tf.s, ox: tf.ox, top: tf.oy - st.p.sheet_h * tf.s, l, t, dw, dh, kx: sk.sketch.w / dw, ky: sk.sketch.h / dh })
}

/// Уменьшенная копия для карточки картинки: среднее по блокам, без лесенки.
pub(super) fn thumb(pm: &Pixmap) -> Image {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let k = (160.0 / w.max(h) as f64).min(1.0);
    let tw = ((w as f64 * k).round() as usize).max(1);
    let th = ((h as f64 * k).round() as usize).max(1);
    let src = pm.data();
    let mut out = vec![0u8; tw * th * 4];
    for ty in 0..th {
        let (y0, y1) = (ty * h / th, ((ty + 1) * h / th).max(ty * h / th + 1));
        for tx in 0..tw {
            let (x0, x1) = (tx * w / tw, ((tx + 1) * w / tw).max(tx * w / tw + 1));
            let mut acc = [0u32; 4];
            for y in y0..y1.min(h) {
                for x in x0..x1.min(w) {
                    let i = (y * w + x) * 4;
                    for c in 0..4 {
                        acc[c] += u32::from(src[i + c]);
                    }
                }
            }
            let n = ((y1.min(h) - y0) * (x1.min(w) - x0)).max(1) as u32;
            let o = (ty * tw + tx) * 4;
            for c in 0..4 {
                out[o + c] = (acc[c] / n) as u8;
            }
        }
    }
    Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&out, tw as u32, th as u32))
}

/// Карточка картинки и состояние кнопок рисования.
pub(super) fn show_pic(st: &State, ui: &AppWindow) {
    let store = ui.global::<Store>();
    let Some(pic) = &st.pic else {
        store.set_has_image(false);
        store.set_has_sketch(false);
        store.set_strokes_n(0);
        store.set_can_undo(false);
        store.set_can_redo(false);
        return;
    };
    store.set_has_image(true);
    store.set_thumb(thumb(&pic.pix));
    let sk = pic.sk.as_ref();
    let blank = sk.is_some_and(|s| s.blank);
    store.set_img_name(if blank { t("Рисунок") } else { pic.name.clone() }.into());
    let size = match sk {
        Some(s) if s.blank => format!("{} × {} {}", fmt_n(s.sketch.w, 0), fmt_n(s.sketch.h, 0), t("мм")),
        _ => format!("{} × {} px", pic.w0, pic.h0),
    };
    let n = sk.map_or(0, |s| s.sketch.strokes.len());
    store.set_img_size(size.into());
    store.set_has_sketch(sk.is_some_and(Sk::has_any));
    store.set_strokes_n(n.min(i32::MAX as usize) as i32);
    store.set_can_undo(sk.is_some_and(|s| !s.undo.is_empty()));
    store.set_can_redo(sk.is_some_and(|s| !s.redo.is_empty()));
    if let Some(s) = sk {
        store.set_width_k(s.sketch.width_k as f32);
    }
}

/// Чистый холст под рисунок `sketch`.
pub(super) fn blank_pic(sketch: Sketch) -> Option<Pic> {
    let (w, h) = sketch.raster_size(CANVAS_SIDE);
    let mut base = Pixmap::new(w as u32, h as u32)?;
    base.fill(tiny_skia::Color::WHITE);
    let sk = Sk::new(sketch, base, true);
    let pix = sk.render();
    let gray = Arc::new(Gray::from_rgba(pix.data(), w, h));
    Some(Pic { name: t("Рисунок"), bytes: Vec::new(), w0: w as u32, h0: h as u32, pix, gray, sk: Some(sk) })
}

/// Рисунок поверх только что открытой картинки (из файла заказа).
pub(super) fn attach(pic: &mut Pic, sketch: Sketch) {
    let sk = Sk::new(sketch, pic.pix.clone(), false);
    pic.pix = sk.render();
    pic.gray = Arc::new(Gray::from_rgba(pic.pix.data(), pic.pix.width() as usize, pic.pix.height() as usize));
    pic.sk = Some(sk);
}

/// Пересчитать картинку после правки штрихов.
fn changed(st: &mut State, ui: &AppWindow, full: bool) {
    let Some(pic) = st.pic.as_mut() else { return };
    if full {
        if let Some(sk) = &pic.sk {
            pic.pix = sk.render();
        }
    }
    pic.gray = Arc::new(Gray::from_rgba(pic.pix.data(), pic.pix.width() as usize, pic.pix.height() as usize));
    st.p_ver += 1;
    show_pic(st, ui);
    schedule_gen(st);
    request_draw(st);
}

/// Включить рисование: на пустом месте — чистый холст по размеру листа,
/// поверх картинки — новый слой штрихов.
pub(super) fn open(st: &mut State, ui: &AppWindow) {
    let store = ui.global::<Store>();
    if st.pic.is_none() {
        // Новый холст по размеру листа, картинка ложится ровно на лист
        let mut p = st.p.clone();
        p.img_scale = 1.0;
        p.img_dx = 0.0;
        p.img_dy = 0.0;
        let sketch = Sketch::new(p.sheet_w, p.sheet_h);
        set_params(st, ui, p, true);
        let Some(pic) = blank_pic(sketch) else { return };
        install(st, ui, pic);
    } else if let Some(pic) = st.pic.as_mut() {
        if pic.sk.is_none() {
            let (_, _, dw, dh) = render::image_rect(&st.p, f64::from(pic.pix.width()), f64::from(pic.pix.height()));
            pic.sk = Some(Sk::new(Sketch::new(r2(dw), r2(dh)), pic.pix.clone(), false));
        }
    }
    let size = f64::from(store.get_brush_size()).clamp(f64::from(store.get_brush_min()), f64::from(store.get_brush_max()));
    store.set_brush_size(size as f32);
    store.set_drawing(true);
    show_pic(st, ui);
    request_draw(st);
}

pub(super) fn close(st: &mut State, ui: &AppWindow) {
    let store = ui.global::<Store>();
    st.live = None;
    store.set_ink(Image::default());
    store.set_drawing(false);
    let mut drop_pic = false;
    if let Some(pic) = st.pic.as_mut() {
        if pic.sk.as_ref().is_some_and(|s| s.sketch.strokes.is_empty()) {
            // Пустой рисунок не нужен: холст убираем, у картинки остаётся только она сама
            if pic.sk.as_ref().is_some_and(|s| s.blank) {
                drop_pic = true;
            } else if let Some(sk) = pic.sk.take() {
                pic.pix = sk.base;
            }
        }
    }
    if drop_pic {
        st.pic = None;
        st.res = None;
        st.p_ver += 1;
        show_stats(st, ui);
    }
    save_brush(st, ui);
    show_pic(st, ui);
    request_draw(st);
}

fn save_brush(st: &mut State, ui: &AppWindow) {
    let store = ui.global::<Store>();
    st.saved.brush_size = f64::from(store.get_brush_size());
    st.saved.brush_soft = f64::from(store.get_brush_soft());
    st.saved.brush_tone = f64::from(store.get_brush_tone());
    st.saved.store();
}

fn begin(st: &mut State, ui: &AppWindow, x: f64, y: f64) {
    let store = ui.global::<Store>();
    if !store.get_drawing() {
        return;
    }
    let Some(m) = map(st, ui) else { return };
    let Some(sk) = st.pic.as_ref().and_then(|p| p.sk.as_ref()) else { return };
    let (blank, wk) = (sk.blank, sk.sketch.width_k);
    let (w, h) = ((ui.get_cw().round() as u32).max(1), (ui.get_ch().round() as u32).max(1));
    let Some(ink) = Pixmap::new(w, h) else { return };
    let erase = store.get_brush_tool() == 1;
    let size = f64::from(store.get_brush_size()).max(0.05);
    let soft = f64::from(store.get_brush_soft()).clamp(0.0, 1.0);
    let tone = f64::from(store.get_brush_tone()).clamp(0.0, 1.0);
    let g = ((1.0 - tone) * 255.0).round() as u8;
    let color = if erase { tiny_skia::Color::WHITE } else { tiny_skia::Color::from_rgba8(g, g, g, 255) };
    // Мягкий край на экране кажется уже: показываем ту ширину, что видна после отпускания
    let width = (size * wk * m.px_per_mm() * (1.0 - 0.45 * soft)) as f32;
    let mask = m.clip(&st.p).and_then(|r| {
        let mut mk = Mask::new(w, h)?;
        mk.fill_path(&PathBuilder::from_rect(r), FillRule::Winding, false, Transform::identity());
        Some(mk)
    });
    store.set_ink_alpha(if erase && !blank { 0.7 } else { 1.0 });
    let mut live = Live { raw: vec![m.to_mm(x, y)], ink, mask, last: (x as f32, y as f32), width, color, erase, size, soft, tone };
    live.dot(x as f32, y as f32);
    st.live = Some(live);
    save_brush(st, ui);
    flush(st);
}

fn moved(st: &mut State, ui: &AppWindow, x: f64, y: f64) {
    let Some(m) = map(st, ui) else { return };
    let Some(live) = st.live.as_mut() else { return };
    live.raw.push(m.to_mm(x, y));
    live.seg(x as f32, y as f32);
    flush(st);
}

/// Слой штриха на экран, не чаще одного раза за кадр.
fn flush(st: &mut State) {
    st.ink_timer.start(TimerMode::SingleShot, Duration::ZERO, || {
        with(|st, ui| {
            if let Some(l) = &st.live {
                let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(l.ink.data(), l.ink.width(), l.ink.height());
                ui.global::<Store>().set_ink(Image::from_rgba8_premultiplied(buf));
            }
        });
    });
}

fn end(st: &mut State, ui: &AppWindow) {
    let Some(live) = st.live.take() else { return };
    st.ink_timer.stop();
    let pts = sketch::smooth(&live.raw, live.size);
    let s = Stroke { erase: live.erase, size: r2(live.size), soft: r2(live.soft), tone: r2(live.tone), pts };
    if let Some(pic) = st.pic.as_mut() {
        if let Some(sk) = pic.sk.as_mut() {
            let (w, h) = (pic.pix.width() as usize, pic.pix.height() as usize);
            sk.sketch.paint(pic.pix.data_mut(), sk.base.data(), w, h, &s);
            sk.sketch.strokes.push(s.clone());
            sk.undo.push(Act::Add(s));
            sk.redo.clear();
        }
    }
    changed(st, ui, false);
    // Слой убираем после перерисовки, чтобы штрих не мигал
    let w = st.ui.clone();
    Timer::single_shot(Duration::from_millis(30), move || {
        if let Some(ui) = w.upgrade() {
            ui.global::<Store>().set_ink(Image::default());
        }
    });
}

fn cancel(st: &mut State, ui: &AppWindow) {
    st.live = None;
    ui.global::<Store>().set_ink(Image::default());
}

fn undo(st: &mut State, ui: &AppWindow) {
    if st.live.is_some() {
        return;
    }
    let Some(sk) = st.pic.as_mut().and_then(|p| p.sk.as_mut()) else { return };
    match sk.undo.pop() {
        Some(Act::Add(s)) => {
            sk.sketch.strokes.pop();
            sk.redo.push(Act::Add(s));
        }
        Some(Act::Clear(old)) => {
            sk.sketch.strokes = old;
            sk.redo.push(Act::Clear(Vec::new()));
        }
        None => return,
    }
    changed(st, ui, true);
}

fn redo(st: &mut State, ui: &AppWindow) {
    if st.live.is_some() {
        return;
    }
    let Some(sk) = st.pic.as_mut().and_then(|p| p.sk.as_mut()) else { return };
    match sk.redo.pop() {
        Some(Act::Add(s)) => {
            sk.sketch.strokes.push(s.clone());
            sk.undo.push(Act::Add(s));
        }
        Some(Act::Clear(_)) => {
            let old = std::mem::take(&mut sk.sketch.strokes);
            sk.undo.push(Act::Clear(old));
        }
        None => return,
    }
    changed(st, ui, true);
}

fn clear(st: &mut State, ui: &AppWindow) {
    let Some(sk) = st.pic.as_mut().and_then(|p| p.sk.as_mut()) else { return };
    if sk.sketch.strokes.is_empty() {
        return;
    }
    let old = std::mem::take(&mut sk.sketch.strokes);
    sk.undo.push(Act::Clear(old));
    sk.redo.clear();
    changed(st, ui, true);
}

fn width_changed(st: &mut State, ui: &AppWindow) {
    let k = f64::from(ui.global::<Store>().get_width_k()).clamp(0.05, 20.0);
    let Some(sk) = st.pic.as_mut().and_then(|p| p.sk.as_mut()) else { return };
    sk.sketch.width_k = r2(k);
    // Ползунок шлёт много значений подряд: пересчитываем рисунок по последнему
    st.width_timer.start(TimerMode::SingleShot, Duration::from_millis(60), || {
        with(|st, ui| changed(st, ui, true));
    });
}

fn export_svg() {
    let out = with(|st, ui| {
        let sk = st.pic.as_ref().and_then(|p| p.sk.as_ref())?;
        if sk.sketch.strokes.is_empty() {
            toast(st, ui, &t("Сначала нарисуйте что-нибудь"), 2500);
            return None;
        }
        let name = format!("{}_{}.svg", files::base_name(&st.p), t("рисунок"));
        Some((name, sk.sketch.svg(&st.p.name).into_bytes()))
    })
    .flatten();
    if let Some((name, bytes)) = out {
        save_out(name, "image/svg+xml", bytes);
    }
}

pub(super) fn bind(ui: &AppWindow) {
    let s = ui.global::<Store>();
    s.on_draw_open(|| {
        with(open);
    });
    s.on_draw_close(|| {
        with(close);
    });
    s.on_stroke_begin(|x, y| {
        with(|st, ui| begin(st, ui, f64::from(x), f64::from(y)));
    });
    s.on_stroke_move(|x, y| {
        with(|st, ui| moved(st, ui, f64::from(x), f64::from(y)));
    });
    s.on_stroke_end(|| {
        with(end);
    });
    s.on_stroke_cancel(|| {
        with(cancel);
    });
    s.on_draw_undo(|| {
        with(undo);
    });
    s.on_draw_redo(|| {
        with(redo);
    });
    s.on_draw_clear(|| {
        with(clear);
    });
    s.on_width_changed(|| {
        with(width_changed);
    });
    s.on_sketch_svg(export_svg);
}
