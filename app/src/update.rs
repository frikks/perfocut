//! Проверка обновлений: разбор ответа GitHub Releases и сравнение версий.
//!
//! Запрашивается `/releases/latest`: GitHub отдаёт только стабильный релиз,
//! предварительные (Pre-release) и черновики в ответ не попадают.

/// Стабильный релиз программы на GitHub.
#[cfg_attr(target_os = "android", allow(dead_code))]
pub const API: &str = "https://api.github.com/repos/frikks/perfocut/releases/latest";

/// Адрес страницы релиза по версии без ведущей `v`.
pub fn release_url(version: &str) -> String {
    format!("https://github.com/frikks/perfocut/releases/tag/v{version}")
}

/// Прямая ссылка на файл релиза: GitHub отдаёт его без страницы.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn asset_url(version: &str, file: &str) -> String {
    format!("https://github.com/frikks/perfocut/releases/download/v{version}/{file}")
}

pub struct Release {
    /// Версия без ведущей `v`, например `0.4.1`
    pub version: String,
    /// Страница релиза для скачивания
    pub url: String,
}

/// Разбирает ответ GitHub. Ошибки технические и короткие: их показывает интерфейс.
pub fn parse(json: &str) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if v.get("prerelease").and_then(serde_json::Value::as_bool) == Some(true)
        || v.get("draft").and_then(serde_json::Value::as_bool) == Some(true)
    {
        return Err("not a stable release".into());
    }
    let tag = v.get("tag_name").and_then(serde_json::Value::as_str).ok_or("no tag_name")?;
    let url = v.get("html_url").and_then(serde_json::Value::as_str).ok_or("no html_url")?;
    let version = tag.trim().trim_start_matches(['v', 'V']).to_owned();
    if version.is_empty() {
        return Err("empty tag_name".into());
    }
    Ok(Release { version, url: url.to_owned() })
}

/// Сравнивает версии по числам: `0.10.0` новее `0.9.9`. Хвост после дефиса не учитывается.
pub fn is_newer(latest: &str, current: &str) -> bool {
    key(latest) > key(current)
}

fn key(v: &str) -> Vec<u64> {
    let core = v.trim().trim_start_matches(['v', 'V']);
    // Предварительные выпуски сравниваем по числовой части: 0.4.1-rc1 > 0.4.0
    let core = core.split(['-', '+']).next().unwrap_or(core);
    let mut out: Vec<u64> = core.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect();
    while out.len() > 1 && out.last() == Some(&0) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{is_newer, parse};

    #[test]
    fn newer_version_wins() {
        assert!(is_newer("v0.4.1", "0.4.0"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0", "0.99.99"));
        assert!(!is_newer("0.4.0", "0.4.0"));
        assert!(!is_newer("0.3.9", "0.4.0"));
        assert!(is_newer("0.4.1-rc1", "0.4.0"));
    }

    #[test]
    fn release_json_is_parsed() {
        let json = r#"{"tag_name":"v0.4.1","html_url":"https://github.com/frikks/perfocut/releases/tag/v0.4.1","prerelease":false,"draft":false}"#;
        let r = parse(json).expect("release");
        assert_eq!(r.version, "0.4.1");
        assert!(r.url.ends_with("/v0.4.1"));
    }

    #[test]
    fn prerelease_is_skipped() {
        let json = r#"{"tag_name":"v0.4.1","html_url":"https://example.com","prerelease":true}"#;
        assert!(parse(json).is_err());
    }

    #[test]
    fn broken_json_is_reported() {
        assert!(parse("<!doctype html>").is_err());
        assert!(parse(r#"{"prerelease":false}"#).is_err());
    }

    #[test]
    fn release_links_point_to_the_tag_and_file() {
        assert_eq!(super::release_url("0.4.1"), "https://github.com/frikks/perfocut/releases/tag/v0.4.1");
        assert_eq!(
            super::asset_url("0.4.1", "PerfoCut.exe"),
            "https://github.com/frikks/perfocut/releases/download/v0.4.1/PerfoCut.exe"
        );
    }
}
