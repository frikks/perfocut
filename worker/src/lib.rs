//! Web Worker для генерации перфорации: считает вне потока интерфейса.
//! Принимает RGBA картинки и JSON параметров, отдаёт JSON с GenResult.

use perfo_core::gen;
use perfo_core::image::Gray;
use perfo_core::params::Params;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn generate(rgba: &[u8], w: u32, h: u32, params_json: &str) -> String {
    let params: Params = match serde_json::from_str(params_json) {
        Ok(p) => p,
        Err(e) => return format!("ERR:{}", e),
    };
    let gray = Gray::from_rgba(rgba, w as usize, h as usize);
    let r = gen::generate(Some(&gray), &params);
    match serde_json::to_string(&r) {
        Ok(s) => s,
        Err(e) => format!("ERR:{}", e),
    }
}
