//! Small-screen controls share the browser service API and engine-specific data.
use super::*;
use serde_json::json;

pub fn reads(id: &str) -> Option<Vec<Read>> {
    match id {
        "proxy-service" => Some(vec![Read::get("status", "/api/proxy/mihomo/status")]),
        "proxy-groups" => Some(vec![
            Read::get("status", "/api/proxy/mihomo/status"),
            Read::get("groups", "/api/proxy/mihomo/groups"),
        ]),
        _ => None,
    }
}
pub fn page(id: &str, readings: &Data) -> Page {
    let d = data(readings, "status");
    let mut p = Page::default();
    if d.is_null() {
        p.note = crate::i18n::tr("代理服务接口不可用，请更新兼容的后台程序后重试。").into();
        return p;
    }
    let running = d["running"] == true;
    if id == "proxy-service" {
        p.row(crate::i18n::tr("代理内核"), &json!("mihomo"));
        p.row(
            crate::i18n::tr("内核版本"),
            &if d["installed"] == true {
                d.get("version")
                    .filter(|v| !v.is_null())
                    .cloned()
                    .unwrap_or(json!(crate::i18n::tr("已安装")))
            } else {
                json!(crate::i18n::tr("未安装"))
            },
        );
        p.row(
            crate::i18n::tr("配置"),
            &json!(if d["configured"] == true {
                crate::i18n::tr("已保存")
            } else {
                crate::i18n::tr("请在 Web 管理页导入")
            }),
        );
        p.row(
            crate::i18n::tr("运行状态"),
            &json!(if running {
                if d["capturing"] == true {
                    crate::i18n::tr("热点代理运行中")
                } else {
                    crate::i18n::tr("本机代理运行中")
                }
            } else {
                crate::i18n::tr("已停止")
            }),
        );
        if !d["last_error"].is_null() {
            p.row(crate::i18n::tr("运行提示"), &d["last_error"]);
        }
        if running {
            p.row(
                crate::i18n::tr("运行模式"),
                &json!(match d["mode"].as_str() {
                    Some("rule") => crate::i18n::tr("规则模式"),
                    Some("global") => crate::i18n::tr("全局模式"),
                    Some("direct") => crate::i18n::tr("直连模式"),
                    _ => crate::i18n::tr("未提供"),
                }),
            );
            p.row(
                crate::i18n::tr("流量范围"),
                &json!(if d["capture"] == "hotspot" {
                    crate::i18n::tr("Wi-Fi 热点设备")
                } else {
                    crate::i18n::tr("仅本机应用")
                }),
            );
        } else {
            p.fields.push(Field::choice(
                "mode",
                crate::i18n::tr("运行模式"),
                &d["mode"],
                &[
                    ("rule", crate::i18n::tr("规则模式")),
                    ("global", crate::i18n::tr("全局模式")),
                    ("direct", crate::i18n::tr("直连模式")),
                ],
            ));
            p.fields.push(Field::choice(
                "capture",
                crate::i18n::tr("流量范围"),
                &d["capture"],
                &[
                    ("local", crate::i18n::tr("仅本机应用")),
                    ("hotspot", crate::i18n::tr("Wi-Fi 热点设备")),
                ],
            ));
        }
        for (action, label, warning, enabled) in [
            (
                "enable",
                crate::i18n::tr("启动代理"),
                crate::i18n::tr("按已保存的范围启用代理。热点连接可能短暂重连。"),
                !running && d["installed"] == true && d["configured"] == true,
            ),
            (
                "disable",
                crate::i18n::tr("停止代理"),
                crate::i18n::tr("停止代理并恢复普通热点路由。"),
                running || d["enabled"] == true,
            ),
            (
                "restart",
                crate::i18n::tr("重启代理"),
                crate::i18n::tr("现有代理连接会中断，然后按保存配置重新连接。"),
                running,
            ),
            (
                "settings",
                crate::i18n::tr("保存运行模式"),
                crate::i18n::tr("保存下次启动使用的策略和流量范围。"),
                !running && !d.is_null(),
            ),
        ] {
            let mut a = Action::new(action, label, warning);
            a.enabled = enabled;
            p.actions.push(a);
        }
        for f in &mut p.fields {
            f.available &= !running;
        }
        p.note = crate::i18n::tr(
            "Web：网络 → 代理 → mihomo。编辑 YAML、DNS 与订阅请在 Web 完成；各代理服务独立运行。",
        )
        .into();
    } else {
        let groups = data(readings, "groups").as_array();
        if let Some(groups) = groups {
            p.items = groups
                .iter()
                .filter_map(|g| {
                    Some(Item {
                        id: g["name"].as_str()?.into(),
                        title: g["name"].as_str()?.into(),
                        subtitle: crate::i18n::format(
                            "当前：{0}",
                            &[shown(&g["selected"]).to_string()],
                        ),
                    })
                })
                .collect();
            if let Some(g) =
                selected(readings).and_then(|name| groups.iter().find(|g| g["name"] == name))
            {
                p.row(crate::i18n::tr("选择组"), &g["name"]);
                p.row(crate::i18n::tr("当前节点"), &g["selected"]);
                if g["selectable"] == true {
                    let mut f = Field::text("choice", crate::i18n::tr("切换到"), &g["selected"]);
                    f.choices = g["choices"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v.as_str())
                        .map(|v| Choice {
                            value: v.into(),
                            label: v.into(),
                        })
                        .collect();
                    p.fields.push(f);
                    let mut a = Action::new(
                        "select",
                        crate::i18n::tr("使用此节点"),
                        crate::i18n::tr("新连接会使用选择的节点；已有连接可能继续使用旧节点。"),
                    );
                    a.enabled = running;
                    p.actions.push(a);
                } else {
                    p.note = crate::i18n::tr("这个组按配置自动选择节点。").into();
                }
            }
        }
        if p.note.is_empty() {
            p.note = if running {
                crate::i18n::tr("先选择一个代理组，再切换节点。")
            } else {
                crate::i18n::tr("启动代理服务后读取代理组。")
            }
            .into();
        }
    }
    p
}
pub fn plan(id: &str, action: &str, p: &Page, readings: &Data) -> Result<Plan> {
    let d = data(readings, "status");
    ensure!(!d.is_null(), "{}", crate::i18n::tr("请先读取代理状态。"));
    let mut plan = if id == "proxy-service" {
        ensure!(
            ["enable", "disable", "restart", "settings"].contains(&action),
            "{}",
            crate::i18n::tr("操作不存在。")
        );
        if action == "settings" {
            ensure!(
                d["running"] != true,
                "{}",
                crate::i18n::tr("请先停止服务。")
            );
            ensure!(p.dirty(), "{}", crate::i18n::tr("没有需要保存的更改。"));
            Plan::one(
                Method::PUT,
                "/api/proxy/mihomo/settings",
                p.changed(&["mode", "capture"])?,
            )
        } else {
            Plan::one(
                Method::POST,
                &format!("/api/proxy/mihomo/{action}"),
                json!({}),
            )
        }
    } else {
        ensure!(
            id == "proxy-groups" && action == "select" && d["running"] == true,
            "{}",
            crate::i18n::tr("请先启动服务并选择代理组。")
        );
        let name = selected(readings)
            .ok_or_else(|| anyhow::anyhow!("{}", crate::i18n::tr("请先选择代理组。")))?;
        let choice = p.value("choice")?;
        let group = data(readings, "groups")
            .as_array()
            .and_then(|a| a.iter().find(|g| g["name"] == name))
            .ok_or_else(|| anyhow::anyhow!("{}", crate::i18n::tr("代理组已变化，请刷新。")))?;
        ensure!(
            group["selectable"] == true
                && group["choices"]
                    .as_array()
                    .is_some_and(|a| a.contains(&choice)),
            "{}",
            crate::i18n::tr("请选择当前组中的有效节点。")
        );
        Plan::one(
            Method::POST,
            "/api/proxy/mihomo/select",
            json!({"group":name,"choice":choice}),
        )
    };
    plan.guards.push(Guard::new(
        Read::get("status", "/api/proxy/mihomo/status"),
        d,
        &["/id", "/running", "/configured", "/mode", "/capture"],
    ));
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn groups_reject_stale_or_automatic_selections() {
        let mut d = Data::new();
        d.insert("status".into(), Ok(json!({"running":true})));
        d.insert("groups".into(),Ok(json!([{"name":"Example group","selected":"A","selectable":true,"choices":["A","B"]}])));
        d.insert("__selection".into(), Ok(json!("Example group")));
        let mut p = page("proxy-groups", &d);
        p.fields[0].value = "B".into();
        assert!(plan("proxy-groups", "select", &p, &d).is_ok());
        d.get_mut("groups").unwrap().as_mut().unwrap()[0]["selectable"] = json!(false);
        assert!(plan("proxy-groups", "select", &p, &d).is_err());
    }
    #[test]
    fn active_policy_is_not_editable() {
        let d = Data::from([(
            "status".into(),
            Ok(json!({"running":true,"mode":"rule","capture":"hotspot"})),
        )]);
        let p = page("proxy-service", &d);
        assert!(p.fields.is_empty());
        assert!(
            p.rows
                .iter()
                .any(|(label, value)| label == "运行模式" && value == "规则模式")
        );
        assert!(plan("proxy-service", "settings", &p, &d).is_err());
    }
}
