use crate::gen::{GenResult, Shape};
use std::f64::consts::PI;
use std::fmt::Write;

const NL: &str = "\r\n";

pub fn num(v: f64, dec: usize) -> String {
    let mut s = format!("{:.*}", dec, v);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

fn n4(v: f64) -> String {
    num(v, 4)
}

/// Окружность в многоугольник с допуском по хорде.
pub fn circle_pts(x: f64, y: f64, d: f64, tol: f64) -> Vec<(f64, f64)> {
    let r = d / 2.0;
    let mut n = if r > tol { (PI / (1.0 - tol / r).acos()).ceil() as usize } else { 8 };
    n = n.clamp(12, 720);
    (0..n)
        .map(|i| {
            let a = 2.0 * PI * i as f64 / n as f64;
            (x + r * a.cos(), y + r * a.sin())
        })
        .collect()
}

fn layer_name(size: f64) -> String {
    format!("HOLES_D{}", num(size, 2).replace('.', "_"))
}

fn sheet_outline(w: f64, h: f64) -> Vec<(f64, f64)> {
    vec![(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]
}

// ---------------- DXF (R12, AC1009) ----------------
pub fn dxf(r: &GenResult) -> String {
    let p = &r.params;
    let (w, h) = (p.sheet_w, p.sheet_h);
    let mut layers: Vec<String> = vec!["CONTOUR".into()];
    let by_layers = p.dxf_layers && p.pattern != "lines" && r.stats.by_size.len() <= 64;
    let lay_of = |s: &Shape| -> String {
        if by_layers {
            layer_name(s.size())
        } else {
            "HOLES".into()
        }
    };
    for s in &r.shapes {
        let l = lay_of(s);
        if !layers.contains(&l) {
            layers.push(l);
        }
    }
    let mut o = String::with_capacity(r.shapes.len() * 90 + 2000);
    let mut g = |code: i32, val: &str| {
        o.push_str(&format!("{:>3}", code));
        o.push_str(NL);
        o.push_str(val);
        o.push_str(NL);
    };
    g(0, "SECTION");
    g(2, "HEADER");
    g(9, "$ACADVER");
    g(1, "AC1009");
    g(9, "$EXTMIN");
    g(10, "0.0");
    g(20, "0.0");
    g(9, "$EXTMAX");
    g(10, &n4(w));
    g(20, &n4(h));
    g(0, "ENDSEC");
    g(0, "SECTION");
    g(2, "TABLES");
    g(0, "TABLE");
    g(2, "LTYPE");
    g(70, "1");
    g(0, "LTYPE");
    g(2, "CONTINUOUS");
    g(70, "0");
    g(3, "Solid line");
    g(72, "65");
    g(73, "0");
    g(40, "0.0");
    g(0, "ENDTAB");
    g(0, "TABLE");
    g(2, "LAYER");
    g(70, &layers.len().to_string());
    for (i, l) in layers.iter().enumerate() {
        g(0, "LAYER");
        g(2, l);
        g(70, "0");
        let color = if i == 0 { 7 } else { ((i - 1) % 6) + 1 };
        g(62, &color.to_string());
        g(6, "CONTINUOUS");
    }
    g(0, "ENDTAB");
    g(0, "ENDSEC");
    g(0, "SECTION");
    g(2, "ENTITIES");
    let poly = |g: &mut dyn FnMut(i32, &str), layer: &str, pts: &[(f64, f64)]| {
        g(0, "POLYLINE");
        g(8, layer);
        g(66, "1");
        g(10, "0.0");
        g(20, "0.0");
        g(30, "0.0");
        g(70, "1");
        for q in pts {
            g(0, "VERTEX");
            g(8, layer);
            g(10, &n4(q.0));
            g(20, &n4(q.1));
            g(30, "0.0");
        }
        g(0, "SEQEND");
        g(8, layer);
    };
    for s in &r.shapes {
        let l = lay_of(s);
        match s {
            Shape::Circle { x, y, d } => {
                g(0, "CIRCLE");
                g(8, &l);
                g(10, &n4(*x));
                g(20, &n4(*y));
                g(30, "0.0");
                g(40, &n4(d / 2.0));
            }
            Shape::Poly { pts, .. } => poly(&mut g, &l, pts),
        }
    }
    if p.outline {
        poly(&mut g, "CONTOUR", &sheet_outline(w, h));
    }
    g(0, "ENDSEC");
    g(0, "EOF");
    o
}

// ---------------- SVG ----------------
pub fn svg(r: &GenResult) -> String {
    let p = &r.params;
    let (w, h) = (p.sheet_w, p.sheet_h);
    let mut o = String::with_capacity(r.shapes.len() * 60 + 1000);
    let _ = write!(
        o,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">\n<title>{}</title>\n<g fill=\"none\" stroke=\"#000\" stroke-width=\"0.1\">\n",
        xml_esc(&p.name),
        w = n4(w),
        h = n4(h)
    );
    if p.outline {
        let _ = writeln!(o, "<rect id=\"contour\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\"/>", n4(w), n4(h));
    }
    o.push_str("<g id=\"holes\">\n");
    for s in &r.shapes {
        match s {
            Shape::Circle { x, y, d } => {
                let _ = writeln!(o, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>", n4(*x), n4(h - y), n4(d / 2.0));
            }
            Shape::Poly { pts, .. } => {
                o.push_str("<path d=\"M");
                for (i, q) in pts.iter().enumerate() {
                    if i > 0 {
                        o.push('L');
                    }
                    let _ = write!(o, "{} {}", n4(q.0), n4(h - q.1));
                }
                o.push_str("Z\"/>\n");
            }
        }
    }
    o.push_str("</g>\n</g>\n</svg>\n");
    o
}

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ---------------- G-code ----------------
pub fn gcode(r: &GenResult) -> String {
    let p = &r.params;
    let (w, h) = (p.sheet_w, p.sheet_h);
    let dec = p.gc_decimals as usize;
    let (ox, oy) = match p.origin.as_str() {
        "center" => (w / 2.0, h / 2.0),
        "tl" => (0.0, h),
        _ => (0.0, 0.0),
    };
    let f = |v: f64| num(v, dec);
    let feed = num(p.cut_speed, 0);
    let mut o = String::with_capacity(r.shapes.len() * 120 + 1000);
    let mut ln = 0u32;
    let mut line = |o: &mut String, s: &str| {
        if s.is_empty() {
            return;
        }
        if p.gc_numbers && !s.starts_with('(') && !s.starts_with('%') {
            ln += 10;
            let _ = write!(o, "N{} ", ln);
        }
        o.push_str(s);
        o.push_str(NL);
    };
    line(&mut o, &format!("(PerfoCut: {} x {} mm, contours {})", f(w), f(h), r.shapes.len() + p.outline as usize));
    for l in p.gc_header.lines() {
        line(&mut o, l.trim());
    }
    let cut_poly = |o: &mut String, line: &mut dyn FnMut(&mut String, &str), pts: &[(f64, f64)]| {
        let s = pts[0];
        line(o, &format!("G00 X{} Y{}", f(s.0 - ox), f(s.1 - oy)));
        line(o, &p.gc_on);
        for (i, q) in pts.iter().enumerate().skip(1) {
            if i == 1 {
                line(o, &format!("G01 X{} Y{} F{}", f(q.0 - ox), f(q.1 - oy), feed));
            } else {
                line(o, &format!("G01 X{} Y{}", f(q.0 - ox), f(q.1 - oy)));
            }
        }
        line(o, &format!("G01 X{} Y{}", f(s.0 - ox), f(s.1 - oy)));
        line(o, &p.gc_off);
    };
    for s in &r.shapes {
        match s {
            Shape::Circle { x, y, d } => {
                if p.gc_arcs {
                    let rr = d / 2.0;
                    line(&mut o, &format!("G00 X{} Y{}", f(x + rr - ox), f(y - oy)));
                    line(&mut o, &p.gc_on);
                    line(&mut o, &format!("G02 X{} Y{} I{} J0 F{}", f(x - rr - ox), f(y - oy), f(-rr), feed));
                    line(&mut o, &format!("G02 X{} Y{} I{} J0", f(x + rr - ox), f(y - oy), f(rr)));
                    line(&mut o, &p.gc_off);
                } else {
                    let pts = circle_pts(*x, *y, *d, 0.02);
                    cut_poly(&mut o, &mut line, &pts);
                }
            }
            Shape::Poly { pts, .. } => cut_poly(&mut o, &mut line, pts),
        }
    }
    if p.outline {
        line(&mut o, "(contour)");
        cut_poly(&mut o, &mut line, &sheet_outline(w, h));
    }
    line(&mut o, &format!("G00 X{} Y{}", f(-ox), f(-oy)));
    for l in p.gc_footer.lines() {
        line(&mut o, l.trim());
    }
    o
}

// ---------------- PLT (HPGL) ----------------
pub fn plt(r: &GenResult) -> String {
    let p = &r.params;
    let k = 40.0; // 40 единиц на мм
    let c = |v: f64| format!("{}", (v * k).round() as i64);
    let mut o = String::with_capacity(r.shapes.len() * 200);
    o.push_str("IN;PA;SP1;");
    o.push_str(NL);
    let put = |pts: &[(f64, f64)], o: &mut String| {
        let _ = write!(o, "PU{},{};PD", c(pts[0].0), c(pts[0].1));
        for q in pts.iter().skip(1) {
            let _ = write!(o, "{},{},", c(q.0), c(q.1));
        }
        let _ = write!(o, "{},{};", c(pts[0].0), c(pts[0].1));
        o.push_str(NL);
    };
    for s in &r.shapes {
        match s {
            Shape::Circle { x, y, d } => put(&circle_pts(*x, *y, *d, 0.02), &mut o),
            Shape::Poly { pts, .. } => put(pts, &mut o),
        }
    }
    if p.outline {
        put(&sheet_outline(p.sheet_w, p.sheet_h), &mut o);
    }
    o.push_str("PU0,0;SP0;");
    o.push_str(NL);
    o
}

// ---------------- EPS ----------------
pub fn eps(r: &GenResult) -> String {
    let p = &r.params;
    let k = 72.0 / 25.4;
    let (w, h) = (p.sheet_w, p.sheet_h);
    let mut o = String::with_capacity(r.shapes.len() * 40 + 1000);
    let _ = write!(
        o,
        "%!PS-Adobe-3.0 EPSF-3.0\n%%Creator: PerfoCut\n%%BoundingBox: 0 0 {} {}\n%%HiResBoundingBox: 0 0 {} {}\n%%EndComments\n",
        (w * k).ceil() as i64,
        (h * k).ceil() as i64,
        n4(w * k),
        n4(h * k)
    );
    let _ = write!(o, "/c {{newpath 0 360 arc closepath stroke}} bind def\n{} {} scale\n0.1 setlinewidth\n", n4(k), n4(k));
    let poly = |o: &mut String, pts: &[(f64, f64)]| {
        let _ = write!(o, "newpath {} {} moveto", n4(pts[0].0), n4(pts[0].1));
        for q in pts.iter().skip(1) {
            let _ = write!(o, " {} {} lineto", n4(q.0), n4(q.1));
        }
        o.push_str(" closepath stroke\n");
    };
    for s in &r.shapes {
        match s {
            Shape::Circle { x, y, d } => {
                let _ = writeln!(o, "{} {} {} c", n4(*x), n4(*y), n4(d / 2.0));
            }
            Shape::Poly { pts, .. } => poly(&mut o, pts),
        }
    }
    if p.outline {
        poly(&mut o, &sheet_outline(w, h));
    }
    o.push_str("showpage\n%%EOF\n");
    o
}

// ---------------- PDF (вектор, одна страница) ----------------
pub fn pdf(r: &GenResult) -> Vec<u8> {
    let p = &r.params;
    let k = 72.0 / 25.4;
    let (w, h) = (p.sheet_w, p.sheet_h);
    let mut c = String::with_capacity(r.shapes.len() * 160 + 200);
    let _ = writeln!(c, "{} 0 0 {} 0 0 cm 0.1 w 0 0 0 RG", n4(k), n4(k));
    let kk = 0.552_284_75;
    let f = |v: f64| num(v, 3);
    for s in &r.shapes {
        match s {
            Shape::Circle { x, y, d } => {
                let rr = d / 2.0;
                let m = rr * kk;
                let _ = writeln!(
                    c,
                    "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c h S",
                    f(x + rr), f(*y),
                    f(x + rr), f(y + m), f(x + m), f(y + rr), f(*x), f(y + rr),
                    f(x - m), f(y + rr), f(x - rr), f(y + m), f(x - rr), f(*y),
                    f(x - rr), f(y - m), f(x - m), f(y - rr), f(*x), f(y - rr),
                    f(x + m), f(y - rr), f(x + rr), f(y - m), f(x + rr), f(*y)
                );
            }
            Shape::Poly { pts, .. } => {
                let _ = write!(c, "{} {} m", f(pts[0].0), f(pts[0].1));
                for q in pts.iter().skip(1) {
                    let _ = write!(c, " {} {} l", f(q.0), f(q.1));
                }
                c.push_str(" h S\n");
            }
        }
    }
    if p.outline {
        let _ = writeln!(c, "0 0 {} {} re S", f(w), f(h));
    }
    let mut out: Vec<u8> = Vec::with_capacity(c.len() + 1000);
    let mut offs = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R /Resources << >> >>",
            n4(w * k),
            n4(h * k)
        ),
    ];
    for (i, ob) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, ob).as_bytes());
    }
    offs.push(out.len());
    out.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", c.len()).as_bytes());
    out.extend_from_slice(c.as_bytes());
    out.extend_from_slice(b"\nendstream\nendobj\n");
    offs.push(out.len());
    // info
    out.extend_from_slice(b"5 0 obj\n<< /Producer (PerfoCut) /Creator (PerfoCut) >>\nendobj\n");
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offs.len() + 1).as_bytes());
    for o in &offs {
        out.extend_from_slice(format!("{:010} 00000 n \n", o).as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{}\n%%EOF\n", offs.len() + 1, xref).as_bytes(),
    );
    out
}

