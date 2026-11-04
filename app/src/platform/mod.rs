//! Всё, что зависит от системы: файлы, сохранение, вибрация, ссылки, настройки.
//! У каждой платформы одинаковый набор функций.

/// Чем закончилось сохранение файла.
pub enum SaveOutcome {
    /// Сохранено по пути
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Saved(String),
    /// Браузер скачал файл с этим именем
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    Downloaded(String),
    /// Пользователь закрыл окно сохранения (есть только на компьютере)
    #[cfg_attr(any(target_os = "android", target_arch = "wasm32"), allow(dead_code))]
    Cancelled,
    Failed(String),
}

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::*;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod desktop;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub use desktop::*;

#[cfg(not(target_arch = "wasm32"))]
mod clock {
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn now_ms() -> f64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
    }

    /// Текущее время UTC в виде 2026-01-31T12:00:00Z
    pub fn now_iso() -> String {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
        // Перевод номера дня в дату по алгоритму Говарда Хиннанта
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i64::from(m <= 2);
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn iso_time_has_fixed_shape() {
            let s = super::now_iso();
            assert_eq!(s.len(), 20);
            assert!(s.starts_with("20") && s.ends_with('Z'));
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub use clock::{now_iso, now_ms};
