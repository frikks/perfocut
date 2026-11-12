//! Экспорт листа с перфорацией в STEP (ISO 10303-21, AP214).
//!
//! Модель — настоящее B-rep тело (`MANIFOLD_SOLID_BREP`): плоскости для листа и
//! цилиндрические поверхности для круглых отверстий. Поэтому файл компактный
//! (десятки сущностей на отверстие, а не сотни треугольников) и открывается
//! в CAD, а не только в просмотрщиках.

use crate::gen::{GenResult, Shape};
use std::collections::HashMap;
use std::fmt::Write;

type P3 = (f64, f64, f64);

/// Шаг округления координат: 1e-4 мм.
const Q: f64 = 1e4;

fn qi(v: f64) -> i64 {
    (v * Q).round() as i64
}

fn qf(v: i64) -> f64 {
    v as f64 / Q
}

fn sql(b: bool) -> &'static str {
    if b {
        ".T"
    } else {
        ".F"
    }
}

/// Сборщик STEP-файла: выдаёт сущности с последовательными номерами и
/// переиспользует точки, направления и вершины.
struct B {
    o: String,
    id: u32,
    verts: HashMap<(i64, i64, i64), u32>,
    dirs: HashMap<(i64, i64, i64), u32>,
}

impl B {
    fn new() -> Self {
        Self {
            o: String::with_capacity(1 << 16),
            id: 0,
            verts: HashMap::new(),
            dirs: HashMap::new(),
        }
    }

    fn next(&mut self) -> u32 {
        self.id += 1;
        self.id
    }

    fn put(&mut self, s: &str) {
        self.o.push_str(s);
    }

    fn point(&mut self, p: P3) -> u32 {
        let i = self.next();
        let _ = write!(
            self.o,
            "#{i}=CARTESIAN_POINT('',({:.4},{:.4},{:.4}));\r\n",
            qf(qi(p.0)),
            qf(qi(p.1)),
            qf(qi(p.2))
        );
        i
    }

    fn vertex(&mut self, p: P3) -> u32 {
        let k = (qi(p.0), qi(p.1), qi(p.2));
        if let Some(v) = self.verts.get(&k) {
            return *v;
        }
        let cp = self.point(p);
        let i = self.next();
        let _ = write!(self.o, "#{i}=VERTEX_POINT('',#{cp});\r\n");
        self.verts.insert(k, i);
        i
    }

    fn dir(&mut self, d: P3) -> u32 {
        let l = (d.0 * d.0 + d.1 * d.1 + d.2 * d.2).sqrt();
        let d = if l > 1e-12 {
            (d.0 / l, d.1 / l, d.2 / l)
        } else {
            (1.0, 0.0, 0.0)
        };
        let k = (qi(d.0), qi(d.1), qi(d.2));
        if let Some(i) = self.dirs.get(&k) {
            return *i;
        }
        let i = self.next();
        let _ = write!(
            self.o,
            "#{i}=DIRECTION('',({:.6},{:.6},{:.6}));\r\n",
            qf(k.0),
            qf(k.1),
            qf(k.2)
        );
        self.dirs.insert(k, i);
        i
    }

    fn axis(&mut self, p: P3, z: P3, x: P3) -> u32 {
        let cp = self.point(p);
        let dz = self.dir(z);
        let dx = self.dir(x);
        let i = self.next();
        let _ = write!(self.o, "#{i}=AXIS2_PLACEMENT_3D('',#{cp},#{dz},#{dx});\r\n");
        i
    }

    fn line(&mut self, p: P3, d: P3, len: f64) -> u32 {
        let cp = self.point(p);
        let dv = self.dir(d);
        let v = self.next();
        let _ = write!(self.o, "#{v}=VECTOR('',#{dv},{:.4});\r\n", len);
        let i = self.next();
        let _ = write!(self.o, "#{i}=LINE('',#{cp},#{v});\r\n");
        i
    }

