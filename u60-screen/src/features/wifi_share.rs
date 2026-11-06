//! Join codes use saved hotspot credentials, never an unsaved settings form.
use super::*;

const NETWORKS: [(&str, &str); 3] = [
    ("2g", "2.4 GHz 热点"),
    ("5g", "5 GHz 热点"),
    ("guest", "访客网络"),
];

pub fn reads(id: &str) -> Option<Vec<Read>> {
    match id {
        "wifi-home" => Some(vec![Read::get("wifi", "/api/wifi/status")]),
        "wifi-share" => Some(vec![
            Read::get("wifi", "/api/wifi/status"),
            Read::get("guest", "/api/wifi/guest"),
        ]),
        _ => None,
    }
}

#[derive(PartialEq, Eq)]
struct Network<'a> {
    ssid: &'a str,
    password: &'a str,
    auth: &'static str,
    hidden: bool,
}

fn network<'a>(readings: &'a Data, id: &str) -> Result<Network<'a>, &'static str> {
    if id == "dual" {
        let a = network(readings, "2g")?;
        let b = network(readings, "5g")?;
        if shared(readings, &a, &b) {
            return Ok(a);
        }
        return Err(crate::i18n::tr("双频设置已变化，请重新选择热点。"));
    }
    let wifi = data(readings, "wifi");
    match flag(&wifi["wifi_onoff"]) {
        Some(true) => {}
        Some(false) => return Err(crate::i18n::tr("Wi-Fi 已关闭，请开启后刷新。")),
        None => return Err(crate::i18n::tr("尚未读取到 Wi-Fi 状态，请刷新。")),
    }
    let (ssid, password, encryption, hidden, active) = match id {
        "2g" | "5g" => (
            &wifi[format!("ssid_{id}")],
            &wifi[format!("key_{id}")],
            &wifi[format!("encryption_{id}")],
            &wifi[format!("hidden_{id}")],
            flag(
                &wifi[if id == "2g" {
                    "radio2_disabled"
                } else {
                    "radio5_disabled"
                }],
            ) == Some(false),
        ),
        "guest" => {
            let guest = data(readings, "guest");
            let active = [
                ("radio2_disabled", "disabled_2g"),
                ("radio5_disabled", "disabled_5g"),
            ]
            .iter()
            .any(|(radio, ssid)| {
                flag(&wifi[*radio]) == Some(false) && flag(&guest[*ssid]) == Some(false)
            });
            (
                &guest["ssid"],
                &guest["key"],
                &guest["encryption"],
                &guest["hidden"],
                active,
            )
        }
        _ => return Err(crate::i18n::tr("请选择要分享的热点。")),
    };
    if !active {
        return Err(crate::i18n::tr(
            "热点未开启或状态不可读，请检查热点设置。\n开启后刷新即可分享。",
        ));
    }
    let ssid = ssid
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 32 && !s.chars().any(char::is_control))
        .ok_or(crate::i18n::tr("未读取到有效热点名称，请刷新。"))?;
    let encryption = encryption
        .as_str()
        .ok_or(crate::i18n::tr("未读取到加密方式，请刷新。"))?
        .to_ascii_lowercase();
    // Transition mode accepts WPA2: the widely supported WPA QR label lets
    // older scanners join it, while pure WPA3 must explicitly use SAE.
    let auth = if encryption == "none" {
        "nopass"
    } else if encryption.contains("sae") {
        if encryption.contains("mixed") || encryption.contains("psk") {
            "WPA"
        } else {
            "SAE"
        }
    } else if encryption.starts_with("psk") {
        "WPA"
    } else {
        return Err(crate::i18n::tr("当前加密方式暂不支持扫码分享。"));
    };
    let password = if auth == "nopass" {
        ""
    } else {
        password
            .as_str()
            .filter(|p| !p.is_empty() && p.len() <= 64 && !p.chars().any(char::is_control))
            .ok_or(crate::i18n::tr("未读取到热点密码，请刷新。"))?
    };
    // An explicitly empty UCI hidden option means the default (broadcast).
    // A missing API field is unknown, not an invented visible-network state.
    let hidden = if hidden.as_str() == Some("") {
        false
    } else {
        flag(hidden).ok_or(crate::i18n::tr("未读取到隐藏热点设置，请刷新。"))?
    };
    Ok(Network {
        ssid,
        password,
        auth,
        hidden,
    })
}

fn shared(readings: &Data, a: &Network<'_>, b: &Network<'_>) -> bool {
    a == b && data(readings, "wifi")["encryption_2g"] == data(readings, "wifi")["encryption_5g"]
}

