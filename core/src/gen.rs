use crate::image::Gray;
use crate::params::Params;
use serde::Serialize;
use std::f64::consts::PI;

/// Отрезок между двумя точками: (начало, конец)
type Segment = ((f64, f64), (f64, f64));

#[derive(Clone, Debug)]
pub enum Shape {
    Circle { x: f64, y: f64, d: f64 },
    Poly { pts: Vec<(f64, f64)>, size: f64 },
}

impl Shape {
    pub fn size(&self) -> f64 {
        match self {
            Shape::Circle { d, .. } => *d,
            Shape::Poly { size, .. } => *size,
        }
    }
    pub fn start(&self) -> (f64, f64) {
        match self {
            Shape::Circle { x, y, d } => (x + d / 2.0, *y),
            Shape::Poly { pts, .. } => pts[0],
        }
    }
    pub fn perimeter(&self) -> f64 {
        match self {
            Shape::Circle { d, .. } => PI * d,
            Shape::Poly { pts, .. } => {
                let mut l = 0.0;
                for i in 0..pts.len() {
                    let a = pts[i];
                    let b = pts[(i + 1) % pts.len()];
                    l += ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
                }
                l
            }
        }
    }
    pub fn area(&self) -> f64 {
        match self {
            Shape::Circle { d, .. } => PI * d * d / 4.0,
            Shape::Poly { pts, .. } => {
                let mut s = 0.0;
                for i in 0..pts.len() {
                    let a = pts[i];
                    let b = pts[(i + 1) % pts.len()];
                    s += a.0 * b.1 - b.0 * a.1;
                }
                (s / 2.0).abs()
            }
        }
    }
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct SizeCount {
    pub size: f64,
    pub count: usize,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Stats {
    pub count: usize,
    pub by_size: Vec<SizeCount>,
    pub open_area_pct: f64,
    pub cut_len_m: f64,
    pub travel_m: f64,
    pub pierces: usize,
    pub time_min: f64,
    pub cost: f64,
    pub cost_cut: f64,
    pub cost_pierce: f64,
    pub cost_material: f64,
    pub cost_fixed: f64,
    pub weight_kg: f64,
    pub removed_edge: usize,
    pub size_min: f64,
    pub size_max: f64,
    pub min_allowed: f64,
    pub max_allowed: f64,
    pub warnings: Vec<String>,
}

pub struct GenResult {
    pub shapes: Vec<Shape>,
    pub stats: Stats,
    pub params: Params,
}

const MAX_SHAPES: f64 = 1_500_000.0;

struct Sampler<'a> {
    img: Option<&'a Gray>,
    left: f64,
    top: f64, // в координатах листа с осью Y вниз
    sx: f64,
    sy: f64,
    p: &'a Params,
}

impl<'a> Sampler<'a> {
    fn new(img: Option<&'a Gray>, p: &'a Params) -> Self {
        let (w, h) = (p.sheet_w, p.sheet_h);
        let (mut sx, mut sy, mut left, mut top) = (1.0, 1.0, 0.0, 0.0);
        if let Some(g) = img {
            let (iw, ih) = (g.w as f64, g.h as f64);
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
            sx = a * p.img_scale;
            sy = b * p.img_scale;
            let (dw, dh) = (iw * sx, ih * sy);
            left = w / 2.0 + p.img_dx - dw / 2.0;
            top = h / 2.0 - p.img_dy - dh / 2.0;
        }
        Sampler { img, left, top, sx, sy, p }
    }

