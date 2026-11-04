//! Перевод строк из кода на Rust и числа в формате выбранного языка.
//! Словарь общий с интерфейсом: `lang/en/LC_MESSAGES/perfocut.po`, ключи на русском.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static EN: AtomicBool = AtomicBool::new(false);
static DICT: OnceLock<HashMap<String, String>> = OnceLock::new();

pub fn set_en(on: bool) {
    EN.store(on, Ordering::Relaxed);
}

pub fn is_en() -> bool {
    EN.load(Ordering::Relaxed)
}

fn unquote(s: &str) -> Option<String> {
    let inner = s.trim().strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut it = inner.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(o) => out.push(o),
            None => {}
        }
    }
    Some(out)
}

// В нашем .po каждая запись в одну строку: msgid "…" / msgstr "…"
fn parse_po(src: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut id: Option<String> = None;
    for line in src.lines() {
        if let Some(rest) = line.strip_prefix("msgid ") {
            id = unquote(rest);
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            if let (Some(k), Some(v)) = (id.take(), unquote(rest)) {
                if !k.is_empty() && !v.is_empty() {
                    map.insert(k, v);
                }
            }
        }
    }
    map
}

fn dict() -> &'static HashMap<String, String> {
    DICT.get_or_init(|| parse_po(include_str!("../lang/en/LC_MESSAGES/perfocut.po")))
}

/// Переводит строку на текущий язык. Пробелы по краям сохраняются, ключ ищется без них.
pub fn t(s: &str) -> String {
    if !is_en() {
        return s.to_owned();
    }
    let core = s.trim();
    match dict().get(core) {
        Some(en) => {
            let lead = &s[..s.len() - s.trim_start().len()];
            let trail = &s[s.trim_end().len()..];
            format!("{lead}{en}{trail}")
        }
        None => s.to_owned(),
    }
}

/// Переводит и подставляет значения вместо `{имя}`.
pub fn tf(s: &str, args: &[(&str, &str)]) -> String {
    let mut out = t(s);
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// Число с разделителями разрядов: «12 345,6» по-русски, «12,345.6» по-английски.
pub fn fmt_n(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return "—".into();
    }
    let s = format!("{:.*}", decimals, v.abs());
    let (int, frac) = match s.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (s.as_str(), None),
    };
    let (group, point) = if is_en() { (',', '.') } else { ('\u{a0}', ',') };
    let mut out = String::with_capacity(s.len() + 4);
    if v < 0.0 && s.bytes().any(|b| b.is_ascii_digit() && b != b'0') {
        out.push('-');
    }
    let n = int.len();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (n - i) % 3 == 0 {
            out.push(group);
        }
        out.push(c);
    }
    if let Some(fr) = frac {
        out.push(point);
        out.push_str(fr);
    }
    out
}

/// Время резки: «8,5 мин» или «2 ч 15 мин».
pub fn fmt_time(min: f64) -> String {
    if min < 60.0 {
        let d = if min < 10.0 { 1 } else { 0 };
        return format!("{} {}", fmt_n(min, d), t("мин"));
    }
    let h = (min / 60.0).floor();
    format!("{} {} {} {}", fmt_n(h, 0), t("ч"), fmt_n((min - h * 60.0).round(), 0), t("мин"))
}

/// Число для поля ввода: точка, без лишних нулей.
pub fn fmt_field(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let mut s = format!("{:.*}", decimals, v);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// Разбирает число из поля: принимает и точку, и запятую.
pub fn parse_num(s: &str) -> Option<f64> {
    let v: f64 = s.trim().replace(',', ".").parse().ok()?;
    v.is_finite().then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_follow_language() {
        set_en(false);
        assert_eq!(fmt_n(12345.67, 1), "12\u{a0}345,7");
        assert_eq!(fmt_n(-0.01, 1), "0,0");
        set_en(true);
        assert_eq!(fmt_n(1234567.0, 0), "1,234,567");
        assert_eq!(t("Резка"), "Cutting");
        assert_eq!(t(" мм"), " mm");
        set_en(false);
    }

    #[test]
    fn field_numbers_drop_trailing_zeros() {
        assert_eq!(fmt_field(12.5, 2), "12.5");
        assert_eq!(fmt_field(3.0, 2), "3");
        assert_eq!(fmt_field(-0.0001, 2), "0");
        assert_eq!(parse_num("1,5"), Some(1.5));
        assert_eq!(parse_num("abc"), None);
    }
}
