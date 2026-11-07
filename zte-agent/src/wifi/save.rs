//! Wi-Fi writes share validation, firmware application and readback. An
//! accepted HTTP request alone must never mean that a setting was saved.
use super::Wireless;
use crate::ubus;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

static WRITER: Mutex<()> = Mutex::new(());
type Failure = (u16, String);
fn failed(error: String) -> Failure {
    (503, error)
}

struct Change {
    path: String,
    value: String,
    firmware: bool,
}

fn scalar(key: &str, value: &Value) -> Result<String, Failure> {
    let text = match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => if *b { "1" } else { "0" }.into(),
        _ => return Err((400, format!("{key} must be a scalar value"))),
    };
    // Values are separate argv entries, never shell code. Do not strip
    // punctuation from SSIDs or passwords: that changes the user's secret.
    if text.chars().any(char::is_control) || text.len() > 256 {
        return Err((400, format!("{key} contains an unsupported value")));
    }
    Ok(text)
}

fn put(
    changes: &mut BTreeMap<String, Change>,
    current: &Wireless,
    path: String,
    value: String,
    firmware: bool,
) -> Result<(), Failure> {
    let section = path.rsplit_once('.').unwrap().0;
    if !current.0.contains_key(section) {
        return Err((
            422,
            format!("Wi-Fi section {section} is unavailable on this firmware"),
        ));
    }
    if let Some(previous) = changes.get(&path) {
        if previous.value != value {
            return Err((400, "Conflicting Wi-Fi settings".into()));
        }
    }
    changes.insert(
        path.clone(),
        Change {
            path,
            value,
            firmware,
        },
    );
    Ok(())
}

