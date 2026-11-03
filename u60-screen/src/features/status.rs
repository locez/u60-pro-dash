use super::*;
use serde_json::json;

pub fn reads(id: &str) -> Option<Vec<Read>> {
    let list: &[(&str, &str)] = match id {
        "signal-live" | "carriers" => &[("signal", "/api/network/signal")],
        "clients" => &[("clients", "/api/network/client-details")],
        "wan" => &[
            ("wan", "/api/network/wan"),
            ("wan6", "/api/network/wan6"),
            ("lan", "/api/network/lan-status"),
            ("rmnet", "/api/network/rmnet"),
        ],
        "resources" => &[
            ("cpu", "/api/cpu"),
            ("memory", "/api/memory"),
            ("thermal", "/api/device/thermal"),
            ("system", "/api/device/system"),
        ],
        "battery-details" => &[
            ("battery", "/api/battery"),
            ("battery_info", "/api/device/battery-info"),
            ("charger", "/api/device/charger"),
            ("charge_control", "/api/device/charge-control"),
        ],
        "traffic" => &[
            ("traffic", "/api/network/speeds"),
            ("interfaces", "/api/network/traffic"),
        ],
        "sim-info" => &[
            ("sim", "/api/sim/info"),
            ("imei", "/api/sim/imei"),
            ("trials", "/api/sim/lock-trials"),
        ],
        "sms-storage" => &[("capacity", "/api/sms/capacity")],
        "device-info" | "backup" => &[("device", "/api/device")],
        "processes" => &[("processes", "/api/system/top")],
        "lan-test" => &[("lan", "/api/network/lan-status")],
        "wireguard" => &[("wireguard", "/api/vpn/wireguard")],
        _ => return None,
    };
    Some(list.iter().map(|(k, p)| Read::get(k, p)).collect())
}
pub fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|v| v.is_finite())
}
pub fn quantity(value: &Value, divisor: f64, unit: &str) -> String {
    number(value)
        .map(|n| format!("{:.1} {unit}", n / divisor))
        .unwrap_or_else(|| crate::i18n::tr("未提供").into())
}
fn values(p: &mut Page, data: &Value, keys: &[(&str, &str)]) {
    for (k, l) in keys {
        p.row(l, &data[*k]);
    }
}
fn state(value: &Value) -> String {
    match text(value).as_str() {
        "Charging" => crate::i18n::tr("充电中").into(),
        "Discharging" => crate::i18n::tr("放电中").into(),
        "Full" => crate::i18n::tr("已充满").into(),
        "Not charging" => crate::i18n::tr("未充电").into(),
        _ => shown(value),
    }
}
pub fn page(id: &str, readings: &Data) -> Page {
    let mut p = Page::default();
    match id {
        "signal-live" => {
            let s = data(readings, "signal");
            values(
                &mut p,
                s,
                &[
                    ("network_type", crate::i18n::tr("网络制式")),
                    ("network_provider", crate::i18n::tr("运营商")),
                ],
            );
            for (prefix, name, band, pci, ch) in [
                (
                    "nr5g",
                    "5G NR",
                    "nr5g_action_band",
                    "nr5g_pci",
                    "nr5g_action_channel",
                ),
                (
                    "lte",
                    "4G LTE",
                    "wan_active_band",
                    "lte_pci",
                    "wan_active_channel",
                ),
            ] {
                let rsrp =
                    number(&s[format!("{prefix}_rsrp")]).filter(|n| (-160.0..=-20.0).contains(n));
                p.rows.push((
                    format!("{name} RSRP"),
                    rsrp.map(|v| format!("{v:.0} dBm"))
                        .unwrap_or_else(|| crate::i18n::tr("未提供").into()),
                ));
                for (suffix, label, unit, min, max) in [
                    ("rsrq", "RSRQ", "dB", -50., 0.),
                    ("snr", "SINR", "dB", -30., 60.),
                    ("rssi", "RSSI", "dBm", -140., 0.),
                ] {
                    let n = number(&s[format!("{prefix}_{suffix}")])
                        .filter(|n| (min..=max).contains(n));
                    p.rows.push((
                        format!("{name} {label}"),
                        n.map(|n| format!("{n:.1} {unit}"))
                            .unwrap_or_else(|| crate::i18n::tr("未提供").into()),
                    ));
                }
                p.row(
                    &crate::i18n::format("{0} 频段", &[name.to_string()]),
                    &s[band],
                );
                p.row(&format!("{name} PCI"), &s[pci]);
                p.row(
                    &crate::i18n::format("{0} 信道", &[name.to_string()]),
                    &s[ch],
                );
            }
            p.row(
                crate::i18n::tr("网络授予下行 / Mbps"),
                &s["bearer"]["ambr_dl_mbps"],
            );
            p.row(
                crate::i18n::tr("网络授予上行 / Mbps"),
                &s["bearer"]["ambr_ul_mbps"],
            );
            p.note =
                crate::i18n::tr("信号通过实时订阅更新。网络授予速率不等于实际测速结果。").into();
        }
        "carriers" => {
            let carriers = carriers(data(readings, "signal"));
            if let Some(id) = selected(readings) {
                if let Some((_, title, rows)) = carriers.iter().find(|(key, _, _)| key == id) {
                    p.title = Some(title.clone());
                    p.rows = rows.clone();
                }
            } else {
                p.items = carriers
                    .into_iter()
                    .map(|(id, title, rows)| Item {
                        id,
                        title,
                        subtitle: rows
                            .iter()
                            .take(2)
                            .map(|(_, v)| v.clone())
                            .collect::<Vec<_>>()
                            .join(" · "),
                    })
                    .collect();
                if p.items.is_empty() {
                    p.note = crate::i18n::tr("设备尚未报告有效的载波记录。").into();
                }
            }
        }
        "clients" => {
            let d = data(readings, "clients");
            if let Some(devices) = d["devices"].as_array() {
                if let Some(id) = selected(readings) {
                    if let Some(device) = devices.iter().find(|v| v["mac"] == id) {
                        p.title = Some(if text(&device["name"]).is_empty() {
                            text(&device["mac"])
                        } else {
                            text(&device["name"])
                        });
                        values(
                            &mut p,
                            device,
                            &[
                                ("name", crate::i18n::tr("设备名称")),
                                ("mac", crate::i18n::tr("MAC 地址")),
                                ("ipv4", "IPv4"),
                                ("ipv6", "IPv6"),
                                ("connected", crate::i18n::tr("当前连接")),
                                ("band", crate::i18n::tr("频段")),
                                ("interface", crate::i18n::tr("网络接口")),
                                ("connected_seconds", crate::i18n::tr("在线时间 / 秒")),
                            ],
                        );
                        values(
                            &mut p,
                            &device["link"],
                            &[
                                ("signal_dbm", crate::i18n::tr("信号 / dBm")),
                                ("upload_mbps", crate::i18n::tr("上行链路 / Mbps")),
                                ("download_mbps", crate::i18n::tr("下行链路 / Mbps")),
                            ],
                        );
                        for (key, label) in [
                            ("upload_bps", crate::i18n::tr("上行速率")),
                            ("download_bps", crate::i18n::tr("下行速率")),
                        ] {
                            p.rows.push((
                                label.into(),
                                quantity(&device["traffic"][key], 1_000_000., "Mbps"),
                            ));
                        }
                        for (key, label) in [
                            ("upload_bytes", crate::i18n::tr("已上传")),
                            ("download_bytes", crate::i18n::tr("已下载")),
                        ] {
                            p.rows.push((
                                label.into(),
                                quantity(&device["traffic"][key], 1_000_000., "MB"),
                            ));
                        }
                        p.note = crate::i18n::tr(
                            "设备流量反映 Wi-Fi 链路；固件没有报告的限速或链路信息显示为未提供。",
                        )
                        .into();
                    }
                } else {
                    p.row(crate::i18n::tr("列表记录"), &json!(devices.len()));
                    p.row(
                        crate::i18n::tr("当前已连接"),
                        &json!(devices.iter().filter(|v| v["connected"] == true).count()),
                    );
                    p.row(crate::i18n::tr("数据来源完整"), &d["inventory_complete"]);
                    p.items = devices
                        .iter()
                        .filter_map(|v| {
                            let id = v["mac"].as_str()?;
                            Some(Item {
                                id: id.into(),
                                title: if text(&v["name"]).is_empty() {
                                    id.into()
                                } else {
                                    text(&v["name"])
                                },
                                subtitle: format!("{} · {}", shown(&v["ipv4"]), shown(&v["band"])),
                            })
                        })
                        .collect();
                }
            }
        }
        "wan" => {
            for (key, label) in [
                ("wan", crate::i18n::tr("蜂窝 IPv4")),
                ("wan6", crate::i18n::tr("蜂窝 IPv6")),
                ("lan", crate::i18n::tr("局域网")),
            ] {
                let d = data(readings, key);
                p.row(
                    &crate::i18n::format("{0} 已连接", &[label.to_string()]),
                    &d["up"],
                );
                p.row(
                    &crate::i18n::format("{0} 接口", &[label.to_string()]),
                    &d["l3_device"],
                );
                for family in ["ipv4-address", "ipv6-address"] {
                    if let Some(addrs) = d[family].as_array() {
                        for address in addrs {
                            p.rows.push((
                                crate::i18n::format("{0} 地址", &[label.to_string()]),
                                format!(
                                    "{}/{}",
                                    shown(&address["address"]),
                                    shown(&address["mask"])
                                ),
                            ));
                        }
                    }
                }
                p.row(&format!("{label} DNS"), &d["dns-server"]);
            }
            p.row(crate::i18n::tr("蜂窝 MTU"), &data(readings, "rmnet")["mtu"]);
        }
        "resources" => {
            let cpu = data(readings, "cpu");
            let mem = data(readings, "memory");
            p.rows.push((
                crate::i18n::tr("CPU 使用率").into(),
                quantity(&cpu["overall"], 1., "%"),
            ));
            if let Some(cores) = cpu["cores"].as_array() {
                for (i, v) in cores.iter().enumerate() {
                    p.rows.push((
                        crate::i18n::format("核心 {0}", &[format!("{}", i + 1)]),
                        quantity(v, 1., "%"),
                    ));
                }
            }
            for (key, label) in [
                ("total_kb", crate::i18n::tr("内存总量")),
                ("available_kb", crate::i18n::tr("可用内存")),
                ("used_kb", crate::i18n::tr("已用内存")),
            ] {
                p.rows
                    .push((label.into(), quantity(&mem[key], 1024., "MiB")));
            }
            if let Some(sensors) = data(readings, "thermal")["sensors"].as_array() {
                for sensor in sensors {
                    p.rows.push((
                        shown(&sensor["name"]),
                        quantity(&sensor["temperature_c"], 1., "°C"),
                    ));
                }
            }
            p.row(
                crate::i18n::tr("运行时间 / 秒"),
                &data(readings, "system")["uptime"],
            );
        }
        "battery-details" => {
            let b = data(readings, "battery");
            p.row(crate::i18n::tr("剩余电量 %"), &b["capacity"]);
            p.rows
                .push((crate::i18n::tr("充电状态").into(), state(&b["status"])));
            p.rows.push((
                crate::i18n::tr("电池温度").into(),
                quantity(&b["temperature"], 10., "°C"),
            ));
            p.rows.push((
                crate::i18n::tr("电池电压").into(),
                quantity(&b["voltage_uv"], 1_000_000., "V"),
            ));
            p.rows.push((
                crate::i18n::tr("电池电流").into(),
                quantity(&b["current_ua"], 1000., "mA"),
            ));
            let charger = data(readings, "charger");
            if !charger["charger_type"].is_null() {
                p.row(crate::i18n::tr("充电器类型"), &charger["charger_type"]);
            }
            p.rows.push((
                crate::i18n::tr("供电状态").into(),
                crate::agent::model::Power::read(b, charger).label().into(),
            ));
            let bypass = flag(&charger["direct_power_supply_mode"]);
            p.row(
                crate::i18n::tr("直供电模式"),
                &bypass.map(Value::Bool).unwrap_or(Value::Null),
            );
            let control = data(readings, "charge_control");
            let enabled = bypass == Some(true);
            let mut action = Action::new(
                if enabled { "bypass-off" } else { "bypass-on" },
                if enabled {
                    crate::i18n::tr("关闭直供电")
                } else {
                    crate::i18n::tr("启用直供电")
                },
                crate::i18n::tr(
                    "启用后接入电源时使用直供电并停止电池充电；关闭后由充电上限策略决定是否恢复充电。",
                ),
            );
            action.enabled = bypass.is_some() && flag(&control["charging_stopped"]) == bypass;
            p.actions.push(action);
            p.note = crate::i18n::tr("直供电与停止充电由同一开关控制。拔掉外接电源后使用电池；充电上限仍在「充电策略」设置。").into();
        }
        "traffic" => {
            let t = data(readings, "traffic");
            p.rows.push((
                crate::i18n::tr("蜂窝下行速率").into(),
                quantity(&t["real_rx_speed"], 125_000., "Mbps"),
            ));
            p.rows.push((
                crate::i18n::tr("蜂窝上行速率").into(),
                quantity(&t["real_tx_speed"], 125_000., "Mbps"),
            ));
            for (key, label) in [
                ("real_rx_bytes", crate::i18n::tr("本次下载")),
                ("real_tx_bytes", crate::i18n::tr("本次上传")),
            ] {
                p.rows
                    .push((label.into(), quantity(&t[key], 1_000_000., "MB")));
            }
            if let Some(ifaces) = data(readings, "interfaces").as_array() {
                for iface in ifaces {
                    p.rows.push((
                        crate::i18n::format(
                            "{0} · 下载 / 上传",
                            &[shown(&iface["name"]).to_string()],
                        ),
                        format!(
                            "{} / {}",
                            quantity(&iface["rx_bytes"], 1_000_000., "MB"),
                            quantity(&iface["tx_bytes"], 1_000_000., "MB")
                        ),
                    ));
                }
            }
            p.note = crate::i18n::tr("接口计数自接口启动累计，不能与运营商账单直接等同。").into();
        }
        "sim-info" => {
            let s = data(readings, "sim");
            values(
                &mut p,
                s,
                &[
                    ("sim_states", crate::i18n::tr("SIM 状态")),
                    ("Operator", crate::i18n::tr("运营商")),
                    ("sim_iccid", "ICCID"),
                    ("sim_imsi", "IMSI"),
                    ("msisdn", crate::i18n::tr("号码")),
                    ("current_sim_slot", crate::i18n::tr("卡槽")),
                    ("pinnumber", crate::i18n::tr("剩余 PIN 次数")),
                    ("puknumber", crate::i18n::tr("剩余 PUK 次数")),
                ],
            );
            p.row("IMEI", &data(readings, "imei")["imei"]);
        }
        "sms-storage" => {
            let d = data(readings, "capacity");
            values(
                &mut p,
                d,
                &[
                    ("sms_nvused_total", crate::i18n::tr("设备已用条数")),
                    ("sms_nv_total", crate::i18n::tr("设备总容量")),
                    ("sms_simused_total", crate::i18n::tr("SIM 已用条数")),
                    ("sms_sim_total", crate::i18n::tr("SIM 总容量")),
                ],
            );
        }
        "device-info" => {
            values(
                &mut p,
                data(readings, "device"),
                &[
                    ("hostname", crate::i18n::tr("设备名称")),
                    ("uptime_secs", crate::i18n::tr("运行时间 / 秒")),
                    ("kernel", crate::i18n::tr("内核版本")),
                    ("load_avg", crate::i18n::tr("系统负载")),
                ],
            );
        }
        "processes" => {
            let d = data(readings, "processes");
            p.row(crate::i18n::tr("进程总数"), &d["total_count"]);
            p.row(crate::i18n::tr("可清理进程"), &d["bloat_count"]);
            if let Some(processes) = d["processes"].as_array() {
                if let Some(id) = selected(readings) {
                    if let Some(proc) = processes.iter().find(|v| text(&v["pid"]) == id) {
                        p.title = Some(shown(&proc["name"]));
                        values(
                            &mut p,
                            proc,
                            &[
                                ("pid", "PID"),
                                ("name", crate::i18n::tr("进程")),
                                ("cpu_pct", crate::i18n::tr("CPU 使用率 %")),
                                ("state", crate::i18n::tr("运行状态")),
                                ("is_bloat", crate::i18n::tr("允许清理")),
                            ],
                        );
                        p.rows.push((
                            crate::i18n::tr("常驻内存").into(),
                            quantity(&proc["rss_kb"], 1024., "MiB"),
                        ));
                    }
                } else {
                    p.items = processes
                        .iter()
                        .map(|v| Item {
                            id: text(&v["pid"]),
                            title: format!("{} · {}", shown(&v["pid"]), shown(&v["name"])),
                            subtitle: format!(
                                "{} · {}",
                                quantity(&v["cpu_pct"], 1., "%"),
                                quantity(&v["rss_kb"], 1024., "MiB")
                            ),
                        })
                        .collect();
                }
            }
            let mut action = Action::new(
                "clean",
                crate::i18n::tr("清理非必要进程"),
                crate::i18n::tr("仅清理后台允许列表中的非必要进程，可能暂时影响对应服务。"),
            );
            action.enabled = d["bloat_count"].as_u64().is_some_and(|n| n > 0);
            p.actions.push(action);
        }
        "wireguard" => {
            let d = data(readings, "wireguard");
            values(
                &mut p,
                d,
                &[
                    ("configured", crate::i18n::tr("已导入配置")),
                    ("enabled", crate::i18n::tr("期望启用")),
                    ("active", crate::i18n::tr("隧道运行")),
                    ("addresses", crate::i18n::tr("隧道地址")),
                    ("interface", crate::i18n::tr("接口")),
                    ("mtu", "MTU"),
                ],
            );
            if let Some(peers) = d["peers"].as_array() {
                for peer in peers {
                    p.rows.push((
                        shown(&peer["name"]),
                        format!(
                            "{} · {}",
                            shown(&peer["endpoint"]),
                            shown(&peer["allowed_ips"])
                        ),
                    ));
                }
            }
            for (id, label) in [
                ("enable", crate::i18n::tr("启用隧道")),
                ("disable", crate::i18n::tr("停止隧道")),
            ] {
                let mut action = Action::new(
                    id,
                    label,
                    crate::i18n::tr("隧道变化会影响经由 WireGuard 的连接。"),
                );
                action.enabled = d["configured"] == true && d["supported"] == true;
                p.actions.push(action);
            }
            let mut config = Field::password(
                "config",
                crate::i18n::tr("导入完整 WireGuard 配置"),
                d["supported"] == true,
            );
            config.multiline = true;
            config.max = 65536;
            p.fields.push(config);
            p.fields.push(Field::boolean(
                "import_enabled",
                crate::i18n::tr("导入后启用"),
                &json!(false),
            ));
            let mut import = Action::new(
                "import",
                crate::i18n::tr("替换 WireGuard 配置"),
                crate::i18n::tr("替换已有配置与私钥；如启用，会重新建立隧道并改变对应子网的路由。"),
            );
            import.enabled = d["supported"] == true;
            p.actions.push(import);
            p.note = crate::i18n::tr("输入标准 [Interface] / [Peer] 配置。后台拒绝执行脚本钩子；此独立隧道按 AllowedIPs 转发子网，配置中的 DNS 不会自动更改系统 DNS。").into();
        }
        "lan-test" => {
            p.note=crate::i18n::tr("局域网吞吐测试需要另一台设备。请让电脑或手机连接此热点，在管理后台打开局域网测速。\n\n机身到本机的测试不会反映 Wi-Fi 链路速度。").into();
            if let Some(addresses) = data(readings, "lan")["ipv4-address"].as_array() {
                for address in addresses {
                    p.row(crate::i18n::tr("热点网关"), &address["address"]);
                }
            }
        }
        "backup" => {
            p.note=crate::i18n::tr("当前后台没有备份/恢复接口。请通过已授权的电脑保存配置与启动文件；备份应包含版本记录，并保存在私有位置。\n\n恢复前先核对固件与安装布局，不要把另一台设备的身份、密钥或 Tailscale 登录状态覆盖到本机。").into();
        }
        _ => {}
    }
    p
}
pub fn plan(id: &str, action: &str, page: &Page, readings: &Data) -> Result<Plan> {
    match (id, action) {
        ("battery-details", "bypass-on" | "bypass-off") => {
            let mode = flag(&data(readings, "charger")["direct_power_supply_mode"]);
            let control = data(readings, "charge_control");
            ensure!(
                mode.is_some() && flag(&control["charging_stopped"]) == mode,
                "{}",
                crate::i18n::tr("请先读取设备设置。")
            );
            let stopped = action == "bypass-on";
            ensure!(
                mode != Some(stopped),
                "{}",
                crate::i18n::tr("没有需要保存的更改。")
            );
            let mut plan = Plan::one(
                Method::PUT,
                "/api/device/charge-control",
                json!({"charging_stopped":stopped}),
            );
            plan.guards.push(Guard::new(
                Read::get("charge_control", "/api/device/charge-control"),
                control,
                &["/charging_stopped", "/manual_override"],
            ));
            Ok(plan)
        }
        ("processes", "clean") => {
            ensure!(
                data(readings, "processes")["bloat_count"]
                    .as_u64()
                    .is_some_and(|n| n > 0),
                "{}",
                crate::i18n::tr("没有可清理进程。")
            );
            Ok(Plan::one(
                Method::POST,
                "/api/system/kill-bloat",
                json!({"all":true}),
            ))
        }
        ("wireguard", "import") => {
            let config = page.value("config")?;
            let config_text = text(&config);
            ensure!(
                config_text.contains("[Interface]") && config_text.contains("[Peer]"),
                "{}",
                crate::i18n::tr("请填写完整的 WireGuard 配置。")
            );
            let mut plan = Plan::one(
                Method::PUT,
                "/api/vpn/wireguard",
                json!({"config":config,"enabled":page.value("import_enabled")?}),
            );
            plan.guards.push(Guard::new(
                Read::get("wireguard", "/api/vpn/wireguard"),
                data(readings, "wireguard"),
                &["/public_key", "/configured", "/addresses", "/peers"],
            ));
            Ok(plan)
        }
        ("wireguard", "enable" | "disable") => Ok(Plan::one(
            Method::POST,
            &format!("/api/vpn/wireguard/{action}"),
            json!({}),
        )),
        _ => anyhow::bail!("{}", crate::i18n::tr("此页没有该操作。")),
    }
}
type Carrier = (String, String, Vec<(String, String)>);
fn carriers(signal: &Value) -> Vec<Carrier> {
    let mut out = vec![];
    for (name, list, signals, pci, ch, band) in [
        (
            "NR",
            "nrca",
            "nrcasig",
            "nr5g_pci",
            "nr5g_action_channel",
            "nr5g_action_band",
        ),
        (
            "LTE",
            "lteca",
            "ltecasig",
            "lte_pci",
            "wan_active_channel",
            "wan_active_band",
        ),
    ] {
        let mut entries = text(&signal[list])
            .split(';')
            .filter_map(|row| {
                let cols = row.split(',').map(str::trim).collect::<Vec<_>>();
                (cols.len() >= 5 && cols[1] != "0" && !(cols[0] == "0" && cols[3] == "0"))
                    .then(|| cols.into_iter().map(String::from).collect::<Vec<_>>())
            })
            .collect::<Vec<_>>();
        let primary_pci = text(&signal[pci]);
        let primary_ch = text(&signal[ch]);
        if !primary_pci.is_empty()
            && !primary_ch.is_empty()
            && !text(&signal[band]).is_empty()
            && !entries
                .iter()
                .any(|c| c[0] == primary_pci && c[3] == primary_ch)
        {
            entries.insert(
                0,
                vec![
                    primary_pci.clone(),
                    text(&signal[band]),
                    "0".into(),
                    primary_ch.clone(),
                    String::new(),
                ],
            );
        }
        let signal_text = text(&signal[signals]);
        let signals = signal_text.split(';').collect::<Vec<_>>();
        let mut secondary = 0;
        for columns in entries {
            let key = format!("{name}:{}:{}:{}", columns[1], columns[0], columns[3]);
            if out.iter().any(|(id, _, _)| *id == key) {
                continue;
            }
            let primary = columns[0] == primary_pci && columns[3] == primary_ch;
            let title = format!("{name} · {} · PCI {}", columns[1], columns[0]);
            let mut rows = vec![
                (crate::i18n::tr("信道").into(), columns[3].clone()),
                (
                    crate::i18n::tr("带宽 / MHz").into(),
                    if columns[4].is_empty() {
                        crate::i18n::tr("未提供").into()
                    } else {
                        columns[4].clone()
                    },
                ),
                (
                    crate::i18n::tr("角色").into(),
                    if primary {
                        crate::i18n::tr("主载波")
                    } else {
                        crate::i18n::tr("辅载波")
                    }
                    .into(),
                ),
            ];
            if !primary {
                if let Some(metrics) = signals.get(secondary) {
                    for (i, v) in metrics.split(',').take(4).enumerate() {
                        rows.push((
                            ["RSRP / dBm", "RSRQ / dB", "SINR / dB", "RSSI / dBm"][i].into(),
                            v.into(),
                        ));
                    }
                }
                secondary += 1;
            }
            out.push((key, title, rows));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reading_units_and_carrier_identity_are_stable() {
        assert_eq!(quantity(&json!(250000), 125000., "Mbps"), "2.0 Mbps");
        assert_eq!(quantity(&Value::Null, 1., "%"), "未提供");
        let s = json!({"lte_pci":"178","wan_active_channel":"301","wan_active_band":"1","lteca":"178,1,0,301,10;12,3,1,1300,20;12,3,1,1300,20;0,0,0,0,0","ltecasig":"-90,-10,8,-60"});
        let c = carriers(&s);
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].0, "LTE:3:12:1300");
        assert!(c[1].2.iter().any(|(k, v)| k == "RSRP / dBm" && v == "-90"));
        let readings = Data::from([(
            "signal".into(),
            Ok(json!({"nr5g_rsrp":"9999","lte_rsrp":"-88"})),
        )]);
        let p = page("signal-live", &readings);
        assert!(
            p.rows
                .iter()
                .any(|(k, v)| k == "5G NR RSRP" && v == "未提供")
        );
    }
}
