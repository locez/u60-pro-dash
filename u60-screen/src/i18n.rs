//! Offline UI translations. Only authored copy enters this module; device data
//! and protocol identifiers must never be used as translation keys.
use serde::{Deserialize, Serialize};
#[cfg(not(test))]
use std::sync::atomic::{AtomicU8, Ordering};
use std::{collections::HashMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum Locale {
    #[serde(rename = "zh-Hant")]
    Traditional = 1,
    #[serde(rename = "en")]
    English = 2,
    #[serde(rename = "ja")]
    Japanese = 3,
    #[serde(rename = "ko")]
    Korean = 4,
    #[default]
    #[serde(rename = "zh-Hans", other)]
    Simplified = 0,
}
impl Locale {
    pub fn code(self) -> &'static str {
        match self {
            Self::Simplified => "zh-Hans",
            Self::Traditional => "zh-Hant",
            Self::English => "en",
            Self::Japanese => "ja",
            Self::Korean => "ko",
        }
    }
    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "zh-Hans" => Some(Self::Simplified),
            "zh-Hant" => Some(Self::Traditional),
            "en" => Some(Self::English),
            "ja" => Some(Self::Japanese),
            "ko" => Some(Self::Korean),
            _ => None,
        }
    }
}
#[cfg(not(test))]
static LOCALE: AtomicU8 = AtomicU8::new(0);
#[cfg(test)]
thread_local! { static LOCALE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) }; }
static CATALOG: OnceLock<HashMap<String, HashMap<String, String>>> = OnceLock::new();
pub fn select(locale: Locale) {
    #[cfg(not(test))]
    LOCALE.store(locale as u8, Ordering::Relaxed);
    #[cfg(test)]
    LOCALE.set(locale as u8);
}
fn current() -> Locale {
    #[cfg(not(test))]
    let code = LOCALE.load(Ordering::Relaxed);
    #[cfg(test)]
    let code = LOCALE.get();
    match code {
        1 => Locale::Traditional,
        2 => Locale::English,
        3 => Locale::Japanese,
        4 => Locale::Korean,
        _ => Locale::Simplified,
    }
}
pub fn translate(source: &str, locale: Locale) -> &str {
    if locale == Locale::Simplified {
        return source;
    }
    CATALOG
        .get_or_init(|| {
            serde_json::from_str(include_str!("../../locales/screen.json"))
                .expect("validated screen translations")
        })
        .get(source)
        .and_then(|entry| entry.get(locale.code()))
        .filter(|s| !s.is_empty())
        .map(String::as_str)
        .unwrap_or(source)
}
pub fn tr(source: &str) -> &str {
    translate(source, current())
}

// Replace numbered placeholders once. Inserted SSIDs, messages, names and errors
// are literal values, even when they contain braces or translation keys.
fn interpolate(template: &str, values: &[String]) -> String {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        if let Some(end) = rest.find('}')
            && let Ok(index) = rest[1..end].parse::<usize>()
            && let Some(value) = values.get(index)
        {
            output.push_str(value);
            rest = &rest[end + 1..];
        } else {
            output.push('{');
            rest = &rest[1..];
        }
    }
    output.push_str(rest);
    output
}
pub fn format(source: &str, values: &[String]) -> String {
    interpolate(tr(source), values)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacements_are_literal_and_can_be_reordered() {
        assert_eq!(
            interpolate("{1} / {0}", &["SSID {1}".into(), "未设置".into()]),
            "未设置 / SSID {1}"
        );
        assert_eq!(interpolate("{3}", &[]), "{3}");
    }
    #[test]
    fn locale_codes_round_trip() {
        for code in ["zh-Hans", "zh-Hant", "en", "ja", "ko"] {
            let locale = Locale::parse(code).unwrap();
            assert_eq!(locale.code(), code);
            assert_eq!(
                serde_json::from_str::<Locale>(&serde_json::to_string(&locale).unwrap()).unwrap(),
                locale
            );
        }
        assert!(Locale::parse("invalid").is_none());
    }
    #[test]
    fn localized_field_labels_do_not_change_user_values_or_wire_types() {
        use crate::features::{Field, Wire};
        for code in ["zh-Hans", "zh-Hant", "en", "ja", "ko"] {
            let locale = Locale::parse(code).unwrap();
            let ssid = serde_json::json!("未设置 {0}");
            let field = Field::text("ssid_2g", translate("网络名称", locale), &ssid);
            assert_eq!(field.encode().unwrap(), ssid);
            let mut enabled = Field::text(
                "enabled",
                translate("开启", locale),
                &serde_json::json!(true),
            );
            enabled.wire = Wire::Boolean;
            assert_eq!(enabled.encode().unwrap(), serde_json::json!(true));
        }
    }
}
