use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub language: crate::i18n::Locale,
    pub dark: bool,
    pub accent: String,
    pub brightness: u8,
    pub sleep_seconds: u16,
    pub probe_target: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: crate::i18n::Locale::default(),
            dark: true,
            accent: "#2563eb".into(),
            brightness: 70,
            sleep_seconds: 60,
            probe_target: String::new(),
        }
    }
}

impl Preferences {
    pub fn load(path: &Path) -> Result<Self> {
        let prefs: Self = serde_json::from_slice(&fs::read(path)?)?;
        prefs.validate()?;
        Ok(prefs)
    }

    pub fn validate(&self) -> Result<()> {
        parse_color(&self.accent)?;
        if !self.probe_target.is_empty() {
            crate::agent::connectivity::target(&self.probe_target)?;
        }
        ensure!(
            (10..=100).contains(&self.brightness),
            "brightness must be 10..100"
        );
        ensure!(
            [0, 15, 30, 60, 120].contains(&self.sleep_seconds),
            "unsupported sleep timeout"
        );
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        write_json_atomic(path, self)
    }
}

pub fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut created = false;
    let result: Result<()> = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        created = true;
        file.write_all(&serde_json::to_vec_pretty(value)?)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    })();
    // Only remove the temporary file if this process actually created it.
    // A leftover file causes create_new to fail; never follow a symlink.
    if result.is_err() && created {
        let _ = fs::remove_file(&tmp);
    }
    result.with_context(|| format!("save preferences to {}", path.display()))
}

pub fn parse_color(value: &str) -> Result<[u8; 3]> {
    let digits = value.strip_prefix('#').unwrap_or(value);
    ensure!(
        digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()),
        "accent must be #RRGGBB"
    );
    let n = u32::from_str_radix(digits, 16)?;
    Ok([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

pub fn readable_accent(rgb: [u8; 3], dark: bool) -> [u8; 3] {
    // The exact chosen color is kept for swatches; text gets >= 4.5:1 contrast.
    let bg = if dark { [24, 24, 24] } else { [255, 255, 255] };
    let mut color = rgb;
    for _ in 0..32 {
        if contrast(color, bg) >= 4.5 {
            break;
        }
        color = color.map(|v| {
            if dark {
                v.saturating_add(8)
            } else {
                v.saturating_sub(8)
            }
        });
    }
    color
}

fn contrast(a: [u8; 3], b: [u8; 3]) -> f32 {
    fn luminance(rgb: [u8; 3]) -> f32 {
        let c = rgb.map(|v| {
            let n = v as f32 / 255.;
            if n <= 0.04045 {
                n / 12.92
            } else {
                ((n + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
    }
    let x = luminance(a);
    let y = luminance(b);
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

pub fn button_ink(rgb: [u8; 3]) -> [u8; 3] {
    if contrast(rgb, [255, 255, 255]) >= contrast(rgb, [0, 0, 0]) {
        [255, 255, 255]
    } else {
        [0, 0, 0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_preferences_preserve_existing_settings() {
        let legacy = r##"{"dark":false,"accent":"#d97706","brightness":80,"sleep_seconds":120}"##;
        let mut prefs: Preferences = serde_json::from_str(legacy).unwrap();
        assert_eq!(prefs.language, crate::i18n::Locale::Simplified);
        for code in ["zh-Hans", "zh-Hant", "en", "ja", "ko"] {
            prefs.language = crate::i18n::Locale::parse(code).unwrap();
            let restored: Preferences =
                serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
            assert_eq!(restored.language.code(), code);
            assert!(!restored.dark);
            assert_eq!(restored.accent, "#d97706");
            assert_eq!(restored.brightness, 80);
            assert_eq!(restored.sleep_seconds, 120);
        }
        let unknown: Preferences =
            serde_json::from_str(r#"{"language":"future-locale","brightness":90}"#).unwrap();
        assert_eq!(unknown.language, crate::i18n::Locale::Simplified);
        assert_eq!(unknown.brightness, 90);
    }
    #[test]
    fn arbitrary_colors_remain_legible_in_both_themes() {
        for dark in [true, false] {
            for rgb in [
                [0, 0, 0],
                [255, 255, 255],
                [37, 99, 235],
                [255, 255, 0],
                [255, 0, 255],
            ] {
                let bg = if dark { [24, 24, 24] } else { [255, 255, 255] };
                assert!(contrast(readable_accent(rgb, dark), bg) >= 4.5);
                assert!(contrast(button_ink(rgb), rgb) >= 4.5);
            }
        }
        assert!(parse_color("#你好").is_err());
        assert!(parse_color("#12").is_err());
    }
}
