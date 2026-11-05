//! Рисунок от руки. Штрихи кисти хранятся вектором: точки в миллиметрах холста,
//! диаметр, мягкость и тон. Для расчёта перфорации штрихи растеризуются в картинку,
//! поэтому рисунок можно дорисовать, поменять толщину всех штрихов и выгрузить в SVG.

use serde::{Deserialize, Serialize};
use std::fmt::Write;

fn one() -> f64 {
    1.0
}

/// Один штрих кисти или ластика.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Stroke {
    /// Ластик возвращает подложку: белый холст или исходную картинку
    #[serde(default)]
    pub erase: bool,
    /// Диаметр кисти, мм
    pub size: f64,
    /// Мягкость края, 0 — резкий, 1 — плавный от центра
    #[serde(default)]
    pub soft: f64,
    /// Темнота краски: 1 — чёрный, 0 — белый
    #[serde(default = "one")]
    pub tone: f64,
    /// Точки в миллиметрах холста, ось Y вниз
    pub pts: Vec<[f64; 2]>,
}

/// Рисунок: размер холста в миллиметрах и штрихи по порядку.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Sketch {
    pub w: f64,
    pub h: f64,
    /// Общий множитель толщины штрихов
    #[serde(default = "one")]
    pub width_k: f64,
    #[serde(default)]
    pub strokes: Vec<Stroke>,
}

impl Sketch {
    pub fn new(w: f64, h: f64) -> Sketch {
        Sketch { w, h, width_k: 1.0, strokes: Vec::new() }
    }

    /// Размер растра под холст: длинная сторона `max_side` пикселей.
    pub fn raster_size(&self, max_side: usize) -> (usize, usize) {
        let (w, h) = (self.w.max(1e-3), self.h.max(1e-3));
        let k = max_side as f64 / w.max(h);
        (((w * k).round() as usize).max(1), ((h * k).round() as usize).max(1))
    }

    /// Все штрихи поверх подложки `base` (RGBA, w×h).
    pub fn render(&self, base: &[u8], w: usize, h: usize) -> Vec<u8> {
        let mut px = base.to_vec();
        for s in &self.strokes {
            self.paint(&mut px, base, w, h, s);
        }
        px
    }

    /// Наносит один штрих на RGBA-пиксели `px` (w×h). Ластик берёт пиксели из `base`.
    pub fn paint(&self, px: &mut [u8], base: &[u8], w: usize, h: usize, s: &Stroke) {
        let kx = w as f64 / self.w.max(1e-6);
        let ky = h as f64 / self.h.max(1e-6);
        let r = s.size * self.width_k.clamp(0.05, 20.0) / 2.0;
        if s.pts.is_empty() || !(r > 0.0) || px.len() < w * h * 4 || base.len() < w * h * 4 {
            return;
        }
        // Сглаживание края примерно на один пиксель, мягкость расширяет переход внутрь
        let aa = 0.5 / kx.min(ky);
        let soft = s.soft.clamp(0.0, 1.0);
        let e1 = r + aa;
        let e0 = (r * (1.0 - soft) - aa).min(e1 - 1e-9);
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for q in &s.pts {
            x0 = x0.min(q[0]);
            y0 = y0.min(q[1]);
            x1 = x1.max(q[0]);
            y1 = y1.max(q[1]);
        }
        let ix0 = (((x0 - e1) * kx).floor().max(0.0) as usize).min(w);
        let iy0 = (((y0 - e1) * ky).floor().max(0.0) as usize).min(h);
        let ix1 = (((x1 + e1) * kx).ceil().max(0.0) as usize).min(w);
        let iy1 = (((y1 + e1) * ky).ceil().max(0.0) as usize).min(h);
        if ix1 <= ix0 || iy1 <= iy0 {
            return;
        }
        let bw = ix1 - ix0;
        let mut cov = vec![0f32; bw * (iy1 - iy0)];
        let n = s.pts.len();
        for i in 0..n.max(2) - 1 {
            let a = s.pts[i.min(n - 1)];
            let b = s.pts[(i + 1).min(n - 1)];
            let sx0 = ((a[0].min(b[0]) - e1) * kx).floor().max(ix0 as f64) as usize;
            let sy0 = ((a[1].min(b[1]) - e1) * ky).floor().max(iy0 as f64) as usize;
            let sx1 = (((a[0].max(b[0]) + e1) * kx).ceil().max(0.0) as usize).min(ix1);
            let sy1 = (((a[1].max(b[1]) + e1) * ky).ceil().max(0.0) as usize).min(iy1);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let ll = dx * dx + dy * dy;
            for y in sy0..sy1 {
                let my = (y as f64 + 0.5) / ky;
                let row = (y - iy0) * bw;
                for x in sx0..sx1 {
                    let c = &mut cov[row + x - ix0];
                    if *c >= 1.0 {
                        continue;
                    }
                    let mx = (x as f64 + 0.5) / kx;
                    let t = if ll > 0.0 { (((mx - a[0]) * dx + (my - a[1]) * dy) / ll).clamp(0.0, 1.0) } else { 0.0 };
                    let (ex, ey) = (mx - a[0] - t * dx, my - a[1] - t * dy);
                    let d = (ex * ex + ey * ey).sqrt();
                    if d >= e1 {
                        continue;
                    }
                    let v = if d <= e0 {
                        1.0
                    } else {
                        let u = 1.0 - (d - e0) / (e1 - e0);
                        u * u * (3.0 - 2.0 * u)
                    } as f32;
                    if v > *c {
                        *c = v;
                    }
                }
            }
        }
        let ink = ((1.0 - s.tone.clamp(0.0, 1.0)) * 255.0) as f32;
        for y in iy0..iy1 {
            for x in ix0..ix1 {
                let c = cov[(y - iy0) * bw + x - ix0];
                if c <= 0.0 {
                    continue;
                }
                let i = (y * w + x) * 4;
                for k in 0..3 {
                    let cur = f32::from(px[i + k]);
                    let to = if s.erase { f32::from(base[i + k]) } else { ink };
                    px[i + k] = (cur + (to - cur) * c).round().clamp(0.0, 255.0) as u8;
                }
                px[i + 3] = 255;
            }
        }
    }

