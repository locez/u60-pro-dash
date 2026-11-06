use super::*;
use serde_json::json;
const ON: &[(&str, &str)] = &[("0", "关闭"), ("1", "开启")];
const DISABLED: &[(&str, &str)] = &[("0", "开启"), ("1", "关闭")];
const ENC: &[(&str, &str)] = &[
    ("none", "开放网络"),
    ("psk+tkip", "WPA / TKIP"),
    ("psk+ccmp", "WPA / AES"),
    ("psk2+ccmp", "WPA2 / AES"),
    ("psk-mixed+ccmp", "WPA / WPA2"),
    ("sae", "WPA3"),
    ("sae-mixed", "WPA2 / WPA3"),
];
fn endpoint(id: &str) -> Option<&'static str> {
    Some(match id {
        "wifi2" | "wifi5" => "/api/wifi/status",
        "guest" => "/api/wifi/guest",
        "charge" => "/api/device/charge-control",
        "power-save" => "/api/device/power-save",
        "fast-boot" => "/api/device/fast-boot",
        "auto-sleep" => "/api/device/auto-sleep",
        "ttl" => "/api/router/ttl",
        "oos" => "/api/modem/oos-policy",
        "watchdog" => "/api/modem/conn-watchdog",
        "tailscale-config" => "/api/tailscale/status",
        _ => return None,
    })
}
pub fn reads(id: &str) -> Option<Vec<Read>> {
    let path = endpoint(id)?;
    Some(vec![if id == "power-save" {
        Read::post(
            "settings",
            path,
            json!({"deviceInfoList":["power_saver_mode"]}),
        )
    } else {
        Read::get("settings", path)
    }])
}
pub fn page(id: &str, readings: &Data) -> Page {
    let mut p = Page::default();
    let d = data(readings, "settings");
    match id {
        "wifi2" | "wifi5" => {
            let band = if id == "wifi2" { "2g" } else { "5g" };
            let radio = if id == "wifi2" {
                "radio2_disabled"
            } else {
                "radio5_disabled"
            };
            p.fields.push(Field::choice(
                "wifi_onoff",
                crate::i18n::tr("Wi-Fi 总开关"),
                &d["wifi_onoff"],
                ON,
            ));
            p.fields.push(Field::choice(
                radio,
                crate::i18n::tr("此频段热点"),
                &d[radio],
                DISABLED,
            ));
            p.fields.push(Field::choice(
                "wifi6_switch",
                "Wi-Fi 6",
                &d["wifi6_switch"],
                ON,
            ));
            for (key, label) in [
                ("ssid", crate::i18n::tr("网络名称")),
                ("encryption", crate::i18n::tr("安全方式")),
                ("key", crate::i18n::tr("新密码")),
                ("channel", crate::i18n::tr("信道")),
                ("htmode", crate::i18n::tr("带宽")),
                ("txpower", crate::i18n::tr("发射功率 %")),
                ("hidden", crate::i18n::tr("隐藏网络名称")),
            ] {
                let setting = key;
                let key = format!("{key}_{band}");
                let field = match setting {
                    "encryption" => Field::choice(&key, label, &d[&key], ENC),
                    "key" => Field::password(&key, label, d.is_object()),
                    "hidden" => Field::choice(&key, label, &d[&key], ON),
                    "txpower" => Field::number(&key, label, &d[&key], 1, 100),
                    "htmode" => Field::choice(
                        &key,
                        label,
                        &d[&key],
                        if band == "2g" {
                            &[("auto", "自动"), ("EHT20", "20 MHz"), ("EHT40", "40 MHz")]
                        } else {
                            &[
                                ("auto", "自动"),
                                ("EHT20", "20 MHz"),
                                ("EHT40", "40 MHz"),
                                ("EHT80", "80 MHz"),
                                ("EHT160", "160 MHz"),
                            ]
                        },
                    ),
                    "channel" => {
                        let channels = if band == "2g" {
                            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
                        } else {
                            vec![
                                36, 40, 44, 48, 52, 56, 60, 64, 100, 104, 108, 112, 116, 120, 124,
                                128, 132, 136, 140, 149, 153, 157, 161, 165,
                            ]
                        };
                        let current = if text(&d[&key]) == "0" {
                            json!("auto")
                        } else {
                            d[&key].clone()
                        };
                        let mut field = Field::choice(
                            &key,
                            label,
                            &current,
                            &[("auto", crate::i18n::tr("自动"))],
                        );
                        for channel in channels {
                            let value = channel.to_string();
                            if !field.choices.iter().any(|c| c.value == value) {
                                field.choices.push(Choice {
                                    label: value.clone(),
                                    value,
                                });
                            }
                        }
                        field
                    }
                    _ => {
                        let mut field = Field::text(&key, label, &d[&key]);
                        field.required = true;
                        field.max = 32;
                        field
                    }
                };
                p.fields.push(field);
            }
            p.row(
                crate::i18n::tr("实际信道"),
                &d[format!("actual_channel_{band}")],
            );
            p.row(crate::i18n::tr("连接数量"), &d[format!("clients_{band}")]);
            p.note = crate::i18n::tr("只保存修改过的字段。密码留空保留现有密码。").into();
        }
        "guest" => {
            for (key, label) in [
                ("disabled_2g", crate::i18n::tr("2.4 GHz 访客网络")),
                ("disabled_5g", crate::i18n::tr("5 GHz 访客网络")),
                ("ssid", crate::i18n::tr("网络名称")),
                ("encryption", crate::i18n::tr("安全方式")),
                ("key", crate::i18n::tr("新密码")),
                ("hidden", crate::i18n::tr("隐藏名称")),
                ("isolate", crate::i18n::tr("隔离访客")),
            ] {
                let wire = format!("guest_{key}");
                p.fields.push(match key {
                    "ssid" => {
                        let mut f = Field::text(&wire, label, &d[key]);
                        f.required = true;
                        f.max = 32;
                        f
                    }
                    "key" => Field::password(&wire, label, d.is_object()),
                    "encryption" => Field::choice(&wire, label, &d[key], ENC),
                    "disabled_2g" | "disabled_5g" => Field::choice(&wire, label, &d[key], DISABLED),
                    _ => Field::choice(&wire, label, &d[key], ON),
                });
            }
            p.fields.push(Field::choice(
                "guest_active_time",
                crate::i18n::tr("访客访问时限"),
                &d["guest_active_time"],
                &[
                    ("0", crate::i18n::tr("不限时")),
                    ("30", crate::i18n::tr("30 分钟")),
                    ("60", crate::i18n::tr("1 小时")),
                    ("120", crate::i18n::tr("2 小时")),
                    ("240", crate::i18n::tr("4 小时")),
                    ("480", crate::i18n::tr("8 小时")),
                    ("720", crate::i18n::tr("12 小时")),
                    ("1440", crate::i18n::tr("24 小时")),
                ],
            ));
            p.row(
                crate::i18n::tr("剩余访问时间 / 秒"),
                &d["remaining_seconds"],
            );
        }
        "charge" => {
            p.fields = vec![
                Field::boolean(
                    "charge_limit_enabled",
                    crate::i18n::tr("限制充电上限"),
                    &d["charge_limit_enabled"],
                ),
                Field::number(
                    "charge_limit",
                    crate::i18n::tr("停止充电电量 %"),
                    &d["charge_limit"],
                    50,
                    100,
                ),
                Field::number(
                    "hysteresis",
                    crate::i18n::tr("恢复充电回差 %"),
                    &d["hysteresis"],
                    1,
                    20,
                ),
                Field::boolean(
                    "charging_stopped",
                    crate::i18n::tr("直供电模式"),
                    &d["charging_stopped"],
                ),
            ];
            p.row(crate::i18n::tr("当前电量 %"), &d["capacity"]);
            p.row(crate::i18n::tr("电池状态"), &d["battery_status"]);
            p.note =
                crate::i18n::tr("直供电会停止电池充电，并覆盖自动充电策略；关闭后恢复充电上限管理。充电回差用于避免频繁启停。").into();
        }
        "power-save" => p.fields.push(Field::choice(
            "power_saver_mode",
            crate::i18n::tr("省电模式"),
            &d["power_saver_mode"],
            ON,
        )),
        "fast-boot" => p.fields.push(Field::choice(
            "fast_boot",
            crate::i18n::tr("快速启动"),
            &d["fast_boot"],
            ON,
        )),
        "auto-sleep" => {
            p.fields.push(Field::boolean(
                "enabled",
                crate::i18n::tr("整机自动休眠"),
                &d["enabled"],
            ));
            let mut f = Field::number(
                "timeout",
                crate::i18n::tr("无客户端等待时间 / 分钟"),
                &d["timeout"],
                1,
                1440,
            );
            f.wire = Wire::String;
            p.fields.push(f);
            p.row(crate::i18n::tr("休眠状态"), &d["status"]);
        }
        "ttl" => {
            p.fields = vec![
                Field::boolean("enabled", crate::i18n::tr("自定义跳数"), &d["enabled"]),
                Field::number("ipv4_ttl", "IPv4 TTL", &d["ipv4_ttl"], 1, 255),
                Field::number(
                    "ipv6_hop_limit",
                    "IPv6 Hop Limit",
                    &d["ipv6_hop_limit"],
                    1,
                    255,
                ),
            ];
            p.row(crate::i18n::tr("规则是否生效"), &d["active"]);
            p.note = crate::i18n::tr("仅作用于蜂窝软件转发路径，硬件加速流量可能绕过。").into();
        }
        "oos" => {
            p.fields = vec![
                Field::boolean("enabled", crate::i18n::tr("无服务省电"), &d["enabled"]),
                Field::number(
                    "grace_min",
                    crate::i18n::tr("等待时间 / 分钟"),
                    &d["grace_min"],
                    1,
                    240,
                ),
                Field::number(
                    "probe_every_min",
                    crate::i18n::tr("搜索间隔 / 分钟"),
                    &d["probe_every_min"],
                    1,
                    240,
                ),
                Field::number(
                    "probe_window_min",
                    crate::i18n::tr("搜索窗口 / 分钟"),
                    &d["probe_window_min"],
                    1,
                    30,
                ),
            ];
            p.row(crate::i18n::tr("策略状态"), &d["status"]["state"]);
        }
        "watchdog" => {
            p.fields = vec![
                Field::boolean("enabled", crate::i18n::tr("断网自动恢复"), &d["enabled"]),
                Field::boolean(
                    "allow_reboot",
                    crate::i18n::tr("允许最终重启恢复"),
                    &d["allow_reboot"],
                ),
            ];
            p.row(crate::i18n::tr("探测来源"), &d["status"]["probe_source"]);
            p.row(crate::i18n::tr("恢复状态"), &d["status"]["state"]);
        }
        "tailscale-config" => {
            p.fields = vec![
                Field::text("hostname", crate::i18n::tr("节点名称"), &d["hostname"]),
                Field::boolean(
                    "hold_wakelock",
                    crate::i18n::tr("保持设备唤醒"),
                    &d["hold_wakelock"],
                ),
                Field::boolean("tailscale_ssh", "Tailscale SSH", &d["tailscale_ssh"]),
                Field::boolean(
                    "accept_routes",
                    crate::i18n::tr("接收远端子网路由"),
                    &d["accept_routes"],
                ),
            ];
            if d["gateway_mode"] == true {
                p.fields.push(Field::boolean(
                    "accept_dns",
                    crate::i18n::tr("接收 Tailscale DNS"),
                    &d["accept_dns"],
                ));
                let mut exit = Field::text(
                    "exit_node",
                    crate::i18n::tr("退出节点 IP（留空关闭）"),
                    &d["exit_node"],
                );
                exit.max = 45;
                p.fields.push(exit);
            }
            let mut routes = Field::text(
                "advertise_routes",
                crate::i18n::tr("此设备发布的子网"),
                &d["advertise_routes"]
                    .as_array()
                    .map(|v| json!(v.iter().map(text).collect::<Vec<_>>().join(",")))
                    .unwrap_or(Value::Null),
            );
            routes.wire = Wire::Strings;
            routes.multiline = true;
            p.fields.push(routes);
            p.note = crate::i18n::tr(
                "发布的路由需在 Tailscale 管理端批准。DNS 与退出节点作用于路由器，热点转发范围保持原配置。",
            )
            .into();
        }
        _ => return p,
    }
    let warning = match id {
        "wifi2" | "wifi5" | "guest" => {
            crate::i18n::tr("应用后热点可能重启，已连接设备需要重新连接。开放网络不要求密码。")
        }
        "auto-sleep" => crate::i18n::tr("没有热点客户端时可能进入整机休眠，需要机身电源键唤醒。"),
        "watchdog" => crate::i18n::tr("更改会影响断网恢复；允许重启时设备可能自动重启。"),
        "tailscale-config" => crate::i18n::tr("会调整此节点的远程连接设置，可能影响现有连接。"),
        _ => crate::i18n::tr("将保存这些设置到设备。"),
    };
    let mut action = Action::new("save", crate::i18n::tr("保存设置"), warning);
    action.enabled = d.is_object();
    p.actions.push(action);
    p
}
pub fn plan(id: &str, action: &str, p: &Page, readings: &Data) -> Result<Plan> {
    ensure!(action == "save", "{}", crate::i18n::tr("未知操作。"));
    let d = data(readings, "settings");
    ensure!(d.is_object(), "{}", crate::i18n::tr("请先读取设备设置。"));
    let keys = p.fields.iter().map(|f| f.key.as_str()).collect::<Vec<_>>();
    let mut body = p.changed(&keys)?;
    ensure!(
        body.as_object().is_some_and(|o| !o.is_empty()),
        "{}",
        crate::i18n::tr("没有需要保存的更改。")
    );
    let path = match id {
        "wifi2" | "wifi5" | "guest" => {
            let (ssid, key, enc) = if id == "guest" {
                (
                    "guest_ssid".into(),
                    "guest_key".into(),
                    "guest_encryption".into(),
                )
            } else {
                let suffix = if id == "wifi2" { "2g" } else { "5g" };
                (
                    format!("ssid_{suffix}"),
                    format!("key_{suffix}"),
                    format!("encryption_{suffix}"),
                )
            };
            for k in [&ssid, &key] {
                if let Some(v) = body.get(k).and_then(Value::as_str) {
                    ensure!(
                        !v.contains(['\'', '"', ';', '$', '`', '\\', '|', '<', '>', '&'])
                            && !v.chars().any(char::is_control),
                        "{}",
                        crate::i18n::tr("设备无法保存输入中的特殊字符，请修改网络名称或密码。")
                    );
                }
            }
            let encryption = p.field(&enc)?.value.as_str();
            let password = p.field(&key)?.value.as_str();
            if !password.is_empty() {
                ensure!(
                    ((8..=63).contains(&password.len())
                        && password.bytes().all(|c| (32..=126).contains(&c)))
                        || (!encryption.contains("sae")
                            && password.len() == 64
                            && password.bytes().all(|c| c.is_ascii_hexdigit())),
                    "{}",
                    crate::i18n::tr(
                        "密码需要 8–63 个可打印字符；非 WPA3 也可使用 64 位 HEX 密钥。"
                    )
                );
            }
            let stored = if id == "guest" { &d["key"] } else { &d[&key] };
            ensure!(
                encryption == "none"
                    || !password.is_empty()
                    || stored.as_str().is_some_and(|s| !s.is_empty()),
                "{}",
                crate::i18n::tr("加密网络需要密码。")
            );
            if id == "wifi5" && (p.field("channel_5g")?.dirty() || p.field("htmode_5g")?.dirty()) {
                let ch = p.field("channel_5g")?.value.parse::<u32>().unwrap_or(0);
                let width = &p.field("htmode_5g")?.value;
                ensure!(
                    !(ch == 165 && !["auto", "EHT20"].contains(&width.as_str())
                        || ch >= 149 && width == "EHT160"
                        || (132..149).contains(&ch)
                            && ["EHT80", "EHT160"].contains(&width.as_str())),
                    "{}",
                    crate::i18n::tr("所选信道不支持该带宽。")
                );
            }
            if id == "guest" {
                "/api/wifi/guest"
            } else {
                "/api/wifi/settings"
            }
        }
        "power-save" => {
            body = json!({"deviceInfoList":p.all(&["power_saver_mode"])?});
            "/api/device/power-save"
        }
        "tailscale-config" => {
            if let Some(routes) = body["advertise_routes"].as_array() {
                for route in routes {
                    ensure!(
                        route
                            .as_str()
                            .is_some_and(|s| s.parse::<ipnet::IpNet>().is_ok()),
                        "{}",
                        crate::i18n::tr("子网需使用 CIDR，例如 10.20.0.0/24。")
                    );
                }
            }
            "/api/tailscale/config"
        }
        _ => endpoint(id)
            .ok_or_else(|| anyhow::anyhow!("{}", crate::i18n::tr("此设置尚不可用。")))?,
    };
    Ok(Plan::one(Method::PUT, path, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_fields_keep_wire_types_and_do_not_overwrite_unknown_settings() {
        let readings = Data::from([(
            "settings".into(),
            Ok(
                json!({"charge_limit_enabled":true,"charge_limit":80,"hysteresis":5,"charging_stopped":false}),
            ),
        )]);
        let mut p = page("charge", &readings);
        assert!(plan("charge", "save", &p, &readings).is_err());
        p.fields
            .iter_mut()
            .find(|f| f.key == "charge_limit")
            .unwrap()
            .value = "85".into();
        let request = plan("charge", "save", &p, &readings)
            .unwrap()
            .requests
            .remove(0);
        assert_eq!(request.body, Some(json!({"charge_limit":85})));
        let missing = Data::new();
        let p = page("charge", &missing);
        assert!(!p.actions[0].enabled);
        assert!(plan("charge", "save", &p, &missing).is_err());
    }
    #[test]
    fn wifi_password_is_blank_and_unsafe_changes_are_refused() {
        let readings = Data::from([(
            "settings".into(),
            Ok(
                json!({"ssid_2g":"Example","key_2g":"existing-test-password","encryption_2g":"psk2+ccmp"}),
            ),
        )]);
        let mut p = page("wifi2", &readings);
        assert!(p.field("key_2g").unwrap().value.is_empty());
        p.fields
            .iter_mut()
            .find(|f| f.key == "ssid_2g")
            .unwrap()
            .value = "New Example".into();
        assert_eq!(
            plan("wifi2", "save", &p, &readings).unwrap().requests[0].body,
            Some(json!({"ssid_2g":"New Example"}))
        );
        p.fields
            .iter_mut()
            .find(|f| f.key == "ssid_2g")
            .unwrap()
            .value = "unsafe$value".into();
        assert!(plan("wifi2", "save", &p, &readings).is_err());
    }
}
