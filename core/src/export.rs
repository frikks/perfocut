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
    let _ = write!(c, "{} 0 0 {} 0 0 cm 0.1 w 0 0 0 RG\n", n4(k), n4(k));
    let kk = 0.552_284_75;
    let f = |v: f64| num(v, 3);
    for s in &r.shapes {
        match s {
            Shape::Circle { x, y, d } => {
                let rr = d / 2.0;
                let m = rr * kk;
                let _ = write!(
                    c,
                    "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c h S\n",
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
        let _ = write!(c, "0 0 {} {} re S\n", f(w), f(h));
    }
    let mut out: Vec<u8> = Vec::with_capacity(c.len() + 1000);
    let mut offs = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let objs = vec![
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
    let mut o = String::from("\u{FEFF}");
    let d = |v: f64, n: usize| format!("{:.*}", n, v).replace('.', ",");
    let _ = write!(o, "Параметр;Значение{NL}");
    let _ = write!(o, "Лист, мм;{} x {}{NL}", d(p.sheet_w, 1), d(p.sheet_h, 1));
    let _ = write!(o, "Толщина, мм;{}{NL}", d(p.thickness, 2));
    let _ = write!(o, "Контуров;{}{NL}", st.count);
    let _ = write!(o, "Врезок;{}{NL}", st.pierces);
    let _ = write!(o, "Длина реза, м;{}{NL}", d(st.cut_len_m, 2));
    let _ = write!(o, "Холостой ход, м;{}{NL}", d(st.travel_m, 2));
    let _ = write!(o, "Время, мин;{}{NL}", d(st.time_min, 1));
    let _ = write!(o, "Перфорация, %;{}{NL}", d(st.open_area_pct, 2));
    let _ = write!(o, "Масса листа, кг;{}{NL}", d(st.weight_kg, 2));
    let _ = write!(o, "Стоимость, {};{}{NL}", p.currency, d(st.cost, 2));
    if !st.by_size.is_empty() {
        let _ = write!(o, "{NL}Размер, мм;Количество{NL}");
        for b in &st.by_size {
            let _ = write!(o, "{};{}{NL}", d(b.size, 2), b.count);
        }
    }
    o
}

pub fn report(r: &GenResult) -> String {
    let st = &r.stats;
    let p = &r.params;
    let mut o = String::new();
    let _ = write!(o, "PerfoCut — паспорт заказа{NL}{NL}");
    let kv = |o: &mut String, k: &str, v: &str| {
        if !v.trim().is_empty() {
            let _ = write!(o, "{:<22}{}{NL}", k, v);
        }
    };
    kv(&mut o, "Изделие:", &p.name);
    kv(&mut o, "Номер заказа:", &p.order_no);
    kv(&mut o, "Клиент:", &p.client);
    kv(&mut o, "Телефон:", &p.phone);
    kv(&mut o, "E-mail:", &p.email);
    kv(&mut o, "Комментарий:", &p.comment);
    let _ = write!(o, "{NL}");
    kv(&mut o, "Лист:", &format!("{} x {} мм, толщина {} мм", num(p.sheet_w, 1), num(p.sheet_h, 1), num(p.thickness, 2)));
    let pat = if p.pattern == "lines" {
        format!("линии, шаг {} мм", num(p.line_pitch, 2))
    } else {
        let g = match p.grid.as_str() {
            "square" => "квадратная",
            "tri90" => "треугольная 2",
            _ => "треугольная",
        };
        let s = match p.shape.as_str() {
            "square" => "квадрат",
            "diamond" => "ромб",
            _ => "круг",
        };
        format!("{}, сетка {}, шаг {} мм", s, g, num(p.pitch, 2))
    };
    kv(&mut o, "Узор:", &pat);
    kv(&mut o, "Контуров:", &st.count.to_string());
    kv(&mut o, "Врезок:", &st.pierces.to_string());
    kv(&mut o, "Длина реза:", &format!("{:.2} м", st.cut_len_m));
    kv(&mut o, "Время резки:", &format!("≈ {:.1} мин", st.time_min));
    kv(&mut o, "Перфорация:", &format!("{:.2} %", st.open_area_pct));
    kv(&mut o, "Масса листа:", &format!("{:.2} кг", st.weight_kg));
    let _ = write!(o, "{NL}Стоимость{NL}");
    kv(&mut o, "  резка:", &format!("{:.2} {}", st.cost_cut, p.currency));
    kv(&mut o, "  врезки:", &format!("{:.2} {}", st.cost_pierce, p.currency));
    if st.cost_material > 0.0 {
        kv(&mut o, "  металл:", &format!("{:.2} {}", st.cost_material, p.currency));
    }
    if st.cost_fixed > 0.0 {
        kv(&mut o, "  подготовка:", &format!("{:.2} {}", st.cost_fixed, p.currency));
    }
    kv(&mut o, "  итого:", &format!("{:.2} {}", st.cost, p.currency));
    if !st.by_size.is_empty() {
        let _ = write!(o, "{NL}Отверстия по размерам{NL}");
        for b in &st.by_size {
            let _ = write!(o, "  {:>8} мм  {:>8} шт{NL}", num(b.size, 2), b.count);
        }
    }
    if !st.warnings.is_empty() {
        let _ = write!(o, "{NL}Замечания{NL}");
        for w in &st.warnings {
            let _ = write!(o, "  - {}{NL}", w);
        }
    }
    o
}
