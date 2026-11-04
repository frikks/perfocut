//! Рисование листа: предпросмотр в окне и картинка PNG для клиента. Только tiny-skia, без GPU.

use perfo_core::gen::Shape;
use perfo_core::params::Params;
use tiny_skia::{
    Color, FillRule, FilterQuality, GradientStop, LinearGradient, Mask, Paint, PathBuilder, Pixmap, PixmapPaint,
    Point, Rect, Shader, SpreadMode, Stroke, StrokeDash, Transform,
};

/// Положение и масштаб листа на холсте (логические пиксели).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub z: f64,
    pub x: f64,
    pub y: f64,
}

impl Default for View {
    fn default() -> Self {
        View { z: 1.0, x: 0.0, y: 0.0 }
    }
}

/// Где лист на холсте: `s` пикселей на мм, (`ox`, `oy`) — левый нижний угол листа.
#[derive(Clone, Copy, Debug)]
pub struct Tf {
    pub s: f64,
    pub ox: f64,
    pub oy: f64,
}

impl Tf {
    pub fn fit(cw: f64, ch: f64, p: &Params, v: &View) -> Tf {
        let pad = if cw < 600.0 { 36.0 } else { 56.0 };
        let base = ((cw - pad * 2.0) / p.sheet_w).min((ch - pad * 2.0) / p.sheet_h).max(1e-6);
        let s = base * v.z;
        Tf { s, ox: cw / 2.0 - p.sheet_w * s / 2.0 + v.x, oy: ch / 2.0 + p.sheet_h * s / 2.0 + v.y }
    }
}

