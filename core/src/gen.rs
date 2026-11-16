use crate::image::Gray;
use crate::params::Params;
use serde::Serialize;
use std::collections::HashMap;
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
    pub fn center(&self) -> (f64, f64) {
        match self {
            Shape::Circle { x, y, .. } => (*x, *y),
            Shape::Poly { pts, .. } => {
                if pts.is_empty() {
                    return (0.0, 0.0);
                }
                let (mut sx, mut sy) = (0.0, 0.0);
                for p in pts {
                    sx += p.0;
                    sy += p.1;
                }
                let n = pts.len() as f64;
                (sx / n, sy / n)
            }
        }
    }
    pub fn start(&self) -> (f64, f64) {
        match self {
            Shape::Circle { x, y, .. } => (*x, *y),
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
        Sampler {
            img,
            left,
            top,
            sx,
            sy,
            p,
        }
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

/// Число вершин на полной окружности радиуса `r` при допуске по хорде 0,02 мм.
fn chord_circle_n(r: f64) -> usize {
    if r > 0.02 {
        (PI / (1.0 - 0.02 / r).acos()).ceil() as usize
    } else {
        8
    }
    .clamp(12, 720)
}

/// Детерминированная псевдослучайная величина [0, 1) из координат и зерна (splitmix64).
/// Одно и то же зерно всегда даёт одну и ту же «случайную» картинку.
fn jitter_u01(seed: u64, x: f64, y: f64) -> f64 {
    let mut z =
        (seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)) ^ x.to_bits() ^ y.to_bits().rotate_left(32);
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

fn dist_to_seg(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let ll = dx * dx + dy * dy;
    let t = if ll > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / ll).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}

/// Упрощение открытой ломаной алгоритмом Рамера — Дугласа — Пекера.
fn simplify_open(pts: &[(f64, f64)], eps: f64) -> Vec<(f64, f64)> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((i, j)) = stack.pop() {
        let (mut best, mut at) = (0.0, 0);
        for k in i + 1..j {
            let d = dist_to_seg(pts[k], pts[i], pts[j]);
            if d > best {
                best = d;
                at = k;
            }
        }
        if best > eps {
            keep[at] = true;
            stack.push((i, at));
            stack.push((at, j));
        }
    }
    pts.iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(p, _)| *p)
        .collect()
}

