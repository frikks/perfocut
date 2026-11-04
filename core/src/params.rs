use serde::{Deserialize, Serialize};

pub const ALGO_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    // Лист
    pub sheet_w: f64,
    pub sheet_h: f64,
    pub border: f64,
    pub outline: bool,
    // Изображение
    pub fit: String, // cover | contain | stretch
    pub img_scale: f64,
    pub img_dx: f64,
    pub img_dy: f64,
    pub brightness: f64,
    pub contrast: f64,
    pub gamma: f64,
    pub negative: bool,
    pub tone_cut: f64,
    // Узор
    pub pattern: String, // dots | lines
    pub grid: String,    // square | tri | tri90
    pub pitch: f64,
    pub angle: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub shape: String, // circle | square | diamond
    pub d_min: f64,
    pub d_max: f64,
    pub fixed: bool,
    pub fixed_sizes: String,
    pub quant: f64,
    pub line_pitch: f64,
    pub line_angle: f64,
    pub w_min: f64,
    pub w_max: f64,
    pub wave: bool,
    pub wave_len: f64,
    pub wave_amp: f64,
    pub step: f64,
    // Лазер и металл
    pub thickness: f64,
    pub min_k: f64,
    pub bridge: f64,
    pub density: f64,
    // Резка и стоимость
    pub cut_speed: f64,
    pub rapid_speed: f64,
    pub pierce_time: f64,
    pub price_m: f64,
    pub price_pierce: f64,
    pub price_m2: f64,
    pub price_fixed: f64,
    pub currency: String,
    // G-code
    pub gc_on: String,
    pub gc_off: String,
    pub gc_header: String,
    pub gc_footer: String,
    pub gc_arcs: bool,
    pub gc_numbers: bool,
    pub gc_decimals: u32,
    pub origin: String, // bl | center | tl
    // DXF
    pub dxf_layers: bool,
    // Заказ
    pub name: String,
    pub order_no: String,
    pub client: String,
    pub phone: String,
    pub email: String,
    pub comment: String,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            sheet_w: 1000.0,
            sheet_h: 600.0,
            border: 20.0,
            outline: true,
            fit: "cover".into(),
            img_scale: 1.0,
            img_dx: 0.0,
            img_dy: 0.0,
            brightness: 0.0,
            contrast: 0.0,
            gamma: 1.0,
            negative: false,
            tone_cut: 0.06,
            pattern: "dots".into(),
            grid: "tri".into(),
            pitch: 12.0,
            angle: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            shape: "circle".into(),
            d_min: 2.0,
            d_max: 10.0,
            fixed: false,
            fixed_sizes: "3, 5, 7, 9".into(),
            quant: 0.5,
            line_pitch: 10.0,
            line_angle: 0.0,
            w_min: 2.0,
            w_max: 8.0,
            wave: false,
            wave_len: 80.0,
            wave_amp: 6.0,
            step: 1.0,
            thickness: 1.5,
            min_k: 1.0,
            bridge: 1.5,
            density: 7.85,
            cut_speed: 4000.0,
            rapid_speed: 30000.0,
            pierce_time: 0.3,
            price_m: 25.0,
            price_pierce: 1.5,
            price_m2: 0.0,
            price_fixed: 0.0,
            currency: "₽".into(),
            gc_on: "M07".into(),
            gc_off: "M08".into(),
            gc_header: "G21\nG90\nG17".into(),
            gc_footer: "M30".into(),
            gc_arcs: true,
            gc_numbers: false,
            gc_decimals: 3,
            origin: "bl".into(),
            dxf_layers: true,
            name: "Панель".into(),
            order_no: String::new(),
            client: String::new(),
            phone: String::new(),
            email: String::new(),
            comment: String::new(),
        }
    }
}

impl Params {
    /// Приводит значения к безопасным границам, чтобы «сумасшедшие» числа не роняли программу.
    pub fn sanitized(&self) -> Params {
        let mut p = self.clone();
        let fin = |v: f64, d: f64| if v.is_finite() { v } else { d };
        p.sheet_w = fin(p.sheet_w, 1000.0).clamp(10.0, 20000.0);
        p.sheet_h = fin(p.sheet_h, 600.0).clamp(10.0, 20000.0);
        let half = p.sheet_w.min(p.sheet_h) / 2.0 - 1.0;
        p.border = fin(p.border, 0.0).clamp(0.0, half.max(0.0));
        p.img_scale = fin(p.img_scale, 1.0).clamp(0.05, 20.0);
        p.brightness = fin(p.brightness, 0.0).clamp(-1.0, 1.0);
        p.contrast = fin(p.contrast, 0.0).clamp(-1.0, 3.0);
        p.gamma = fin(p.gamma, 1.0).clamp(0.1, 5.0);
        p.tone_cut = fin(p.tone_cut, 0.0).clamp(0.0, 0.95);
        p.pitch = fin(p.pitch, 12.0).clamp(0.5, 1000.0);
        p.line_pitch = fin(p.line_pitch, 10.0).clamp(0.5, 1000.0);
        p.d_min = fin(p.d_min, 0.0).max(0.0);
        p.d_max = fin(p.d_max, 1.0).max(0.01);
        p.w_min = fin(p.w_min, 0.0).max(0.0);
        p.w_max = fin(p.w_max, 1.0).max(0.01);
        p.wave_len = fin(p.wave_len, 80.0).clamp(1.0, 100000.0);
        p.wave_amp = fin(p.wave_amp, 0.0).clamp(0.0, 10000.0);
        p.quant = fin(p.quant, 0.0).clamp(0.0, 100.0);
        p.step = fin(p.step, 1.0).clamp(0.1, 100.0);
        p.thickness = fin(p.thickness, 1.0).clamp(0.0, 100.0);
        p.min_k = fin(p.min_k, 1.0).clamp(0.0, 10.0);
        p.bridge = fin(p.bridge, 1.0).clamp(0.0, 100.0);
        p.density = fin(p.density, 7.85).clamp(0.0, 30.0);
        p.cut_speed = fin(p.cut_speed, 4000.0).clamp(1.0, 1e6);
        p.rapid_speed = fin(p.rapid_speed, 30000.0).clamp(1.0, 1e7);
        p.pierce_time = fin(p.pierce_time, 0.3).clamp(0.0, 60.0);
        p.gc_decimals = p.gc_decimals.clamp(1, 6);
        p
    }

    pub fn fixed_list(&self) -> Vec<f64> {
        let mut v: Vec<f64> = self
            .fixed_sizes
            .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .filter_map(|s| s.trim().replace(',', ".").parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        v
    }
}