impl View {
    /// Масштаб вокруг точки (mx, my): точка под курсором остаётся на месте.
    pub fn zoom_at(&mut self, cw: f64, ch: f64, mx: f64, my: f64, f: f64) {
        if !f.is_finite() || f <= 0.0 {
            return;
        }
        let nz = (self.z * f).clamp(0.2, 80.0);
        let f = nz / self.z;
        let cx = cw / 2.0 + self.x;
        let cy = ch / 2.0 + self.y;
        self.x += (mx - cx) * (1.0 - f);
        self.y += (my - cy) * (1.0 - f);
        self.z = nz;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn parse(s: &str) -> Option<Rgb> {
        let h = s.trim().trim_start_matches('#');
        let full: String = match h.len() {
            3 => h.chars().flat_map(|c| [c, c]).collect(),
            6 => h.to_owned(),
            _ => return None,
        };
        let n = u32::from_str_radix(&full, 16).ok()?;
        Some(Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
    }

    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    fn color(self) -> Color {
        Color::from_rgba8(self.0, self.1, self.2, 255)
    }

    // Светлее или темнее на p процентов шкалы, как в прошлой версии
    fn shade(self, p: f64) -> Color {
        let f = |v: u8| (f64::from(v) + p / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8;
        Color::from_rgba8(f(self.0), f(self.1), f(self.2), 255)
    }
}

/// Цвета для рисования: лист и фон выбирает пользователь, остальное из темы.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub sheet: Rgb,
    pub back: Rgb,
    pub card: Color,
    pub ink: Color,
    pub frame: Color,
    pub dim: Color,
}

fn solid(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

fn stroke(w: f32) -> Stroke {
    Stroke { width: w, ..Stroke::default() }
}

/// Контуры всех отверстий одним путём. Отверстия вне холста пропускаются.
fn shapes_path(shapes: &[Shape], t: &Tf, clip: (f64, f64)) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let (cw, ch) = clip;
    for sh in shapes {
        match sh {
            Shape::Circle { x, y, d } => {
                let cx = t.ox + x * t.s;
                let cy = t.oy - y * t.s;
                let r = d * t.s / 2.0;
                if cx + r < 0.0 || cy + r < 0.0 || cx - r > cw || cy - r > ch {
                    continue;
                }
                pb.push_circle(cx as f32, cy as f32, r.max(0.05) as f32);
            }
            Shape::Poly { pts, size } => {
                let Some(first) = pts.first() else { continue };
                let fx = t.ox + first.0 * t.s;
                let fy = t.oy - first.1 * t.s;
                let reach = size * t.s * 2.0 + 2.0;
                if fx + reach < 0.0 || fy + reach < 0.0 || fx - reach > cw || fy - reach > ch {
                    continue;
                }
                pb.move_to(fx as f32, fy as f32);
                for q in &pts[1..] {
                    pb.line_to((t.ox + q.0 * t.s) as f32, (t.oy - q.1 * t.s) as f32);
                }
                pb.close();
            }
        }
    }
    pb.finish()
}

/// Размещение картинки на листе (мм, ось Y вниз) — та же формула, что в ядре.
pub fn image_rect(p: &Params, iw: f64, ih: f64) -> (f64, f64, f64, f64) {
    let (w, h) = (p.sheet_w, p.sheet_h);
    let (a, b) = match p.fit.as_str() {
        "contain" => {
            let s = (w / iw).min(h / ih);
            (s, s)
        }
        "stretch" => (w / iw, h / ih),
        _ => {
            let s = (w / iw).max(h / ih);
            (s, s)
        }
    };
    let (dw, dh) = (iw * a * p.img_scale, ih * b * p.img_scale);
    (w / 2.0 + p.img_dx - dw / 2.0, h / 2.0 - p.img_dy - dh / 2.0, dw, dh)
}

/// Рисует лист в режиме 0 «Металл», 1 «Контуры» или 2 «Оригинал».
#[allow(clippy::too_many_arguments)]
pub fn paint_sheet(
    pm: &mut Pixmap,
    ts: Transform,
    t: &Tf,
    clip: (f64, f64),
    p: &Params,
    shapes: &[Shape],
    mode: i32,
    look: &Look,
    pic: Option<&Pixmap>,
) {
    let sw = p.sheet_w * t.s;
    let sh = p.sheet_h * t.s;
    let top = t.oy - sh;
    let Some(rect) = Rect::from_xywh(t.ox as f32, top as f32, sw as f32, sh as f32) else { return };
    let inner = Rect::from_xywh((t.ox + 0.5) as f32, (top + 0.5) as f32, (sw - 1.0).max(0.5) as f32, (sh - 1.0).max(0.5) as f32);
    match mode {
        1 => {
            pm.fill_rect(rect, &solid(look.card), ts, None);
            let b = p.border * t.s;
            if let Some(fr) = Rect::from_xywh((t.ox + b + 0.5) as f32, (top + b + 0.5) as f32, (sw - 2.0 * b - 1.0) as f32, (sh - 2.0 * b - 1.0) as f32) {
                let mut st = stroke(1.0);
                st.dash = StrokeDash::new(vec![4.0, 4.0], 0.0);
                pm.stroke_path(&PathBuilder::from_rect(fr), &solid(look.frame), &st, ts, None);
            }
            if let Some(path) = shapes_path(shapes, t, clip) {
                let w = (t.s * 0.15).clamp(0.5, 1.0) as f32;
                pm.stroke_path(&path, &solid(look.ink), &stroke(w), ts, None);
            }
            if let Some(r) = inner {
                pm.stroke_path(&PathBuilder::from_rect(r), &solid(Color::from_rgba8(0xe8, 0x83, 0x3a, 255)), &stroke(1.0), ts, None);
            }
        }
        2 => {
            pm.fill_rect(rect, &solid(Color::WHITE), ts, None);
            let mut mask = Mask::new(pm.width(), pm.height());
            if let Some(m) = mask.as_mut() {
                m.fill_path(&PathBuilder::from_rect(rect), FillRule::Winding, true, ts);
            }
            if let Some(pic) = pic {
                let (iw, ih) = (f64::from(pic.width()), f64::from(pic.height()));
                let (l, tp, dw, dh) = image_rect(p, iw, ih);
                let it = Transform::from_row((dw * t.s / iw) as f32, 0.0, 0.0, (dh * t.s / ih) as f32, (t.ox + l * t.s) as f32, (top + tp * t.s) as f32);
                let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..PixmapPaint::default() };
                pm.draw_pixmap(0, 0, pic.as_ref(), &paint, ts.pre_concat(it), mask.as_ref());
            }
            if let Some(path) = shapes_path(shapes, t, clip) {
                pm.stroke_path(&path, &solid(Color::from_rgba8(0xe8, 0x83, 0x3a, 217)), &stroke(1.0), ts, mask.as_ref());
            }
        }
        _ => {
            let stops = vec![
                GradientStop::new(0.0, look.sheet.shade(12.0)),
                GradientStop::new(0.55, look.sheet.color()),
                GradientStop::new(1.0, look.sheet.shade(-14.0)),
            ];
            let shader = LinearGradient::new(
                Point::from_xy(t.ox as f32, top as f32),
                Point::from_xy((t.ox + sw) as f32, t.oy as f32),
                stops,
                SpreadMode::Pad,
                Transform::identity(),
            )
            .unwrap_or(Shader::SolidColor(look.sheet.color()));
            let paint = Paint { shader, anti_alias: true, ..Paint::default() };
            pm.fill_rect(rect, &paint, ts, None);
            if let Some(path) = shapes_path(shapes, t, clip) {
                pm.fill_path(&path, &solid(look.back.color()), FillRule::Winding, ts, None);
            }
            if let Some(r) = inner {
                pm.stroke_path(&PathBuilder::from_rect(r), &solid(Color::from_rgba8(0, 0, 0, 89)), &stroke(1.0), ts, None);
            }
        }
    }
}

/// Размерные линии под листом и слева от него. Подписи рисует интерфейс.
pub fn paint_dims(pm: &mut Pixmap, ts: Transform, t: &Tf, p: &Params, color: Color) {
    let sw = p.sheet_w * t.s;
    let sh = p.sheet_h * t.s;
    let (ox, oy) = (t.ox, t.oy);
    let top = oy - sh;
    let yb = oy + 20.0;
    let xl = ox - 20.0;
    let mut pb = PathBuilder::new();
    let mut line = |a: f64, b: f64, c: f64, d: f64| {
        pb.move_to((a + 0.5) as f32, (b + 0.5) as f32);
        pb.line_to((c + 0.5) as f32, (d + 0.5) as f32);
    };
    line(ox, oy + 6.0, ox, yb + 6.0);
    line(ox + sw, oy + 6.0, ox + sw, yb + 6.0);
    line(ox, yb, ox + sw, yb);
    line(ox - 6.0, oy, xl - 6.0, oy);
    line(ox - 6.0, top, xl - 6.0, top);
    line(xl, top, xl, oy);
    for (x, y) in [(ox, yb), (ox + sw, yb), (xl, oy), (xl, top)] {
        line(x - 4.0, y + 4.0, x + 4.0, y - 4.0);
    }
    if let Some(path) = pb.finish() {
        pm.stroke_path(&path, &solid(color), &stroke(1.0), ts, None);
    }
}

/// Вид «металл» во весь лист для PNG: не больше `max_side` пикселей по длинной стороне.
pub fn metal_pixmap(p: &Params, shapes: &[Shape], look: &Look, max_side: f64) -> Option<Pixmap> {
    let k = (max_side / p.sheet_w.max(p.sheet_h)).min(6.0);
    let w = (p.sheet_w * k).round().max(1.0);
    let h = (p.sheet_h * k).round().max(1.0);
    let mut pm = Pixmap::new(w as u32, h as u32)?;
    let t = Tf { s: k, ox: 0.0, oy: h };
    paint_sheet(&mut pm, Transform::identity(), &t, (w, h), p, shapes, 0, look, None);
    Some(pm)
}

/// Пример для первого знакомства: градиент, тёмное пятно и светлая волна.
pub fn sample_pixmap() -> Option<Pixmap> {
    let (w, h) = (1200u32, 720u32);
    let mut pm = Pixmap::new(w, h)?;
    let lin = LinearGradient::new(
        Point::from_xy(0.0, 0.0),
        Point::from_xy(w as f32, 0.0),
        vec![GradientStop::new(0.0, Color::from_rgba8(0xf4, 0xf4, 0xf4, 255)), GradientStop::new(1.0, Color::from_rgba8(0x3a, 0x3a, 0x3a, 255))],
        SpreadMode::Pad,
        Transform::identity(),
    )?;
    let bg = Rect::from_xywh(0.0, 0.0, w as f32, h as f32)?;
    pm.fill_rect(bg, &Paint { shader: lin, anti_alias: true, ..Paint::default() }, Transform::identity(), None);
    let rad = tiny_skia::RadialGradient::new(
        Point::from_xy(380.0, 360.0),
        Point::from_xy(380.0, 360.0),
        300.0,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(0, 0, 0, 255)),
            GradientStop::new(0.7, Color::from_rgba8(0x22, 0x22, 0x22, 255)),
            GradientStop::new(1.0, Color::from_rgba8(40, 40, 40, 0)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )?;
    let disc = PathBuilder::from_circle(380.0, 360.0, 300.0)?;
    pm.fill_path(&disc, &Paint { shader: rad, anti_alias: true, ..Paint::default() }, FillRule::Winding, Transform::identity(), None);
    let mut pb = PathBuilder::new();
    let mut i = 0u32;
    while i <= w {
        let y = 560.0 + 70.0 * (f64::from(i) / 120.0).sin();
        if i == 0 {
            pb.move_to(i as f32, y as f32);
        } else {
            pb.line_to(i as f32, y as f32);
        }
        i += 8;
    }
    let wave = pb.finish()?;
    pm.stroke_path(&wave, &solid(Color::WHITE), &stroke(46.0), Transform::identity(), None);
    Some(pm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_point_under_cursor() {
        let p = Params::default();
        let mut v = View::default();
        let before = Tf::fit(800.0, 600.0, &p, &v);
        let (mx, my) = (200.0, 150.0);
        let sheet_x = (mx - before.ox) / before.s;
        v.zoom_at(800.0, 600.0, mx, my, 2.0);
        let after = Tf::fit(800.0, 600.0, &p, &v);
        assert!(((mx - after.ox) / after.s - sheet_x).abs() < 1e-9);
        v.zoom_at(800.0, 600.0, mx, my, 1e9);
        assert_eq!(v.z, 80.0);
    }

    #[test]
    fn colors_parse_short_and_long_hex() {
        assert_eq!(Rgb::parse("#9a9ea3"), Some(Rgb(0x9a, 0x9e, 0xa3)));
        assert_eq!(Rgb::parse("fff"), Some(Rgb(255, 255, 255)));
        assert_eq!(Rgb::parse("#12"), None);
        assert_eq!(Rgb(1, 2, 255).hex(), "#0102ff");
    }

    #[test]
    fn png_view_has_holes_in_back_color() {
        let p = Params::default();
        let shapes = vec![Shape::Circle { x: 500.0, y: 300.0, d: 100.0 }];
        let look = Look {
            sheet: Rgb(100, 100, 100),
            back: Rgb(255, 0, 0),
            card: Color::WHITE,
            ink: Color::BLACK,
            frame: Color::BLACK,
            dim: Color::BLACK,
        };
        let pm = metal_pixmap(&p, &shapes, &look, 1000.0).expect("pixmap");
        assert_eq!((pm.width(), pm.height()), (1000, 600));
        let c = pm.pixel(500, 300).expect("pixel");
        assert_eq!((c.red(), c.green(), c.blue()), (255, 0, 0));
        assert!(sample_pixmap().is_some());
    }
}