/// Жадная оптимизация порядка резки (Nearest Neighbor с пространственным хешированием).
///
/// Ячейка сетки подстраивается под плотность раскладки: около одной фигуры на ячейку.
/// Так разреженные узоры не деградируют — на шаге хватает ближайших колец сетки.
fn optimize_path(shapes: Vec<Shape>, cell_size: f64) -> Vec<Shape> {
    let n = shapes.len();
    if n <= 2 {
        return shapes;
    }
    let centers: Vec<(f64, f64)> = shapes.iter().map(Shape::center).collect();
    let (mut x0, mut y0) = (f64::INFINITY, f64::INFINITY);
    let (mut x1, mut y1) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &(x, y) in &centers {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let side = (n as f64).sqrt().ceil().max(1.0);
    let span = (x1 - x0).max(y1 - y0).max(1.0);
    let cs = cell_size.max(span / side).max(5.0);
    let max_ring = (span / cs).ceil() as i32 + 1;

    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::with_capacity(n / 4 + 1);
    for (idx, &(cx, cy)) in centers.iter().enumerate() {
        let gx = (cx / cs).floor() as i32;
        let gy = (cy / cs).floor() as i32;
        grid.entry((gx, gy)).or_default().push(idx);
    }

    let mut visited = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let mut cur = (0.0, 0.0);

    for _ in 0..n {
        let gx0 = (cur.0 / cs).floor() as i32;
        let gy0 = (cur.1 / cs).floor() as i32;
        let mut best_idx = None;
        let mut best_d2 = f64::INFINITY;

        for r in 0i32..=max_ring {
            for dy in -r..=r {
                for dx in -r..=r {
                    if r > 0 && dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    if let Some(bucket) = grid.get(&(gx0 + dx, gy0 + dy)) {
                        for &idx in bucket {
                            if visited[idx] {
                                continue;
                            }
                            let p = centers[idx];
                            let d2 = (p.0 - cur.0).powi(2) + (p.1 - cur.1).powi(2);
                            if d2 < best_d2 {
                                best_d2 = d2;
                                best_idx = Some(idx);
                            }
                        }
                    }
                }
            }
            if best_idx.is_some() && r >= 1 {
                break;
            }
        }

        let chosen = match best_idx {
            Some(idx) => idx,
            None => {
                let mut fb = 0;
                let mut fb_d2 = f64::INFINITY;
                for (idx, &p) in centers.iter().enumerate() {
                    if !visited[idx] {
                        let d2 = (p.0 - cur.0).powi(2) + (p.1 - cur.1).powi(2);
                        if d2 < fb_d2 {
                            fb_d2 = d2;
                            fb = idx;
                        }
                    }
                }
                fb
            }
        };

        visited[chosen] = true;
        cur = centers[chosen];
        order.push(chosen);
    }

    let mut opt: Vec<Option<Shape>> = shapes.into_iter().map(Some).collect();
    order
        .into_iter()
        .filter_map(|idx| opt[idx].take())
        .collect()
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
        warnings.push(tr(
            en,
            "Загрузите изображение, чтобы получить узор",
            "Load an image to get a pattern",
        ));
    }

    if p.pattern == "lines" {
        let lp = p.line_pitch;
        let k = if p.wave {
            2.0 * PI * p.wave_amp / p.wave_len
        } else {
            0.0
        };
        let cos_f = 1.0 / (1.0 + k * k).sqrt();
        let cap = lp * cos_f - p.bridge;
        max_allowed = cap;
        let wmin = p.w_min.max(min_allowed);
        let mut wmax = p.w_max;
        if wmax > cap {
            warnings.push(if en {
                format!(
                    "Max width limited to {} mm to keep the bridge at least {} mm",
                    dec(cap.max(0.0), en),
                    dec(p.bridge, en)
                )
            } else {
                format!(
                    "Макс. ширина ограничена до {} мм, чтобы перемычка была не меньше {} мм",
                    dec(cap.max(0.0), en),
                    dec(p.bridge, en)
                )
            });
            wmax = cap;
        }
        if p.w_min < min_allowed {
            warnings.push(if en {
                format!(
                    "Min width raised to {} mm: a slot in {} mm metal cannot be narrower",
                    dec(min_allowed, en),
                    dec(p.thickness, en)
                )
            } else {
                format!(
                    "Мин. ширина поднята до {} мм: для металла {} мм прорезь не может быть уже",
                    dec(min_allowed, en),
                    dec(p.thickness, en)
                )
            });
        }
        let est = (w * h) / (lp * p.step);
        if est > MAX_SHAPES * 4.0 {
            warnings.push(tr(
                en,
                "Слишком мелкий шаг линий или выборки для такого листа",
                "Line pitch or sampling step is too small for this sheet",
            ));
        } else if wmax < wmin {
            warnings.push(tr(
                en,
                "Шаг линий слишком мал для заданной перемычки и мин. ширины",
                "Line pitch is too small for this bridge and min width",
            ));
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
                let mut runs: Vec<(i64, i64, Vec<Segment>)> = Vec::new();
                let mut cur: Vec<Segment> = Vec::new();
                let mut cur_start = 0i64;

                for s in 0..=n_steps {
                    let u = -reach + s as f64 * p.step + p.offset_x;
                    let (lv, dv) = if p.wave {
                        let ph = 2.0 * PI * u / p.wave_len;
                        (
                            v0 + p.wave_amp * ph.sin(),
                            p.wave_amp * 2.0 * PI / p.wave_len * ph.cos(),
                        )
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
                                let ww = if prop {
                                    wmax * t
                                } else {
                                    wmin + (wmax - wmin) * tt
                                };
                                // Ширина траектории меньше номинала на kerf (отрицательный — шире).
                                let half = (ww - p.kerf) / 2.0;
                                if ww >= wmin - 1e-9 && half > 0.005 {
                                    let l = (x + nx * half, y + ny * half);
                                    let r = (x - nx * half, y - ny * half);
                                    if inside(l.0, l.1) && inside(r.0, r.1) {
                                        ok = Some((l, r));
                                    }
                                }
                            }
                        }
                    }
                    match ok {
                        Some(pair) => {
                            if cur.is_empty() {
                                cur_start = s;
                            }
                            cur.push(pair);
                        }
                        None => {
                            if !cur.is_empty() {
                                runs.push((cur_start, s - 1, std::mem::take(&mut cur)));
                            }
                        }
                    }
                }
                if !cur.is_empty() {
                    runs.push((cur_start, n_steps, cur));
                }

                // Проверка перемычки вдоль линии: если разрыв между соседними прорезями меньше p.bridge,
                // укорачиваем смежные торцы на половину нехватки — итоговый зазор ровно p.bridge.
                let runs_len = runs.len();
                for idx in 0..runs_len.saturating_sub(1) {
                    let gap_steps = runs[idx + 1].0 - runs[idx].1;
                    // Мост металла между прорезями меньше зазора траекторий на kerf.
                    let gap_mm = (gap_steps as f64 * p.step - p.kerf).max(0.0);
                    let trim = ((p.bridge - gap_mm) / p.step / 2.0).ceil() as usize;
                    if trim > 0 {
                        let keep_left = runs[idx].2.len().saturating_sub(trim);
                        runs[idx].2.truncate(keep_left);
                        let drop_right = trim.min(runs[idx + 1].2.len());
                        runs[idx + 1].2.drain(0..drop_right);
                    }
                }

                let mut line_shapes = Vec::new();
                for (_, _, run) in runs {
                    if run.len() < 2 {
                        continue;
                    }
                    let len = (run.len() - 1) as f64 * p.step;
                    if len < wmin.max(p.step) {
                        continue;
                    }
                    let left_raw: Vec<(f64, f64)> = run.iter().map(|q| q.0).collect();
                    let right_raw: Vec<(f64, f64)> = run.iter().rev().map(|q| q.1).collect();

                    let mut pts = simplify_open(&left_raw, 0.02);
                    pts.extend(simplify_open(&right_raw, 0.02));

                    let maxw = run
                        .iter()
                        .map(|q| ((q.0 .0 - q.1 .0).powi(2) + (q.0 .1 - q.1 .1).powi(2)).sqrt())
                        .fold(0.0, f64::max);
                    line_shapes.push(Shape::Poly {
                        pts,
                        size: round2(maxw),
                    });
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
                format!(
                    "Max size limited to {} mm to keep the bridge at least {} mm",
                    dec(cap.max(0.0), en),
                    dec(p.bridge, en)
                )
            } else {
                format!(
                    "Макс. размер ограничен до {} мм, чтобы перемычка была не меньше {} мм",
                    dec(cap.max(0.0), en),
                    dec(p.bridge, en)
                )
            });
            dmax = cap;
        }
        let dmin = p.d_min.max(min_allowed);
        if p.d_min < min_allowed {
            warnings.push(if en {
                format!(
                    "Min size raised to {} mm: a hole in {} mm metal cannot be smaller",
                    dec(min_allowed, en),
                    dec(p.thickness, en)
                )
            } else {
                format!(
                    "Мин. размер поднят до {} мм: для металла {} мм отверстие не может быть меньше",
                    dec(min_allowed, en),
                    dec(p.thickness, en)
                )
            });
        }
        let mut fixed = if p.fixed { p.fixed_list() } else { vec![] };
        if p.fixed {
            let before = fixed.len();
            fixed.retain(|d| *d >= dmin - 1e-9 && *d <= dmax + 1e-9);
            if fixed.len() < before {
                warnings.push(tr(
                    en,
                    "Часть заданных размеров вне допустимого диапазона и не используется",
                    "Some fixed sizes are out of the allowed range and are not used",
                ));
            }
            if fixed.is_empty() {
                warnings.push(tr(
                    en,
                    "Нет ни одного допустимого заданного размера",
                    "None of the fixed sizes is allowed",
                ));
            }
        }
        let est = (w * h) / (pch * pch);
        if est > MAX_SHAPES {
            warnings.push(if en {
                format!(
                    "Pitch is too small: about {} holes. Increase the pitch",
                    count(est, en)
                )
            } else {
                format!(
                    "Слишком мелкий шаг: около {} отверстий. Увеличьте шаг",
                    count(est, en)
                )
            });
        } else if dmax < dmin {
            warnings.push(tr(
                en,
                "Шаг слишком мал для заданной перемычки и мин. размера",
                "Pitch is too small for this bridge and min size",
            ));
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
                        let dp = dmax * t;
                        if dp < dmin - 1e-9 {
                            continue;
                        }
                        dp
                    } else {
                        dmin + (dmax - dmin) * tt
                    };
                    if fixed.is_empty() && p.quant > 0.0 {
                        d = ((d / p.quant + 1e-9).floor() * p.quant).max(dmin);
                    }
                    if !fixed.is_empty() {
                        d = *fixed
                            .iter()
                            .min_by(|a, b2| (*a - d).abs().partial_cmp(&(*b2 - d).abs()).unwrap())
                            .unwrap();
                    }
                    let d = if fixed.is_empty() { round2(d) } else { d };
                    // Случайный разброс: диаметр гуляет на ±jitter % по зерну,
                    // детерминированно от координат. Крупные не превышают каплю по перемычке.
                    let d = if p.jitter > 0.0 && fixed.is_empty() {
                        let u = jitter_u01(p.seed, x, y) * 2.0 - 1.0;
                        (d * (1.0 + u * p.jitter / 100.0)).min(max_allowed)
                    } else {
                        d
                    };
                    // Компенсация реза: траектория меньше номинала на kerf (отрицательный — раздувает).
                    let d = round2(d - p.kerf);
                    if d <= 0.01 {
                        continue;
                    }
                    // Локальные точки фигуры до поворота и переноса. Круг — особый случай:
                    // остаётся окружностью (дуги G02, цилиндры в STEP), остальное — полигон.
                    let poly: Option<Vec<(f64, f64)>> = match p.shape.as_str() {
                        "square" => {
                            let hs = d / 2.0;
                            Some(vec![(hs, hs), (-hs, hs), (-hs, -hs), (hs, -hs)])
                        }
                        "diamond" => {
                            let hs = d / 2.0;
                            Some(vec![(hs, 0.0), (0.0, hs), (-hs, 0.0), (0.0, -hs)])
                        }
                        "oval" => {
                            let (rx, ry) = (d / 2.0, d / 2.0 * p.shape_aspect);
                            let n = chord_circle_n(rx.max(ry));
                            Some(
                                (0..n)
                                    .map(|i| {
                                        let a = 2.0 * PI * i as f64 / n as f64;
                                        (rx * a.cos(), ry * a.sin())
                                    })
                                    .collect(),
                            )
                        }
                        "triangle" => {
                            let hh = d * p.shape_aspect / 2.0;
                            Some(vec![(0.0, hh), (-d / 2.0, -hh), (d / 2.0, -hh)])
                        }
                        "star" => {
                            let r = d / 2.0;
                            let n = p.star_points as usize;
                            Some(
                                (0..2 * n)
                                    .map(|i| {
                                        let rr = if i % 2 == 0 { r } else { r * p.star_inner };
                                        let a = PI / 2.0 + PI * i as f64 / n as f64;
                                        (rr * a.cos(), rr * a.sin())
                                    })
                                    .collect(),
                            )
                        }
                        "hex" => Some(
                            (0..6)
                                .map(|i| {
                                    let a = PI / 3.0 * i as f64;
                                    (d / 2.0 * a.cos(), d / 2.0 * a.sin())
                                })
                                .collect(),
                        ),
                        "slot" => {
                            let r = d / 2.0;
                            // Длина задаётся пропорцией (длина/ширина); меньше ширины не бывает,
                            // при пропорции 1 и ниже получается круг.
                            let c = (d * p.shape_aspect.max(1.0) - d) / 2.0;
                            let n = (chord_circle_n(r) / 2).max(6);
                            let mut pts: Vec<(f64, f64)> = Vec::with_capacity(2 * n + 2);
                            // Правый торец: от -90° до +90° через 0°.
                            for i in 0..=n {
                                let a = -PI / 2.0 + PI * i as f64 / n as f64;
                                pts.push((c + r * a.cos(), r * a.sin()));
                            }
                            // Левый торец: от 90° до 270° через 180°.
                            for i in 0..=n {
                                let a = PI / 2.0 + PI * i as f64 / n as f64;
                                pts.push((-c + r * a.cos(), r * a.sin()));
                            }
                            Some(pts)
                        }
                        _ => None,
                    };
                    let sh = match poly {
                        None => {
                            let e = d / 2.0;
                            if x - e < b - 1e-9
                                || x + e > w - b + 1e-9
                                || y - e < b - 1e-9
                                || y + e > h - b + 1e-9
                            {
                                removed_edge += 1;
                                continue;
                            }
                            Shape::Circle { x, y, d }
                        }
                        Some(local) => {
                            // Габариты после поворота — точная проверка кромки для любого угла.
                            let (mut ex, mut ey) = (0.0f64, 0.0f64);
                            for &(px, py) in &local {
                                let (rx, ry) = (px * ca - py * sa, px * sa + py * ca);
                                ex = ex.max(rx.abs());
                                ey = ey.max(ry.abs());
                            }
                            if x - ex < b - 1e-9
                                || x + ex > w - b + 1e-9
                                || y - ey < b - 1e-9
                                || y + ey > h - b + 1e-9
                            {
                                removed_edge += 1;
                                continue;
                            }
                            Shape::Poly {
                                pts: local
                                    .iter()
                                    .map(|q| (x + q.0 * ca - q.1 * sa, y + q.0 * sa + q.1 * ca))
                                    .collect(),
                                size: d,
                            }
                        }
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

    // У узора «линии» порядок уже задан построчно серпантином — оптимизация только ломает его.
    let shapes = if p.pattern == "lines" {
        shapes
    } else {
        optimize_path(shapes, p.pitch * 2.0)
    };

    finish(shapes, warnings, removed_edge, min_allowed, max_allowed, p)
}

