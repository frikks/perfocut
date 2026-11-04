use perfo_core::*;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (w, h): (usize, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let rgba = std::fs::read(&a[1]).unwrap();
    let p: params::Params = serde_json::from_str(&std::fs::read_to_string(&a[4]).unwrap()).unwrap();
    let g = image::Gray::from_rgba(&rgba, w, h);
    let t = std::time::Instant::now();
    let r = gen::generate(Some(&g), &p);
    eprintln!("gen {:?} count {}", t.elapsed(), r.stats.count);
    for f in ["dxf", "svg", "nc", "plt", "eps", "pdf", "csv", "report", "stats"] {
        let ext = match f { "report" => "txt", "stats" => "json", x => x };
        std::fs::write(format!("{}/out.{}", a[5], ext), export_format(&r, f).unwrap()).unwrap();
    }
}