pub(crate) struct HomeJoin {
    pub ssid: String,
    pub password: zeroize::Zeroizing<String>,
    pub payload: zeroize::Zeroizing<String>,
    pub band: &'static str,
    pub selection: &'static str,
    pub switchable: bool,
    pub protected: bool,
    pub qr_version: u8,
}
/// One credential set identifies a network; Wi-Fi QR has no frequency field.
/// Never pretend different names/passwords/security settings are one network.
pub(crate) fn home(readings: &Data, preferred: &str) -> Result<HomeJoin, &'static str> {
    let (a, b) = (network(readings, "2g"), network(readings, "5g"));
    let both = a.is_ok() && b.is_ok();
    let merged = matches!((&a,&b),(Ok(a),Ok(b)) if shared(readings,a,b));
    // Match QR density across bands. Text length may change the required QR
    // version, but switching bands must not change its apparent footprint.
    let qr_version = [&a, &b]
        .into_iter()
        .filter_map(|n| n.as_ref().ok())
        .filter_map(|n| super::qr_version(&n.payload()))
        .max()
        .unwrap_or(1);
    let (network, selection, band) = if merged {
        (a?, "dual", "2.4 + 5 GHz")
    } else if (preferred == "5g" && b.is_ok()) || a.is_err() && b.is_ok() {
        (b?, "5g", "5 GHz")
    } else {
        (a?, "2g", "2.4 GHz")
    };
    Ok(HomeJoin {
        ssid: network.ssid.into(),
        password: zeroize::Zeroizing::new(network.password.into()),
        payload: zeroize::Zeroizing::new(network.payload()),
        band,
        selection,
        switchable: both && !merged,
        protected: network.auth != "nopass",
        qr_version,
    })
}

fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '\\' | ';' | ',' | ':' | '"') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

impl Network<'_> {
    fn payload(&self) -> String {
        format!(
            "WIFI:T:{};S:{};P:{};H:{};;",
            self.auth,
            escape(self.ssid),
            escape(self.password),
            self.hidden
        )
    }
}

