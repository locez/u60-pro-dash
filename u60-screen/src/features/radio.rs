use super::network::first;
use super::*;
use serde_json::json;
const WARNING: &str = "限制频段或小区可能导致无服务，需要在机身或本地连接下解除限制。应用请求被接受不代表已成功注册。";
const STC: [(&str, &str, &str); 4] = [
    ("stc_lte_collect_timer", "lte_collect_timer", "LTE 采集周期"),
    (
        "stc_nrsa_collect_timer",
        "nrsa_collect_timer",
        "NR SA 采集周期",
    ),
    (
        "stc_lte_white_list_num_max",
        "lte_whitelist_max",
        "LTE 白名单数量",
    ),
    (
        "stc_nrsa_white_list_num_max",
        "nrsa_whitelist_max",
        "NR SA 白名单数量",
    ),
];
pub fn reads(id: &str) -> Option<Vec<Read>> {
    let v: &[(&str, &str)] = match id {
        "nr-bands" | "lte-bands" | "nr-lock" | "lte-lock" => &[("signal", "/api/network/signal")],
        "neighbors" => &[
            ("nr", "/api/cell/neighbors/nr"),
            ("lte", "/api/cell/neighbors/lte"),
        ],
        "stc" => &[
            ("params", "/api/cell/stc/params"),
            ("status", "/api/cell/stc/status"),
        ],
        "signal-test" => &[
            ("progress", "/api/cell/signal-detect/progress"),
            ("results", "/api/cell/signal-detect/results"),
        ],
        "speedtest" => &[
            ("servers", "/api/speedtest/servers"),
            ("progress", "/api/speedtest/progress"),
        ],
        "at-terminal" => &[("port", "/api/at/port")],
        _ => return None,
    };
    Some(v.iter().map(|(k, p)| Read::get(k, p)).collect())
}
pub fn page(id: &str, readings: &Data) -> Page {
    let mut p = Page::default();
    match id {
        "nr-bands" | "lte-bands" => {
            let mut bands = Field::text("bands", "频段列表（逗号分隔）", &json!(""));
            bands.required = true;
            bands.max = 512;
            p.fields.push(bands);
            if id == "nr-bands" {
                p.fields.push(Field::choice(
                    "mode",
                    "5G 模式",
                    &json!("both"),
                    &[("both", "NSA 与 SA"), ("nsa", "仅 NSA"), ("sa", "仅 SA")],
                ));
            }
            p.actions
                .push(Action::new("apply", "应用频段偏好", WARNING));
            p.actions.push(Action::new(
                "reset",
                "恢复自动选频",
                "清除 LTE 和 NR 的频段限制，设备可能重新注册网络。",
            ));
            p.row(
                "当前工作频段",
                &data(readings, "signal")[if id == "nr-bands" {
                    "nr5g_action_band"
                } else {
                    "wan_active_band"
                }],
            );
            p.note="此接口不提供当前限制列表；工作频段不等于已设置的偏好。请输入完整列表，例如 1,3,78。".into();
        }
        "nr-lock" | "lte-lock" => {
            let nr = id == "nr-lock";
            p.fields = vec![
                Field::number(
                    "pci",
                    "物理小区 PCI",
                    &json!(""),
                    0,
                    if nr { 1007 } else { 503 },
                ),
                Field::number(
                    "earfcn",
                    if nr { "NR-ARFCN" } else { "EARFCN" },
                    &json!(""),
                    0,
                    if nr { 3_279_165 } else { 262_143 },
                ),
            ];
            if nr {
                p.fields
                    .push(Field::number("band", "NR 频段", &json!(""), 1, 1024));
            }
            p.actions.push(Action::new("apply", "锁定此小区", WARNING));
            p.actions.push(Action::new(
                "reset",
                "解除全部小区锁定",
                "恢复自动选择服务小区，可能重新注册网络。",
            ));
            let s = data(readings, "signal");
            p.row("当前 PCI", &s[if nr { "nr5g_pci" } else { "lte_pci" }]);
            p.row(
                "当前信道",
                &s[if nr {
                    "nr5g_action_channel"
                } else {
                    "wan_active_channel"
                }],
            );
            p.note = "PCI 不是物理基站唯一编号，需同时确认频段和信道。".into();
        }
        "neighbors" => {
            for (key, name) in [("nr", "5G NR"), ("lte", "4G LTE")] {
                if let Some(list) = data(readings, key)["cell_list"].as_array() {
                    for c in list {
                        p.rows.push((
                            format!("{name} · {} · PCI {}", shown(&c["band"]), shown(&c["pci"])),
                            format!(
                                "信道 {} · RSRP {} dBm",
                                shown(&c["earfcn"]),
                                shown(&c["rsrp"])
                            ),
                        ));
                    }
                }
            }
            p.actions.push(Action::new(
                "scan",
                "重新扫描邻区",
                "扫描可能短暂影响蜂窝数据。固件可能保留以前的邻区结果。",
            ));
            p.note = "邻区不是额外的聚合载波，也不能直接作为物理站点数量。".into();
        }
        "stc" => {
            let params = data(readings, "params");
            for (wire, alias, label) in STC {
                let mut f = Field::number(
                    wire,
                    label,
                    first(params, &[wire, alias]),
                    0,
                    i32::MAX as i64,
                );
                f.wire = Wire::String;
                p.fields.push(f);
            }
            p.row(
                "STC 状态",
                first(data(readings, "status"), &["stc_enable", "status"]),
            );
            for (action, label) in [
                ("save", "保存小区策略"),
                ("enable", "启用 STC"),
                ("disable", "停止 STC"),
                ("reset", "重置 STC"),
            ] {
                let mut a = Action::new(action, label, WARNING);
                a.enabled = !params.is_null() && !data(readings, "status").is_null();
                p.actions.push(a);
            }
        }
        "signal-test" => {
            let d = data(readings, "progress");
            p.row("检测进度 %", &d["progress"]);
            p.row("检测运行", &d["running"]);
            if let Some(list) = data(readings, "results")["record_list"].as_array() {
                for c in list {
                    p.rows.push((
                        format!(
                            "{} · {} · PCI {}",
                            shown(&c["type"]),
                            shown(&c["band"]),
                            shown(&c["pci"])
                        ),
                        format!(
                            "信道 {}\nRSRP {} dBm\nRSRQ {} dB · SINR {} dB",
                            shown(&c["earfcn"]),
                            shown(&c["rsrp"]),
                            shown(&c["rsrq"]),
                            shown(&c["sinr"])
                        ),
                    ));
                }
            }
            for (action, label) in [("start", "开始信号检测"), ("stop", "停止信号检测")]
            {
                let mut a = Action::new(
                    action,
                    label,
                    "检测期间蜂窝连接可能中断。离开页面后检测仍可能继续，请查看实际进度。",
                );
                a.enabled = !d.is_null() && (action == "stop" || flag(&d["running"]) != Some(true));
                p.actions.push(a);
            }
        }
        "speedtest" => {
            let progress = data(readings, "progress");
            for (k, l) in [
                ("phase", "测速阶段"),
                ("progress", "进度 %"),
                ("server", "测速服务器"),
                ("live_speed_mbps", "当前速度 / Mbps"),
                ("download_mbps", "下载 / Mbps"),
                ("upload_mbps", "上传 / Mbps"),
                ("ping_ms", "延迟 / ms"),
                ("jitter_ms", "抖动 / ms"),
            ] {
                p.row(l, &progress[k]);
            }
            for (k, l) in [
                ("download_bytes", "测试已下载"),
                ("upload_bytes", "测试已上传"),
            ] {
                p.rows
                    .push((l.into(), super::status::quantity(&progress[k], 1e6, "MB")));
            }
            let mut server = Field::text("server_id", "测速服务器", &json!(""));
            server.required = true;
            server.wire = Wire::Integer;
            server.range = Some((1, i64::MAX));
            if let Some(servers) = data(readings, "servers").as_array() {
                server.choices = servers
                    .iter()
                    .filter(|s| super::status::number(&s["id"]).is_some_and(|n| n > 0.))
                    .map(|s| Choice {
                        value: text(&s["id"]),
                        label: format!("{} · {}", shown(&s["sponsor"]), shown(&s["name"])),
                    })
                    .collect();
            }
            server.available = !server.choices.is_empty();
            p.fields.push(server);
            let active =
                ["latency", "download", "upload"].contains(&text(&progress["phase"]).as_str());
            let mut start = Action::new(
                "start",
                "开始互联网测速",
                "测试消耗移动流量，15 秒下载加约 10 MB 上传；高速网络可能消耗数百 MB，并影响其他设备网速。",
            );
            start.enabled = !active && !progress.is_null();
            p.actions.push(start);
            let mut stop = Action::new(
                "stop",
                "停止测速",
                "请求停止当前测试，等待正在进行的网络请求结束。",
            );
            stop.enabled = active;
            p.actions.push(stop);
            p.note = "测试 U60 到互联网的链路，不包含热点客户端的 Wi-Fi 吞吐。".into();
        }
        "at-terminal" => {
            p.row("AT 端口", first(data(readings, "port"), &["port", "path"]));
            let mut command = Field::text("command", "AT 指令", &json!(""));
            command.required = true;
            command.max = 512;
            p.fields = vec![
                command,
                Field::number("timeout", "超时 / 秒", &json!(3), 1, 30),
            ];
            p.actions.push(Action::new("send","执行指令","指令直接发送给调制解调器；写配置、重启或擦除指令可能中断服务或造成持久更改。请逐字核对。"));
            let result = data(readings, "last_result");
            if !result.is_null() {
                p.row("耗时 / ms", &result["elapsed_ms"]);
                p.content = text(&result["response"]);
                p.actions.push(Action::new("clear-local", "清空显示", ""));
            }
        }
        _ => {}
    }
    p
}
pub fn plan(id: &str, action: &str, p: &Page, readings: &Data) -> Result<Plan> {
    match id {
        "nr-bands" | "lte-bands" => {
            if action == "reset" {
                return Ok(Plan::one(Method::POST, "/api/cell/band/reset", json!({})));
            }
            ensure!(action == "apply", "操作不存在。");
            let b = bands(
                &text(&p.value("bands")?),
                if id == "nr-bands" { 1024 } else { 88 },
            )?;
            if id == "lte-bands" {
                return Ok(Plan::one(
                    Method::POST,
                    "/api/cell/band/lte",
                    json!({"is_lte_band":"1","lte_band_mask":b,"is_gw_band":"0","gw_band_mask":""}),
                ));
            }
            let mode = text(&p.value("mode")?);
            let modes = if mode == "both" {
                vec!["nsa", "sa"]
            } else {
                vec![mode.as_str()]
            };
            Ok(Plan::many(
                modes
                    .into_iter()
                    .map(|m| {
                        Request::json(
                            Method::POST,
                            "/api/cell/band/nr",
                            json!({"nr5g_type":m,"nr5g_band":b}),
                        )
                    })
                    .collect(),
            ))
        }
        "nr-lock" | "lte-lock" => {
            if action == "reset" {
                return Ok(Plan::one(Method::POST, "/api/cell/lock/reset", json!({})));
            }
            ensure!(action == "apply", "操作不存在。");
            let mut body = p.all(&["pci", "earfcn"])?;
            if id == "nr-lock" {
                body["band"] = p.value("band")?;
            }
            for v in body.as_object_mut().unwrap().values_mut() {
                *v = json!(text(v));
            }
            Ok(Plan::one(
                Method::POST,
                if id == "nr-lock" {
                    "/api/cell/lock/nr"
                } else {
                    "/api/cell/lock/lte"
                },
                body,
            ))
        }
        "neighbors" if action == "scan" => Ok(Plan::one(
            Method::POST,
            "/api/cell/neighbors/scan",
            json!({}),
        )),
        "stc" => {
            if action == "save" {
                let keys = STC.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();
                Ok(Plan::one(
                    Method::PUT,
                    "/api/cell/stc/params",
                    p.all(&keys)?,
                ))
            } else {
                ensure!(
                    ["enable", "disable", "reset"].contains(&action),
                    "操作不存在。"
                );
                Ok(Plan::one(
                    Method::POST,
                    &format!("/api/cell/stc/{action}"),
                    json!({}),
                ))
            }
        }
        "signal-test" => {
            ensure!(["start", "stop"].contains(&action), "操作不存在。");
            Ok(Plan::one(
                Method::POST,
                &format!("/api/cell/signal-detect/{action}"),
                json!({}),
            ))
        }
        "speedtest" => {
            ensure!(["start", "stop"].contains(&action), "操作不存在。");
            let body = if action == "start" {
                let id = p.value("server_id")?;
                ensure!(
                    data(readings, "servers")
                        .as_array()
                        .is_some_and(|v| v.iter().any(|s| text(&s["id"]) == text(&id))),
                    "服务器列表已变化。"
                );
                json!({"server_id":id})
            } else {
                json!({})
            };
            Ok(Plan::one(
                Method::POST,
                &format!("/api/speedtest/{action}"),
                body,
            ))
        }
        "at-terminal" if action == "send" => {
            let command = text(&p.value("command")?).trim().to_owned();
            ensure!(
                command.to_uppercase().starts_with("AT")
                    && !command
                        .chars()
                        .any(|c| c.is_control() || "'`$;|&".contains(c)),
                "请输入一条 AT 指令，不可包含控制字符或 ' ` $ ; | &。"
            );
            Ok(Plan::one(
                Method::POST,
                "/api/at/send",
                json!({"command":command,"timeout":p.value("timeout")?}),
            ))
        }
        _ => anyhow::bail!("操作不存在。"),
    }
}
fn bands(value: &str, max: u16) -> Result<String> {
    let list = value
        .split(|c: char| c.is_whitespace() || ",;".contains(c))
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_start_matches(['b', 'B', 'n', 'N']).parse::<u16>())
        .collect::<std::result::Result<std::collections::BTreeSet<_>, _>>()?;
    ensure!(
        !list.is_empty() && list.iter().all(|n| (1..=max).contains(n)),
        "频段列表需包含 1–{max} 之间的数字。"
    );
    Ok(list
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(","))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn radio_locks_use_firmware_fields_and_canonical_band_lists() {
        let mut p = page("nr-bands", &Data::new());
        p.fields[0].value = "n78, 41,78".into();
        let plan = plan("nr-bands", "apply", &p, &Data::new()).unwrap();
        assert_eq!(plan.requests.len(), 2);
        assert_eq!(
            plan.requests[0].body.as_ref().unwrap(),
            &json!({"nr5g_type":"nsa","nr5g_band":"41,78"})
        );
        assert_eq!(plan.requests[1].body.as_ref().unwrap()["nr5g_type"], "sa");
        assert!(bands("0,3", 88).is_err());
    }
}
