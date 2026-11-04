//! Перевод настроек между интерфейсом (`Par`, числа f32) и ядром (`Params`, числа f64).

use crate::Par;
use perfo_core::params::Params;

// f32 из интерфейса даёт «хвосты» вроде 0.100000001; округляем до 6 знаков
fn from_ui(v: f32) -> f64 {
    let d = f64::from(v);
    if d.is_finite() {
        (d * 1e6).round() / 1e6
    } else {
        d
    }
}

macro_rules! par_conv {
    (num: $($n:ident),* ; text: $($t:ident),* ; flag: $($b:ident),* $(;)?) => {
        /// Настройки из интерфейса. Числа, которые пользователь не трогал, берутся из `prev`
        /// без потери точности (важно для значений из файла заказа).
        pub fn to_params(par: &Par, prev: &Params) -> Params {
            Params {
                $($n: if par.$n == prev.$n as f32 { prev.$n } else { from_ui(par.$n) },)*
                $($t: par.$t.to_string(),)*
                $($b: par.$b,)*
                gc_decimals: par.gc_decimals.clamp(1, 6) as u32,
                lang: prev.lang.clone(),
            }
        }

        pub fn to_par(p: &Params) -> Par {
            Par {
                $($n: p.$n as f32,)*
                $($t: p.$t.as_str().into(),)*
                $($b: p.$b,)*
                gc_decimals: p.gc_decimals as i32,
            }
        }
    };
}

par_conv!(
    num: sheet_w, sheet_h, border, img_scale, img_dx, img_dy, brightness, contrast, gamma, tone_cut,
        pitch, angle, offset_x, offset_y, d_min, d_max, quant, line_pitch, line_angle, w_min, w_max,
        wave_len, wave_amp, step, thickness, min_k, bridge, density, cut_speed, rapid_speed, pierce_time,
        price_m, price_pierce, price_m2, price_fixed;
    text: fit, pattern, grid, shape, size_mode, fixed_sizes, currency, gc_on, gc_off, gc_header, gc_footer,
        origin, name, order_no, client, phone, email, comment;
    flag: outline, negative, fixed, wave, gc_arcs, gc_numbers, dxf_layers;
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_untouched_precision() {
        let p = Params {
            sheet_w: 1234.5678,
            name: "Тест".into(),
            ..Params::default()
        };
        let mut par = to_par(&p);
        par.pitch = 0.1;
        let back = to_params(&par, &p);
        assert_eq!(back.sheet_w, 1234.5678);
        assert_eq!(back.pitch, 0.1);
        assert_eq!(back.name, "Тест");
        assert_eq!(back.lang, p.lang);
    }
}