    /// Окружность с осью +Z и началом в точке (cx+r, cy, z), обход против часовой.
    fn circle(&mut self, c: P3, r: f64) -> u32 {
        let ax = self.axis(c, (0.0, 0.0, 1.0), (1.0, 0.0, 0.0));
        let i = self.next();
        let _ = write!(self.o, "#{i}=CIRCLE('',#{ax},{:.4});\r\n", r);
        i
    }

    fn plane(&mut self, p: P3, n: P3) -> u32 {
        // Опорное направление должно быть перпендикулярно нормали.
        let x = if n.2.abs() > 0.9 {
            (1.0, 0.0, 0.0)
        } else {
            (0.0, 0.0, 1.0)
        };
        let ax = self.axis(p, n, x);
        let i = self.next();
        let _ = write!(self.o, "#{i}=PLANE('',#{ax});\r\n");
        i
    }

    fn cylinder(&mut self, c: P3, r: f64) -> u32 {
        let ax = self.axis(c, (0.0, 0.0, 1.0), (1.0, 0.0, 0.0));
        let i = self.next();
        let _ = write!(self.o, "#{i}=CYLINDRICAL_SURFACE('',#{ax},{:.4});\r\n", r);
        i
    }

    fn edge(&mut self, a: u32, b: u32, curve: u32, same: bool) -> u32 {
        let i = self.next();
        let _ = write!(
            self.o,
            "#{i}=EDGE_CURVE('',#{a},#{b},#{curve},{}.);\r\n",
            sql(same)
        );
        i
    }

    fn oriented(&mut self, e: u32, same: bool) -> u32 {
        let i = self.next();
        let _ = write!(self.o, "#{i}=ORIENTED_EDGE('',*,*,#{e},{}.);\r\n", sql(same));
        i
    }

    fn loop_of(&mut self, es: &[(u32, bool)]) -> u32 {
        let mut list = String::with_capacity(es.len() * 8);
        for (k, (e, same)) in es.iter().enumerate() {
            if k > 0 {
                list.push(',');
            }
            let oe = self.oriented(*e, *same);
            let _ = write!(list, "#{oe}");
        }
        let i = self.next();
        let _ = write!(self.o, "#{i}=EDGE_LOOP('',({list}));\r\n");
        i
    }

    fn bound(&mut self, lp: u32, outer: bool) -> u32 {
        let i = self.next();
        let kind = if outer { "FACE_OUTER_BOUND" } else { "FACE_BOUND" };
        let _ = write!(self.o, "#{i}={kind}('',#{lp},.T.);\r\n");
        i
    }

    fn face(&mut self, bounds: &[u32], surf: u32, same: bool) -> u32 {
        let mut list = String::with_capacity(bounds.len() * 8);
        for (k, b) in bounds.iter().enumerate() {
            if k > 0 {
                list.push(',');
            }
            let _ = write!(list, "#{b}");
        }
        let i = self.next();
        let _ = write!(
            self.o,
            "#{i}=ADVANCED_FACE('',({list}),#{surf},{}.);\r\n",
            sql(same)
        );
        i
    }
}

