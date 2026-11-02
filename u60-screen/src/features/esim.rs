use super::*;
use serde_json::json;
pub fn reads(id: &str) -> Option<Vec<Read>> {
    let list: &[(&str, &str)] = match id {
        "esim-profiles" => &[
            ("chip", "/api/esim/chip"),
            ("inventory", "/api/esim/profiles"),
            ("switch", "/api/esim/switch"),
        ],
        "esim-download" => &[
            ("inventory", "/api/esim/profiles"),
            ("download", "/api/esim/download"),
        ],
        "esim-receipts" => &[("receipts", "/api/esim/notifications")],
        _ => return None,
    };
    Some(list.iter().map(|(k, p)| Read::get(k, p)).collect())
}
fn fresh(d: &Value) -> bool {
    d["profiles"].is_array() && d["stale"] == false && d["updating"] == false
}
fn profile(readings: &Data) -> Option<&Value> {
    let selected = selected(readings)?;
    data(readings, "inventory")["profiles"]
        .as_array()?
        .iter()
        .find(|p| p["iccid"] == selected)
}
pub fn page(id: &str, readings: &Data) -> Page {
    let mut p = Page::default();
    let inventory = data(readings, "inventory");
    match id {
        "esim-profiles" => {
            let sw = data(readings, "switch");
            p.row("切换阶段", &sw["state"]);
            p.row("列表过期", &inventory["stale"]);
            if selected(readings).is_some() {
                if let Some(v) = profile(readings) {
                    p.title = Some(
                        super::network::first(v, &["nickname", "name", "provider"])
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .unwrap_or("eSIM 配置")
                            .into(),
                    );
                    for (k, l) in [
                        ("iccid", "ICCID"),
                        ("provider", "运营商"),
                        ("name", "套餐"),
                        ("state", "配置状态"),
                        ("class", "配置类型"),
                    ] {
                        p.row(l, &v[k]);
                    }
                    let mut f = Field::text("nickname", "自定义名称", &json!(text(&v["nickname"])));
                    f.max = 64;
                    p.fields.push(f);
                    for (a, l, w) in [
                        ("nickname", "保存名称", "空名称将清除自定义名称。"),
                        (
                            "enable",
                            "启用此 eSIM",
                            "切换 SIM 身份并重新注册网络，蜂窝数据、通话与短信会暂时中断。新套餐可能要求不同 APN 或漫游设置。",
                        ),
                        (
                            "delete",
                            "删除此 eSIM",
                            "永久删除此套餐配置，激活码通常不能再次使用。请确认已具备运营商的恢复方式。",
                        ),
                    ] {
                        let mut a =
                            Action::new(a, l, &format!("ICCID：{}\n{}", shown(&v["iccid"]), w));
                        a.enabled =
                            fresh(inventory) && (a.id == "nickname" || v["state"] == "disabled");
                        if a.id == "delete" {
                            a.phrase = Some("DELETE".into());
                        }
                        p.actions.push(a);
                    }
                } else {
                    p.note = "配置已不存在，请返回并刷新。".into();
                }
            } else {
                p.row("eSIM EID", &data(readings, "chip")["eid"]);
                p.row("芯片版本", &data(readings, "chip")["firmware"]);
                p.rows.push((
                    "剩余存储".into(),
                    super::status::quantity(
                        &data(readings, "chip")["free_nvm_bytes"],
                        1024.,
                        "KiB",
                    ),
                ));
                if let Some(profiles) = inventory["profiles"].as_array() {
                    p.items = profiles
                        .iter()
                        .filter_map(|v| {
                            Some(Item {
                                id: v["iccid"].as_str()?.into(),
                                title: super::network::first(v, &["nickname", "name", "provider"])
                                    .as_str()
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or("eSIM 配置")
                                    .into(),
                                subtitle: format!(
                                    "{} · {}",
                                    shown(&v["provider"]),
                                    shown(&v["state"])
                                ),
                            })
                        })
                        .collect();
                }
                p.note="点击配置查看、重命名、启用或删除。下载新配置请进入「下载 eSIM」。运营商回执由后台自动重试。".into();
            }
        }
        "esim-download" => {
            let d = data(readings, "download");
            let phase = text(&d["state"]);
            for (k, l) in [
                ("state", "下载阶段"),
                ("step", "当前步骤"),
                ("provider", "运营商"),
                ("name", "套餐名称"),
                ("iccid", "配置 ICCID"),
                ("error", "错误信息"),
            ] {
                p.row(l, &d[k]);
            }
            if ["idle", "done", "failed", "cancelled"].contains(&phase.as_str()) {
                p.fields
                    .push(Field::password("activation_code", "激活码 LPA:1$…", true));
                p.fields.push(Field::password(
                    "confirmation_code",
                    "运营商确认码（可选）",
                    true,
                ));
                let mut a = Action::new(
                    "start",
                    "获取套餐预览",
                    "连接运营商服务器读取套餐信息，之后仍需确认才安装。预览需要在两分钟内确认或取消。",
                );
                a.enabled = !d.is_null() && inventory["agent_instance"].is_u64();
                p.actions.push(a);
            }
            if phase == "confirm" {
                let mut a = Action::new(
                    "confirm",
                    "确认安装此套餐",
                    &format!(
                        "运营商：{}\n套餐：{}\nICCID：{}\n安装可能消耗一次性激活码。",
                        shown(&d["provider"]),
                        shown(&d["name"]),
                        shown(&d["iccid"])
                    ),
                );
                a.enabled = inventory["agent_instance"].is_u64();
                p.actions.push(a);
            }
            if ["running", "confirm"].contains(&phase.as_str()) {
                let mut a = Action::new(
                    "cancel",
                    "取消当前下载",
                    "请求在可取消阶段停止。已经写入芯片的安装过程无法靠取消撤回。",
                );
                a.enabled = !text(&d["step"]).starts_with("es10b_load_bound_profile_package");
                p.actions.push(a);
            }
            if !["idle", "running", "confirm", "cancelling", ""].contains(&phase.as_str()) {
                p.actions.push(Action::new(
                    "dismiss",
                    "清除结果显示",
                    "只关闭本次结果，不删除已安装的套餐。",
                ));
            }
            p.note="安装完成不代表已启用或联网；请到配置列表启用，并分别核对卡片切换、调制解调器身份与网络注册。".into();
        }
        "esim-receipts" => {
            let d = data(readings, "receipts");
            p.row("待处理回执", &d["count"]);
            p.row("状态过期", &d["stale"]);
            if let Some(pending) = d["pending"].as_array() {
                for v in pending {
                    p.rows.push((
                        format!("{} · {}", shown(&v["seq"]), shown(&v["operation"])),
                        format!(
                            "{}\n尝试 {} 次 · {}",
                            shown(&v["address"]),
                            shown(&v["attempts"]),
                            shown(&v["last_error"])
                        ),
                    ));
                }
            }
            let mut a = Action::new(
                "process",
                "重试待处理回执",
                "向运营商发送待处理的配置安装、启用或删除回执。后台会继续按策略重试。",
            );
            a.enabled = d["count"].as_u64().is_some_and(|n| n > 0);
            p.actions.push(a);
        }
        _ => {}
    }
    p
}
pub fn plan(id: &str, action: &str, p: &Page, readings: &Data) -> Result<Plan> {
    let inventory = data(readings, "inventory");
    match id {
        "esim-profiles" => {
            ensure!(fresh(inventory), "配置列表正在更新或已过期，请刷新。");
            let v = profile(readings).ok_or_else(|| anyhow::anyhow!("配置已不存在。"))?;
            ensure!(
                ["nickname", "enable", "delete"].contains(&action),
                "操作不存在。"
            );
            ensure!(
                action == "nickname" || v["state"] == "disabled",
                "请选择未启用的配置。"
            );
            let mut body = json!({"iccid":v["iccid"]});
            if action == "nickname" {
                body["nickname"] = p.value("nickname")?;
                ensure!(
                    !text(&body["nickname"]).chars().any(char::is_control),
                    "名称不能包含控制字符。"
                );
            }
            let mut plan = Plan::one(Method::POST, &format!("/api/esim/{action}"), body);
            plan.guards.push(Guard::new(
                Read::get("chip", "/api/esim/chip"),
                data(readings, "chip"),
                &["/eid"],
            ));
            plan.guards.push(Guard::new(
                Read::get("inventory", "/api/esim/profiles"),
                inventory,
                &[
                    "/agent_instance",
                    "/generation",
                    "/profiles",
                    "/stale",
                    "/updating",
                ],
            ));
            Ok(plan)
        }
        "esim-download" => {
            if action == "start" {
                let code = text(&p.value("activation_code")?);
                let parts = code.split('$').collect::<Vec<_>>();
                ensure!(
                    parts.len() >= 3
                        && parts[0].eq_ignore_ascii_case("LPA:1")
                        && parts[1].len() <= 253
                        && !code.chars().any(char::is_whitespace),
                    "激活码格式应为 LPA:1$服务器$激活标识。"
                );
                let url = reqwest::Url::parse(&format!("https://{}", parts[1]))?;
                ensure!(
                    url.host_str().is_some()
                        && url.username().is_empty()
                        && url.password().is_none()
                        && url.path() == "/"
                        && url.query().is_none()
                        && url.fragment().is_none(),
                    "激活服务器地址无效。"
                );
                let confirmation = p.value("confirmation_code")?;
                if parts.get(4) == Some(&"1") {
                    ensure!(!text(&confirmation).is_empty(), "该套餐需要运营商确认码。");
                }
                return Ok(Plan::one(
                    Method::POST,
                    "/api/esim/download",
                    json!({"activation_code":code,"confirmation_code":confirmation,"preview":true}),
                ));
            }
            let d = data(readings, "download");
            let instance = inventory["agent_instance"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("无法核对后台实例，请刷新。"))?;
            let started = d["started_at"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or_else(|| anyhow::anyhow!("没有可操作的下载记录。"))?;
            let mut body = json!({"expected":{"agent_instance":instance,"started_at":started}});
            let (method, path) = match action {
                "confirm" => {
                    ensure!(d["state"] == "confirm", "套餐不再等待确认。");
                    body["accept"] = json!(true);
                    (Method::POST, "/api/esim/download/confirm")
                }
                "cancel" => (Method::POST, "/api/esim/download/cancel"),
                "dismiss" => (Method::DELETE, "/api/esim/download"),
                _ => anyhow::bail!("操作不存在。"),
            };
            Ok(Plan::one(method, path, body))
        }
        "esim-receipts" if action == "process" => Ok(Plan::one(
            Method::POST,
            "/api/esim/notifications/process",
            json!({}),
        )),
        _ => anyhow::bail!("操作不存在。"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmation_identifies_the_reviewed_download_and_deletion_refuses_active_profile() {
        let d = Data::from([
            (
                "inventory".into(),
                Ok(json!({"agent_instance":12,"profiles":[],"stale":false,"updating":false})),
            ),
            (
                "download".into(),
                Ok(json!({"state":"confirm","started_at":1234,"provider":"Example"})),
            ),
        ]);
        let p = page("esim-download", &d);
        let b = plan("esim-download", "confirm", &p, &d).unwrap().requests[0]
            .body
            .clone()
            .unwrap();
        assert_eq!(
            b,
            json!({"accept":true,"expected":{"agent_instance":12,"started_at":1234}})
        );
        let d = Data::from([
            (
                "inventory".into(),
                Ok(
                    json!({"profiles":[{"iccid":"example","state":"enabled"}],"stale":false,"updating":false}),
                ),
            ),
            ("__selection".into(), Ok(json!("example"))),
        ]);
        assert!(plan("esim-profiles", "delete", &page("esim-profiles", &d), &d).is_err());
    }
}
