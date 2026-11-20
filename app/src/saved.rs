//! Настройки программы между запусками: тема, язык, форматы, цвета и последние параметры.

use perfo_core::params::Params;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Saved {
    pub theme: i32,
    /// Последний применённый шаблон перфорации
    pub template_id: String,
    /// Признак миграции на шесть техно-тем (0..5)
    pub theme_v2: bool,
    /// Акцентный цвет: 0 цитрус, 1 изумруд, 2 лазурь, 3 аметист, 4 роза, 5 магма
    pub accent: i32,
    /// "ru" или "en"; пусто при первом запуске — тогда язык системы
    pub lang: String,
    pub vibro: bool,
    pub lock_ratio: bool,
    pub ratio: f64,
    pub formats: BTreeMap<String, bool>,
    pub sheet_color: String,
    pub back_color: String,
    pub view_mode: i32,
    /// Признак того, что режим просмотра выбран пользователем (для миграции значения по умолчанию)
    pub view_mode_v2: bool,
    pub params: Option<Params>,
    /// Кисть для рисования: диаметр в мм, мягкость и тон 0..1
    pub brush_size: f64,
    pub brush_soft: f64,
    pub brush_tone: f64,
    /// Когда последний раз проверяли обновления, мс с 1970 года; 0 — ещё не проверяли
    pub update_check: f64,
    /// Последняя известная версия релиза: точка на шестерёнке видна сразу при запуске
    pub update_version: String,
    /// Пресеты техпроцесса: материал, толщина, kerf, скорости, врезки и цены
    pub presets: Vec<Preset>,
}

/// Пресет техпроцесса: применяется к «Металл и лазер» и ценам одним выбором.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Preset {
    pub name: String,
    pub thickness: f64,
    pub min_k: f64,
    pub bridge: f64,
    pub kerf: f64,
    pub density: f64,
    pub cut_speed: f64,
    pub rapid_speed: f64,
    pub pierce_time: f64,
    pub price_m: f64,
    pub price_pierce: f64,
    pub price_kg: f64,
    pub price_fixed: f64,
    pub currency: String,
}

impl Default for Preset {
    fn default() -> Self {
        Preset {
            name: String::new(),
            thickness: 1.0,
            min_k: 1.0,
            bridge: 1.5,
            kerf: 0.0,
            density: 7.85,
            cut_speed: 4000.0,
            rapid_speed: 30000.0,
            pierce_time: 0.3,
            price_m: 25.0,
            price_pierce: 1.5,
            price_kg: 0.0,
            price_fixed: 0.0,
            currency: "₽".into(),
        }
    }
}

/// Штатные пресеты при первом запуске: стартовая точка, свои сохраняются поверх.
pub fn default_presets() -> Vec<Preset> {
    let base = |name: &str, thickness: f64, kerf: f64, cut: f64, pierce: f64| Preset {
        name: name.into(),
        thickness,
        kerf,
        cut_speed: cut,
        pierce_time: pierce,
        ..Preset::default()
    };
    vec![
        base("Сталь 1 мм", 1.0, 0.08, 4000.0, 0.3),
        base("Сталь 2 мм", 2.0, 0.1, 2500.0, 0.5),
        base("Нерж. 1,5 мм", 1.5, 0.1, 3000.0, 0.4),
    ]
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            theme: 0,
            theme_v2: false,
            template_id: String::new(),
            accent: 0,
            lang: String::new(),
            vibro: true,
            lock_ratio: false,
            ratio: 0.0,
            formats: BTreeMap::new(),
            sheet_color: "#9a9ea3".into(),
            back_color: "#f2c48d".into(),
            view_mode: 1,
            view_mode_v2: false,
            params: None,
            brush_size: 8.0,
            brush_soft: 0.25,
            brush_tone: 1.0,
            update_check: 0.0,
            update_version: String::new(),
            presets: default_presets(),
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
        let s: Saved =
            serde_json::from_str(r#"{"theme":3,"unknown":1,"params":{"pitch":9}}"#).expect("json");
        assert_eq!(s.theme, 3);
        assert!(s.vibro);
        assert_eq!(s.params.map(|p| p.pitch), Some(9.0));
        assert_eq!(lang_from_locale("ru-RU"), "ru");
        assert_eq!(lang_from_locale("en_US"), "en");
        // У старого файла пресетов нет — приходят штатные (поле заполнится из Default).
        assert_eq!(s.presets.len(), default_presets().len());
    }

    #[test]
    fn presets_survive_roundtrip() {
        let mut s = Saved::default();
        assert!(!s.presets.is_empty());
        let mut pr = s.presets[0].clone();
        pr.kerf = 0.17;
        pr.name = "Мой".into();
        s.presets.push(pr);
        let txt = serde_json::to_string(&s).expect("json");
        let back: Saved = serde_json::from_str(&txt).expect("load");
        assert_eq!(back.presets.len(), s.presets.len());
        let back_pr = back.presets.last().expect("preset");
        assert_eq!(back_pr.kerf, 0.17);
        assert_eq!(back_pr.name, "Мой");
        // Старый файл без поля пресетов: список наполняется штатными.
        let old: Saved = serde_json::from_str(r#"{"theme":0}"#).expect("json");
        assert_eq!(old.presets.len(), default_presets().len());
        // А явно пустой список сохраняется пустым: удаление пресетов не откатывается.
        let empty: Saved = serde_json::from_str(r#"{"presets":[]}"#).expect("json");
        assert!(empty.presets.is_empty());
    }
}