// ---------------- CSV и отчёт ----------------
pub fn csv(r: &GenResult) -> String {
    let st = &r.stats;
    let p = &r.params;
    let en = p.lang == "en";
    let t = |ru: &'static str, e: &'static str| if en { e } else { ru };
    let mut o = String::from("\u{FEFF}");
    let d = |v: f64, n: usize| {
        let s = format!("{:.*}", n, v);
        if en { s } else { s.replace('.', ",") }
    };
    let _ = write!(o, "{};{}{NL}", t("Параметр", "Parameter"), t("Значение", "Value"));
    let _ = write!(o, "{};{} x {}{NL}", t("Лист, мм", "Sheet, mm"), d(p.sheet_w, 1), d(p.sheet_h, 1));
    let _ = write!(o, "{};{}{NL}", t("Толщина, мм", "Thickness, mm"), d(p.thickness, 2));
    let _ = write!(o, "{};{}{NL}", t("Контуров", "Contours"), st.count);
    let _ = write!(o, "{};{}{NL}", t("Врезок", "Pierces"), st.pierces);
    let _ = write!(o, "{};{}{NL}", t("Длина реза, м", "Cut length, m"), d(st.cut_len_m, 2));
    let _ = write!(o, "{};{}{NL}", t("Холостой ход, м", "Rapid travel, m"), d(st.travel_m, 2));
    let _ = write!(o, "{};{}{NL}", t("Время, мин", "Time, min"), d(st.time_min, 1));
    let _ = write!(o, "{};{}{NL}", t("Перфорация, %", "Open area, %"), d(st.open_area_pct, 2));
    let _ = write!(o, "{};{}{NL}", t("Масса листа, кг", "Sheet weight, kg"), d(st.weight_kg, 2));
    let _ = write!(o, "{}, {};{}{NL}", t("Стоимость", "Cost"), p.currency, d(st.cost, 2));
    if !st.by_size.is_empty() {
        let _ = write!(o, "{NL}{};{}{NL}", t("Размер, мм", "Size, mm"), t("Количество", "Count"));
        for b in &st.by_size {
            let _ = write!(o, "{};{}{NL}", d(b.size, 2), b.count);
        }
    }
    o
}