fn plan(body: &Value, current: &Wireless, guest: bool) -> Result<Vec<Change>, Failure> {
    let fields = body
        .as_object()
        .ok_or((400, "expected JSON object".into()))?;
    let mut changes = BTreeMap::new();
    for (key, input) in fields {
        let value = scalar(key, input)?;
        let mut paths = Vec::new();
        let mut firmware = false;
        if guest {
            let (option, bands): (&str, &[&str]) = match key.as_str() {
                "guest_ssid" => ("ssid", &["guest_2g", "guest_5g"]),
                "guest_key" => ("key", &["guest_2g", "guest_5g"]),
                "guest_encryption" => ("encryption", &["guest_2g", "guest_5g"]),
                "guest_disabled" => ("disabled", &["guest_2g", "guest_5g"]),
                "guest_disabled_2g" => ("disabled", &["guest_2g"]),
                "guest_disabled_5g" => ("disabled", &["guest_5g"]),
                "guest_hidden" => ("hidden", &["guest_2g", "guest_5g"]),
                "guest_isolate" => ("isolate", &["guest_2g", "guest_5g"]),
                "guest_active_time" => ("guest_active_time", &["guest_2g", "guest_5g"]),
                _ => return Err((400, "Unknown guest Wi-Fi setting".into())),
            };
            firmware = matches!(option, "disabled" | "guest_active_time");
            for band in bands {
                paths.push(format!("wireless.{band}.{option}"));
            }
        } else if matches!(key.as_str(), "wifi_onoff" | "wifi6_switch") {
            let path = current
                .mbb_path(key)
                .ok_or((422, "Wi-Fi master settings are unavailable".into()))?;
            firmware = path.starts_with("wireless.");
            paths.push(path);
        } else if matches!(key.as_str(), "radio2_disabled" | "radio5_disabled") {
            let radio = if key == "radio2_disabled" {
                "wifi0"
            } else {
                "wifi1"
            };
            paths.push(format!("wireless.{radio}.disabled"));
        } else if let Some((option, band @ ("2g" | "5g"))) = key.rsplit_once('_') {
            let radio = if band == "2g" { "wifi0" } else { "wifi1" };
            match option {
                "ssid" | "key" | "encryption" | "hidden" => {
                    paths.push(format!("wireless.main_{band}.{option}"))
                }
                "channel" | "htmode" | "txpower" => {
                    let option = if option == "txpower" {
                        "txpowerpercent"
                    } else {
                        option
                    };
                    paths.push(format!("wireless.{radio}.{option}"));
                    firmware = true;
                }
                _ => return Err((400, "Unknown Wi-Fi setting".into())),
            }
        } else {
            return Err((400, "Unknown Wi-Fi setting".into()));
        }
        for path in paths {
            let option = path.rsplit_once('.').unwrap().1;
            let valid = match option {
                "disabled" | "hidden" | "isolate" | "wifi_onoff" | "wifi6_switch" => {
                    matches!(value.as_str(), "0" | "1")
                }
                "ssid" => !value.is_empty() && value.len() <= 32,
                "key" => {
                    value.is_empty()
                        || (8..=63).contains(&value.len()) && value.is_ascii()
                        || value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
                }
                "txpowerpercent" => value.parse::<u8>().is_ok_and(|v| (1..=100).contains(&v)),
                "channel" => {
                    value == "auto"
                        || value == "0"
                        || value.parse::<u16>().is_ok_and(|v| {
                            if path.contains("wifi0") {
                                (1..=14).contains(&v)
                            } else {
                                (36..=144).contains(&v) && v % 4 == 0
                                    || [149, 153, 157, 161, 165].contains(&v)
                            }
                        })
                }
                "htmode" => {
                    value == "auto"
                        || ["EHT", "HE", "VHT", "HT"].iter().any(|prefix| {
                            value.strip_prefix(prefix).is_some_and(|width| {
                                matches!(width, "20" | "40")
                                    || !path.contains("wifi0") && matches!(width, "80" | "160")
                            })
                        })
                }
                "guest_active_time" => value.parse::<u32>().is_ok_and(|v| v <= 1440),
                // The firmware supports additional auth suites; preserve
                // existing clients rather than narrowing them to UI choices.
                "encryption" => !value.is_empty(),
                _ => false,
            };
            if !valid {
                return Err((400, format!("Invalid value for {key}")));
            }
            let stored = if option == "guest_active_time" {
                // Public clients use minutes; the firmware stores seconds.
                (value.parse::<u32>().unwrap() * 60).to_string()
            } else {
                value.clone()
            };
            put(&mut changes, current, path, stored, firmware)?;
        }
    }

    if !guest {
        if let Some(input) = fields.get("wifi6_switch") {
            let enabled = scalar("wifi6_switch", input)? == "1";
            if !enabled && current.get("zte_mbb.mlo_enable") == "1" {
                return Err((
                    409,
                    "Disable MLO before selecting Wi-Fi compatibility mode".into(),
                ));
            }
            for (radio, modern, legacy) in [("wifi0", "11beg", "11ng"), ("wifi1", "11bea", "11ac")]
            {
                let path = format!("wireless.{radio}.htmode");
                let width = changes
                    .get(&path)
                    .map(|c| c.value.clone())
                    .unwrap_or_else(|| current.get(&format!("{radio}.htmode")));
                let width = standard_width(&width, enabled, radio == "wifi0")?;
                // Width conversion is intentional when changing the standard.
                changes.remove(&path);
                put(&mut changes, current, path, width, true)?;
                put(
                    &mut changes,
                    current,
                    format!("wireless.{radio}.hwmode"),
                    if enabled { modern } else { legacy }.into(),
                    true,
                )?;
            }
        }
    }
    Ok(changes.into_values().collect())
}

fn standard_width(width: &str, modern: bool, band2: bool) -> Result<String, Failure> {
    if width == "auto" {
        return Ok(width.into());
    }
    let number = width.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    if !matches!(number, "20" | "40") && (band2 || !matches!(number, "80" | "160")) {
        return Err((
            422,
            "Cannot determine the current Wi-Fi channel width".into(),
        ));
    }
    Ok(format!(
        "{}{number}",
        if modern {
            "EHT"
        } else if band2 {
            "HT"
        } else {
            "VHT"
        }
    ))
}

trait Backend {
    fn read(&mut self) -> Result<Wireless, String>;
    fn set(&mut self, path: &str, value: &str) -> Result<(), String>;
    fn commit(&mut self, package: &str) -> Result<(), String>;
    fn apply(&mut self, firmware: &Value) -> Result<(), String>;
    fn ready(&mut self) -> Result<bool, String>;
}