fn finish(
    shapes: Vec<Shape>,
    mut warnings: Vec<String>,
    removed_edge: usize,
    min_allowed: f64,
    max_allowed: f64,
    p: Params,
) -> GenResult {
    let (w, h) = (p.sheet_w, p.sheet_h);
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
            st.by_size.push(SizeCount {
                size: sizes[i],
                count: j - i,
            });
            i = j;
        }
    }
    st.size_min = if smin.is_finite() { smin } else { 0.0 };
    st.size_max = smax;
    st.open_area_pct = area / (w * h) * 100.0;
    st.cut_len_m = cut / 1000.0;
    st.travel_m = travel / 1000.0;
    st.pierces = shapes.len() + if p.outline { 1 } else { 0 };
    st.time_min =
        cut / p.cut_speed + travel / p.rapid_speed + st.pierces as f64 * p.pierce_time / 60.0;
    st.cost_cut = st.cut_len_m * p.price_m;
    st.cost_pierce = st.pierces as f64 * p.price_pierce;
    st.cost_material = w * h / 1e6 * p.price_m2;
    st.cost_fixed = p.price_fixed;
    st.cost = st.cost_cut + st.cost_pierce + st.cost_material + st.cost_fixed;
    st.weight_kg = (w * h - area) * p.thickness * p.density * 1e-6;
    // STEP — единственный формат, который растёт на ~1,2 КБ на отверстие: предупреждаем заранее.
    if shapes.len() > 20_000 {
        let mb = (shapes.len() as f64 * 1.2 / 1024.0).round().max(1.0) as u64;
        warnings.push(if p.lang == "en" {
            format!(
                "STEP will be large: about {} holes, ~{mb} MB. For CAD prefer STL or a smaller area",
                count(shapes.len() as f64, true)
            )
        } else {
            format!(
                "STEP будет большим: около {} отверстий, ~{mb} МБ. Для CAD лучше STL или меньшее поле",
                count(shapes.len() as f64, false)
            )
        });
    }
    st.warnings = warnings;
    GenResult {
        shapes,
        stats: st,
        params: p,
    }
}

fn dec(v: f64, en: bool) -> String {
    let s = crate::export::num(v, 2);
    if en {
        s
    } else {
        s.replace('.', ",")
    }
}

fn count(v: f64, en: bool) -> String {
    let digits = (v.round().max(0.0) as u64).to_string();
    let sep = if en { ',' } else { '\u{a0}' };
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

fn tr(en: bool, ru: &str, eng: &str) -> String {
    if en {
        eng.to_string()
    } else {
        ru.to_string()
    }
}