    /// Штрихи в SVG: размеры в миллиметрах, ластик белым цветом.
    pub fn svg(&self, title: &str) -> String {
        let n = |v: f64| crate::export::num(v, 2);
        let mut o = String::new();
        let _ = write!(
            o,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">\n<title>{t}</title>\n<rect width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>\n<g fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\n",
            w = n(self.w),
            h = n(self.h),
            t = title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        );
        for s in &self.strokes {
            if s.pts.is_empty() {
                continue;
            }
            let g = if s.erase { 255 } else { ((1.0 - s.tone.clamp(0.0, 1.0)) * 255.0).round() as u8 };
            let _ = write!(
                o,
                "<path class=\"{}\" stroke=\"#{g:02x}{g:02x}{g:02x}\" stroke-width=\"{}\" d=\"M",
                if s.erase { "eraser" } else { "brush" },
                n(s.size * self.width_k)
            );
            for (i, q) in s.pts.iter().enumerate() {
                if i > 0 {
                    o.push('L');
                }
                let _ = write!(o, "{} {}", n(q[0]), n(q[1]));
            }
            if s.pts.len() == 1 {
                // Точка: отрезок нулевой длины с круглыми концами
                let _ = write!(o, "L{} {}", n(s.pts[0][0]), n(s.pts[0][1]));
            }
            o.push_str("\"/>\n");
        }
        o.push_str("</g>\n</svg>\n");
        o
    }
}

fn dist_to_seg(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let ll = dx * dx + dy * dy;
    let t = if ll > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / ll).clamp(0.0, 1.0) } else { 0.0 };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

fn simplify(pts: &[[f64; 2]], eps: f64, out: &mut Vec<[f64; 2]>) {
    // Рамер — Дуглас — Пекер без рекурсии
    let n = pts.len();
    if n < 3 {
        out.extend_from_slice(pts);
        return;
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
    out.extend(pts.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p));
}

