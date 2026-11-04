//! Файлы: чтение картинок, пример, форматы экспорта, архив ZIP и файл заказа .perfo.

use crate::i18n::t;
use crate::render::{self, Look};
use perfo_core::gen::GenResult;
use perfo_core::image::Gray;
use perfo_core::params::Params;
use perfo_core::zip;
use std::io::Cursor;
use tiny_skia::{IntSize, Pixmap};

/// Картинка после чтения: уменьшена до 1600 px, прозрачность залита белым.
pub struct Decoded {
    pub pix: Pixmap,
    pub gray: Gray,
    pub w0: u32,
    pub h0: u32,
    pub thumb: (Vec<u8>, u32, u32),
}

const MAX_SIDE: f64 = 1600.0;

fn flatten_white(px: &mut [u8]) {
    for p in px.as_chunks_mut::<4>().0 {
        let a = u32::from(p[3]);
        for c in &mut p[..3] {
            *c = ((u32::from(*c) * a + 255 * (255 - a)) / 255) as u8;
        }
        p[3] = 255;
    }
}

pub fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    use image::{DynamicImage, ImageDecoder, ImageReader};
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| e.to_string())?;
    let mut dec = reader.into_decoder().map_err(|e| e.to_string())?;
    // Фото с телефона хранят поворот отдельно (EXIF)
    let orient = dec.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(dec).map_err(|e| e.to_string())?;
    img.apply_orientation(orient);
    let (w0, h0) = (img.width(), img.height());
    if w0 == 0 || h0 == 0 {
        return Err(t("пустая картинка"));
    }
    let k = (MAX_SIDE / f64::from(w0.max(h0))).min(1.0);
    let w = ((f64::from(w0) * k).round() as u32).max(1);
    let h = ((f64::from(h0) * k).round() as u32).max(1);
    let small = if k < 1.0 { img.resize_exact(w, h, image::imageops::FilterType::Triangle) } else { img };
    let mut thumb = small.thumbnail(160, 160).to_rgba8();
    flatten_white(thumb.as_mut());
    let (tw, th) = (thumb.width(), thumb.height());
    let mut rgba = small.into_rgba8();
    flatten_white(rgba.as_mut());
    let gray = Gray::from_rgba(rgba.as_raw(), w as usize, h as usize);
    let size = IntSize::from_wh(w, h).ok_or_else(|| t("пустая картинка"))?;
    let pix = Pixmap::from_vec(rgba.into_raw(), size).ok_or_else(|| t("пустая картинка"))?;
    Ok(Decoded { pix, gray, w0, h0, thumb: (thumb.into_raw(), tw, th) })
}

/// PNG из готовых пикселей (без прозрачности, поэтому умножение на альфу не мешает).
pub fn encode_png(pm: &Pixmap) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(pm.data(), pm.width(), pm.height(), image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

pub fn sample_png() -> Result<Vec<u8>, String> {
    let pm = render::sample_pixmap().ok_or_else(|| t("пустая картинка"))?;
    encode_png(&pm)
}

/// Формат экспорта: код для ядра, расширение, название и пояснение (переводятся при показе).
pub struct Format {
    pub id: &'static str,
    pub ext: &'static str,
    pub title: &'static str,
    pub note: &'static str,
    pub on: bool,
}

pub const FORMATS: [Format; 10] = [
    Format { id: "dxf", ext: "dxf", title: "DXF", note: "CypCut, AutoCAD · R12, окружности", on: true },
    Format { id: "nc", ext: "nc", title: "G-code .nc", note: "NCStudio, Unicut", on: true },
    Format { id: "cnc", ext: "cnc", title: "G-code .cnc", note: "Тот же код с расширением .cnc", on: false },
    Format { id: "svg", ext: "svg", title: "SVG", note: "Inkscape, LightBurn, веб", on: true },
    Format { id: "pdf", ext: "pdf", title: "PDF", note: "Вектор для согласования и печати", on: true },
    Format { id: "eps", ext: "eps", title: "EPS", note: "CorelDRAW, Illustrator", on: false },
    Format { id: "plt", ext: "plt", title: "PLT", note: "HPGL, плоттеры", on: false },
    Format { id: "png", ext: "png", title: "PNG", note: "Вид для клиента", on: true },
    Format { id: "csv", ext: "csv", title: "CSV", note: "Сводка для Excel", on: false },
    Format { id: "report", ext: "txt", title: "Паспорт заказа", note: "Текст с расчётом", on: true },
];

pub fn mime_for(ext: &str) -> &'static str {
    match ext {
        "dxf" => "application/dxf",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "csv" => "text/csv",
        "txt" | "nc" | "cnc" | "plt" => "text/plain",
        "zip" => "application/zip",
        "eps" => "application/postscript",
        _ => "application/octet-stream",
    }
}

/// Имя файла из названия изделия и номера заказа без запрещённых символов.
pub fn base_name(p: &Params) -> String {
    let clean = |s: &str| -> String {
        let mut out = String::new();
        for c in s.trim().chars() {
            out.push(if "\\/:*?\"<>|".contains(c) || c.is_whitespace() { '_' } else { c });
        }
        out
    };
    let name = clean(&p.name);
    let mut parts = vec![if name.is_empty() { "perfocut".to_owned() } else { name }];
    let no = clean(&p.order_no);
    if !no.is_empty() {
        parts.push(no);
    }
    parts.join("_")
}

