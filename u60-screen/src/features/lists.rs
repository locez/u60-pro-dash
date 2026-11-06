//! Stable-identity list editors; destructive actions re-read the reviewed list.
use super::network::{bit, first, lan_host};
use super::*;
use serde_json::json;
const CUT: &str = "更改可能中断网络。请核对所选记录；未保存的编辑不会被启用或删除操作一起提交。";
pub fn reads(id: &str) -> Option<Vec<Read>> {
    let v: &[(&str, &str)] = match id {
        "apn" => &[
            ("profiles", "/api/router/apn/profiles"),
            ("mode", "/api/router/apn/mode"),
            ("automatic", "/api/router/apn/auto-profiles"),
        ],
        "ports" => &[
            ("rules", "/api/router/firewall/port-forward"),
            ("firewall", "/api/router/firewall"),
            ("lan", "/api/router/lan"),
        ],
        "domain" => &[("rules", "/api/router/domain-filter")],
        "operators" => &[
            ("scan", "/api/modem/scan/status"),
            ("operators", "/api/modem/scan/results"),
            ("registration", "/api/modem/register/result"),
            ("signal", "/api/network/signal"),
        ],
        _ => return None,
    };
    Some(v.iter().map(|(k, p)| Read::get(k, p)).collect())
}
fn array<'a>(d: &'a Value, key: &str) -> &'a [Value] {
    d[key].as_array().map(Vec::as_slice).unwrap_or_default()
}
fn rule_id(v: &Value, i: usize) -> String {
    let id = text(first(v, &["id", "section_id"]));
    if id.is_empty() { i.to_string() } else { id }
}
fn rule<'a>(d: &'a Value, id: &str) -> Result<&'a Value> {
    let found = array(d, "rule_list")
        .iter()
        .enumerate()
        .filter(|(i, v)| rule_id(v, *i) == id)
        .map(|(_, v)| v)
        .collect::<Vec<_>>();
    ensure!(
        found.len() == 1,
        "{}",
        crate::i18n::tr("记录已变化或标识重复，请刷新。")
    );
    Ok(found[0])
}
fn required(key: &str, label: &str, value: &Value, max: usize) -> Field {
    let mut f = Field::text(key, label, value);
    f.required = true;
    f.max = max;
    f
}
fn new_item(p: &mut Page, label: &str) {
    p.items.insert(
        0,
        Item {
            id: "new".into(),
            title: label.into(),
            subtitle: crate::i18n::tr("填写后确认保存").into(),
        },
    );
}
pub fn page(id: &str, readings: &Data) -> Page {
    let mut p = Page::default();
    let selected = selected(readings);
    match id {
        "apn" => {
            let d = data(readings, "profiles");
            let profiles = array(d, "apnListArray");
            if let Some(id) = selected {
                let found = profiles.iter().find(|v| text(&v["profileId"]) == id);
                if id != "new" && found.is_none() {
                    p.note = crate::i18n::tr("配置已不存在，请返回列表刷新。").into();
                    return p;
                }
                let defaults =
                    json!({"profilename":"","wanapn":"","pdpType":3,"pppAuthMode":0,"username":""});
                let v = found.unwrap_or(&defaults);
                p.title = Some(if id == "new" {
                    crate::i18n::tr("新增 APN").into()
                } else {
                    shown(&v["profilename"])
                });
                p.fields = vec![
                    required(
                        "profilename",
                        crate::i18n::tr("配置名称"),
                        &v["profilename"],
                        128,
                    ),
                    required("wanapn", "APN", &v["wanapn"], 128),
                    Field::choice(
                        "pdpType",
                        crate::i18n::tr("IP 类型"),
                        &v["pdpType"],
                        &[("1", "IPv4"), ("2", "IPv6"), ("3", "IPv4 + IPv6")],
                    ),
                    Field::choice(
                        "pppAuthMode",
                        crate::i18n::tr("认证方式"),
                        &v["pppAuthMode"],
                        &[("0", crate::i18n::tr("无")), ("1", "PAP"), ("2", "CHAP")],
                    ),
                    Field::text(
                        "username",
                        crate::i18n::tr("用户名"),
                        &json!(text(&v["username"])),
                    ),
                    Field::password("password", crate::i18n::tr("新密码"), true),
                ];
                p.actions
                    .push(Action::new("save", crate::i18n::tr("保存 APN"), CUT));
                if found.is_some() {
                    p.row(crate::i18n::tr("当前启用"), &v["isEnable"]);
                    p.actions
                        .push(Action::new("activate", crate::i18n::tr("启用此配置"), CUT));
                    let mut a = Action::new(
                        "delete",
                        crate::i18n::tr("删除此配置"),
                        crate::i18n::tr("永久删除此 APN 配置。当前启用的配置不能删除。"),
                    );
                    a.enabled = flag(&v["isEnable"]) == Some(false);
                    p.actions.push(a);
                }
                for f in &mut p.fields {
                    f.available &= d["apnListArray"].is_array();
                }
            } else {
                p.fields.push(Field::choice(
                    "apn_mode",
                    crate::i18n::tr("APN 选择"),
                    &data(readings, "mode")["apn_mode"],
                    &[
                        ("0", crate::i18n::tr("运营商自动配置")),
                        ("1", crate::i18n::tr("手动配置")),
                    ],
                ));
                p.actions
                    .push(Action::new("mode", crate::i18n::tr("保存选择方式"), CUT));
                p.items = profiles
                    .iter()
                    .filter_map(|v| {
                        let id = text(&v["profileId"]);
                        (!id.is_empty()).then(|| Item {
                            id,
                            title: shown(&v["profilename"]),
                            subtitle: format!(
                                "{} · {}",
                                shown(&v["wanapn"]),
                                if flag(&v["isEnable"]) == Some(true) {
                                    crate::i18n::tr("已启用")
                                } else {
                                    crate::i18n::tr("未启用")
                                }
                            ),
                        })
                    })
                    .collect();
                if d["apnListArray"].is_array() {
                    new_item(&mut p, crate::i18n::tr("新增 APN"));
                }
                if let Some(list) = data(readings, "automatic")["apnListArray"].as_array() {
                    for v in list {
                        p.rows.push((
                            crate::i18n::tr("运营商配置").into(),
                            format!("{} · {}", shown(&v["profilename"]), shown(&v["wanapn"])),
                        ));
                    }
                }
                p.note = crate::i18n::tr(
                    "编辑手动配置时密码留空保留原值。选择手动模式后，启用指定配置才用于蜂窝连接。",
                )
                .into();
            }
        }
        "ports" | "domain" => {
            let d = data(readings, "rules");
            let entries = array(d, "rule_list");
            let is_port = id == "ports";
            if let Some(selected) = selected {
                p.title = Some(if selected == "new" {
                    if is_port {
                        crate::i18n::tr("新增端口转发")
                    } else {
                        crate::i18n::tr("新增域名过滤")
                    }
                    .into()
                } else {
                    if is_port {
                        crate::i18n::tr("转发规则")
                    } else {
                        crate::i18n::tr("域名过滤")
                    }
                    .into()
                });
                if selected == "new" {
                    if is_port {
                        p.fields = vec![
                            required("name", crate::i18n::tr("规则名称"), &json!(""), 64),
                            Field::choice(
                                "protocol",
                                crate::i18n::tr("协议"),
                                &json!("tcp"),
                                &[("tcp", "TCP"), ("udp", "UDP"), ("tcp+udp", "TCP + UDP")],
                            ),
                            Field::number(
                                "wan_port",
                                crate::i18n::tr("外部端口"),
                                &json!(""),
                                1,
                                65535,
                            ),
                            required("lan_ip", crate::i18n::tr("目标 IPv4"), &json!(""), 15),
                            Field::number(
                                "lan_port",
                                crate::i18n::tr("内部端口"),
                                &json!(""),
                                1,
                                65535,
                            ),
                            bit("enabled", crate::i18n::tr("启用规则"), &json!("1")),
                        ];
                    } else {
                        p.fields
                            .push(required("domain", crate::i18n::tr("域名"), &json!(""), 253));
                    }
                    p.actions.push(Action::new(
                        "add",
                        crate::i18n::tr("添加规则"),
                        if is_port {
                            crate::i18n::tr(
                                "把入站端口映射至指定局域网设备。目标应启用自己的防火墙。",
                            )
                        } else {
                            crate::i18n::tr("阻止此域名访问；可能影响依赖它的服务。")
                        },
                    ));
                } else if let Ok(v) = rule(d, selected) {
                    if is_port {
                        for (keys, label) in [
                            (&["name", "comment"][..], crate::i18n::tr("名称")),
                            (&["protocol", "proto"], crate::i18n::tr("协议")),
                            (&["wan_port", "src_dport"], crate::i18n::tr("外部端口")),
                            (&["lan_ip", "dest_ip"], crate::i18n::tr("目标 IP")),
                            (&["lan_port", "dest_port"], crate::i18n::tr("内部端口")),
                        ] {
                            p.row(label, first(v, keys));
                        }
                    } else {
                        p.row(crate::i18n::tr("域名"), first(v, &["domain", "fqdn"]));
                    }
                    p.actions.push(Action::new(
                        "delete",
                        crate::i18n::tr("删除规则"),
                        crate::i18n::tr(
                            "永久移除此规则。提交前会再次核对规则列表，列表变化时取消操作。",
                        ),
                    ));
                    p.note = crate::i18n::tr("修改已有规则时，请删除后按新参数重新添加。").into();
                } else {
                    p.note = crate::i18n::tr("记录已变化，请返回列表刷新。").into();
                }
            } else {
                if is_port {
                    p.fields.push(bit(
                        "enabled",
                        crate::i18n::tr("端口转发服务"),
                        first(
                            data(readings, "firewall"),
                            &["port_forward_switch", "portforward_enable"],
                        ),
                    ));
                } else {
                    p.fields.push(bit(
                        "enabled",
                        crate::i18n::tr("域名过滤服务"),
                        &d["enable"],
                    ));
                }
                p.actions
                    .push(Action::new("switch", crate::i18n::tr("保存服务开关"), CUT));
                p.items = entries
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Item {
                        id: rule_id(v, i),
                        title: shown(first(
                            v,
                            if is_port {
                                &["name", "comment"]
                            } else {
                                &["domain", "fqdn"]
                            },
                        )),
                        subtitle: if is_port {
                            format!(
                                "{} {} → {}:{}",
                                shown(first(v, &["protocol", "proto"])),
                                shown(first(v, &["wan_port", "src_dport"])),
                                shown(first(v, &["lan_ip", "dest_ip"])),
                                shown(first(v, &["lan_port", "dest_port"]))
                            )
                        } else {
                            crate::i18n::tr("点按查看和删除").into()
                        },
                    })
                    .collect();
                if d["rule_list"].is_array() {
                    new_item(&mut p, crate::i18n::tr("添加规则"));
                }
            }
        }
        "operators" => {
            let list = operators(data(readings, "operators"));
            p.row(
                crate::i18n::tr("扫描状态"),
                &data(readings, "scan")["m_netselect_status"],
            );
            p.row(
                crate::i18n::tr("注册结果"),
                &data(readings, "registration")["m_netselect_result"],
            );
            p.row(
                crate::i18n::tr("选择方式"),
                &data(readings, "signal")["net_select_mode"],
            );
            if let Some(id) = selected {
                if let Some(v) = list.iter().find(|v| v.id == id) {
                    p.title = Some(v.name.clone());
                    p.row(crate::i18n::tr("网络 PLMN"), &json!(v.plmn));
                    p.row(crate::i18n::tr("接入制式"), &json!(v.rat));
                    p.row(crate::i18n::tr("可用状态"), &json!(v.status));
                    let mut a = Action::new(
                        "register",
                        crate::i18n::tr("注册此网络"),
                        crate::i18n::tr(
                            "切换运营商可能中断网络，漫游可能产生费用。需要 SIM 与套餐允许注册该网络。",
                        ),
                    );
                    a.enabled = v.allowed();
                    p.actions.push(a);
                }
            } else {
                p.items = list
                    .into_iter()
                    .map(|v| Item {
                        id: v.id,
                        title: v.name,
                        subtitle: format!("{} · {} · {}", v.plmn, v.rat, v.status),
                    })
                    .collect();
                p.actions.push(Action::new(
                    "scan",
                    crate::i18n::tr("扫描运营商"),
                    crate::i18n::tr(
                        "扫描可能持续数分钟，其间蜂窝数据可能中断。离开页面不会停止设备扫描。",
                    ),
                ));
                p.actions
                    .push(Action::new("auto", crate::i18n::tr("自动选择运营商"), CUT));
            }
            p.note = crate::i18n::tr(
                "列表可能包含固件缓存。注册请求被接受不等于蜂窝已联网；请核对实时网络状态。",
            )
            .into();
        }
        _ => {}
    }
    p
}
pub fn plan(id: &str, action: &str, p: &Page, readings: &Data) -> Result<Plan> {
    let selected = selected(readings);
    let mut plan = match id {
        "apn" => {
            if action == "mode" {
                return Ok(Plan::one(
                    Method::PUT,
                    "/api/router/apn/mode",
                    json!({"apn_mode":text(&p.value("apn_mode")?).parse::<u8>()?}),
                ));
            }
            let d = data(readings, "profiles");
            ensure!(
                d["apnListArray"].is_array(),
                "{}",
                crate::i18n::tr("无法读取 APN 列表。")
            );
            let selected =
                selected.ok_or_else(|| anyhow::anyhow!("{}", crate::i18n::tr("请先选择配置。")))?;
            let found = array(d, "apnListArray")
                .iter()
                .filter(|v| text(&v["profileId"]) == selected)
                .collect::<Vec<_>>();
            ensure!(
                selected == "new" || found.len() == 1,
                "{}",
                crate::i18n::tr("配置标识已变化。")
            );
            let existing = found.first().copied();
            match action {
                "save" => {
                    let mut body = p.all(&[
                        "profilename",
                        "wanapn",
                        "pdpType",
                        "pppAuthMode",
                        "username",
                    ])?;
                    for k in ["pdpType", "pppAuthMode"] {
                        body[k] = json!(text(&body[k]).parse::<u8>()?);
                    }
                    ensure!(
                        !array(d, "apnListArray")
                            .iter()
                            .any(|v| text(&v["profileId"]) != selected
                                && v["profilename"] == body["profilename"]),
                        "{}",
                        crate::i18n::tr("配置名称已存在。")
                    );
                    let password = p.value("password")?;
                    body["password"] = if text(&password).is_empty() {
                        existing.map(|v| v["password"].clone()).unwrap_or(json!(""))
                    } else {
                        password
                    };
                    if let Some(v) = existing {
                        ensure!(
                            !body["password"].is_null(),
                            "{}",
                            crate::i18n::tr("原密码不可读，请填写新密码。")
                        );
                        body["profileId"] = v["profileId"].clone();
                    }
                    Plan::one(
                        if existing.is_some() {
                            Method::PUT
                        } else {
                            Method::POST
                        },
                        "/api/router/apn/profiles",
                        body,
                    )
                }
                "activate" | "delete" => {
                    let v = existing.ok_or_else(|| {
                        anyhow::anyhow!("{}", crate::i18n::tr("请选择已有配置。"))
                    })?;
                    if action == "delete" {
                        ensure!(
                            flag(&v["isEnable"]) == Some(false),
                            "{}",
                            crate::i18n::tr("不能删除当前启用或状态未知的配置。")
                        );
                    }
                    Plan::one(
                        Method::POST,
                        &format!("/api/router/apn/profiles/{action}"),
                        json!({"profileId":v["profileId"]}),
                    )
                }
                _ => anyhow::bail!("{}", crate::i18n::tr("操作不存在。")),
            }
        }
        "ports" | "domain" => {
            let port = id == "ports";
            let api = if port {
                "/api/router/firewall/port-forward"
            } else {
                "/api/router/domain-filter"
            };
            let d = data(readings, "rules");
            match action {
                "switch" => {
                    let value = p.value("enabled")?;
                    return Ok(if port {
                        Plan::one(
                            Method::PUT,
                            "/api/router/firewall/port-forward/switch",
                            json!({"port_forward_switch":value,"portforward_enable":if value=="1"{1}else{0}}),
                        )
                    } else {
                        Plan::one(Method::PUT, api, json!({"enable":if value=="1"{1}else{0}}))
                    });
                }
                "add" => {
                    ensure!(
                        d["rule_list"].is_array(),
                        "{}",
                        crate::i18n::tr("无法读取规则列表。")
                    );
                    let body = if port {
                        port_body(p, data(readings, "lan"), array(d, "rule_list"))?
                    } else {
                        let domain = text(&p.value("domain")?).trim().to_ascii_lowercase();
                        ensure!(
                            super::network::domain(&domain),
                            "{}",
                            crate::i18n::tr("请输入有效域名，例如 ads.example.com。")
                        );
                        ensure!(
                            !array(d, "rule_list").iter().any(|v| text(first(
                                v,
                                &["domain", "fqdn"]
                            ))
                            .eq_ignore_ascii_case(&domain)),
                            "{}",
                            crate::i18n::tr("此域名已存在。")
                        );
                        json!({"action":"add","domain":domain,"fqdn":domain,"enabled":"1","enable":1})
                    };
                    Plan::one(if port { Method::POST } else { Method::PUT }, api, body)
                }
                "delete" => {
                    let id = selected
                        .ok_or_else(|| anyhow::anyhow!("{}", crate::i18n::tr("请先选择规则。")))?;
                    rule(d, id)?;
                    let mut body = json!({"action":"delete","id":id,"section_id":[id]});
                    if port {
                        body["expected_rules"] = d["rule_list"].clone();
                    }
                    Plan::one(if port { Method::POST } else { Method::PUT }, api, body)
                }
                _ => anyhow::bail!("{}", crate::i18n::tr("操作不存在。")),
            }
        }
        "operators" => match action {
            "auto" => Plan::one(
                Method::PUT,
                "/api/modem/network-mode",
                json!({"net_select_mode":"auto_select"}),
            ),
            "scan" => {
                let s = text(&data(readings, "scan")["m_netselect_status"]).to_lowercase();
                ensure!(
                    !["scanning", "running", "in_progress", "1"].contains(&s.as_str()),
                    "{}",
                    crate::i18n::tr("扫描仍在运行。")
                );
                Plan::one(Method::POST, "/api/modem/scan", json!({}))
            }
            "register" => {
                let list = operators(data(readings, "operators"));
                let v = list
                    .iter()
                    .find(|v| Some(v.id.as_str()) == selected)
                    .ok_or_else(|| anyhow::anyhow!("{}", crate::i18n::tr("网络已不在列表中。")))?;
                ensure!(v.allowed(), "{}", crate::i18n::tr("此运营商不可选择。"));
                Plan::one(
                    Method::POST,
                    "/api/modem/register",
                    json!({"m_mcc_mnc":v.plmn,"m_rat":v.rat}),
                )
            }
            _ => anyhow::bail!("{}", crate::i18n::tr("操作不存在。")),
        },
        _ => anyhow::bail!("{}", crate::i18n::tr("操作不存在。")),
    };
    let guard = match id {
        "apn" => Some(("profiles", "/api/router/apn/profiles", "/apnListArray")),
        "ports" => Some(("rules", "/api/router/firewall/port-forward", "/rule_list")),
        "domain" => Some(("rules", "/api/router/domain-filter", "/rule_list")),
        _ => None,
    };
    if let Some((key, path, pointer)) = guard {
        plan.guards.push(Guard::new(
            Read::get(key, path),
            data(readings, key),
            &[pointer],
        ));
    }
    if id == "ports" && action == "add" {
        plan.guards.push(Guard::new(
            Read::get("lan", "/api/router/lan"),
            data(readings, "lan"),
            &["/lan_ipaddr", "/lan_netmask"],
        ));
    }
    Ok(plan)
}
fn port_body(p: &Page, lan: &Value, existing: &[Value]) -> Result<Value> {
    let mut b = p.all(&[
        "name", "protocol", "wan_port", "lan_ip", "lan_port", "enabled",
    ])?;
    let name = text(&b["name"]);
    ensure!(
        !name
            .chars()
            .any(|c| c.is_control() || "'\";$`\\|<>&".contains(c)),
        "{}",
        crate::i18n::tr("规则名称包含固件不能存储的字符。")
    );
    lan_host(&text(&b["lan_ip"]), lan)?;
    let external = b["wan_port"].as_u64().unwrap();
    for r in existing {
        let proto = text(first(r, &["protocol", "proto"])).to_lowercase();
        let same =
            proto == text(&b["protocol"]) || proto == "tcp+udp" || b["protocol"] == "tcp+udp";
        let ports = text(first(r, &["wan_port", "src_dport"]));
        let mut range = ports.split([':', '-']);
        let a = range.next().and_then(|s| s.parse::<u64>().ok());
        let z = range.next().and_then(|s| s.parse::<u64>().ok()).or(a);
        ensure!(
            !same || !a.zip(z).is_some_and(|(a, z)| (a..=z).contains(&external)),
            "{}",
            crate::i18n::tr("此外部端口与已有协议规则冲突。")
        );
    }
    b["wan_port"] = json!(text(&b["wan_port"]));
    b["lan_port"] = json!(text(&b["lan_port"]));
    b["action"] = json!("add");
    b["comment"] = b["name"].clone();
    b["proto"] = b["protocol"].clone();
    b["src_dport"] = b["wan_port"].clone();
    b["dest_ip"] = b["lan_ip"].clone();
    b["dest_port"] = b["lan_port"].clone();
    Ok(b)
}
struct Operator {
    id: String,
    name: String,
    plmn: String,
    rat: String,
    status: String,
}
impl Operator {
    fn allowed(&self) -> bool {
        (5..=6).contains(&self.plmn.len())
            && self.plmn.bytes().all(|b| b.is_ascii_digit())
            && !self.rat.is_empty()
            && !matches!(self.status.as_str(), "3" | "forbidden")
    }
}
fn operators(data: &Value) -> Vec<Operator> {
    let raw = &data["m_netselect_contents"];
    let rows = if let Some(v) = raw.as_array() {
        v.clone()
    } else {
        text(raw)
            .split(';')
            .filter_map(|s| {
                let p = s.split(',').map(str::trim).collect::<Vec<_>>();
                (p.len() == 4).then(|| json!({"status":p[0],"name":p[1],"plmn":p[2],"rat":p[3]}))
            })
            .collect()
    };
    let mut out: Vec<Operator> = vec![];
    for r in rows {
        let plmn = text(first(&r, &["mcc_mnc", "plmn"]));
        let rat = text(&r["rat"]);
        let id = format!("{plmn}/{rat}");
        if out.iter().any(|v| v.id == id) {
            continue;
        }
        out.push(Operator {
            id,
            name: shown(first(&r, &["name", "operator_name", "plmn", "mcc_mnc"])),
            plmn,
            rat,
            status: text(&r["status"]).to_lowercase(),
        });
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deleting_an_indexed_rule_carries_the_reviewed_list_and_guard() {
        let d = Data::from([
            (
                "rules".into(),
                Ok(json!({"rule_list":[{"name":"sample","wan_port":"80"}]})),
            ),
            ("__selection".into(), Ok(json!("0"))),
        ]);
        let p = page("ports", &d);
        let plan = plan("ports", "delete", &p, &d).unwrap();
        assert_eq!(plan.guards.len(), 1);
        assert_eq!(
            plan.requests[0].body.as_ref().unwrap()["expected_rules"],
            data(&d, "rules")["rule_list"]
        );
        assert!(!plan.guards[0].matches(&json!({"rule_list":[]})));
    }
    #[test]
    fn operators_parse_both_firmware_shapes_and_forbid_invalid_registration() {
        let rows = operators(
            &json!({"m_netselect_contents":"1,Alpha,00101,7;3,Beta,00102,7;1,Alpha,00101,7"}),
        );
        assert_eq!(rows.len(), 2);
        assert!(rows[0].allowed());
        assert!(!rows[1].allowed());
        assert!(super::super::network::domain("ads.example.com"));
    }
}