    /// Тон 0..1 (1 = максимальное отверстие) в точке листа (X, Y вверх), усреднённый по радиусу r (мм).
    fn tone(&self, x: f64, y: f64, r: f64) -> Option<f64> {
        let g = self.img?;
        let yd = self.p.sheet_h - y;
        let px = (x - self.left) / self.sx;
        let py = (yd - self.top) / self.sy;
        if px < 0.0 || py < 0.0 || px >= g.w as f64 || py >= g.h as f64 {
            return None;
        }
        let rx = (r / self.sx).max(0.5);
        let ry = (r / self.sy).max(0.5);
        let mut v = g.mean(px - rx, py - ry, px + rx, py + ry);
        v = (v - 0.5) * (1.0 + self.p.contrast) + 0.5 + self.p.brightness;
        v = v.clamp(0.0, 1.0).powf(self.p.gamma);
        let t = if self.p.negative { v } else { 1.0 - v };
        Some(t)
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

pub fn generate(img: Option<&Gray>, params: &Params) -> GenResult {
    let p = params.sanitized();
    let en = p.lang == "en";
    let prop = p.size_mode == "prop";
    let mut warnings = Vec::new();
    let mut removed_edge = 0usize;
    let min_allowed = (p.min_k * p.thickness).max(0.0);
    let (w, h) = (p.sheet_w, p.sheet_h);
    let b = p.border;
    let sampler = Sampler::new(img, &p);
    let mut shapes: Vec<Shape> = Vec::new();
    let max_allowed;

    if p.pattern != "lines" && p.pattern != "dots" {
        max_allowed = 0.0;
        warnings.push(tr(en, "Неизвестный тип узора", "Unknown pattern type"));
        return finish(shapes, warnings, removed_edge, min_allowed, max_allowed, p);
    }
    if img.is_none() {
        warnings.push(tr(en, "Загрузите изображение, чтобы получить узор", "Load an image to get a pattern"));
    }

    if p.pattern == "lines" {
        let lp = p.line_pitch;
        let k = if p.wave { 2.0 * PI * p.wave_amp / p.wave_len } else { 0.0 };
        let cos_f = 1.0 / (1.0 + k * k).sqrt();
        let cap = lp * cos_f - p.bridge;
        max_allowed = cap;
        let wmin = p.w_min.max(min_allowed);
        let mut wmax = p.w_max;
        if wmax > cap {
            warnings.push(if en {
                format!("Max width limited to {:.2} mm to keep the bridge at least {:.2} mm", cap.max(0.0), p.bridge)
            } else {
                format!("Макс. ширина ограничена до {:.2} мм, чтобы перемычка была не меньше {:.2} мм", cap.max(0.0), p.bridge)
            });
            wmax = cap;
        }
        if p.w_min < min_allowed {
            warnings.push(if en {
                format!("Min width raised to {:.2} mm: a slot in {:.2} mm metal cannot be narrower", min_allowed, p.thickness)
            } else {
                format!("Мин. ширина поднята до {:.2} мм: для металла {:.2} мм прорезь не может быть уже", min_allowed, p.thickness)
            });
        }
        let est = (w * h) / (lp * p.step);
        if est > MAX_SHAPES * 4.0 {
            warnings.push(tr(en, "Слишком мелкий шаг линий или выборки для такого листа", "Line pitch or sampling step is too small for this sheet"));
        } else if wmax < wmin {
            warnings.push(tr(en, "Шаг линий слишком мал для заданной перемычки и мин. ширины", "Line pitch is too small for this bridge and min width"));
        } else if img.is_some() {
            let (cx, cy) = (w / 2.0, h / 2.0);
            let rad = p.line_angle.to_radians();
            let (ca, sa) = (rad.cos(), rad.sin());
            let reach = (w * w + h * h).sqrt() / 2.0 + p.wave_amp + lp;
            let m = (reach / lp).ceil() as i64;
            let n_steps = ((2.0 * reach) / p.step).ceil() as i64;
            let inside = |x: f64, y: f64| x >= b && x <= w - b && y >= b && y <= h - b;
            for j in -m..=m {
                let v0 = j as f64 * lp + p.offset_y;
                let mut runs: Vec<Vec<Segment>> = Vec::new();
                let mut cur: Vec<Segment> = Vec::new();
                for s in 0..=n_steps {
                    let u = -reach + s as f64 * p.step + p.offset_x;
                    let (lv, dv) = if p.wave {
                        let ph = 2.0 * PI * u / p.wave_len;
                        (v0 + p.wave_amp * ph.sin(), p.wave_amp * 2.0 * PI / p.wave_len * ph.cos())
                    } else {
                        (v0, 0.0)
                    };
                    let x = cx + u * ca - lv * sa;
                    let y = cy + u * sa + lv * ca;
                    let (tx0, ty0) = (ca - dv * sa, sa + dv * ca);
                    let tl = (tx0 * tx0 + ty0 * ty0).sqrt();
                    let (nx, ny) = (-ty0 / tl, tx0 / tl);
                    let mut ok = None;
                    if x > -lp && x < w + lp && y > -lp && y < h + lp {
                        if let Some(t) = sampler.tone(x, y, lp / 2.0) {
                            if t >= p.tone_cut {
                                let tt = ((t - p.tone_cut) / (1.0 - p.tone_cut)).clamp(0.0, 1.0);
                                let ww = if prop { wmax * t } else { wmin + (wmax - wmin) * tt };
                                let l = (x + nx * ww / 2.0, y + ny * ww / 2.0);
                                let r = (x - nx * ww / 2.0, y - ny * ww / 2.0);
                                if ww >= wmin - 1e-9 && inside(l.0, l.1) && inside(r.0, r.1) {
                                    ok = Some((l, r));
                                }
                            }
                        }
                    }
                    match ok {
                        Some(pair) => cur.push(pair),
                        None => {
                            if !cur.is_empty() {
                                runs.push(std::mem::take(&mut cur));
                            }
                        }
                    }
                }
                if !cur.is_empty() {
                    runs.push(cur);
                }
                let mut line_shapes = Vec::new();
                for run in runs {
                    if run.len() < 2 {
                        continue;
                    }
                    let len = (run.len() - 1) as f64 * p.step;
                    if len < wmin.max(p.step) {
                        continue;
                    }
                    let mut pts: Vec<(f64, f64)> = run.iter().map(|q| q.0).collect();
                    pts.extend(run.iter().rev().map(|q| q.1));
                    let maxw = run
                        .iter()
                        .map(|q| ((q.0 .0 - q.1 .0).powi(2) + (q.0 .1 - q.1 .1).powi(2)).sqrt())
                        .fold(0.0, f64::max);
                    line_shapes.push(Shape::Poly { pts, size: round2(maxw) });
                }
                if j.rem_euclid(2) == 1 {
                    line_shapes.reverse();
                }
                shapes.extend(line_shapes);
            }
        }
    } else {
        let pch = p.pitch;
        let cap = match (p.shape.as_str(), p.grid.as_str()) {
            ("square", "square") => pch - p.bridge,
            ("square", _) => (pch - p.bridge) / std::f64::consts::SQRT_2,
            _ => pch - p.bridge,
        };
        max_allowed = cap;
        let mut dmax = p.d_max;
        if dmax > cap {
            warnings.push(if en {
                format!("Max size limited to {:.2} mm to keep the bridge at least {:.2} mm", cap.max(0.0), p.bridge)
            } else {
                format!("Макс. размер ограничен до {:.2} мм, чтобы перемычка была не меньше {:.2} мм", cap.max(0.0), p.bridge)
            });
            dmax = cap;
        }
        let dmin = p.d_min.max(min_allowed);
        if p.d_min < min_allowed {
            warnings.push(if en {
                format!("Min size raised to {:.2} mm: a hole in {:.2} mm metal cannot be smaller", min_allowed, p.thickness)
            } else {
                format!("Мин. размер поднят до {:.2} мм: для металла {:.2} мм отверстие не может быть меньше", min_allowed, p.thickness)
            });
        }
        let mut fixed = if p.fixed { p.fixed_list() } else { vec![] };
        if p.fixed {
            let before = fixed.len();
            fixed.retain(|d| *d >= dmin - 1e-9 && *d <= dmax + 1e-9);
            if fixed.len() < before {
                warnings.push(tr(en, "Часть заданных размеров вне допустимого диапазона и не используется", "Some fixed sizes are out of the allowed range and are not used"));
            }
            if fixed.is_empty() {
                warnings.push(tr(en, "Нет ни одного допустимого заданного размера", "None of the fixed sizes is allowed"));
            }
        }
        let est = (w * h) / (pch * pch);
        if est > MAX_SHAPES {
            warnings.push(if en {
                format!("Pitch is too small: about {} holes. Increase the pitch", est.round() as u64)
            } else {
                format!("Слишком мелкий шаг: около {} отверстий. Увеличьте шаг", est.round() as u64)
            });
        } else if dmax < dmin {
            warnings.push(tr(en, "Шаг слишком мал для заданной перемычки и мин. размера", "Pitch is too small for this bridge and min size"));
        } else if img.is_some() && !(p.fixed && fixed.is_empty()) {
            let (cx, cy) = (w / 2.0, h / 2.0);
            let rad = p.angle.to_radians();
            let (ca, sa) = (rad.cos(), rad.sin());
            let reach = (w * w + h * h).sqrt() / 2.0 + p.offset_x.abs() + p.offset_y.abs() + pch;
            let s3 = 3f64.sqrt() / 2.0;
            let (row_h, col_w) = match p.grid.as_str() {
                "tri" => (pch * s3, pch),
                "tri90" => (pch, pch * s3),
                _ => (pch, pch),
            };
            let nj = (reach / row_h).ceil() as i64 + 1;
            let ni = (reach / col_w).ceil() as i64 + 1;
            // экстенты фигуры по осям с учётом поворота
            let ext = |d: f64| -> (f64, f64) {
                match p.shape.as_str() {
                    "square" => {
                        let e = d / 2.0 * (ca.abs() + sa.abs());
                        (e, e)
                    }
                    "diamond" => {
                        let e = d / 2.0 * ca.abs().max(sa.abs());
                        (e, e)
                    }
                    _ => (d / 2.0, d / 2.0),
                }
            };
            for j in -nj..=nj {
                let mut row: Vec<Shape> = Vec::new();
                for i in -ni..=ni {
                    let (lx, ly) = match p.grid.as_str() {
                        "tri" => (
                            i as f64 * col_w + if j.rem_euclid(2) == 1 { pch / 2.0 } else { 0.0 },
                            j as f64 * row_h,
                        ),
                        "tri90" => (
                            i as f64 * col_w,
                            j as f64 * row_h + if i.rem_euclid(2) == 1 { pch / 2.0 } else { 0.0 },
                        ),
                        _ => (i as f64 * col_w, j as f64 * row_h),
                    };
                    let x = cx + lx * ca - ly * sa + p.offset_x;
                    let y = cy + lx * sa + ly * ca + p.offset_y;
                    if x < b || x > w - b || y < b || y > h - b {
                        continue;
                    }
                    let t = match sampler.tone(x, y, pch / 2.0) {
                        Some(t) => t,
                        None => continue,
                    };
                    if t < p.tone_cut {
                        continue;
                    }
                    let tt = ((t - p.tone_cut) / (1.0 - p.tone_cut)).clamp(0.0, 1.0);
                    let mut d = if prop {
                        // как SVG tracing: диаметр от нуля пропорционален тону; меньше минимума не режем
                        let dp = dmax * t;
                        if dp < dmin - 1e-9 {
                            continue;
                        }
                        dp
                    } else {
                        dmin + (dmax - dmin) * tt
                    };
                    if fixed.is_empty() && p.quant > 0.0 {
                        // квантование: округляем вниз до шага, но не ниже минимума
                        d = ((d / p.quant + 1e-9).floor() * p.quant).max(dmin);
                    }
                    if !fixed.is_empty() {
                        d = *fixed
                            .iter()
                            .min_by(|a, b2| (*a - d).abs().partial_cmp(&(*b2 - d).abs()).unwrap())
                            .unwrap();
                    }
                    let (ex, ey) = ext(d);
                    if x - ex < b - 1e-9 || x + ex > w - b + 1e-9 || y - ey < b - 1e-9 || y + ey > h - b + 1e-9 {
                        removed_edge += 1;
                        continue;
                    }
                    let d = if fixed.is_empty() { round2(d) } else { d };
                    let sh = match p.shape.as_str() {
                        "square" => {
                            let hs = d / 2.0;
                            let c = [(hs, hs), (-hs, hs), (-hs, -hs), (hs, -hs)];
                            Shape::Poly {
                                pts: c.iter().map(|q| (x + q.0 * ca - q.1 * sa, y + q.0 * sa + q.1 * ca)).collect(),
                                size: d,
                            }
                        }
                        "diamond" => {
                            let hs = d / 2.0;
                            let c = [(hs, 0.0), (0.0, hs), (-hs, 0.0), (0.0, -hs)];
                            Shape::Poly {
                                pts: c.iter().map(|q| (x + q.0 * ca - q.1 * sa, y + q.0 * sa + q.1 * ca)).collect(),
                                size: d,
                            }
                        }
                        _ => Shape::Circle { x, y, d },
                    };
                    row.push(sh);
                }
                if j.rem_euclid(2) == 1 {
                    row.reverse();
                }
                shapes.extend(row);
            }
        }
    }

    finish(shapes, warnings, removed_edge, min_allowed, max_allowed, p)
}

fn finish(
    shapes: Vec<Shape>,
    warnings: Vec<String>,
    removed_edge: usize,
    min_allowed: f64,
    max_allowed: f64,
    p: Params,
) -> GenResult {
    let (w, h) = (p.sheet_w, p.sheet_h);
    // Статистика
    let mut st = Stats {
        count: shapes.len(),
        removed_edge,
        ..Stats::default()
    };
    st.min_allowed = round2(min_allowed);
    st.max_allowed = round2(max_allowed.max(0.0));
    let mut area = 0.0;
    let mut cut = 0.0;
    let mut travel = 0.0;
    let mut last = (0.0, 0.0);
    let mut smin = f64::INFINITY;
    let mut smax: f64 = 0.0;
    let mut sizes: Vec<f64> = Vec::with_capacity(shapes.len());
    for s in &shapes {
        area += s.area();
        cut += s.perimeter();
        let st0 = s.start();
        travel += ((st0.0 - last.0).powi(2) + (st0.1 - last.1).powi(2)).sqrt();
        last = st0;
        let sz = s.size();
        smin = smin.min(sz);
        smax = smax.max(sz);
        sizes.push(sz);
    }
    if p.outline {
        cut += 2.0 * (w + h);
        travel += (last.0.powi(2) + last.1.powi(2)).sqrt();
    }
    if p.pattern != "lines" {
        sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut i = 0;
        while i < sizes.len() {
            let mut j = i;
            while j < sizes.len() && (sizes[j] - sizes[i]).abs() < 1e-9 {
                j += 1;
            }
            st.by_size.push(SizeCount { size: sizes[i], count: j - i });
            i = j;
        }
    }
    st.size_min = if smin.is_finite() { smin } else { 0.0 };
    st.size_max = smax;
    st.open_area_pct = area / (w * h) * 100.0;
    st.cut_len_m = cut / 1000.0;
    st.travel_m = travel / 1000.0;
    st.pierces = shapes.len() + if p.outline { 1 } else { 0 };
    st.time_min = cut / p.cut_speed + travel / p.rapid_speed + st.pierces as f64 * p.pierce_time / 60.0;
    st.cost_cut = st.cut_len_m * p.price_m;
    st.cost_pierce = st.pierces as f64 * p.price_pierce;
    st.cost_material = w * h / 1e6 * p.price_m2;
    st.cost_fixed = p.price_fixed;
    st.cost = st.cost_cut + st.cost_pierce + st.cost_material + st.cost_fixed;
    // масса листа после перфорации: мм² * мм = мм³; г/см³ -> кг: мм³ * 1e-3 см³ * ρ г / 1000
    st.weight_kg = (w * h - area) * p.thickness * p.density * 1e-6;
    st.warnings = warnings;
    GenResult { shapes, stats: st, params: p }
}

fn tr(en: bool, ru: &str, eng: &str) -> String {
    if en { eng.to_string() } else { ru.to_string() }
}