struct Device;
impl Backend for Device {
    fn read(&mut self) -> Result<Wireless, String> {
        Wireless::load()
    }
    fn set(&mut self, path: &str, value: &str) -> Result<(), String> {
        ubus::uci_set_no_commit(path, value)
    }
    fn commit(&mut self, package: &str) -> Result<(), String> {
        ubus::uci_commit(package)
    }
    fn apply(&mut self, firmware: &Value) -> Result<(), String> {
        let method = if firmware.as_object().is_some_and(Map::is_empty) {
            "reload"
        } else {
            "set"
        };
        let reply = ubus::call_bounded("zwrt_wlan", method, Some(&firmware.to_string()), 5)?;
        if reply["error"].as_str().is_some_and(|e| !e.is_empty())
            || matches!(reply["result"].as_str(), Some("failure" | "fail" | "error"))
        {
            return Err("The firmware rejected the Wi-Fi change".into());
        }
        Ok(())
    }
    fn ready(&mut self) -> Result<bool, String> {
        let report = ubus::call_bounded("zwrt_wlan", "report", Some("{}"), 1)?;
        Ok(report["load_status"].as_str() == Some("idle"))
    }
}

fn save(
    backend: &mut impl Backend,
    body: &Value,
    guest: bool,
    settle: Duration,
    wait: Duration,
) -> Result<Value, Failure> {
    let current = backend.read().map_err(failed)?;
    let changes = plan(body, &current, guest)?;
    let changed: Vec<_> = changes
        .iter()
        .filter(|c| current.0.get(&c.path) != Some(&c.value))
        .collect();
    if changed.is_empty() {
        return Ok(json!({"status":"ok", "note":"no changes"}));
    }
    let mut firmware = json!({});
    let mut packages = std::collections::BTreeSet::new();
    for change in changed {
        if change.firmware {
            let mut components = change.path.split('.');
            components.next();
            let section = components.next().unwrap();
            let option = components.next().unwrap();
            if firmware.get(section).is_none() {
                firmware[section] = json!({});
            }
            firmware[section][option] = json!(change.value);
        } else {
            backend.set(&change.path, &change.value).map_err(failed)?;
            packages.insert(change.path.split('.').next().unwrap());
        }
    }
    for package in packages {
        backend.commit(package).map_err(failed)?;
    }
    backend.apply(&firmware).map_err(failed)?;
    // The firmware queues a reload. Allow it to start before treating idle
    // plus a UCI read as confirmation. Poll reads only; never replay a write.
    std::thread::sleep(settle);
    let deadline = Instant::now() + wait;
    loop {
        if backend.ready().map_err(failed)? {
            let actual = backend.read().map_err(failed)?;
            if changes
                .iter()
                .all(|c| actual.0.get(&c.path) == Some(&c.value))
            {
                return Ok(json!({"status":"ok", "verified":true}));
            }
        }
        if Instant::now() >= deadline {
            return Err((503, "Wi-Fi settings were not confirmed after applying. Reconnect and check the actual state before retrying.".into()));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

pub(super) fn settings(body: &[u8], guest: bool) -> (u16, Value) {
    let body: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return (400, json!({"ok":false, "error":"invalid JSON"})),
    };
    let _guard = match WRITER.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
        Err(_) => {
            return (
                409,
                json!({"ok":false, "error":"Another Wi-Fi change is in progress"}),
            )
        }
    };
    match save(
        &mut Device,
        &body,
        guest,
        Duration::from_secs(2),
        Duration::from_secs(10),
    ) {
        Ok(data) => (200, json!({"ok":true, "data":data})),
        Err((status, error)) => (status, json!({"ok":false, "error":error})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = "wireless.zte_mbb=zte_mbb\nwireless.zte_mbb.wifi_onoff='1'\nwireless.wifi0=wifi-device\nwireless.wifi0.hwmode='11beg'\nwireless.wifi0.htmode='EHT40'\nwireless.wifi0.txpowerpercent='100'\nwireless.wifi1=wifi-device\nwireless.wifi1.hwmode='11bea'\nwireless.wifi1.htmode='EHT160'\nwireless.wifi1.txpowerpercent='100'\nwireless.main_2g=wifi-iface\nwireless.main_2g.ssid='Example'\nwireless.main_5g=wifi-iface\nwireless.guest_2g=wifi-iface\nwireless.guest_5g=wifi-iface\n";

    struct Fake {
        config: Wireless,
        calls: Vec<String>,
        fail: &'static str,
        ignore: &'static str,
    }
    impl Default for Fake {
        fn default() -> Self {
            Self {
                config: Wireless::from_show(CONFIG),
                calls: Vec::new(),
                fail: "",
                ignore: "",
            }
        }
    }
    impl Backend for Fake {
        fn read(&mut self) -> Result<Wireless, String> {
            if self.fail == "read" {
                return Err("config unavailable".into());
            }
            Ok(Wireless(self.config.0.clone()))
        }
        fn set(&mut self, path: &str, value: &str) -> Result<(), String> {
            self.calls.push(format!("set {path}"));
            if self.fail == "set" {
                return Err("setting refused".into());
            }
            if self.ignore != path {
                self.config.0.insert(path.into(), value.into());
            }
            Ok(())
        }
        fn commit(&mut self, package: &str) -> Result<(), String> {
            self.calls.push(format!("commit {package}"));
            if self.fail == "commit" {
                Err("commit refused".into())
            } else {
                Ok(())
            }
        }
        fn apply(&mut self, firmware: &Value) -> Result<(), String> {
            self.calls.push("apply".into());
            if self.fail == "apply" {
                return Err("reload refused".into());
            }
            for (section, values) in firmware.as_object().unwrap() {
                for (option, value) in values.as_object().unwrap() {
                    let path = format!("wireless.{section}.{option}");
                    if self.ignore != path {
                        self.config.0.insert(path, value.as_str().unwrap().into());
                    }
                }
            }
            Ok(())
        }
        fn ready(&mut self) -> Result<bool, String> {
            Ok(self.fail != "busy")
        }
    }
    fn run(fake: &mut Fake, body: Value, guest: bool) -> Result<Value, Failure> {
        save(fake, &body, guest, Duration::ZERO, Duration::ZERO)
    }

    #[test]
    fn omitted_switch_uses_real_modes_and_saves_in_existing_section() {
        let mut fake = Fake::default();
        assert_eq!(fake.config.wifi_standard(), Some("1"));
        assert!(!fake.config.0.contains_key("wireless.zte_mbb.wifi6_switch"));
        assert_eq!(
            run(&mut fake, json!({"wifi6_switch":"1"}), false).unwrap()["verified"],
            true
        );
        assert_eq!(fake.config.mbb("wifi6_switch"), "1");
        assert_eq!(fake.calls, ["apply"]);
        assert_eq!(fake.config.get("wifi0.hwmode"), "11beg");
        assert_eq!(fake.config.get("wifi1.hwmode"), "11bea");
        fake.calls.clear();
        assert_eq!(
            run(&mut fake, json!({"wifi6_switch":"1"}), false).unwrap()["note"],
            "no changes"
        );
        assert!(fake.calls.is_empty());
    }

    #[test]
    fn standard_changes_both_radios_and_preserves_channel_width() {
        let mut fake = Fake::default();
        run(&mut fake, json!({"wifi6_switch":"0"}), false).unwrap();
        assert_eq!(fake.config.get("wifi0.hwmode"), "11ng");
        assert_eq!(fake.config.get("wifi1.hwmode"), "11ac");
        assert_eq!(fake.config.get("wifi0.htmode"), "HT40");
        assert_eq!(fake.config.get("wifi1.htmode"), "VHT160");
        run(&mut fake, json!({"wifi6_switch":"1"}), false).unwrap();
        assert_eq!(fake.config.get("wifi0.hwmode"), "11beg");
        assert_eq!(fake.config.get("wifi1.hwmode"), "11bea");
        assert_eq!(fake.config.get("wifi1.htmode"), "EHT160");
    }

    #[test]
    fn unknown_or_mixed_radio_modes_do_not_default_to_disabled() {
        let mut config = Wireless::from_show("wireless.zte_mbb=zte_mbb\n");
        assert_eq!(config.wifi_standard(), None);
        config
            .0
            .insert("wireless.wifi0.hwmode".into(), "11beg".into());
        config
            .0
            .insert("wireless.wifi1.hwmode".into(), "11ac".into());
        assert_eq!(config.wifi_standard(), None);
        config
            .0
            .insert("wireless.wifi0.hwmode".into(), "11ng".into());
        assert_eq!(config.wifi_standard(), Some("0"));
    }

    #[test]
    fn passwords_and_ssids_keep_literal_punctuation_on_both_paths() {
        let secret = "a'\"$`\\|<>;&12345";
        let mut fake = Fake::default();
        run(
            &mut fake,
            json!({"ssid_2g":"Guest's & Wi-Fi", "key_2g":secret}),
            false,
        )
        .unwrap();
        assert_eq!(fake.config.get("main_2g.key"), secret);
        assert_eq!(fake.config.get("main_2g.ssid"), "Guest's & Wi-Fi");
        run(&mut fake, json!({"guest_key":secret}), true).unwrap();
        assert_eq!(fake.config.get("guest_2g.key"), secret);
        assert_eq!(fake.config.get("guest_5g.key"), secret);
    }

    #[test]
    fn power_changes_use_the_firmware_percentage_interface() {
        let mut fake = Fake::default();
        run(&mut fake, json!({"txpower_2g":80, "txpower_5g":40}), false).unwrap();
        assert_eq!(fake.calls, ["apply"]);
        assert_eq!(fake.config.get("wifi0.txpowerpercent"), "80");
        assert_eq!(fake.config.get("wifi1.txpowerpercent"), "40");
    }

    #[test]
    fn guest_minutes_are_persisted_as_seconds_on_both_radios() {
        let mut fake = Fake::default();
        run(&mut fake, json!({"guest_active_time":"60"}), true).unwrap();
        assert_eq!(fake.calls, ["apply"]);
        assert_eq!(fake.config.get("guest_2g.guest_active_time"), "3600");
        assert_eq!(fake.config.get("guest_5g.guest_active_time"), "3600");
        run(&mut fake, json!({"guest_active_time":"0"}), true).unwrap();
        assert_eq!(fake.config.get("guest_2g.guest_active_time"), "0");
    }

    #[test]
    fn validates_all_targets_before_any_write() {
        for (body, guest) in [
            (json!({"ssid_2g":"valid", "wifi6_switch":{}}), false),
            (json!({"ssid_2g":"valid", "unknown_setting":"1"}), false),
            (json!({"ssid_2g":"valid", "txpower_5g":101}), false),
            (json!({"guest_ssid":"valid"}), true),
        ] {
            let mut fake = Fake::default();
            fake.config.0.remove("wireless.guest_5g");
            assert!(run(&mut fake, body, guest).is_err());
            assert!(fake.calls.is_empty());
        }
    }

    #[test]
    fn set_commit_apply_and_verification_errors_never_report_success() {
        for fail in ["read", "set", "commit", "apply", "busy"] {
            let mut fake = Fake {
                fail,
                ..Fake::default()
            };
            let error = run(&mut fake, json!({"ssid_2g":"Changed"}), false).unwrap_err();
            assert_eq!(error.0, 503);
            assert!(fake.calls.iter().filter(|c| *c == "apply").count() <= 1);
        }
        for (body, ignored, guest) in [
            (
                json!({"wifi6_switch":"1"}),
                "wireless.zte_mbb.wifi6_switch",
                false,
            ),
            (
                json!({"key_2g":"New-Example-Only"}),
                "wireless.main_2g.key",
                false,
            ),
            (
                json!({"guest_ssid":"Guest"}),
                "wireless.guest_5g.ssid",
                true,
            ),
        ] {
            let mut fake = Fake {
                ignore: ignored,
                ..Fake::default()
            };
            assert_eq!(run(&mut fake, body, guest).unwrap_err().0, 503);
            assert_eq!(fake.calls.iter().filter(|c| *c == "apply").count(), 1);
        }
    }
}