pub fn report(r: &GenResult) -> String {
    let st = &r.stats;
    let p = &r.params;
    let en = p.lang == "en";
    let t = |ru: &'static str, e: &'static str| if en { e } else { ru };
    let (mm, m, min, kg, pcs) = if en { ("mm", "m", "min", "kg", "pcs") } else { ("мм", "м", "мин", "кг", "шт") };
    let mut o = String::new();
    let _ = write!(o, "{}{NL}{NL}", t("PerfoCut — паспорт заказа", "PerfoCut — order sheet"));
    let kv = |o: &mut String, k: &str, v: &str| {
        if !v.trim().is_empty() {
            let _ = write!(o, "{:<22}{}{NL}", k, v);
        }
    };
    kv(&mut o, t("Изделие:", "Product:"), &p.name);
    kv(&mut o, t("Номер заказа:", "Order no.:"), &p.order_no);
    kv(&mut o, t("Клиент:", "Client:"), &p.client);
    kv(&mut o, t("Телефон:", "Phone:"), &p.phone);
    kv(&mut o, "E-mail:", &p.email);
    kv(&mut o, t("Комментарий:", "Comment:"), &p.comment);
    let _ = write!(o, "{NL}");
    kv(
        &mut o,
        t("Лист:", "Sheet:"),
        &format!("{} x {} {mm}, {} {} {mm}", num(p.sheet_w, 1), num(p.sheet_h, 1), t("толщина", "thickness"), num(p.thickness, 2)),
    );
    let pat = if p.pattern == "lines" {
        format!("{} {} {mm}", t("линии, шаг", "lines, pitch"), num(p.line_pitch, 2))
    } else {
        let g = match p.grid.as_str() {
            "square" => t("квадратная", "square"),
            "tri90" => t("треугольная 2", "triangular 2"),
            _ => t("треугольная", "triangular"),
        };
        let s = match p.shape.as_str() {
            "square" => t("квадрат", "square"),
            "diamond" => t("ромб", "diamond"),
            _ => t("круг", "circle"),
        };
        format!("{}, {} {}, {} {} {mm}", s, t("сетка", "grid"), g, t("шаг", "pitch"), num(p.pitch, 2))
    };
    kv(&mut o, t("Узор:", "Pattern:"), &pat);
    kv(&mut o, t("Контуров:", "Contours:"), &st.count.to_string());
    kv(&mut o, t("Врезок:", "Pierces:"), &st.pierces.to_string());
    kv(&mut o, t("Длина реза:", "Cut length:"), &format!("{:.2} {m}", st.cut_len_m));
    kv(&mut o, t("Время резки:", "Cutting time:"), &format!("≈ {:.1} {min}", st.time_min));
    kv(&mut o, t("Перфорация:", "Open area:"), &format!("{:.2} %", st.open_area_pct));
    kv(&mut o, t("Масса листа:", "Sheet weight:"), &format!("{:.2} {kg}", st.weight_kg));
    let _ = write!(o, "{NL}{}{NL}", t("Стоимость", "Cost"));
    kv(&mut o, t("  резка:", "  cutting:"), &format!("{:.2} {}", st.cost_cut, p.currency));
    kv(&mut o, t("  врезки:", "  pierces:"), &format!("{:.2} {}", st.cost_pierce, p.currency));
    if st.cost_material > 0.0 {
        kv(&mut o, t("  металл:", "  metal:"), &format!("{:.2} {}", st.cost_material, p.currency));
    }
    if st.cost_fixed > 0.0 {
        kv(&mut o, t("  подготовка:", "  setup:"), &format!("{:.2} {}", st.cost_fixed, p.currency));
    }
    kv(&mut o, t("  итого:", "  total:"), &format!("{:.2} {}", st.cost, p.currency));
    if !st.by_size.is_empty() {
        let _ = write!(o, "{NL}{}{NL}", t("Отверстия по размерам", "Holes by size"));
        for b in &st.by_size {
            let _ = write!(o, "  {:>8} {mm}  {:>8} {pcs}{NL}", num(b.size, 2), b.count);
        }
    }
    if !st.warnings.is_empty() {
        let _ = write!(o, "{NL}{}{NL}", t("Замечания", "Notes"));
        for w in &st.warnings {
            let _ = write!(o, "  - {}{NL}", w);
        }
    }
    o
}