/// Сглаживает линию, нарисованную рукой: убирает дрожание и лишние точки, концы на месте.
pub fn smooth(raw: &[[f64; 2]], size: f64) -> Vec<[f64; 2]> {
    let gap = (size * 0.06).max(0.05);
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(raw.len());
    for (i, q) in raw.iter().enumerate() {
        if !q[0].is_finite() || !q[1].is_finite() {
            continue;
        }
        let last = i + 1 == raw.len();
        match pts.last() {
            Some(l) if !last && (q[0] - l[0]).hypot(q[1] - l[1]) < gap => {}
            Some(l) if last && (q[0] - l[0]).hypot(q[1] - l[1]) < 1e-9 => {}
            _ => pts.push(*q),
        }
    }
    // Чайкин, два прохода
    for _ in 0..2 {
        if pts.len() < 3 {
            break;
        }
        let mut nx = Vec::with_capacity(pts.len() * 2);
        nx.push(pts[0]);
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            nx.push([a[0] * 0.75 + b[0] * 0.25, a[1] * 0.75 + b[1] * 0.25]);
            nx.push([a[0] * 0.25 + b[0] * 0.75, a[1] * 0.25 + b[1] * 0.75]);
        }
        nx.push(pts[pts.len() - 1]);
        pts = nx;
    }
    let mut out = Vec::with_capacity(pts.len());
    simplify(&pts, (size * 0.02).max(0.02), &mut out);
    for q in &mut out {
        q[0] = (q[0] * 100.0).round() / 100.0;
        q[1] = (q[1] * 100.0).round() / 100.0;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white(w: usize, h: usize) -> Vec<u8> {
        vec![255u8; w * h * 4]
    }

    fn at(px: &[u8], w: usize, x: usize, y: usize) -> u8 {
        px[(y * w + x) * 4]
    }

    #[test]
    fn brush_darkens_and_eraser_restores() {
        let mut sk = Sketch::new(100.0, 50.0);
        let (w, h) = sk.raster_size(200);
        assert_eq!((w, h), (200, 100));
        let base = white(w, h);
        sk.strokes.push(Stroke { erase: false, size: 10.0, soft: 0.0, tone: 1.0, pts: vec![[10.0, 25.0], [90.0, 25.0]] });
        let px = sk.render(&base, w, h);
        assert_eq!(at(&px, w, 100, 50), 0);
        assert_eq!(at(&px, w, 100, 5), 255);
        // половина тона даёт серый
        sk.strokes[0].tone = 0.5;
        let px = sk.render(&base, w, h);
        assert!((120..=135).contains(&at(&px, w, 100, 50)));
        sk.strokes[0].tone = 1.0;
        sk.strokes.push(Stroke { erase: true, size: 10.0, soft: 0.0, tone: 1.0, pts: vec![[50.0, 25.0]] });
        let px = sk.render(&base, w, h);
        assert_eq!(at(&px, w, 100, 50), 255);
        assert_eq!(at(&px, w, 40, 50), 0);
        // толщина всех штрихов меняется одним множителем
        sk.strokes.pop();
        sk.width_k = 2.0;
        let px = sk.render(&base, w, h);
        assert_eq!(at(&px, w, 100, 37), 0);
    }

    #[test]
    fn soft_edge_fades() {
        let mut sk = Sketch::new(100.0, 100.0);
        let base = white(100, 100);
        sk.strokes.push(Stroke { erase: false, size: 40.0, soft: 1.0, tone: 1.0, pts: vec![[50.0, 50.0]] });
        let px = sk.render(&base, 100, 100);
        let (c, m, e) = (at(&px, 100, 50, 50), at(&px, 100, 60, 50), at(&px, 100, 69, 50));
        assert!(c < 20 && c < m && m < e && e > 200, "{c} {m} {e}");
    }

    #[test]
    fn smooth_keeps_ends_and_drops_jitter() {
        let raw: Vec<[f64; 2]> = (0..=200).map(|i| [i as f64 * 0.5, if i % 2 == 0 { 0.02 } else { -0.02 }]).collect();
        let s = smooth(&raw, 4.0);
        assert!(s.len() < 10, "{}", s.len());
        assert_eq!(s.first().copied(), Some([0.0, 0.02]));
        assert_eq!(s.last().copied(), Some([100.0, 0.02]));
        assert_eq!(smooth(&[[1.0, 2.0]], 3.0), vec![[1.0, 2.0]]);
    }

    #[test]
    fn json_and_svg() {
        let mut sk = Sketch::new(300.0, 200.0);
        sk.strokes.push(Stroke { erase: false, size: 6.0, soft: 0.3, tone: 1.0, pts: vec![[1.0, 2.0], [30.0, 40.5]] });
        sk.strokes.push(Stroke { erase: true, size: 3.0, soft: 0.0, tone: 1.0, pts: vec![[5.0, 5.0]] });
        let j = serde_json::to_string(&sk).unwrap();
        let back: Sketch = serde_json::from_str(&j).unwrap();
        assert_eq!(back, sk);
        let old: Sketch = serde_json::from_str(r#"{"w":10,"h":5,"strokes":[{"size":2,"pts":[[1,1]]}]}"#).unwrap();
        assert_eq!((old.width_k, old.strokes[0].tone, old.strokes[0].erase), (1.0, 1.0, false));
        let svg = sk.svg("A & B");
        assert!(svg.contains("width=\"300mm\""));
        assert!(svg.contains("stroke=\"#000000\" stroke-width=\"6\" d=\"M1 2L30 40.5\""));
        assert!(svg.contains("class=\"eraser\" stroke=\"#ffffff\" stroke-width=\"3\" d=\"M5 5L5 5\""));
        assert!(svg.contains("A &amp; B"));
    }
}
