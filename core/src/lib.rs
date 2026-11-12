//! Ядро PerfoCut: генерация перфорации, проверки под лазер, расчёт стоимости и экспорт.
//! Без интерфейса: одинаково работает в Windows, Android и браузере.

pub mod export;
pub mod gen;
pub mod image;
pub mod params;
pub mod sketch;
pub mod step;
pub mod zip;

use gen::GenResult;

/// Выгружает результат в формат по его коду (`dxf`, `nc`, `svg`, `pdf`…).
/// Возвращает `None` для неизвестного кода.
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
        "stl" => export::stl(r).into_bytes(),
        "step" => step::step(r).into_bytes(),
        "stats" => serde_json::to_vec_pretty(&r.stats).ok()?,
        "settings" => {
            let v = serde_json::json!({ "app": "PerfoCut", "algo_version": params::ALGO_VERSION, "params": r.params });
            serde_json::to_vec_pretty(&v).ok()?
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gen::Shape;
    use params::Params;

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
        let p = Params {
            grid: "square".into(),
            size_mode: "prop".into(),
            tone_cut: 0.0,
            quant: 0.0,
            d_max: 10.0,
            ..Params::default()
        };
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
        let p = Params { lang: "en".into(), d_min: 0.1, ..p };
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
                let p = Params {
                    grid: grid.into(),
                    shape: shape.into(),
                    angle: 15.0,
                    d_max: 50.0,
                    ..Params::default()
                };
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
        let p = Params {
            pattern: "lines".into(),
            wave: true,
            ..Params::default()
        };
        let r = gen::generate(Some(&g), &p);
        assert!(r.stats.count > 10);
        for f in ["dxf", "svg", "nc", "plt", "eps", "pdf", "csv", "report", "stats", "settings", "stl", "step"] {
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

    #[test]
    fn warnings_use_locale_decimals() {
        let g = gradient(200, 120);
        let p = Params {
            pattern: "lines".into(),
            line_pitch: 9.05,
            w_max: 20.0,
            bridge: 1.5,
            ..Params::default()
        };
        let r = gen::generate(Some(&g), &p);
        let w = r.stats.warnings.iter().find(|w| w.starts_with("Макс. ширина")).expect("warning");
        assert!(w.contains("до 7,55 мм") && w.contains("не меньше 1,5 мм"), "{w}");
        let r = gen::generate(Some(&g), &Params { lang: "en".into(), ..p });
        assert!(r.stats.warnings.iter().any(|w| w.contains("to 7.55 mm")), "{:?}", r.stats.warnings);
    }
}