// ---------------- 3D геометрия листа (общая для STL и STEP) ----------------
type V3 = (f64, f64, f64);
type Tri = (V3, V3, V3);

fn poly_pts(s: &Shape) -> Vec<(f64, f64)> {
    match s {
        Shape::Circle { x, y, d } => circle_pts(*x, *y, *d, 0.05),
        Shape::Poly { pts, .. } => pts.clone(),
    }
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

/// Контур против часовой стрелки: от этого зависит направление нормалей.
fn to_ccw(mut pts: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    if signed_area(&pts) < 0.0 {
        pts.reverse();
    }
    pts
}

/// Триангуляция плоской грани с отверстиями (earcut).
fn cap_tris(outer: &[(f64, f64)], holes: &[Vec<(f64, f64)>]) -> Vec<[(f64, f64); 3]> {
    let mut data: Vec<f64> = Vec::with_capacity((outer.len() + holes.iter().map(Vec::len).sum::<usize>()) * 2);
    let mut starts: Vec<usize> = Vec::with_capacity(holes.len());
    for p in outer {
        data.push(p.0);
        data.push(p.1);
    }
    for h in holes {
        starts.push(data.len() / 2);
        for p in h {
            data.push(p.0);
            data.push(p.1);
        }
    }
    let mut out = Vec::new();
    match earcutr::earcut(&data, &starts, 2) {
        Ok(idx) if idx.len() >= 3 => {
            for t in idx.chunks_exact(3) {
                let g = |i: usize| (data[i * 2], data[i * 2 + 1]);
                out.push([g(t[0]), g(t[1]), g(t[2])]);
            }
        }
        // Вырожденный случай: хотя бы не оставляем лист без крышек
        _ => {
            for i in 1..outer.len().saturating_sub(1) {
                out.push([outer[0], outer[i], outer[i + 1]]);
            }
        }
    }
    out
}

/// Замкнутое тело листа: верх и низ с отверстиями, торцы и стенки отверстий.
fn sheet_tris(r: &GenResult) -> Vec<Tri> {
    let p = &r.params;
    let (w, h, th) = (p.sheet_w, p.sheet_h, p.thickness.max(0.1));
    let outer = vec![(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
    let holes: Vec<Vec<(f64, f64)>> = r
        .shapes
        .iter()
        .map(poly_pts)
        .filter(|h| h.len() >= 3)
        .map(to_ccw)
        .collect();

    let caps = cap_tris(&outer, &holes);
    let mut t: Vec<Tri> = Vec::with_capacity(caps.len() * 2 + holes.iter().map(Vec::len).sum::<usize>() * 2 + 8);

    // Верх: нормаль +Z, низ: нормаль −Z (обратный обход)
    for [a, b, c] in &caps {
        t.push(((a.0, a.1, th), (b.0, b.1, th), (c.0, c.1, th)));
        t.push(((a.0, a.1, 0.0), (c.0, c.1, 0.0), (b.0, b.1, 0.0)));
    }

    // Торцы листа: наружу
    for i in 0..outer.len() {
        let a = outer[i];
        let b = outer[(i + 1) % outer.len()];
        t.push(((a.0, a.1, 0.0), (b.0, b.1, 0.0), (b.0, b.1, th)));
        t.push(((a.0, a.1, 0.0), (b.0, b.1, th), (a.0, a.1, th)));
    }

    // Стенки отверстий: нормаль внутрь отверстия
    for hole in &holes {
        for i in 0..hole.len() {
            let a = hole[i];
            let b = hole[(i + 1) % hole.len()];
            t.push(((a.0, a.1, 0.0), (b.0, b.1, th), (b.0, b.1, 0.0)));
            t.push(((a.0, a.1, 0.0), (a.0, a.1, th), (b.0, b.1, th)));
        }
    }
    t
}

// ---------------- 3D STL (ASCII) ----------------
pub fn stl(r: &GenResult) -> String {
    let tris = sheet_tris(r);
    let mut o = String::with_capacity(tris.len() * 220 + 128);
    o.push_str("solid PerfoCutSheet\r\n");
    for (a, b, c) in tris {
        // нормаль через векторное произведение
        let u = (b.0 - a.0, b.1 - a.1, b.2 - a.2);
        let v = (c.0 - a.0, c.1 - a.1, c.2 - a.2);
        let n = (u.1 * v.2 - u.2 * v.1, u.2 * v.0 - u.0 * v.2, u.0 * v.1 - u.1 * v.0);
        let l = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt();
        let n = if l > 1e-12 { (n.0 / l, n.1 / l, n.2 / l) } else { (0.0, 0.0, 0.0) };
        let _ = write!(
            o,
            "facet normal {:.4} {:.4} {:.4}\r\n  outer loop\r\n    vertex {:.4} {:.4} {:.4}\r\n    vertex {:.4} {:.4} {:.4}\r\n    vertex {:.4} {:.4} {:.4}\r\n  endloop\r\nendfacet\r\n",
            n.0, n.1, n.2, a.0, a.1, a.2, b.0, b.1, b.2, c.0, c.1, c.2
        );
    }
    o.push_str("endsolid PerfoCutSheet\r\n");
    o
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

    fn key(v: V3) -> (i64, i64, i64) {
        (
            (v.0 * 1e4).round() as i64,
            (v.1 * 1e4).round() as i64,
            (v.2 * 1e4).round() as i64,
        )
    }

    /// Замкнутая оболочка: каждое ребро принадлежит ровно двум треугольникам.
    #[test]
    fn mesh_is_watertight() {
        let mut edges: HashMap<((i64, i64, i64), (i64, i64, i64)), i32> = HashMap::new();
        for (a, b, c) in sheet_tris(&sample()) {
            for (u, v) in [(a, b), (b, c), (c, a)] {
                let (ku, kv) = (key(u), key(v));
                let k = if ku <= kv { (ku, kv) } else { (kv, ku) };
                *edges.entry(k).or_insert(0) += 1;
            }
        }
        let bad = edges.values().filter(|n| **n != 2).count();
        assert_eq!(bad, 0, "рёбер с неверной кратностью: {bad}");
    }

    /// Нормали смотрят наружу: объём положительный и равен площади листа на толщину.
    #[test]
    fn mesh_normals_point_outward() {
        let tris = sheet_tris(&sample());
        let mut v6 = 0.0;
        for (a, b, c) in &tris {
            let cross = (
                b.1 * c.2 - b.2 * c.1,
                b.2 * c.0 - b.0 * c.2,
                b.0 * c.1 - b.1 * c.0,
            );
            v6 += a.0 * cross.0 + a.1 * cross.1 + a.2 * cross.2;
        }
        let vol = v6 / 6.0;
        // 6000 − два круга (π·25 + π·9) − четырёхугольник (площадь 240), толщина 2 мм
        let expect = (6000.0 - 78.54 - 28.27 - 240.0) * 2.0;
        assert!(vol > 0.0, "объём отрицательный: {vol}");
        assert!((vol - expect).abs() < expect * 0.02, "объём {vol}, ожидалось {expect}");
    }

    /// Крышки не должны закрывать отверстия.
    #[test]
    fn caps_have_holes() {
        let r = sample();
        let centers: Vec<(f64, f64)> = r
            .shapes
            .iter()
            .map(|s| match s {
                Shape::Circle { x, y, .. } => (*x, *y),
                Shape::Poly { pts, .. } => (
                    pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64,
                    pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64,
                ),
            })
            .collect();

        let in_tri = |p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
            let sign = |p1: (f64, f64), p2: (f64, f64), p3: (f64, f64)| {
                (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
            };
            let d1 = sign(p, a, b);
            let d2 = sign(p, b, c);
            let d3 = sign(p, c, a);
            let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
            let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
            !(neg && pos)
        };

        for (a, b, c) in sheet_tris(&r) {
            // только горизонтальные крышки
            if !(a.2 == b.2 && b.2 == c.2) {
                continue;
            }
            for q in &centers {
                assert!(
                    !in_tri(*q, (a.0, a.1), (b.0, b.1), (c.0, c.1)),
                    "крышка перекрывает отверстие в {q:?}"
                );
            }
        }
    }
}
