//! Localize recognized carrier brands, never arbitrary network-provided names.
pub fn label(name: &str) -> &str {
    let name = name.trim();
    let normalized: String = name
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '_'))
        .flat_map(char::to_uppercase)
        .collect();
    match normalized.as_str() {
        "CHINAMOBILE" | "CMCC" | "中国移动" | "中國移動" => crate::i18n::tr("中国移动"),
        "CHINAUNICOM" | "UNICOM" | "CUCC" | "中国联通" | "中國聯通" => {
            crate::i18n::tr("中国联通")
        }
        "CHINATELECOM" | "CTCC" | "中国电信" | "中國電信" => crate::i18n::tr("中国电信"),
        "CHINABROADNET" | "CHINABROADCASTINGNETWORK" | "CBN" | "中国广电" | "中國廣電" => {
            crate::i18n::tr("中国广电")
        }
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Locale};
    #[test]
    fn known_brands_follow_locale_and_unknown_names_remain_literal() {
        for locale in [
            Locale::Simplified,
            Locale::Traditional,
            Locale::English,
            Locale::Japanese,
            Locale::Korean,
        ] {
            i18n::select(locale);
            assert_eq!(label("  CHINA MOBILE  "), i18n::tr("中国移动"));
            assert_eq!(label("中國聯通"), i18n::tr("中国联通"));
            assert_eq!(label("China Telecom"), i18n::tr("中国电信"));
            assert_eq!(label("CBN"), i18n::tr("中国广电"));
            assert_eq!(label("Example Mobile {0}"), "Example Mobile {0}");
            assert_eq!(label("CMCC-HK"), "CMCC-HK");
        }
        i18n::select(Locale::Simplified);
    }
}
