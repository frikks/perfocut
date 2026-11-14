//! Настройки программы между запусками: тема, язык, форматы, цвета и последние параметры.

use perfo_core::params::Params;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Saved {
    pub theme: i32,
    /// "ru" или "en"; пусто при первом запуске — тогда язык системы
    pub lang: String,
    pub vibro: bool,
    pub lock_ratio: bool,
    pub ratio: f64,
    pub formats: BTreeMap<String, bool>,
    pub sheet_color: String,
    pub back_color: String,
    pub view_mode: i32,
    pub params: Option<Params>,
    /// Кисть для рисования: диаметр в мм, мягкость и тон 0..1
    pub brush_size: f64,
    pub brush_soft: f64,
    pub brush_tone: f64,
    /// Когда последний раз проверяли обновления, мс с 1970 года; 0 — ещё не проверяли
    pub update_check: f64,
    /// Последняя известная версия релиза: точка на шестерёнке видна сразу при запуске
    pub update_version: String,
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            theme: 0,
            lang: String::new(),
            vibro: true,
            lock_ratio: false,
            ratio: 0.0,
            formats: BTreeMap::new(),
            sheet_color: "#9a9ea3".into(),
            back_color: "#f2c48d".into(),
            view_mode: 0,
            params: None,
            brush_size: 8.0,
            brush_soft: 0.25,
            brush_tone: 1.0,
            update_check: 0.0,
            update_version: String::new(),
        }
    }
}

impl Saved {
    pub fn load() -> Saved {
        crate::platform::load_settings()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn store(&self) {
        if let Ok(s) = serde_json::to_string(self) {
            crate::platform::store_settings(&s);
        }
    }
}

/// Язык по системе: русский для русского, украинского, белорусского и казахского, иначе английский.
pub fn lang_from_locale(loc: &str) -> &'static str {
    let l = loc.to_lowercase();
    if ["ru", "uk", "be", "kk"].iter().any(|p| l.starts_with(p)) {
        "ru"
    } else {
        "en"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_or_partial_settings_still_load() {
        let s: Saved = serde_json::from_str(r#"{"theme":3,"unknown":1,"params":{"pitch":9}}"#).expect("json");
        assert_eq!(s.theme, 3);
        assert!(s.vibro);
        assert_eq!(s.params.map(|p| p.pitch), Some(9.0));
        assert_eq!(lang_from_locale("ru-RU"), "ru");
        assert_eq!(lang_from_locale("en_US"), "en");
    }
}