pub fn page(readings: &Data) -> Page {
    let mut page = Page::default();
    let Some(id) = selected(readings) else {
        page.content = crate::i18n::tr("选择热点，用手机扫码连接。").into();
        let mut networks = vec![];
        if network(readings, "dual").is_ok() {
            networks.push(("dual", "双频合一热点"));
        } else {
            networks.extend_from_slice(&NETWORKS[..2]);
        }
        networks.push(NETWORKS[2]);
        page.items = networks
            .iter()
            .map(|(id, label)| {
                let subtitle = match network(readings, id) {
                    Ok(network) => network.ssid.to_owned(),
                    Err(message) => message.lines().next().unwrap_or(message).to_owned(),
                };
                Item {
                    id: (*id).into(),
                    title: crate::i18n::tr(label).into(),
                    subtitle,
                }
            })
            .collect();
        return page;
    };
    page.title = if id == "dual" {
        Some(crate::i18n::tr("双频合一热点").into())
    } else {
        NETWORKS
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, name)| crate::i18n::tr(name).into())
    };
    match network(readings, id) {
        Ok(network) => {
            page.qr_caption = network.ssid.into();
            page.qr = Some(network.payload());
            page.note = crate::i18n::tr("用手机相机或 WLAN 扫码连接，无需手动输入密码。").into();
            if network.auth == "SAE" {
                page.note.push_str(crate::i18n::tr(
                    "此热点使用 WPA3，需要手机支持 SAE 扫码连接。",
                ));
            }
        }
        Err(message) => page.content = message.into(),
    }
    page
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn readings() -> Data {
        Data::from([
            (
                "wifi".into(),
                Ok(json!({
                    "wifi_onoff":"1","radio2_disabled":"0","radio5_disabled":"0",
                    "ssid_2g":"Example 2G","ssid_5g":"Example 5G",
                    "key_2g":"example-long-passphrase","key_5g":"other-long-passphrase",
                    "encryption_2g":"sae-mixed","encryption_5g":"sae",
                    "hidden_2g":"0","hidden_5g":"1"
                })),
            ),
            (
                "guest".into(),
                Ok(
                    json!({"ssid":"Example Guest","key":"unused", "disabled_2g":"0", "disabled_5g":"1", "encryption":"none","hidden":""}),
                ),
            ),
        ])
    }
    #[test]
    fn overview_combines_only_identical_active_credentials_and_falls_back() {
        let mut d = readings();
        assert!(home(&d, "2g").unwrap().switchable);
        assert_eq!(home(&d, "5g").unwrap().selection, "5g");
        assert_eq!(
            home(&d, "2g").unwrap().qr_version,
            home(&d, "5g").unwrap().qr_version
        );
        let wifi = d.get_mut("wifi").unwrap().as_mut().unwrap();
        for key in ["ssid", "key", "encryption", "hidden"] {
            wifi[format!("{key}_5g")] = wifi[format!("{key}_2g")].clone();
        }
        let combined = home(&d, "5g").unwrap();
        assert_eq!(combined.selection, "dual");
        assert!(!combined.switchable);
        assert_eq!(page(&d).items.len(), 2);
        let original = d.clone();
        for (key, value) in [
            ("key_5g", json!("different-password")),
            ("encryption_5g", json!("psk2")),
            ("hidden_5g", json!("1")),
            ("ssid_5g", json!("Other network")),
        ] {
            let mut changed = original.clone();
            changed.get_mut("wifi").unwrap().as_mut().unwrap()[key] = value;
            assert!(home(&changed, "2g").unwrap().switchable, "{key}");
        }
        d.get_mut("wifi").unwrap().as_mut().unwrap()["radio2_disabled"] = json!("1");
        assert_eq!(home(&d, "2g").unwrap().selection, "5g");
        d.get_mut("wifi").unwrap().as_mut().unwrap()["wifi_onoff"] = json!("0");
        assert!(home(&d, "2g").is_err());
    }
    #[test]
    fn join_codes_escape_credentials_and_keep_security_and_hidden_flags() {
        let n = Network {
            ssid: "测试;Wi:Fi",
            password: "A\\b;c,d:e\"f#0123456789",
            auth: "WPA",
            hidden: true,
        };
        assert_eq!(
            n.payload(),
            "WIFI:T:WPA;S:测试\\;Wi\\:Fi;P:A\\\\b\\;c\\,d\\:e\\\"f#0123456789;H:true;;"
        );
        let d = readings();
        assert_eq!(
            network(&d, "2g").unwrap().payload(),
            "WIFI:T:WPA;S:Example 2G;P:example-long-passphrase;H:false;;"
        );
        assert!(
            network(&d, "5g")
                .unwrap()
                .payload()
                .starts_with("WIFI:T:SAE;")
        );
        assert_eq!(
            network(&d, "guest").unwrap().payload(),
            "WIFI:T:nopass;S:Example Guest;P:;H:false;;"
        );
    }
    #[test]
    fn unavailable_hotspots_never_offer_a_join_code() {
        let mut d = readings();
        d.insert("__selection".into(), Ok(json!("guest")));
        d.get_mut("guest").unwrap().as_mut().unwrap()["disabled_2g"] = json!("1");
        assert!(page(&d).qr.is_none());
        for (key, value) in [
            ("wifi_onoff", json!("0")),
            ("key_2g", json!("")),
            ("encryption_2g", json!("")),
            ("hidden_2g", Value::Null),
            ("radio2_disabled", Value::Null),
        ] {
            let mut d = readings();
            d.insert("__selection".into(), Ok(json!("2g")));
            d.get_mut("wifi").unwrap().as_mut().unwrap()[key] = value;
            assert!(page(&d).qr.is_none(), "invalid field {key}");
        }
        let mut d = readings();
        d.insert("__selection".into(), Ok(json!("2g")));
        d.insert("wifi".into(), Err(Error::Unavailable));
        assert!(page(&d).qr.is_none());
    }
    #[test]
    fn selection_uses_the_saved_band_and_refresh_replaces_the_code() {
        let mut d = readings();
        assert!(page(&d).qr.is_none());
        assert_eq!(page(&d).items.len(), 3);
        d.insert("__selection".into(), Ok(json!("5g")));
        let p = page(&d);
        assert_eq!(p.qr_caption, "Example 5G");
        assert!(
            p.qr.as_deref()
                .unwrap()
                .contains("P:other-long-passphrase;")
        );
        assert!(
            !p.content.contains("other-long-passphrase")
                && !p.note.contains("other-long-passphrase")
        );
        d.get_mut("wifi").unwrap().as_mut().unwrap()["key_5g"] = json!("updated-long-passphrase");
        assert!(
            page(&d)
                .qr
                .as_deref()
                .unwrap()
                .contains("P:updated-long-passphrase;")
        );
    }
}
