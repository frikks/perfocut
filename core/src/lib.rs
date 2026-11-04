pub mod export;
pub mod gen;
pub mod image;
pub mod params;
pub mod zip;

use gen::{GenResult, Shape};
use params::Params;
use std::sync::Mutex;

struct State {
    img: Option<image::Gray>,
    res: Option<GenResult>,
    out: Vec<u8>,
    zip: Option<zip::ZipWriter>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        *g = Some(State { img: None, res: None, out: Vec::new(), zip: None });
    }
    f(g.as_mut().unwrap())
}

unsafe fn slice<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 || ptr.is_null() {
        &[]
    } else {
        std::slice::from_raw_parts(ptr, len)
    }
}

/// Бинарное представление геометрии для предпросмотра: f32 LE.
/// [kind, ...] где круг: 0, x, y, d; многоугольник: 1, n, x0, y0, ...
pub fn geometry_bytes(r: &GenResult) -> Vec<u8> {
    let mut v: Vec<f32> = Vec::with_capacity(r.shapes.len() * 4);
    for s in &r.shapes {
        match s {
            Shape::Circle { x, y, d } => v.extend_from_slice(&[0.0, *x as f32, *y as f32, *d as f32]),
            Shape::Poly { pts, .. } => {
                v.push(1.0);
                v.push(pts.len() as f32);
                for q in pts {
                    v.push(q.0 as f32);
                    v.push(q.1 as f32);
                }
            }
        }
    }
    let mut b = Vec::with_capacity(v.len() * 4);
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
    b
}

pub fn export_format(r: &GenResult, fmt: &str) -> Option<Vec<u8>> {
    Some(match fmt {
        "dxf" => export::dxf(r).into_bytes(),
        "svg" => export::svg(r).into_bytes(),
        "nc" | "cnc" | "gcode" | "tap" => export::gcode(r).into_bytes(),
        "plt" => export::plt(r).into_bytes(),
        "eps" => export::eps(r).into_bytes(),
        "pdf" => export::pdf(r),
        "csv" => export::csv(r).into_bytes(),
        "report" => export::report(r).into_bytes(),
        "stats" => serde_json::to_vec_pretty(&r.stats).ok()?,
        "settings" => {
            let v = serde_json::json!({ "app": "PerfoCut", "algo_version": params::ALGO_VERSION, "params": r.params });
            serde_json::to_vec_pretty(&v).ok()?
        }
        _ => return None,
    })
}

// ---------------- WASM / C ABI ----------------