pub fn format_bytes(f: &Format, r: &GenResult, look: &Look) -> Result<Vec<u8>, String> {
    if f.id == "png" {
        let pm = render::metal_pixmap(&r.params, &r.shapes, look, 4000.0).ok_or_else(|| t("пустая картинка"))?;
        return encode_png(&pm);
    }
    perfo_core::export_format(r, f.id).ok_or_else(|| format!("format {}", f.id))
}

pub fn zip_formats(sel: &[&Format], r: &GenResult, look: &Look) -> Result<Vec<u8>, String> {
    let base = base_name(&r.params);
    let mut z = zip::ZipWriter::default();
    for f in sel {
        z.add(&format!("{base}.{}", f.ext), &format_bytes(f, r, look)?);
    }
    Ok(z.finish())
}

/// Файл заказа: настройки, исходная картинка и, если узор готов, просмотр, паспорт и DXF.
pub fn order_bytes(r: &GenResult, src_name: &str, src: &[u8], look: &Look, saved_at: &str) -> Result<Vec<u8>, String> {
    let ext: String = src_name.rsplit('.').next().unwrap_or("png").to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let ext = if ext.is_empty() || ext.len() > 5 { "png".to_owned() } else { ext };
    let mut settings = serde_json::json!({
        "app": "PerfoCut",
        "algo_version": perfo_core::params::ALGO_VERSION,
        "params": r.params,
    });
    settings["source_name"] = src_name.into();
    settings["saved"] = saved_at.into();
    let mut z = zip::ZipWriter::default();
    z.add("settings.json", &serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?);
    z.add(&format!("source.{ext}"), src);
    if r.stats.count > 0 {
        let pm = render::metal_pixmap(&r.params, &r.shapes, look, 2400.0).ok_or_else(|| t("пустая картинка"))?;
        z.add("preview.png", &encode_png(&pm)?);
        for (name, id) in [("report.txt", "report"), ("stats.json", "stats"), ("cut.dxf", "dxf")] {
            if let Some(b) = perfo_core::export_format(r, id) {
                z.add(name, &b);
            }
        }
    }
    Ok(z.finish())
}

/// Содержимое файла заказа: настройки и, если есть, исходная картинка.
pub struct Order {
    pub params: Params,
    pub source: Option<(String, Vec<u8>)>,
}

pub fn read_order(bytes: &[u8]) -> Result<Order, String> {
    let list = zip::list(bytes).ok_or_else(|| t("Файл заказа повреждён или сжат другой программой"))?;
    let get = |want: &str| {
        list.iter()
            .find(|e| e.method == 0 && (e.name == want || e.name.ends_with(&format!("/{want}"))))
            .and_then(|e| bytes.get(e.offset..e.offset + e.len))
    };
    let st = get("settings.json").ok_or_else(|| t("В архиве нет settings.json"))?;
    let obj: serde_json::Value = serde_json::from_slice(st).map_err(|_| t("settings.json не читается"))?;
    let pv = obj.get("params").cloned().unwrap_or_else(|| obj.clone());
    let params: Params = serde_json::from_value(pv).map_err(|_| t("settings.json не читается"))?;
    let source = list.iter().find(|e| e.method == 0 && e.name.starts_with("source.")).and_then(|e| {
        let data = bytes.get(e.offset..e.offset + e.len)?.to_vec();
        let ext = e.name.trim_start_matches("source");
        let stem = obj
            .get("source_name")
            .and_then(|v| v.as_str())
            .map(|s| s.rsplit_once('.').map_or(s, |(a, _)| a).to_owned())
            .unwrap_or_else(|| "source".to_owned());
        Some((format!("{stem}{ext}"), data))
    });
    Ok(Order { params, source })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Color;

    fn look() -> Look {
        let c = Color::BLACK;
        Look { sheet: render::Rgb(150, 150, 150), back: render::Rgb(240, 200, 140), card: c, ink: c, frame: c, dim: c }
    }

    #[test]
    fn order_round_trip_restores_params_and_source() {
        let png = sample_png().expect("sample");
        let dec = decode(&png).expect("decode");
        assert_eq!((dec.w0, dec.h0), (1200, 720));
        let p = Params {
            name: "Фасад 1".into(),
            order_no: "A/7".into(),
            pitch: 15.0,
            ..Params::default()
        };
        let r = perfo_core::gen::generate(Some(&dec.gray), &p);
        assert!(r.stats.count > 100);
        let bytes = order_bytes(&r, "фото.png", &png, &look(), "2026-01-01T00:00:00Z").expect("order");
        let o = read_order(&bytes).expect("read");
        assert_eq!(o.params.pitch, 15.0);
        assert_eq!(o.params.name, "Фасад 1");
        let (name, data) = o.source.expect("source");
        assert_eq!(name, "фото.png");
        assert_eq!(data, png);
        assert_eq!(base_name(&p), "Фасад_1_A_7");
    }

    #[test]
    fn every_format_exports() {
        let dec = decode(&sample_png().expect("sample")).expect("decode");
        let r = perfo_core::gen::generate(Some(&dec.gray), &Params::default());
        let all: Vec<&Format> = FORMATS.iter().collect();
        let z = zip_formats(&all, &r, &look()).expect("zip");
        assert_eq!(zip::list(&z).map(|l| l.len()), Some(FORMATS.len()));
    }

    #[test]
    fn broken_files_give_readable_errors() {
        assert!(decode(b"not an image").is_err());
        assert!(read_order(b"PK nonsense").is_err());
    }
}