/// Заголовок: контекст приложения, изделие, единицы измерения (мм, радианы).
/// Возвращает `PRODUCT_DEFINITION_SHAPE` и контекст представления.
fn header(b: &mut B) -> (u32, u32) {
    b.put("ISO-10303-21;\r\nHEADER;\r\n");
    b.put("FILE_DESCRIPTION(('PerfoCut perforated sheet'),'2;1');\r\n");
    b.put("FILE_NAME('sheet.step','2026-11-09T00:00:00',('PerfoCut'),('PerfoCut'),'PerfoCut','PerfoCut','');\r\n");
    b.put("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\r\n");
    b.put("ENDSEC;\r\nDATA;\r\n");

    let ctx = b.next();
    let _ = write!(b.o, "#{ctx}=APPLICATION_CONTEXT('automotive design');\r\n");
    let proto = b.next();
    let _ = write!(
        b.o,
        "#{proto}=APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{ctx});\r\n"
    );
    let pctx = b.next();
    let _ = write!(b.o, "#{pctx}=PRODUCT_CONTEXT('',#{ctx},'mechanical');\r\n");
    let prod = b.next();
    let _ = write!(b.o, "#{prod}=PRODUCT('PerfoCut','PerfoCut','',(#{pctx}));\r\n");
    let pdf = b.next();
    let _ = write!(b.o, "#{pdf}=PRODUCT_DEFINITION_FORMATION('','',#{prod});\r\n");
    let pdctx = b.next();
    let _ = write!(
        b.o,
        "#{pdctx}=PRODUCT_DEFINITION_CONTEXT('part definition',#{ctx},'design');\r\n"
    );
    let pd = b.next();
    let _ = write!(b.o, "#{pd}=PRODUCT_DEFINITION('design','',#{pdf},#{pdctx});\r\n");
    let pds = b.next();
    let _ = write!(b.o, "#{pds}=PRODUCT_DEFINITION_SHAPE('','',#{pd});\r\n");

    let mm = b.next();
    let _ = write!(b.o, "#{mm}=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\r\n");
    let rad = b.next();
    let _ = write!(b.o, "#{rad}=(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.));\r\n");
    let sr = b.next();
    let _ = write!(
        b.o,
        "#{sr}=(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT());\r\n"
    );
    let unc = b.next();
    let _ = write!(
        b.o,
        "#{unc}=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-06),#{mm},'distance_accuracy_value','confusion accuracy');\r\n"
    );
    let gctx = b.next();
    let _ = write!(
        b.o,
        "#{gctx}=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{unc}))GLOBAL_UNIT_ASSIGNED_CONTEXT((#{mm},#{rad},#{sr}))REPRESENTATION_CONTEXT('',''));\r\n"
    );
    (pds, gctx)
}

/// Убирает соседние совпадающие точки (с учётом округления координат).
fn clean_poly(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
    for &p in pts {
        let q = (qf(qi(p.0)), qf(qi(p.1)));
        let same = out
            .last()
            .map_or(false, |l| (l.0 - q.0).abs() < 1e-9 && (l.1 - q.1).abs() < 1e-9);
        if !same {
            out.push(q);
        }
    }
    if out.len() >= 2 {
        let (f, l) = (out[0], out[out.len() - 1]);
        if (f.0 - l.0).abs() < 1e-9 && (f.1 - l.1).abs() < 1e-9 {
            out.pop();
        }
    }
    out
}

/// Ключ фигуры для отбрасывания полных дублей: иначе в теле появятся
/// две совпадающие стенки, и оболочка станет невалидной.
fn shape_key(s: &Shape) -> String {
    let mut k = String::new();
    match s {
        Shape::Circle { x, y, d } => {
            let _ = write!(k, "c{}:{}:{}", qi(*x), qi(*y), qi(*d));
        }
        Shape::Poly { pts, .. } => {
            k.push('p');
            for p in clean_poly(pts) {
                let _ = write!(k, "{}:{};", qi(p.0), qi(p.1));
            }
        }
    }
    k
}

fn signed_area(pts: &[(f64, f64)]) -> f64 {
    let mut a = 0.0;
    for i in 0..pts.len() {
        let p = pts[i];
        let q = pts[(i + 1) % pts.len()];
        a += p.0 * q.1 - q.0 * p.1;
    }
    a / 2.0
}

