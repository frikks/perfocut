//! Собирает интерфейс из .slint и вшивает английский перевод из lang/en.
//!
//! Шрифты и значок окна лежат в `assets/` как zlib + base64 (текстом, чтобы репозиторий
//! был без двоичных файлов). Здесь они распаковываются в OUT_DIR, а Slint находит их
//! через include path: `import "fonts/Inter-Regular.ttf"`, `@image-url("icons/app.png")`.

use std::fs;
use std::path::{Path, PathBuf};

fn decode_base64(text: &str) -> Vec<u8> {
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for v in text.bytes().filter_map(val) {
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

fn unpack_assets(src: &Path, dst: &Path) {
    for dir in ["fonts", "icons"] {
        let from = src.join(dir);
        let to = dst.join(dir);
        fs::create_dir_all(&to).expect("assets dir");
        let entries = fs::read_dir(&from).unwrap_or_else(|e| panic!("{}: {e}", from.display()));
        for entry in entries {
            let path = entry.expect("assets entry").path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let Some(target) = name.strip_suffix(".zlib.b64") else { continue };
            let text = fs::read_to_string(&path).expect("read asset");
            let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(&decode_base64(&text))
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            fs::write(to.join(target), bytes).expect("write asset");
        }
    }
}

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("assets");
    unpack_assets(Path::new("assets"), &out);
    let cfg = slint_build::CompilerConfiguration::new()
        .with_include_paths(vec![out])
        .with_default_translation_context(slint_build::DefaultTranslationContext::None)
        .with_bundled_translations("lang");
    if let Err(e) = slint_build::compile_with_config("ui/app.slint", cfg) {
        panic!("Slint UI: {e}");
    }
    println!("cargo:rerun-if-changed=lang");
    println!("cargo:rerun-if-changed=assets");
}
