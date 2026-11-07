//! Router and modem forms, using the agent's wire contracts and explicit units.
use super::*;
use serde_json::json;
use std::net::Ipv4Addr;

const CUT: &str = "更改可能中断热点设备的网络连接。请确认地址、服务和当前运营商支持此设置。";
pub fn reads(id: &str) -> Option<Vec<Read>> {
    let list: &[(&str, &str)] = match id {
        "lan" => &[("settings", "/api/router/lan")],
        "mobile-data" => &[
            ("settings", "/api/modem/data"),
            ("modem", "/api/modem/status"),
        ],
        "network-mode" => &[("settings", "/api/network/signal")],
        "usage" => &[("settings", "/api/data-usage")],
        "firewall" => &[
            ("settings", "/api/router/firewall"),
            ("rules", "/api/router/firewall/filter-rules"),
        ],
        "nat" => &[("settings", "/api/router/firewall")],
        "dmz" => &[
            ("settings", "/api/router/firewall"),
            ("lan", "/api/router/lan"),
        ],
        "upnp" => &[("settings", "/api/router/firewall/upnp")],
        "qos" => &[("settings", "/api/router/qos")],
        "vpn" => &[("settings", "/api/router/vpn")],
        "dns-config" | "doh" => &[("settings", "/api/dns/status")],
        "dns-cache" => &[("cache", "/api/dns/cache")],
        _ => return None,
    };
    Some(list.iter().map(|(k, p)| Read::get(k, p)).collect())
}
pub fn first<'a>(value: &'a Value, keys: &[&str]) -> &'a Value {
    keys.iter()
        .find_map(|k| value.get(*k).filter(|v| !v.is_null()))
        .unwrap_or(&Value::Null)
}
pub fn bit(key: &str, label: &str, value: &Value) -> Field {
    let normalized = flag(value)
        .map(|b| json!(if b { "1" } else { "0" }))
        .unwrap_or(Value::Null);
    Field::choice(
        key,
        label,
        &normalized,
        &[
            ("0", crate::i18n::tr("关闭")),
            ("1", crate::i18n::tr("开启")),
        ],
    )
}
pub fn page(id: &str, readings: &Data) -> Page {
    let mut p = Page::default();
    let d = data(readings, "settings");
    match id {
        "lan" => {
            p.fields = vec![
                Field::text("lan_ipaddr", crate::i18n::tr("网关 IPv4"), &d["lan_ipaddr"]),
                Field::text(
                    "lan_netmask",
                    crate::i18n::tr("子网掩码"),
                    &d["lan_netmask"],
                ),
                bit(
                    "dhcp_enable",
                    crate::i18n::tr("DHCP 服务"),
                    &d["dhcp_enable"],
                ),
                Field::text(
                    "dhcp_start",
                    crate::i18n::tr("地址池起始地址"),
                    &pool_start(d),
                ),
                Field::text(
                    "dhcp_end",
                    crate::i18n::tr("地址池结束地址"),
                    &d["dhcp_end"],
                ),
                Field::text(
                    "dhcp_lease_time",
                    crate::i18n::tr("租期（秒 / m / h / d）"),
                    &d["dhcp_lease_time"],
                ),
            ];
            p.note=crate::i18n::tr("地址池填写完整 IPv4 地址。租期支持 3600、30m、12h、1d。更换网关后，客户端需要重新获取地址。").into();
        }
        "mobile-data" => {
            for (key, label) in [
                ("enable", crate::i18n::tr("蜂窝数据")),
                ("roam_enable", crate::i18n::tr("数据漫游")),
                ("connect_mode", crate::i18n::tr("自动连接")),
            ] {
                p.fields.push(bit(key, label, &d[key]));
            }
            p.row(crate::i18n::tr("连接状态"), &d["connect_status"]);
            p.row(
                crate::i18n::tr("射频模式"),
                &data(readings, "modem")["operate_mode"],
            );
            p.note =
                crate::i18n::tr("数据漫游可能产生运营商费用。首次开启漫游时需同时开启蜂窝数据。")
                    .into();
        }
        "network-mode" => p.fields.push(Field::choice(
            "net_select",
            crate::i18n::tr("首选网络"),
            &d["net_select"],
            &[
                ("WL_AND_5G", crate::i18n::tr("自动 5G / 4G / 3G")),
                ("LTE_AND_5G", "5G NSA（LTE + NR）"),
                ("Only_5G", crate::i18n::tr("仅 5G SA")),
                ("Only_LTE", crate::i18n::tr("仅 4G LTE")),
                ("WCDMA_AND_LTE", "4G / 3G"),
                ("Only_WCDMA", crate::i18n::tr("仅 3G")),
            ],
        )),
        "usage" => {
            for (key, label) in [
                ("day", crate::i18n::tr("今日")),
                ("month", crate::i18n::tr("本月")),
                ("total", crate::i18n::tr("累计")),
            ] {
                p.rows.push((
                    crate::i18n::format("{0}下载 / 上传", &[label.to_string()]),
                    format!(
                        "{} / {}",
                        super::status::quantity(&d[key]["rx_bytes"], 1e9, "GB"),
                        super::status::quantity(&d[key]["tx_bytes"], 1e9, "GB")
                    ),
                ));
            }
            let l = &d["limit"];
            let c = &d["cycle"];
            p.fields = vec![
                Field::boolean(
                    "limit_enabled",
                    crate::i18n::tr("月度流量限制"),
                    &l["enabled"],
                ),
                Field::number(
                    "bytes",
                    crate::i18n::tr("月度额度 / 字节"),
                    &l["bytes"],
                    1,
                    109_951_162_777_600,
                ),
                Field::number(
                    "warning_percent",
                    crate::i18n::tr("告警百分比"),
                    &l["warning_percent"],
                    1,
                    100,
                ),
                Field::boolean(
                    "auto_disconnect",
                    crate::i18n::tr("超额断网"),
                    &l["auto_disconnect"],
                ),
                Field::boolean(
                    "cycle_enabled",
                    crate::i18n::tr("自动重置周期"),
                    &c["enabled"],
                ),
                Field::number(
                    "reset_day",
                    crate::i18n::tr("每月重置日"),
                    &c["reset_day"],
                    1,
                    31,
                ),
            ];
            for f in &mut p.fields {
                f.available &= if ["cycle_enabled", "reset_day"].contains(&f.key.as_str()) {
                    c["available"] == true
                } else {
                    l["available"] == true
                };
            }
            p.note = crate::i18n::tr(
                "额度单位为字节，1 GB = 1000000000 字节。设备统计与运营商账单可能存在差异。",
            )
            .into();
        }
        "firewall" => {
            p.fields = vec![
                bit(
                    "firewall_switch",
                    crate::i18n::tr("防火墙"),
                    first(d, &["firewall_switch", "firewall_enable"]),
                ),
                Field::choice(
                    "firewall_level",
                    crate::i18n::tr("防护等级"),
                    first(d, &["firewall_level", "level"]),
                    &[
                        ("low", crate::i18n::tr("低")),
                        ("medium", crate::i18n::tr("中")),
                        ("high", crate::i18n::tr("高")),
                    ],
                ),
            ];
            if let Some(rules) = data(readings, "rules")["rule_list"].as_array() {
                for rule in rules {
                    p.rows
                        .push((crate::i18n::tr("过滤规则").into(), rule.to_string()));
                }
            }
            p.note = crate::i18n::tr(
                "关闭或降低防护可能让设备上的服务从蜂窝网络可达；提高防护可能阻断现有连接。",
            )
            .into();
        }
        "nat" => {
            p.fields.push(bit(
                "nat_switch",
                crate::i18n::tr("NAT 地址转换"),
                first(d, &["nat_switch", "nat_enable"]),
            ));
            p.note=crate::i18n::tr("固件提供 NAT 开关，不提供可保证的“全锥形 NAT”或公网地址。运营商 CGNAT 不由此开关控制。").into();
        }
        "dmz" => {
            p.fields = vec![
                bit(
                    "dmz_enabled",
                    "DMZ",
                    first(d, &["dmz_enabled", "dmz_enable"]),
                ),
                Field::text("dmz_ip", crate::i18n::tr("目标 IPv4"), &d["dmz_ip"]),
            ];
            p.note = crate::i18n::tr(
                "将未被其他规则匹配的入站连接转交给此局域网设备。目标设备应自行启用防火墙。",
            )
            .into();
        }
        "upnp" => p.fields.push(bit(
            "upnp_switch",
            crate::i18n::tr("UPnP 自动映射"),
            first(d, &["upnp_switch", "enable_upnp", "enable"]),
        )),
        "qos" => {
            p.fields.push(bit(
                "qos_switch",
                crate::i18n::tr("QoS 流量优先级"),
                &d["qos_switch"],
            ));
            p.note =
                crate::i18n::tr("使用固件自带策略。此接口不支持自定义单设备带宽与优先级。").into();
        }
        "vpn" => {
            for (key, label) in [
                ("l2tp_passthrough", crate::i18n::tr("L2TP 穿透")),
                ("pptp_passthrough", crate::i18n::tr("PPTP 穿透")),
                ("ipsec_passthrough", crate::i18n::tr("IPsec 穿透")),
                ("alg_sip_enable", "SIP ALG"),
            ] {
                p.fields.push(bit(key, label, &d[key]));
            }
        }
        "dns-config" | "doh" => {
            p.fields.push(Field::choice(
                "mode",
                crate::i18n::tr("解析方式"),
                &d["mode"],
                &[
                    ("automatic", crate::i18n::tr("运营商自动分配")),
                    ("manual", crate::i18n::tr("手动 DNS")),
                    ("doh", crate::i18n::tr("HTTPS 加密 / DoH")),
                    ("dot", crate::i18n::tr("TLS 加密 / DoT")),
                ],
            ));
            for (key, label) in [
                ("primary", crate::i18n::tr("首选 DNS IPv4")),
                ("secondary", crate::i18n::tr("备用 DNS IPv4")),
                ("doh_url", crate::i18n::tr("DoH HTTPS 地址")),
                ("dot_endpoint", crate::i18n::tr("DoT 主机 / 端口")),
                ("dot_server_name", crate::i18n::tr("DoT 证书域名")),
            ] {
                p.fields.push(Field::text(key, label, &d[key]));
            }
            p.row(crate::i18n::tr("加密代理运行"), &d["proxy_running"]);
            p.row(crate::i18n::tr("运营商 DNS"), &d["carrier_servers"]);
            let result = data(readings, "last_result");
            if !result.is_null() {
                p.row(crate::i18n::tr("检测耗时 / ms"), &result["latency_ms"]);
                p.row(crate::i18n::tr("保存后验证"), &result["verified"]);
            }
            p.actions.push(Action::new(
                "test",
                crate::i18n::tr("检测当前草稿"),
                crate::i18n::tr("向候选 DNS 发起解析测试，不修改设备 DNS 设置。"),
            ));
            p.note=crate::i18n::tr("固件手动 DNS 仅接受 IPv4。加密模式同样需要首选 DNS 以解析服务器域名。保存前后台强制检测，应用后失败会尝试恢复原设置。").into();
        }
        "dns-cache" => {
            let d = data(readings, "cache");
            if let Some(entries) = d["entries"].as_array() {
                p.row(crate::i18n::tr("缓存条目"), &json!(entries.len()));
                for e in entries {
                    p.rows.push((
                        shown(first(e, &["name", "domain"])),
                        format!(
                            "{} · TTL {}",
                            shown(first(e, &["type"])),
                            shown(first(e, &["ttl_secs", "ttl", "remaining_ttl"]))
                        ),
                    ));
                }
            }
            p.actions.push(Action::new("clear",crate::i18n::tr("清空 DNS 缓存"),crate::i18n::tr("清除设备与代理缓存；下一次未命中的查询重新访问解析器。客户端仍可能保留自己的缓存。")));
        }
        _ => {}
    }
    if !p.fields.is_empty() {
        p.actions
            .insert(0, Action::new("save", crate::i18n::tr("保存设置"), CUT));
    }
    for action in &mut p.actions {
        action.enabled &= if id == "dns-cache" {
            readings.get("cache").is_some_and(Result::is_ok)
        } else {
            !d.is_null()
        };
    }
    p
}
pub fn plan(id: &str, action: &str, p: &Page, readings: &Data) -> Result<Plan> {
    let mut requests = vec![];
    let mut put = |path: &str, body: Value| {
        requests.push(Request::json(Method::PUT, path, body));
    };
    if id == "dns-cache" && action == "clear" {
        return Ok(Plan::one(Method::POST, "/api/dns/cache/clear", json!({})));
    }
    if ["dns-config", "doh"].contains(&id) {
        ensure!(
            ["save", "test"].contains(&action),
            "{}",
            crate::i18n::tr("操作不存在。")
        );
        let body = dns_body(p)?;
        let mut plan = Plan::one(
            if action == "test" {
                Method::POST
            } else {
                Method::PUT
            },
            if action == "test" {
                "/api/dns/test"
            } else {
                "/api/dns/apply"
            },
            body,
        );
        plan.keep_draft = action == "test";
        return Ok(plan);
    }
    ensure!(
        action == "save" && p.dirty(),
        "{}",
        crate::i18n::tr("没有需要保存的更改。")
    );
    match id {
        "lan" => put("/api/router/lan", lan_body(p)?),
        "mobile-data" => {
            let mut body = p.all(&["enable", "roam_enable", "connect_mode"])?;
            for key in ["enable", "roam_enable", "connect_mode"] {
                body[key] = json!(text(&body[key]).parse::<i64>()?);
            }
            ensure!(
                body["roam_enable"] != 1
                    || flag(&data(readings, "settings")["roam_enable"]) == Some(true)
                    || body["enable"] == 1,
                "{}",
                crate::i18n::tr("首次开启漫游时请同时开启蜂窝数据。")
            );
            body["cid"] = json!(1);
            if body["enable"] == 0 {
                body["connect_status"] = json!("disconnected");
            }
            put("/api/modem/data", body);
        }
        "network-mode" => put("/api/modem/network-mode", p.changed(&["net_select"])?),
        "usage" => {
            if [
                "limit_enabled",
                "bytes",
                "warning_percent",
                "auto_disconnect",
            ]
            .iter()
            .any(|k| p.field(k).is_ok_and(Field::dirty))
            {
                let mut b = p.all(&["bytes", "warning_percent", "auto_disconnect"])?;
                b["enabled"] = p.value("limit_enabled")?;
                put("/api/data-usage/limit", b);
            }
            if ["cycle_enabled", "reset_day"]
                .iter()
                .any(|k| p.field(k).is_ok_and(Field::dirty))
            {
                put(
                    "/api/data-usage/cycle",
                    json!({"enabled":p.value("cycle_enabled")?,"reset_day":p.value("reset_day")?}),
                );
            }
        }
        "firewall" => {
            if p.field("firewall_switch")?.dirty() {
                let s = p.value("firewall_switch")?;
                put(
                    "/api/router/firewall/switch",
                    json!({"enable":if s=="1"{1}else{0},"firewall_switch":s}),
                );
            }
            if p.field("firewall_level")?.dirty() {
                let l = p.value("firewall_level")?;
                put(
                    "/api/router/firewall/level",
                    json!({"firewall_level":l,"level":l}),
                );
            }
        }
        "nat" => {
            let s = p.value("nat_switch")?;
            put(
                "/api/router/firewall/nat",
                json!({"enable":if s=="1"{1}else{0},"nat_switch":s}),
            );
        }
        "upnp" => {
            let s = p.value("upnp_switch")?;
            put(
                "/api/router/firewall/upnp",
                json!({"enable_upnp":if s=="1"{1}else{0},"upnp_switch":s}),
            );
        }
        "dmz" => {
            let s = p.value("dmz_enabled")?;
            let ip = p.value("dmz_ip")?;
            if s == "1" {
                lan_host(&text(&ip), data(readings, "lan"))?;
            }
            put(
                "/api/router/firewall/dmz",
                json!({"dmz_enabled":s,"dmz_enable":if s=="1"{1}else{0},"dmz_ip":ip}),
            );
        }
        "qos" => put("/api/router/qos", p.changed(&["qos_switch"])?),
        "vpn" => put(
            "/api/router/vpn",
            p.changed(&[
                "l2tp_passthrough",
                "pptp_passthrough",
                "ipsec_passthrough",
                "alg_sip_enable",
            ])?,
        ),
        _ => anyhow::bail!("{}", crate::i18n::tr("操作不存在。")),
    }
    let mut plan = Plan::many(requests);
    if id == "lan" {
        plan.guards.push(Guard::new(
            Read::get("settings", "/api/router/lan"),
            data(readings, "settings"),
            &[""],
        ));
    }
    if id == "dmz" {
        plan.guards.push(Guard::new(
            Read::get("lan", "/api/router/lan"),
            data(readings, "lan"),
            &["/lan_ipaddr", "/lan_netmask"],
        ));
    }
    Ok(plan)
}
fn pool_start(d: &Value) -> Value {
    let s = &d["dhcp_start"];
    match (
        text(s).parse::<u32>(),
        text(&d["lan_ipaddr"]).parse::<Ipv4Addr>(),
        text(&d["lan_netmask"]).parse::<Ipv4Addr>(),
    ) {
        (Ok(n), Ok(ip), Ok(mask)) if n <= !u32::from(mask) => {
            json!(Ipv4Addr::from((u32::from(ip) & u32::from(mask)) + n).to_string())
        }
        _ => s.clone(),
    }
}
fn mask(value: &str) -> Result<u32> {
    let m = u32::from(value.parse::<Ipv4Addr>()?);
    ensure!(
        m.count_ones() == m.leading_ones() && (8..=30).contains(&m.leading_ones()),
        "{}",
        crate::i18n::tr("子网掩码必须连续，范围 /8–/30。")
    );
    Ok(m)
}
pub fn ipv4(value: &str) -> Result<Ipv4Addr> {
    let ip = value
        .parse::<Ipv4Addr>()
        .map_err(|_| anyhow::anyhow!("{}", crate::i18n::tr("请输入有效 IPv4 地址。")))?;
    ensure!(
        !ip.is_unspecified() && !ip.is_multicast() && !ip.is_broadcast() && !ip.is_loopback(),
        "{}",
        crate::i18n::tr("地址不能是未指定、回环、组播或广播地址。")
    );
    Ok(ip)
}
pub fn lan_host(value: &str, lan: &Value) -> Result<Ipv4Addr> {
    let ip = ipv4(value)?;
    let router = ipv4(&text(&lan["lan_ipaddr"]))?;
    let m = mask(&text(&lan["lan_netmask"]))?;
    let n = u32::from(router) & m;
    let v = u32::from(ip);
    ensure!(
        v > n && v < (n | !m) && v & m == n && ip != router,
        "{}",
        crate::i18n::tr("目标必须在当前局域网内，且不能是网关、网络或广播地址。")
    );
    Ok(ip)
}
fn lan_body(p: &Page) -> Result<Value> {
    let b = p.all(&[
        "lan_ipaddr",
        "lan_netmask",
        "dhcp_enable",
        "dhcp_start",
        "dhcp_end",
        "dhcp_lease_time",
    ])?;
    let addr = ipv4(&text(&b["lan_ipaddr"]))?;
    ensure!(
        addr.is_private(),
        "{}",
        crate::i18n::tr("网关必须为私有 IPv4 地址。")
    );
    let m = mask(&text(&b["lan_netmask"]))?;
    let n = u32::from(addr) & m;
    ensure!(
        u32::from(addr) > n && u32::from(addr) < (n | !m),
        "{}",
        crate::i18n::tr("网关不能是网络或广播地址。")
    );
    let s = lan_host(&text(&b["dhcp_start"]), &b)?;
    let e = lan_host(&text(&b["dhcp_end"]), &b)?;
    let (start, end, router) = (u32::from(s), u32::from(e), u32::from(addr));
    ensure!(
        start <= end && !(start..=end).contains(&router),
        "{}",
        crate::i18n::tr("地址池顺序错误或包含网关地址。")
    );
    let lease = text(&b["dhcp_lease_time"]);
    let (num, mult) = match lease.chars().last() {
        Some('m') => (&lease[..lease.len() - 1], 60),
        Some('h') => (&lease[..lease.len() - 1], 3600),
        Some('d') => (&lease[..lease.len() - 1], 86400),
        _ => (lease.as_str(), 1),
    };
    let seconds = num.parse::<u64>().ok().and_then(|n| n.checked_mul(mult));
    ensure!(
        seconds.is_some_and(|n| (120..=31_536_000).contains(&n)),
        "{}",
        crate::i18n::tr("租期需在 120 秒至 365 天之间。")
    );
    let mut out = b;
    out["dhcp_start"] = json!(start - n);
    out["dhcp_start"] = json!(text(&out["dhcp_start"]));
    out["ipaddr"] = json!(addr.to_string());
    out["netmask"] = out["lan_netmask"].clone();
    out["ignore"] = json!(if out["dhcp_enable"] == "1" { 0 } else { 1 });
    out["zte_start"] = json!(s.to_string());
    out["zte_end"] = json!(e.to_string());
    out["leasetime"] = json!(lease);
    Ok(out)
}
fn dns_body(p: &Page) -> Result<Value> {
    let mut b = p.all(&[
        "mode",
        "primary",
        "secondary",
        "doh_url",
        "dot_endpoint",
        "dot_server_name",
    ])?;
    let mode = text(&b["mode"]);
    if mode == "automatic" {
        b["primary"] = json!("");
        b["secondary"] = json!("");
    } else {
        ipv4(&text(&b["primary"]))?;
        if !text(&b["secondary"]).is_empty() {
            ipv4(&text(&b["secondary"]))?;
            ensure!(
                b["primary"] != b["secondary"],
                "{}",
                crate::i18n::tr("首选和备用 DNS 不能相同。")
            );
        }
    }
    if mode == "doh" {
        let u = reqwest::Url::parse(&text(&b["doh_url"]))?;
        ensure!(
            u.scheme() == "https"
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
                && u.fragment().is_none(),
            "{}",
            crate::i18n::tr("DoH 需要不含账户和片段的 HTTPS 地址。")
        );
    }
    if mode == "dot" {
        ensure!(
            !text(&b["dot_endpoint"]).is_empty() && domain(&text(&b["dot_server_name"])),
            "{}",
            crate::i18n::tr("请填写 DoT 主机和有效证书域名。")
        );
    }
    Ok(b)
}
pub fn domain(d: &str) -> bool {
    d.len() <= 253
        && d.split('.').count() >= 2
        && d.split('.').all(|s| {
            !s.is_empty()
                && s.len() <= 63
                && !s.starts_with('-')
                && !s.ends_with('-')
                && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lan_pool_is_an_address_in_ui_and_offset_on_wire() {
        let d = Data::from([(
            "settings".into(),
            Ok(
                json!({"lan_ipaddr":"10.20.0.1","lan_netmask":"255.255.255.0","dhcp_enable":"1","dhcp_start":"100","dhcp_end":"10.20.0.200","dhcp_lease_time":"12h"}),
            ),
        )]);
        let mut p = page("lan", &d);
        assert_eq!(p.field("dhcp_start").unwrap().value, "10.20.0.100");
        p.fields[3].value = "10.20.0.110".into();
        let b = plan("lan", "save", &p, &d).unwrap().requests[0]
            .body
            .clone()
            .unwrap();
        assert_eq!(b["dhcp_start"], "110");
        assert_eq!(b["zte_start"], "10.20.0.110");
        assert_eq!(b["ignore"], 0);
        p.fields[3].value = "10.20.0.1".into();
        assert!(plan("lan", "save", &p, &d).is_err());
    }
    #[test]
    fn mobile_numeric_payload_and_unknown_state_are_validated() {
        let d = Data::from([(
            "settings".into(),
            Ok(json!({"enable":1,"roam_enable":0,"connect_mode":1})),
        )]);
        let mut p = page("mobile-data", &d);
        p.fields[0].value = "0".into();
        let b = plan("mobile-data", "save", &p, &d).unwrap().requests[0]
            .body
            .clone()
            .unwrap();
        assert_eq!(
            b,
            json!({"cid":1,"enable":0,"roam_enable":0,"connect_mode":1,"connect_status":"disconnected"})
        );
        p.fields[1].value = "1".into();
        assert!(plan("mobile-data", "save", &p, &d).is_err());
        assert!(
            page("nat", &Data::new())
                .fields
                .iter()
                .all(|f| !f.available)
        );
    }
    #[test]
    fn dns_uses_guarded_transaction_and_does_not_allow_url_credentials() {
        let d = Data::from([(
            "settings".into(),
            Ok(
                json!({"mode":"doh","primary":"1.1.1.1","secondary":"","doh_url":"https://dns.example.com/dns-query","dot_endpoint":"","dot_server_name":""}),
            ),
        )]);
        let mut p = page("doh", &d);
        let a = plan("doh", "save", &p, &d).unwrap();
        assert_eq!(a.requests.len(), 1);
        assert_eq!(a.requests[0].path, "/api/dns/apply");
        p.fields[3].value = "https://user:pass@dns.example.com".into();
        assert!(plan("doh", "save", &p, &d).is_err());
    }
}