#[no_mangle]
pub extern "C" fn pc_alloc(n: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(n.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

#[no_mangle]
pub unsafe extern "C" fn pc_free(p: *mut u8, n: usize) {
    if !p.is_null() {
        drop(Vec::from_raw_parts(p, 0, n.max(1)));
    }
}

#[no_mangle]
pub extern "C" fn pc_out_ptr() -> *const u8 {
    with(|s| s.out.as_ptr())
}

#[no_mangle]
pub extern "C" fn pc_out_len() -> usize {
    with(|s| s.out.len())
}

#[no_mangle]
pub extern "C" fn pc_defaults() -> usize {
    with(|s| {
        s.out = serde_json::to_vec(&Params::default()).unwrap_or_default();
        s.out.len()
    })
}

#[no_mangle]
pub extern "C" fn pc_version() -> u32 {
    params::ALGO_VERSION
}

#[no_mangle]
pub unsafe extern "C" fn pc_set_image(ptr: *const u8, w: u32, h: u32) {
    let d = slice(ptr, (w as usize) * (h as usize) * 4);
    let g = if w > 0 && h > 0 { Some(image::Gray::from_rgba(d, w as usize, h as usize)) } else { None };
    with(|s| s.img = g);
}

#[no_mangle]
pub unsafe extern "C" fn pc_generate(ptr: *const u8, len: usize) -> i32 {
    let d = slice(ptr, len);
    let parsed: Result<Params, _> = serde_json::from_slice(d);
    with(|s| match parsed {
        Ok(p) => {
            let r = gen::generate(s.img.as_ref(), &p);
            s.out = serde_json::to_vec(&r.stats).unwrap_or_default();
            s.res = Some(r);
            0
        }
        Err(e) => {
            s.out = format!("Ошибка параметров / parameter error: {e}").into_bytes();
            1
        }
    })
}

#[no_mangle]
pub extern "C" fn pc_geometry() -> usize {
    with(|s| {
        s.out = s.res.as_ref().map(geometry_bytes).unwrap_or_default();
        s.out.len()
    })
}

#[no_mangle]
pub unsafe extern "C" fn pc_export(ptr: *const u8, len: usize) -> i32 {
    let fmt = String::from_utf8_lossy(slice(ptr, len)).to_string();
    with(|s| match s.res.as_ref().and_then(|r| export_format(r, &fmt)) {
        Some(b) => {
            s.out = b;
            0
        }
        None => {
            s.out.clear();
            1
        }
    })
}

#[no_mangle]
pub extern "C" fn pc_zip_new() {
    with(|s| s.zip = Some(zip::ZipWriter::default()));
}

#[no_mangle]
pub unsafe extern "C" fn pc_zip_add(np: *const u8, nl: usize, dp: *const u8, dl: usize) {
    let name = String::from_utf8_lossy(slice(np, nl)).to_string();
    let data = slice(dp, dl);
    with(|s| {
        if let Some(z) = s.zip.as_mut() {
            z.add(&name, data);
        }
    });
}

#[no_mangle]
pub extern "C" fn pc_zip_finish() -> usize {
    with(|s| {
        s.out = s.zip.take().map(|z| z.finish()).unwrap_or_default();
        s.out.len()
    })
}

#[no_mangle]
pub unsafe extern "C" fn pc_zip_list(ptr: *const u8, len: usize) -> i32 {
    let d = slice(ptr, len);
    let l = zip::list(d);
    with(|s| match l {
        Some(v) => {
            s.out = serde_json::to_vec(&v).unwrap_or_default();
            0
        }
        None => {
            s.out.clear();
            1
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: usize, h: usize) -> image::Gray {
        let mut d = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let v = (255.0 * x as f64 / (w - 1) as f64) as u8;
                let i = (y * w + x) * 4;
                d[i] = v;
                d[i + 1] = v;
                d[i + 2] = v;
                d[i + 3] = 255;
            }
        }
        image::Gray::from_rgba(&d, w, h)
    }

    #[test]
    fn prop_mode_like_svg_tracing() {
        let g = gradient(200, 120);
        let mut p = Params::default();
        p.grid = "square".into();
        p.size_mode = "prop".into();
        p.tone_cut = 0.0;
        p.quant = 0.0;
        p.d_max = 10.0;
        let r = gen::generate(Some(&g), &p);
        assert!(r.stats.count > 100);
        let dmin = p.d_min.max(p.thickness * p.min_k);
        for s in &r.shapes {
            assert!(s.size() >= dmin - 1e-6 && s.size() <= 10.0 + 1e-6);
        }
        // размер пропорционален тону: левый край (чёрный) около макс., правее меньше
        let mut c: Vec<(f64, f64)> = r.shapes.iter().filter_map(|s| if let Shape::Circle { x, d, .. } = s { Some((*x, *d)) } else { None }).collect();
        c.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        assert!(c.first().unwrap().1 > c.last().unwrap().1);
        p.lang = "en".into();
        p.d_min = 0.1;
        let r2 = gen::generate(Some(&g), &p);
        assert!(r2.stats.warnings.iter().any(|w| w.starts_with("Min size raised")));
        let mut lp = p.clone();
        lp.pattern = "lines".into();
        let r3 = gen::generate(Some(&g), &lp);
        assert!(r3.stats.count > 5);
    }

    #[test]
    fn bridges_and_edges_hold() {
        let g = gradient(200, 120);
        for grid in ["square", "tri", "tri90"] {
            for shape in ["circle", "square", "diamond"] {
                let mut p = Params::default();
                p.grid = grid.into();
                p.shape = shape.into();
                p.angle = 15.0;
                p.d_max = 50.0;
                let r = gen::generate(Some(&g), &p);
                assert!(r.stats.count > 100, "{grid} {shape}: {}", r.stats.count);
                for s in &r.shapes {
                    let sz = s.size();
                    assert!(sz >= p.thickness * p.min_k - 1e-6);
                    assert!(sz <= r.stats.max_allowed + 1e-6);
                }
                // кромка: все точки внутри поля
                for s in &r.shapes {
                    let pts = match s {
                        Shape::Circle { x, y, d } => export::circle_pts(*x, *y, *d, 0.01),
                        Shape::Poly { pts, .. } => pts.clone(),
                    };
                    for q in pts {
                        assert!(q.0 >= p.border - 0.02 && q.0 <= p.sheet_w - p.border + 0.02);
                        assert!(q.1 >= p.border - 0.02 && q.1 <= p.sheet_h - p.border + 0.02);
                    }
                }
                // перемычка для кругов: проверка соседей
                if shape == "circle" {
                    let c: Vec<(f64, f64, f64)> = r
                        .shapes
                        .iter()
                        .filter_map(|s| if let Shape::Circle { x, y, d } = s { Some((*x, *y, *d)) } else { None })
                        .collect();
                    for (i, a) in c.iter().enumerate().step_by(7) {
                        for b in c.iter().skip(i + 1) {
                            let dist = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                            if dist < 3.0 * p.pitch {
                                assert!(dist - a.2 / 2.0 - b.2 / 2.0 >= p.bridge - 1e-6);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn lines_and_exports() {
        let g = gradient(200, 120);
        let mut p = Params::default();
        p.pattern = "lines".into();
        p.wave = true;
        let r = gen::generate(Some(&g), &p);
        assert!(r.stats.count > 10);
        for f in ["dxf", "svg", "nc", "plt", "eps", "pdf", "csv", "report", "stats", "settings"] {
            let b = export_format(&r, f).unwrap();
            assert!(b.len() > 50, "{f}");
        }
        let mut z = zip::ZipWriter::default();
        z.add("a.txt", b"hello");
        z.add("папка/б.json", b"{}");
        let zb = z.finish();
        let l = zip::list(&zb).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(&zb[l[0].offset..l[0].offset + l[0].len], b"hello");
    }
}
