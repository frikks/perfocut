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
        let mut c: Vec<(f64, f64)> = r
            .shapes
            .iter()
            .filter_map(|s| {
                if let Shape::Circle { x, d, .. } = s {
                    Some((*x, *d))
                } else {
                    None
                }
            })
            .collect();
        c.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        assert!(c.first().unwrap().1 > c.last().unwrap().1);
        let p = Params {
            lang: "en".into(),
            d_min: 0.1,
            ..p
        };
        let r2 = gen::generate(Some(&g), &p);
        assert!(r2
            .stats
            .warnings
            .iter()
            .any(|w| w.starts_with("Min size raised")));
        let mut lp = p.clone();
        lp.pattern = "lines".into();
        let r3 = gen::generate(Some(&g), &lp);
        assert!(r3.stats.count > 5);
    }

    #[test]
    fn bridges_and_edges_hold() {
        let g = gradient(200, 120);
        for grid in ["square", "tri", "tri90"] {
            for shape in [
                "circle", "square", "diamond", "oval", "star", "triangle", "hex", "slot",
            ] {
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
                        .filter_map(|s| {
                            if let Shape::Circle { x, y, d } = s {
                                Some((*x, *y, *d))
                            } else {
                                None
                            }
                        })
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
    fn rings_pattern_works() {
        let g = gradient(200, 120);
        let p = Params {
            pattern: "rings".into(),
            angle: 15.0,
            d_max: 50.0,
            ..Params::default()
        };
        let r = gen::generate(Some(&g), &p);
        assert!(r.stats.count > 100, "{}", r.stats.count);
        // кап по перемычке — как у квадратной сетки: шаг минус перемычка
        assert!((r.stats.max_allowed - (p.pitch - p.bridge)).abs() < 1e-9);
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
        // перемычка для кругов: соседи на своём кольце и на соседних
        let c: Vec<(f64, f64, f64)> = r
            .shapes
            .iter()
            .filter_map(|s| {
                if let Shape::Circle { x, y, d } = s {
                    Some((*x, *y, *d))
                } else {
                    None
                }
            })
            .collect();
        assert!(c.len() > 100);
        for (i, a) in c.iter().enumerate().step_by(7) {
            for b in c.iter().skip(i + 1) {
                let dist = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                if dist < 3.0 * p.pitch {
                    assert!(
                        dist - a.2 / 2.0 - b.2 / 2.0 >= p.bridge - 1e-6,
                        "bridge {dist:.3} {} {}",
                        a.2,
                        b.2
                    );
                }
            }
        }
        // отверстие в центре листа на месте (первое кольцо — одна точка)
        let center = c
            .iter()
            .min_by(|a, b| {
                let da = (a.0 - 500.0).powi(2) + (a.1 - 300.0).powi(2);
                let db = (b.0 - 500.0).powi(2) + (b.1 - 300.0).powi(2);
                da.partial_cmp(&db).unwrap()
            })
            .expect("center hole");
        assert!((center.0 - 500.0).abs() < 1e-6 && (center.1 - 300.0).abs() < 1e-6);
        // все формы строятся и экспортируются
        for shape in [
            "circle", "square", "diamond", "oval", "star", "triangle", "hex", "slot",
        ] {
            let p = Params {
                pattern: "rings".into(),
                shape: shape.into(),
                d_max: 50.0,
                ..Params::default()
            };
            let r = gen::generate(Some(&g), &p);
            assert!(r.stats.count > 10, "{shape}: {}", r.stats.count);
            for fmt in ["svg", "dxf", "gcode", "step"] {
                assert!(super::export_format(&r, fmt).is_some(), "{shape} {fmt}");
            }
        }
    }

    #[test]
    fn density_mode_packs_dark_areas() {
        let g = gradient(200, 120);
        let p = Params {
            size_mode: "dense".into(),
            tone_cut: 0.0,
            d_min: 3.0,
            d_max: 50.0, // в плотностном режиме не должен влиять
            ..Params::default()
        };
        let r = gen::generate(Some(&g), &p);
        assert!(r.stats.count > 100, "{}", r.stats.count);
        // все отверстия одного размера: постоянный диаметр без kerf
        let first = r.shapes[0].size();
        assert!(r.shapes.iter().all(|s| (s.size() - first).abs() < 1e-9));
        // тёмный левый край плотнее светлого правого: шаг растёт от перемычки до полного
        let dark = r.shapes.iter().filter(|s| s.start().0 < 250.0).count();
        let light = r.shapes.iter().filter(|s| s.start().0 > 750.0).count();
        assert!(dark > light * 2, "dark {dark} vs light {light}");
        // перемычка и кромка
        for s in &r.shapes {
            assert!((s.size() - first).abs() < 1e-9);
        }
        let c: Vec<(f64, f64, f64)> = r
            .shapes
            .iter()
            .filter_map(|s| {
                if let Shape::Circle { x, y, d } = s {
                    Some((*x, *y, *d))
                } else {
                    None
                }
            })
            .collect();
        for (i, a) in c.iter().enumerate().step_by(7) {
            for b2 in c.iter().skip(i + 1) {
                let dist = ((a.0 - b2.0).powi(2) + (a.1 - b2.1).powi(2)).sqrt();
                if dist < 3.0 * p.pitch {
                    assert!(
                        dist - a.2 / 2.0 - b2.2 / 2.0 >= p.bridge - 1e-6,
                        "bridge {dist:.3} {} {}",
                        a.2,
                        b2.2
                    );
                }
            }
        }
        // заданные размеры в плотностном режиме не применяются
        let pf = Params {
            fixed: true,
            fixed_sizes: "7.0".into(),
            ..p.clone()
        };
        let rf = gen::generate(Some(&g), &pf);
        assert!(rf.shapes.iter().all(|s| (s.size() - first).abs() < 1e-9));
        // экспорт работает
        for fmt in ["svg", "dxf", "gcode"] {
            assert!(super::export_format(&r, fmt).is_some(), "{fmt}");
        }
    }

    #[test]
    fn stipple_pattern_works() {
        let g = gradient(200, 120);
        let p = Params {
            pattern: "stipple".into(),
            tone_cut: 0.0,
            d_min: 3.0,
            ..Params::default()
        };
        let r = gen::generate(Some(&g), &p);
        assert!(r.stats.count > 500, "{}", r.stats.count);
        // все отверстия одного размера: постоянный диаметр без kerf
        let first = r.shapes[0].size();
        assert!(r.shapes.iter().all(|s| (s.size() - first).abs() < 1e-9));
        // детерминизм: то же зерно — тот же узор
        let r2 = gen::generate(Some(&g), &p);
        assert_eq!(r2.stats.count, r.stats.count);
        assert_eq!(r2.shapes[0].start(), r.shapes[0].start());
        // тёмный левый край плотнее светлого правого
        let dark = r.shapes.iter().filter(|s| s.start().0 < 250.0).count();
        let light = r.shapes.iter().filter(|s| s.start().0 > 750.0).count();
        assert!(dark > light * 2, "dark {dark} vs light {light}");
        // перемычка: случайное размещение не сближает отверстия ближе размера + перемычки
        let c: Vec<(f64, f64, f64)> = r
            .shapes
            .iter()
            .filter_map(|s| {
                if let Shape::Circle { x, y, d } = s {
                    Some((*x, *y, *d))
                } else {
                    None
                }
            })
            .collect();
        for (i, a) in c.iter().enumerate().step_by(7) {
            for b2 in c.iter().skip(i + 1) {
                let dist = ((a.0 - b2.0).powi(2) + (a.1 - b2.1).powi(2)).sqrt();
                if dist < 3.0 * p.pitch {
                    assert!(
                        dist - a.2 / 2.0 - b2.2 / 2.0 >= p.bridge - 1e-6,
                        "bridge {dist:.3} {} {}",
                        a.2,
                        b2.2
                    );
                }
            }
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
        for fmt in ["svg", "dxf", "gcode"] {
            assert!(super::export_format(&r, fmt).is_some(), "{fmt}");
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
        for f in [
            "dxf", "svg", "nc", "plt", "eps", "pdf", "csv", "report", "stats", "settings", "stl",
            "step",
        ] {
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
    fn kerf_shifts_toolpath_sizes() {
        let g = gradient(200, 120);
        let base = gen::generate(Some(&g), &Params::default());
        let k = gen::generate(
            Some(&g),
            &Params {
                kerf: 0.6,
                ..Params::default()
            },
        );
        assert!(!k.shapes.is_empty());
        assert!((k.stats.size_min - (base.stats.size_min - 0.6)).abs() < 1e-9);
        assert!((k.stats.size_max - (base.stats.size_max - 0.6)).abs() < 1e-9);
        // Отрицательный kerf расширяет
        let neg = gen::generate(
            Some(&g),
            &Params {
                kerf: -0.4,
                ..Params::default()
            },
        );
        assert!((neg.stats.size_max - (base.stats.size_max + 0.4)).abs() < 1e-9);
        // Линии: ширина прорези тоже с учётом kerf
        let lb = gen::generate(
            Some(&g),
            &Params {
                pattern: "lines".into(),
                ..Params::default()
            },
        );
        let lk = gen::generate(
            Some(&g),
            &Params {
                pattern: "lines".into(),
                kerf: 0.5,
                ..Params::default()
            },
        );
        assert!((lk.stats.size_min - (lb.stats.size_min - 0.5)).abs() < 1e-9);
        // Слишком большой kerf убирает фигуры совсем
        let zero = gen::generate(
            Some(&g),
            &Params {
                kerf: 50.0,
                ..Params::default()
            },
        );
        assert!(zero.shapes.is_empty());
    }

    #[test]
    fn new_shapes_build_and_export() {
        let g = gradient(200, 120);
        for (shape, pts_expected) in [
            ("oval", 12usize),
            ("star", 10),
            ("triangle", 3),
            ("hex", 6),
            ("slot", 14),
        ] {
            let r = gen::generate(
                Some(&g),
                &Params {
                    shape: shape.into(),
                    d_max: 8.0,
                    ..Params::default()
                },
            );
            assert!(r.stats.count > 10, "{shape}: {}", r.stats.count);
            for s in &r.shapes {
                match s {
                    gen::Shape::Poly { pts, .. } => {
                        assert!(pts.len() >= pts_expected, "{shape}: {}", pts.len());
                        // У овала и прорези число вершин зависит от диаметра — проверяем минимум.
                        if matches!(shape, "triangle" | "hex" | "star") {
                            assert_eq!(pts.len(), pts_expected, "{shape}");
                        }
                    }
                    _ => panic!("{shape}: ожидался полигон"),
                }
            }
            // Все экспорты работают с новыми фигурами.
            for f in ["dxf", "svg", "nc", "plt", "eps", "pdf", "stl", "step"] {
                let b = export_format(&r, f).unwrap();
                assert!(b.len() > 50, "{shape}/{f}");
            }
            // Имя фигуры попадает в отчёт.
            let rep = String::from_utf8(export_format(&r, "report").unwrap()).unwrap();
            let name = match shape {
                "oval" => "овал",
                "star" => "звезда",
                "triangle" => "треугольник",
                "hex" => "шестиугольник",
                _ => "прорезь",
            };
            assert!(rep.contains(name), "{shape}: отчёт без имени фигуры");
        }
    }

    #[test]
    fn star_and_slot_params_work() {
        let g = gradient(200, 120);
        // 6 лучей — 12 вершин вместо 10 по умолчанию.
        let s6 = gen::generate(
            Some(&g),
            &Params {
                shape: "star".into(),
                star_points: 6,
                d_max: 8.0,
                ..Params::default()
            },
        );
        assert!(!s6.shapes.is_empty());
        for s in &s6.shapes {
            match s {
                gen::Shape::Poly { pts, .. } => assert_eq!(pts.len(), 12),
                _ => panic!("star: ожидался полигон"),
            }
        }
        // Пропорция 2 у прорези: длина вдвое больше ширины (габаритные отношения).
        let sl = gen::generate(
            Some(&g),
            &Params {
                shape: "slot".into(),
                shape_aspect: 2.0,
                d_max: 8.0,
                ..Params::default()
            },
        );
        assert!(!sl.shapes.is_empty());
        for s in &sl.shapes {
            match s {
                gen::Shape::Poly { pts, .. } => {
                    let (mut x0, mut x1) = (f64::INFINITY, f64::NEG_INFINITY);
                    let (mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY);
                    for &(px, py) in pts {
                        x0 = x0.min(px);
                        x1 = x1.max(px);
                        y0 = y0.min(py);
                        y1 = y1.max(py);
                    }
                    let ratio = (x1 - x0) / (y1 - y0);
                    assert!((ratio - 2.0).abs() < 0.05, "ratio={ratio}");
                }
                _ => panic!("slot: ожидался полигон"),
            }
        }
    }

    #[test]
    fn jitter_is_deterministic() {
        let g = gradient(200, 120);
        let sizes = |r: &gen::GenResult| -> Vec<u64> {
            r.shapes.iter().map(|s| s.size().to_bits()).collect()
        };
        let base = gen::generate(Some(&g), &Params::default());
        let p = Params {
            jitter: 15.0,
            seed: 7,
            ..Params::default()
        };
        let j1 = gen::generate(Some(&g), &p);
        let j2 = gen::generate(Some(&g), &p);
        assert!(!j1.shapes.is_empty());
        // Одно зерно — один результат.
        assert_eq!(sizes(&j1), sizes(&j2), "разброс недетерминирован");
        // Разброс действительно меняет размеры...
        assert_ne!(sizes(&j1), sizes(&base), "разброс не изменил размеры");
        // ...а другое зерно даёт другую картинку.
        let j3 = gen::generate(
            Some(&g),
            &Params {
                jitter: 15.0,
                seed: 8,
                ..Params::default()
            },
        );
        assert_ne!(sizes(&j1), sizes(&j3), "зерно не влияет на разброс");
        // Перемычка держится: соседние круги не сливаются.
        let c: Vec<(f64, f64, f64)> = j1
            .shapes
            .iter()
            .filter_map(|s| match s {
                gen::Shape::Circle { x, y, d } => Some((*x, *y, *d)),
                _ => None,
            })
            .collect();
        let p = Params {
            jitter: 15.0,
            ..Params::default()
        };
        for (i, a) in c.iter().enumerate() {
            for b in c.iter().skip(i + 1) {
                let dist = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                if dist < 3.0 * p.pitch {
                    assert!(dist - a.2 / 2.0 - b.2 / 2.0 >= p.bridge - 1e-6);
                }
            }
        }
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
        let w = r
            .stats
            .warnings
            .iter()
            .find(|w| w.starts_with("Макс. ширина"))
            .expect("warning");
        assert!(
            w.contains("до 7,55 мм") && w.contains("не меньше 1,5 мм"),
            "{w}"
        );
        let r = gen::generate(
            Some(&g),
            &Params {
                lang: "en".into(),
                ..p
            },
        );
        assert!(
            r.stats.warnings.iter().any(|w| w.contains("to 7.55 mm")),
            "{:?}",
            r.stats.warnings
        );
    }
}
