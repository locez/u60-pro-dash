//! Appearance only: shared by the agent and the native screen. Credentials and
//! screen-lock secrets are kept in separate files and cannot be changed here.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::Path,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Home {
    pub show_radio: bool,
    pub show_wifi: bool,
    pub show_services: bool,
    pub show_tailscale: bool,
    pub show_wireguard: bool,
    pub show_mihomo: bool,
    pub metric_1: String,
    pub metric_2: String,
    pub metric_3: String,
}
impl Default for Home {
    fn default() -> Self {
        Self {
            show_radio: true,
            show_wifi: true,
            show_services: true,
            show_tailscale: true,
            show_wireguard: true,
            show_mihomo: true,
            metric_1: "clients".into(),
            metric_2: "month".into(),
            metric_3: "temperature".into(),
        }
    }
}
pub fn defaults() -> Value {
    json!({"language":"zh-Hans","dark":true,"follow_system":false,"accent":"#2563eb",
        "brightness":70,"sleep_seconds":60,"probe_target":"","home":Home::default()})
}
fn merge(value: &mut Value, patch: &Value) -> Result<(), String> {
    let fields = patch.as_object().ok_or("Preferences must be an object")?;
    for (key, changed) in fields {
        let current = value
            .get_mut(key)
            .ok_or_else(|| format!("Unknown preference: {key}"))?;
        if current.is_object() {
            merge(current, changed)?;
        } else {
            *current = changed.clone();
        }
    }
    Ok(())
}
pub fn validate(value: &Value) -> Result<(), String> {
    if !matches!(
        value["language"].as_str(),
        Some("zh-Hans" | "zh-Hant" | "en" | "ja" | "ko")
    ) {
        return Err("Unsupported screen language".into());
    }
    for key in ["dark", "follow_system"] {
        if !value[key].is_boolean() {
            return Err(format!("Invalid {key}"));
        }
    }
    let color = value["accent"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('#');
    if color.len() != 6 || !color.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Accent must be #RRGGBB".into());
    }
    if !value["brightness"]
        .as_u64()
        .is_some_and(|v| (10..=100).contains(&v))
    {
        return Err("Brightness must be 10–100%".into());
    }
    if !value["sleep_seconds"]
        .as_u64()
        .is_some_and(|v| [0, 15, 30, 60, 120].contains(&v))
    {
        return Err("Unsupported screen timeout".into());
    }
    let target = value["probe_target"]
        .as_str()
        .ok_or("Invalid probe target")?;
    if !target.is_empty() {
        let addr: std::net::SocketAddr = target
            .parse()
            .map_err(|_| "Use an IP address and TCP port")?;
        let ip = addr.ip().to_canonical();
        if addr.port() == 0
            || ip.is_unspecified()
            || ip.is_loopback()
            || ip.is_multicast()
            || matches!(ip,std::net::IpAddr::V4(v) if v.is_broadcast())
        {
            return Err("Invalid probe destination".into());
        }
    }
    let home: Home =
        serde_json::from_value(value["home"].clone()).map_err(|_| "Invalid home layout")?;
    for metric in [&home.metric_1, &home.metric_2, &home.metric_3] {
        if ![
            "none",
            "clients",
            "month",
            "temperature",
            "cpu",
            "memory",
            "uptime",
            "battery",
        ]
        .contains(&metric.as_str())
        {
            return Err("Unsupported home metric".into());
        }
    }
    Ok(())
}
pub fn read(path: &Path) -> Result<Value, String> {
    let mut value = defaults();
    let mut file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(value),
        Err(_) => return Err("Cannot read screen preferences".into()),
    };
    if !file
        .metadata()
        .map_err(|_| "Cannot read preferences metadata")?
        .is_file()
    {
        return Err("Invalid preference file".into());
    }
    let mut bytes = Vec::new();
    Read::take(&mut file, 16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read preferences")?;
    if bytes.len() > 16 * 1024 {
        return Err("Preference file is too large".into());
    }
    let stored: Value = serde_json::from_slice(&bytes).map_err(|_| "Invalid preference file")?;
    merge(&mut value, &stored)?;
    validate(&value)?;
    Ok(value)
}
pub fn patch(path: &Path, patch: &Value) -> Result<Value, String> {
    let parent = path.parent().ok_or("Invalid preference path")?;
    fs::create_dir_all(parent).map_err(|_| "Cannot create preference directory")?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path.with_extension("lock"))
        .map_err(|_| "Cannot lock screen preferences")?;
    // Closing the descriptor releases flock, including all error paths.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(
            "Screen preferences are being changed; retry after the current operation".into(),
        );
    }
    let mut value = read(path)?;
    let previous = value.clone();
    merge(&mut value, patch)?;
    let changed: Vec<_> = ["metric_1", "metric_2", "metric_3"]
        .into_iter()
        .filter(|key| patch["home"].get(key).is_some())
        .collect();
    if changed.len() == 1 {
        let key = changed[0];
        let target = value["home"][key].clone();
        if target != "none" {
            for other in ["metric_1", "metric_2", "metric_3"] {
                if other != key && value["home"][other] == target {
                    value["home"][other] = previous["home"][key].clone();
                    break;
                }
            }
        }
    }
    validate(&value)?;
    let tmp = path.with_extension(format!("{}.new", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|_| "Cannot stage screen preferences")?;
    let result = (|| {
        file.write_all(
            &serde_json::to_vec_pretty(&value).map_err(|_| "Cannot encode preferences")?,
        )
        .map_err(|_| "Cannot write preferences")?;
        file.sync_all().map_err(|_| "Cannot persist preferences")?;
        fs::rename(&tmp, path).map_err(|_| "Cannot replace preferences")?;
        fs::File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|_| "Cannot sync preference directory")?;
        Ok(value)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
/// Apply only leaves changed by this editor. Concurrent changes to other fields
/// from the browser/screen are preserved under the file lock.
pub fn diff(before: &Value, after: &Value) -> Value {
    let mut result = serde_json::Map::new();
    if let Some(fields) = after.as_object() {
        for (key, value) in fields {
            if before.get(key) != Some(value) {
                result.insert(
                    key.clone(),
                    if value.is_object() {
                        diff(&before[key], value)
                    } else {
                        value.clone()
                    },
                );
            }
        }
    }
    Value::Object(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_editors_preserve_other_fields_and_reject_invalid_or_secret_changes() {
        let dir = std::env::temp_dir().join(format!("u60-pref-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("appearance.json");
        let old = read(&path).unwrap();
        patch(&path, &json!({"home":{"show_services":false}})).unwrap();
        let mut native = old.clone();
        native["brightness"] = json!(90);
        let saved = patch(&path, &diff(&old, &native)).unwrap();
        assert_eq!(saved["brightness"], 90);
        assert_eq!(saved["home"]["show_services"], false);
        for bad in [
            json!({"home":{"metric_1":"bad"}}),
            json!({"brightness":2}),
            json!({"pin":"123456"}),
            json!({"home":{"show_wifi":1}}),
        ] {
            assert!(patch(&path, &bad).is_err());
        }
        assert_eq!(read(&path).unwrap(), saved);
        let swapped = patch(&path, &json!({"home":{"metric_1":"month"}})).unwrap();
        assert_eq!(swapped["home"]["metric_1"], "month");
        assert_eq!(swapped["home"]["metric_2"], "clients");
        let hidden = patch(
            &path,
            &json!({"home":{"metric_2":"none","metric_3":"none"}}),
        )
        .unwrap();
        assert_eq!(hidden["home"]["metric_1"], "month");
        fs::remove_dir_all(dir).unwrap();
    }
}