/// STEP-файл с телом листа.
pub fn step(r: &GenResult) -> String {
    let p = &r.params;
    let (w, h, th) = (p.sheet_w, p.sheet_h, p.thickness.max(0.1));
    let mut b = B::new();
    let (pds, gctx) = header(&mut b);

    // --- Вершины листа
    let vb: [u32; 4] = [
        b.vertex((0.0, 0.0, 0.0)),
        b.vertex((w, 0.0, 0.0)),
        b.vertex((w, h, 0.0)),
        b.vertex((0.0, h, 0.0)),
    ];
    let vt: [u32; 4] = [
        b.vertex((0.0, 0.0, th)),
        b.vertex((w, 0.0, th)),
        b.vertex((w, h, th)),
        b.vertex((0.0, h, th)),
    ];

    // --- Рёбра низа и верха
    let mut eb = [0u32; 4];
    let mut et = [0u32; 4];
    let seg_b = [
        ((0.0, 0.0, 0.0), (1.0, 0.0, 0.0), w),
        ((w, 0.0, 0.0), (0.0, 1.0, 0.0), h),
        ((w, h, 0.0), (-1.0, 0.0, 0.0), w),
        ((0.0, h, 0.0), (0.0, -1.0, 0.0), h),
    ];
    let seg_t = [
        ((0.0, 0.0, th), (1.0, 0.0, 0.0), w),
        ((w, 0.0, th), (0.0, 1.0, 0.0), h),
        ((w, h, th), (-1.0, 0.0, 0.0), w),
        ((0.0, h, th), (0.0, -1.0, 0.0), h),
    ];
    for i in 0..4 {
        let (a, b2) = (i, (i + 1) % 4);
        let l = b.line(seg_b[i].0, seg_b[i].1, seg_b[i].2);
        eb[i] = b.edge(vb[a], vb[b2], l, true);
        let l = b.line(seg_t[i].0, seg_t[i].1, seg_t[i].2);
        et[i] = b.edge(vt[a], vt[b2], l, true);
    }
    let mut vv = [0u32; 4];
    for i in 0..4 {
        let l = b.line(
            match i {
                0 => (0.0, 0.0, 0.0),
                1 => (w, 0.0, 0.0),
                2 => (w, h, 0.0),
                _ => (0.0, h, 0.0),
            },
            (0.0, 0.0, 1.0),
            th,
        );
        vv[i] = b.edge(vb[i], vt[i], l, true);
    }

    // --- Отверстия: собираем петли для крышек и грани стенок
    let mut top_inner: Vec<u32> = Vec::new();
    let mut bot_inner: Vec<u32> = Vec::new();
    let mut walls: Vec<u32> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for s in &r.shapes {
        if !seen.insert(shape_key(s)) {
            continue;
        }
        match s {
            Shape::Circle { x, y, d } => {
                let rad = d / 2.0;
                if rad <= 1e-6 {
                    continue;
                }
                // Полная окружность — одно ребро с совпадающими вершинами
                let v_top = b.vertex((x + rad, *y, th));
                let v_bot = b.vertex((x + rad, *y, 0.0));
                let c_top = b.circle((*x, *y, th), rad);
                let c_bot = b.circle((*x, *y, 0.0), rad);
                let e_top = b.edge(v_top, v_top, c_top, true);
                let e_bot = b.edge(v_bot, v_bot, c_bot, true);

                // Верхняя крышка: внутренняя петля по часовой (обход .F.)
                let lt = b.loop_of(&[(e_top, false)]);
                top_inner.push(lt);
                // Нижняя крышка: обход против часовой (.T.)
                let lb = b.loop_of(&[(e_bot, true)]);
                bot_inner.push(lb);

                // Стенка: цилиндр, нормаль внутрь отверстия → same_sense = .F.
                let cyl = b.cylinder((*x, *y, 0.0), rad);
                let wb = b.loop_of(&[(e_bot, false)]);
                let wt = b.loop_of(&[(e_top, true)]);
                let bd_b = b.bound(wb, true);
                let bd_t = b.bound(wt, false);
                walls.push(b.face(&[bd_b, bd_t], cyl, false));
            }
            Shape::Poly { pts, .. } => {
                let mut poly = clean_poly(pts);
                if poly.len() < 3 {
                    continue;
                }
                if signed_area(&poly) < 0.0 {
                    poly.reverse();
                }
                let n = poly.len();
                let va: Vec<u32> = poly.iter().map(|q| b.vertex((q.0, q.1, 0.0))).collect();
                let vb2: Vec<u32> = poly.iter().map(|q| b.vertex((q.0, q.1, th))).collect();
                let mut te = vec![0u32; n];
                let mut be = vec![0u32; n];
                let mut ve = vec![0u32; n];
                for i in 0..n {
                    let j = (i + 1) % n;
                    let (x0, y0) = poly[i];
                    let (x1, y1) = poly[j];
                    let (dx, dy) = (x1 - x0, y1 - y0);
                    let len = (dx * dx + dy * dy).sqrt();
                    if len <= 1e-9 {
                        continue;
                    }
                    let l = b.line((x0, y0, 0.0), (dx, dy, 0.0), len);
                    be[i] = b.edge(va[i], va[j], l, true);
                    let l = b.line((x0, y0, th), (dx, dy, 0.0), len);
                    te[i] = b.edge(vb2[i], vb2[j], l, true);
                    let l = b.line((x0, y0, 0.0), (0.0, 0.0, 1.0), th);
                    ve[i] = b.edge(va[i], vb2[i], l, true);
                }

                // Внутренние петли крышек
                let top: Vec<(u32, bool)> = (0..n).rev().map(|i| (te[i], false)).collect();
                top_inner.push(b.loop_of(&top));
                let bot: Vec<(u32, bool)> = (0..n).map(|i| (be[i], true)).collect();
                bot_inner.push(b.loop_of(&bot));

                // Стенки: плоские грани, нормаль внутрь отверстия
                for i in 0..n {
                    let j = (i + 1) % n;
                    let (x0, y0) = poly[i];
                    let (x1, y1) = poly[j];
                    let (dx, dy) = (x1 - x0, y1 - y0);
                    let len = (dx * dx + dy * dy).sqrt();
                    if len <= 1e-9 {
                        continue;
                    }
                    let nrm = (-dy / len, dx / len, 0.0);
                    let pl = b.plane((x0, y0, 0.0), nrm);
                    let lp = b.loop_of(&[(be[i], false), (ve[i], true), (te[i], true), (ve[j], false)]);
                    let bd = b.bound(lp, true);
                    walls.push(b.face(&[bd], pl, true));
                }
            }
        }
    }

    // --- Крышки
    let plane_top = b.plane((0.0, 0.0, th), (0.0, 0.0, 1.0));
    let plane_bot = b.plane((0.0, 0.0, 0.0), (0.0, 0.0, 1.0));

    let outer_top = b.loop_of(&[(et[0], true), (et[1], true), (et[2], true), (et[3], true)]);
    let outer_bot = b.loop_of(&[(eb[0], false), (eb[3], false), (eb[2], false), (eb[1], false)]);

    let mut top_bounds = Vec::with_capacity(1 + top_inner.len());
    top_bounds.push(b.bound(outer_top, true));
    for lp in top_inner {
        top_bounds.push(b.bound(lp, false));
    }
    let face_top = b.face(&top_bounds, plane_top, true);

    let mut bot_bounds = Vec::with_capacity(1 + bot_inner.len());
    bot_bounds.push(b.bound(outer_bot, true));
    for lp in bot_inner {
        bot_bounds.push(b.bound(lp, false));
    }
    let face_bot = b.face(&bot_bounds, plane_bot, false);

    // --- Боковые грани листа (нормали наружу)
    let mut faces = vec![face_top, face_bot];
    let sides = [
        ((0.0, 0.0, 0.0), (0.0, -1.0, 0.0), [(eb[0], true), (vv[1], true), (et[0], false), (vv[0], false)]),
        ((w, 0.0, 0.0), (1.0, 0.0, 0.0), [(eb[1], true), (vv[2], true), (et[1], false), (vv[1], false)]),
        ((0.0, h, 0.0), (0.0, 1.0, 0.0), [(eb[2], true), (vv[3], true), (et[2], false), (vv[2], false)]),
        ((0.0, 0.0, 0.0), (-1.0, 0.0, 0.0), [(eb[3], true), (vv[0], true), (et[3], false), (vv[3], false)]),
    ];
    for (p0, nrm, edges) in sides {
        let pl = b.plane(p0, nrm);
        let lp = b.loop_of(&edges);
        let bd = b.bound(lp, true);
        faces.push(b.face(&[bd], pl, true));
    }
    faces.extend(walls);

    // --- Оболочка и тело
    let mut list = String::with_capacity(faces.len() * 8);
    for (i, f) in faces.iter().enumerate() {
        if i > 0 {
            list.push(',');
        }
        let _ = write!(list, "#{f}");
    }
    let shell = b.next();
    let _ = write!(b.o, "#{shell}=CLOSED_SHELL('',({list}));\r\n");
    let brep = b.next();
    let _ = write!(b.o, "#{brep}=MANIFOLD_SOLID_BREP('PerfoCutSheet',#{shell});\r\n");

    // --- Представление формы
    let org = b.point((0.0, 0.0, 0.0));
    let dz = b.dir((0.0, 0.0, 1.0));
    let dx = b.dir((1.0, 0.0, 0.0));
    let axp = b.next();
    let _ = write!(b.o, "#{axp}=AXIS2_PLACEMENT_3D('',#{org},#{dz},#{dx});\r\n");
    let shp = b.next();
    let _ = write!(
        b.o,
        "#{shp}=ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{axp},#{brep}),#{gctx});\r\n"
    );
    let sdr = b.next();
    let _ = write!(
        b.o,
        "#{sdr}=SHAPE_DEFINITION_REPRESENTATION(#{pds},#{shp});\r\n"
    );

    b.put("ENDSEC;\r\nEND-ISO-10303-21;\r\n");
    b.o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::Stats;
    use crate::params::Params;
    use std::collections::HashMap;

    fn sample() -> GenResult {
        let p = Params {
            sheet_w: 100.0,
            sheet_h: 60.0,
            thickness: 2.0,
            ..Params::default()
        };
        GenResult {
            shapes: vec![
                Shape::Circle { x: 20.0, y: 20.0, d: 10.0 },
                Shape::Circle { x: 50.0, y: 30.0, d: 6.0 },
                Shape::Poly {
                    pts: vec![(70.0, 10.0), (85.0, 12.0), (80.0, 30.0), (68.0, 26.0)],
                    size: 0.0,
                },
            ],
            stats: Stats::default(),
            params: p,
        }
    }

    /// Разбор простых сущностей: id -> (тип, аргументы внутри скобок).
    fn entities(s: &str) -> HashMap<u32, (String, String)> {
        let mut m = HashMap::new();
        for line in s.split("\r\n") {
            if !line.starts_with('#') {
                continue;
            }
            let Some(eq) = line.find('=') else { continue };
            let id: u32 = line[1..eq].parse().unwrap_or(0);
            let rest = &line[eq + 1..];
            let Some(op) = rest.find('(') else { continue };
            let kind = rest[..op].to_string();
            let body = rest[op + 1..].trim_end_matches([';', ')']).to_string();
            m.insert(id, (kind, body));
        }
        m
    }

    fn refs(body: &str) -> Vec<u32> {
        let mut out = Vec::new();
        let b = body.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'#' {
                let mut j = i + 1;
                let mut n = 0u32;
                while j < b.len() && b[j].is_ascii_digit() {
                    n = n * 10 + (b[j] - b'0') as u32;
                    j += 1;
                }
                if j > i + 1 {
                    out.push(n);
                    i = j;
                    continue;
                }
            }
            i += 1;
        }
        out
    }

    #[test]
    fn all_references_resolve() {
        let s = step(&sample());
        let m = entities(&s);
        for (id, (_, body)) in &m {
            for r in refs(body) {
                assert!(m.contains_key(&r), "сущность #{id} ссылается на несуществующую #{r}");
            }
        }
    }

    /// Дубли фигур и совпадающие точки полигона не ломают тело.
    #[test]
    fn duplicates_are_dropped() {
        let mut r = sample();
        let dup: Vec<Shape> = r.shapes.clone();
        r.shapes.extend(dup);
        r.shapes.push(Shape::Poly {
            pts: vec![
                (10.0, 40.0),
                (10.0, 40.0),
                (20.0, 40.0),
                (20.0, 50.0),
                (20.0, 50.0),
            ],
            size: 0.0,
        });
        let s = step(&r);
        let m = entities(&s);
        for (id, (_, body)) in &m {
            for x in refs(body) {
                assert!(m.contains_key(&x), "сущность #{id} ссылается на #{x}");
            }
        }
        let cyl = m.values().filter(|(k, _)| k == "CYLINDRICAL_SURFACE").count();
        assert_eq!(cyl, 2, "цилиндров: {cyl}");

        // То же самое без дублей и с чистыми точками даёт идентичный файл.
        let mut clean = sample();
        clean.shapes.push(Shape::Poly {
            pts: vec![(10.0, 40.0), (20.0, 40.0), (20.0, 50.0)],
            size: 0.0,
        });
        assert_eq!(step(&clean), s);
    }

    /// Каждое ребро используется ровно двумя ориентированными рёбрами,
    /// причём в противоположных направлениях.
    #[test]
    fn edges_used_twice_opposite() {
        let s = step(&sample());
        let m = entities(&s);
        let mut use_count: HashMap<u32, (i32, i32)> = HashMap::new();
        for (_, (kind, body)) in &m {
            if kind != "ORIENTED_EDGE" {
                continue;
            }
            let r = refs(body);
            let edge = *r.last().unwrap();
            let same = body.trim_end().ends_with(".T.") || body.contains(".T.");
            let e = use_count.entry(edge).or_insert((0, 0));
            if same {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
        }
        for (edge, (t, f)) in use_count {
            assert_eq!(t + f, 2, "ребро #{edge} использовано {} раз", t + f);
            assert_eq!(t, 1, "ребро #{edge}: ориентаций .T. = {t}");
            assert_eq!(f, 1, "ребро #{edge}: ориентаций .F. = {f}");
        }
    }

    /// Петли замкнуты: конец одного ориентированного ребра совпадает с началом следующего.
    #[test]
    fn loops_are_closed() {
        let s = step(&sample());
        let m = entities(&s);

        // Концы рёбер: EDGE_CURVE('',#a,#b,#curve,.T.)
        let mut ends: HashMap<u32, (u32, u32)> = HashMap::new();
        for (id, (kind, body)) in &m {
            if kind == "EDGE_CURVE" {
                let r = refs(body);
                ends.insert(*id, (r[0], r[1]));
            }
        }
        // ORIENTED_EDGE: (edge, same)
        let mut oe: HashMap<u32, (u32, bool)> = HashMap::new();
        for (id, (kind, body)) in &m {
            if kind == "ORIENTED_EDGE" {
                let r = refs(body);
                oe.insert(*id, (*r.last().unwrap(), body.contains(".T.")));
            }
        }

        for (id, (kind, body)) in &m {
            if kind != "EDGE_LOOP" {
                continue;
            }
            let seq: Vec<u32> = refs(body);
            assert!(!seq.is_empty(), "петля #{id} пустая");
            let mut start = None;
            let mut end = None;
            for o in seq {
                let (e, same) = oe[&o];
                let (a, b2) = ends[&e];
                let (s0, e0) = if same { (a, b2) } else { (b2, a) };
                if let Some(prev) = end {
                    assert_eq!(prev, s0, "петля #{id}: разрыв между рёбрами");
                } else {
                    start = Some(s0);
                }
                end = Some(e0);
            }
            assert_eq!(start, end, "петля #{id} не замкнута");
        }
    }
}
