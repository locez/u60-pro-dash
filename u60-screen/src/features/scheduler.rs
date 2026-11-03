//! Device-local schedules with named templates and an explicit custom API mode.
use super::*;
use serde_json::json;
const ACTIONS: &[(&str, &str)] = &[
    ("airplane", "开启飞行模式"),
    ("data", "关闭蜂窝数据"),
    ("guest", "关闭访客 Wi-Fi"),
    ("reboot", "重启设备"),
    ("powersave", "开启省电模式"),
    ("custom", "自定义后台操作"),
];
pub fn reads(id: &str) -> Option<Vec<Read>> {
    (id == "scheduler").then(|| vec![Read::get("jobs", "/api/scheduler/jobs")])
}
fn template(kind: &str, restore: bool) -> Option<Value> {
    Some(match kind {
        "airplane" => {
            json!({"method":"POST","path":"/api/modem/airplane","body":{"operate_mode":if restore{"ONLINE"}else{"LPM"}}})
        }
        "data" => {
            if restore {
                json!({"method":"PUT","path":"/api/modem/data","body":{"cid":1,"enable":1}})
            } else {
                json!({"method":"PUT","path":"/api/modem/data","body":{"cid":1,"enable":0,"connect_status":"disconnected"}})
            }
        }
        "guest" => {
            json!({"method":"PUT","path":"/api/wifi/guest","body":{"guest_disabled_2g":if restore{"0"}else{"1"},"guest_disabled_5g":if restore{"0"}else{"1"}}})
        }
        "powersave" => {
            json!({"method":"PUT","path":"/api/device/power-save","body":{"deviceInfoList":{"power_saver_mode":if restore{"0"}else{"1"}}}})
        }
        "reboot" if !restore => json!({"method":"POST","path":"/api/device/reboot"}),
        _ => return None,
    })
}
fn job(d: &Data) -> Option<(usize, &Value)> {
    let id = selected(d)?;
    data(d, "jobs")
        .as_array()?
        .iter()
        .enumerate()
        .find(|(_, j)| text(&j["id"]) == id)
}
fn local_time(timestamp: i64) -> String {
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        let t = timestamp as _;
        if libc::localtime_r(&t, &mut tm).is_null() {
            return String::new();
        }
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min
        )
    }
}
fn parse_local(value: &str) -> Result<i64> {
    let parts = value
        .split(['-', ' ', ':'])
        .map(str::parse::<i32>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| anyhow::anyhow!("{}", crate::i18n::tr("日期格式应为 YYYY-MM-DD HH:MM。")))?;
    ensure!(
        parts.len() == 5 && (2020..=2099).contains(&parts[0]),
        "{}",
        crate::i18n::tr("日期格式应为 YYYY-MM-DD HH:MM。")
    );
    let ts = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_year = parts[0] - 1900;
        tm.tm_mon = parts[1] - 1;
        tm.tm_mday = parts[2];
        tm.tm_hour = parts[3];
        tm.tm_min = parts[4];
        tm.tm_isdst = -1;
        libc::mktime(&mut tm) as i64
    };
    ensure!(
        local_time(ts) == value,
        "{}",
        crate::i18n::tr("日期或时间不存在，请核对。")
    );
    Ok(ts)
}
fn time(value: &str) -> Result<()> {
    let parts = value.split(':').collect::<Vec<_>>();
    ensure!(
        parts.len() == 2
            && parts[0].len() == 2
            && parts[1].len() == 2
            && parts[0].parse::<u8>().is_ok_and(|h| h < 24)
            && parts[1].parse::<u8>().is_ok_and(|m| m < 60),
        "{}",
        crate::i18n::tr("时间必须为 HH:MM（24 小时制）。")
    );
    Ok(())
}
fn schedule_label(v: &Value) -> String {
    if v["type"] == "once" {
        v["at"]
            .as_i64()
            .map(local_time)
            .unwrap_or_else(|| crate::i18n::tr("日期不可用").into())
    } else {
        crate::i18n::format(
            "{0} · 周 {1}",
            &[
                shown(&v["time"]).to_string(),
                v["days"]
                    .as_array()
                    .map(|d| {
                        d.iter()
                            .filter_map(|v| v.as_u64().map(|n| (n + 1).to_string()))
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .unwrap_or_default()
                    .to_string(),
            ],
        )
    }
}
pub fn page(_: &str, d: &Data) -> Page {
    let mut p = Page::default();
    let Some(selection) = selected(d) else {
        if let Some(jobs) = data(d, "jobs").as_array() {
            p.rows
                .push((crate::i18n::tr("任务数").into(), jobs.len().to_string()));
            p.items = jobs
                .iter()
                .filter(|j| j["id"].is_u64())
                .map(|j| Item {
                    id: text(&j["id"]),
                    title: shown(&j["name"]),
                    subtitle: format!(
                        "{} · {}",
                        shown(&j["enabled"]),
                        schedule_label(&j["schedule"])
                    ),
                })
                .collect();
            p.items.push(Item {
                id: "new".into(),
                title: crate::i18n::tr("新增计划任务").into(),
                subtitle: crate::i18n::tr("按设备本地时间执行").into(),
            });
        }
        p.note =
            crate::i18n::tr("任务由设备后台执行，屏幕熄灭后仍生效。请先核对设备日期、时间和时区。")
                .into();
        return p;
    };
    let empty = json!({"name":"","enabled":true,"schedule":{"type":"recurring","time":"20:00","days":[0,1,2,3,4,5,6]},"action":template("airplane",false)});
    let old = if selection == "new" {
        Some(&empty)
    } else {
        job(d).map(|(_, v)| v)
    };
    let Some(old) = old else {
        p.note = crate::i18n::tr("任务已不存在，请返回刷新。").into();
        return p;
    };
    p.title = Some(if selection == "new" {
        crate::i18n::tr("新增计划任务").into()
    } else {
        shown(&old["name"])
    });
    if selection != "new" {
        for (k, l) in [
            ("last_run", crate::i18n::tr("最近执行")),
            ("last_restore", crate::i18n::tr("最近恢复")),
        ] {
            p.rows.push((
                l.into(),
                old[k]
                    .as_i64()
                    .map(local_time)
                    .unwrap_or_else(|| crate::i18n::tr("尚无记录").into()),
            ));
        }
        p.row(crate::i18n::tr("最近返回状态"), &old["last_status"]);
        if !old["last_error"].is_null() {
            p.note = crate::i18n::tr("上次操作失败，请核对对应功能的状态与设置。").into();
        }
    }
    let kind = ACTIONS
        .iter()
        .find(|(k, _)| {
            template(k, false).as_ref() == Some(&old["action"])
                && (old["restore"].is_null()
                    || template(k, true).is_some_and(|t| t["body"] == old["restore"]["body"]))
        })
        .map(|(k, _)| *k)
        .unwrap_or("custom");
    let s = &old["schedule"];
    p.fields = vec![
        Field::text("name", crate::i18n::tr("任务名称"), &old["name"]),
        Field::boolean("enabled", crate::i18n::tr("启用任务"), &old["enabled"]),
        Field::choice(
            "schedule",
            crate::i18n::tr("执行方式"),
            &s["type"],
            &[
                ("recurring", crate::i18n::tr("按星期重复")),
                ("once", crate::i18n::tr("仅执行一次")),
            ],
        ),
        Field::text(
            "time",
            crate::i18n::tr("重复任务时间（HH:MM）"),
            &json!(s["time"].as_str().unwrap_or("20:00")),
        ),
        Field::text(
            "days",
            crate::i18n::tr("重复星期（1–7，逗号分隔）"),
            &json!(
                s["days"]
                    .as_array()
                    .map(|d| d
                        .iter()
                        .filter_map(|v| v.as_u64().map(|n| (n + 1).to_string()))
                        .collect::<Vec<_>>()
                        .join(","))
                    .unwrap_or("1,2,3,4,5,6,7".into())
            ),
        ),
        Field::text(
            "at",
            crate::i18n::tr("单次日期（YYYY-MM-DD HH:MM）"),
            &json!(s["at"].as_i64().map(local_time).unwrap_or_default()),
        ),
        Field::choice("action", crate::i18n::tr("执行操作"), &json!(kind), ACTIONS),
        Field::boolean(
            "restore",
            crate::i18n::tr("随后重新启用"),
            &json!(old["restore"].is_object()),
        ),
        Field::text(
            "restore_time",
            crate::i18n::tr("恢复时间（HH:MM）"),
            &json!(old["restore"]["time"].as_str().unwrap_or("08:00")),
        ),
    ];
    p.fields[0].required = true;
    p.fields[0].max = 80;
    // Advanced fields are ignored for named templates. Existing opaque bodies
    // stay in memory and are preserved on a metadata-only edit.
    p.fields.extend([
        Field::choice(
            "method",
            crate::i18n::tr("请求方式"),
            &json!(old["action"]["method"].as_str().unwrap_or("POST")),
            &[
                ("GET", "GET"),
                ("POST", "POST"),
                ("PUT", "PUT"),
                ("DELETE", "DELETE"),
            ],
        ),
        Field::text(
            "path",
            crate::i18n::tr("后台路径 /api/…"),
            &json!(old["action"]["path"].as_str().unwrap_or("")),
        ),
        Field::password("body", crate::i18n::tr("请求参数 JSON"), true),
        Field::boolean(
            "clear_body",
            crate::i18n::tr("清除已有请求参数"),
            &json!(false),
        ),
        Field::password("restore_body", crate::i18n::tr("恢复参数 JSON"), true),
        Field::boolean(
            "clear_restore_body",
            crate::i18n::tr("清除已有恢复参数"),
            &json!(false),
        ),
    ]);
    for f in &mut p.fields {
        if f.key == "body" || f.key == "restore_body" {
            f.multiline = true;
            f.max = 8192;
        }
    }
    p.actions.push(Action::new("save",crate::i18n::tr("保存计划任务"),crate::i18n::tr("后台将按此时间自动执行操作，届时可能中断网络或重启设备。新建任务会立即启用；恢复时间早于执行时间时会在次日恢复。")));
    if selection != "new" {
        p.actions.push(Action::new(
            "toggle",
            if old["enabled"] == true {
                crate::i18n::tr("停用任务")
            } else {
                crate::i18n::tr("启用任务")
            },
            crate::i18n::tr("更改计划任务状态；已开始的操作不会被撤回。"),
        ));
        p.actions.push(Action::new(
            "delete",
            crate::i18n::tr("删除任务"),
            crate::i18n::tr("删除后不再执行此任务，也不会执行尚未到时的恢复操作。"),
        ));
    }
    if p.note.is_empty() {
        p.note=crate::i18n::tr("时间使用设备本地时区。自定义字段只在选择「自定义后台操作」时生效，已有请求参数不会回显；命名模板的恢复操作会重新开启对应功能。").into();
    }
    p
}
fn body(p: &Page, key: &str, clear: &str, old: &Value) -> Result<Value> {
    let raw = text(&p.value(key)?);
    if p.value(clear)? == true {
        ensure!(
            raw.is_empty(),
            "{}",
            crate::i18n::tr("清除参数时请不要同时填写新参数。")
        );
        return Ok(Value::Null);
    }
    if raw.is_empty() {
        Ok(old.clone())
    } else {
        serde_json::from_str(&raw)
            .map_err(|_| anyhow::anyhow!("{}", crate::i18n::tr("自定义参数必须为有效 JSON。")))
    }
}
pub fn plan(_: &str, action: &str, p: &Page, d: &Data) -> Result<Plan> {
    let existing = job(d);
    let old = existing.map(|(_, v)| v).unwrap_or(&Value::Null);
    let mut result = match action {
        "delete" => {
            ensure!(old["id"].is_u64(), "{}", crate::i18n::tr("任务已不存在。"));
            Plan::one(
                Method::DELETE,
                "/api/scheduler/jobs",
                json!({"id":old["id"]}),
            )
        }
        "toggle" => {
            ensure!(old["id"].is_u64(), "{}", crate::i18n::tr("任务已不存在。"));
            if old["enabled"] != true && old["schedule"]["type"] == "once" {
                ensure!(
                    old["schedule"]["at"].as_i64().unwrap_or(0)
                        > unsafe { libc::time(std::ptr::null_mut()) },
                    "{}",
                    crate::i18n::tr("单次任务时间已过，请先编辑日期。")
                );
            }
            Plan::one(
                Method::PUT,
                "/api/scheduler/jobs/toggle",
                json!({"id":old["id"],"enabled":old["enabled"]!=true}),
            )
        }
        "save" => {
            let kind = text(&p.value("action")?);
            let action = if kind == "custom" {
                let path = text(&p.value("path")?);
                ensure!(
                    path.starts_with("/api/")
                        && !path.contains(['?', '#', '\\'])
                        && !path.split('/').any(|c| c == "..")
                        && !path.starts_with("/api/scheduler/")
                        && !path.starts_with("/api/auth/")
                        && ![
                            "/api/tailscale/setup",
                            "/api/tailscale/logout",
                            "/api/tailscale/update"
                        ]
                        .contains(&path.as_str()),
                    "{}",
                    crate::i18n::tr("此路径不能作为计划任务执行。")
                );
                let mut a = json!({"method":p.value("method")?,"path":path});
                let b = body(p, "body", "clear_body", &old["action"]["body"])?;
                if !b.is_null() {
                    a["body"] = b;
                }
                a
            } else {
                template(&kind, false)
                    .ok_or_else(|| anyhow::anyhow!("{}", crate::i18n::tr("请选择有效操作。")))?
            };
            let schedule = match text(&p.value("schedule")?).as_str() {
                "once" => {
                    let at = parse_local(&text(&p.value("at")?))?;
                    ensure!(
                        at > unsafe { libc::time(std::ptr::null_mut()) },
                        "{}",
                        crate::i18n::tr("单次任务日期必须在未来。")
                    );
                    json!({"type":"once","at":at})
                }
                "recurring" => {
                    let t = text(&p.value("time")?);
                    time(&t)?;
                    let mut days = text(&p.value("days")?)
                        .split(',')
                        .map(|s| s.trim().parse::<u8>())
                        .collect::<std::result::Result<Vec<_>, _>>()
                        .map_err(|_| {
                            anyhow::anyhow!(
                                "{}",
                                crate::i18n::tr("星期请填写 1 至 7，并以逗号分隔。")
                            )
                        })?;
                    ensure!(
                        !days.is_empty() && days.iter().all(|n| (1..=7).contains(n)),
                        "{}",
                        crate::i18n::tr("星期请填写 1 至 7。")
                    );
                    days.sort_unstable();
                    days.dedup();
                    json!({"type":"recurring","time":t,"days":days.into_iter().map(|n|n-1).collect::<Vec<_>>()})
                }
                _ => anyhow::bail!("{}", crate::i18n::tr("请选择执行方式。")),
            };
            let restore = if p.value("restore")? == true {
                let t = text(&p.value("restore_time")?);
                time(&t)?;
                let b = if kind == "custom" {
                    body(
                        p,
                        "restore_body",
                        "clear_restore_body",
                        &old["restore"]["body"],
                    )?
                } else {
                    template(&kind, true).ok_or_else(|| {
                        anyhow::anyhow!(
                            "{}",
                            crate::i18n::tr("重启操作没有恢复步骤，请关闭「随后恢复」。")
                        )
                    })?["body"]
                        .clone()
                };
                let mut r = json!({"time":t});
                if !b.is_null() {
                    r["body"] = b;
                }
                r
            } else {
                Value::Null
            };
            let mut b = json!({"name":p.value("name")?,"schedule":schedule,"action":action,"restore":restore});
            let method = if selected(d) == Some("new") {
                ensure!(
                    p.value("enabled")? == true,
                    "{}",
                    crate::i18n::tr("新任务创建后自动启用；如需暂停，请创建后停用。")
                );
                Method::POST
            } else {
                ensure!(old["id"].is_u64(), "{}", crate::i18n::tr("任务已不存在。"));
                b["id"] = old["id"].clone();
                b["enabled"] = p.value("enabled")?;
                Method::PUT
            };
            Plan::one(method, "/api/scheduler/jobs", b)
        }
        _ => anyhow::bail!("{}", crate::i18n::tr("不支持的任务操作。")),
    };
    if let Some((index, _)) = existing {
        result.guards.push(Guard::new(
            Read::get("jobs", "/api/scheduler/jobs"),
            data(d, "jobs"),
            &[&format!("/{index}")],
        ));
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weekdays_and_restore_use_backend_schema() {
        let d = Data::from([
            ("jobs".into(), Ok(json!([]))),
            ("__selection".into(), Ok(json!("new"))),
        ]);
        let mut p = page("scheduler", &d);
        for (k, v) in [
            ("name", "Weekday radio"),
            ("days", "1,3,5"),
            ("restore", "true"),
        ] {
            p.fields.iter_mut().find(|f| f.key == k).unwrap().value = v.into();
        }
        let b = plan("scheduler", "save", &p, &d).unwrap().requests[0]
            .body
            .clone()
            .unwrap();
        assert_eq!(b["schedule"]["days"], json!([0, 2, 4]));
        assert_eq!(b["action"]["body"]["operate_mode"], "LPM");
        assert_eq!(b["restore"]["body"]["operate_mode"], "ONLINE");
        assert!(parse_local("2030-02-30 20:00").is_err());
        assert!(time("24:00").is_err());
    }
    #[test]
    fn custom_metadata_edit_preserves_body_and_disallows_recursive_schedule() {
        let job = json!({"id":1,"name":"Example","enabled":false,"schedule":{"type":"recurring","time":"20:00","days":[0]},"action":{"method":"PUT","path":"/api/example","body":{"opaque":"test-placeholder"}}});
        let d = Data::from([
            ("jobs".into(), Ok(json!([job]))),
            ("__selection".into(), Ok(json!("1"))),
        ]);
        let mut p = page("scheduler", &d);
        let b = plan("scheduler", "save", &p, &d).unwrap().requests[0]
            .body
            .clone()
            .unwrap();
        assert_eq!(b["action"]["body"]["opaque"], "test-placeholder");
        p.fields.iter_mut().find(|f| f.key == "path").unwrap().value = "/api/scheduler/jobs".into();
        assert!(plan("scheduler", "save", &p, &d).is_err());
    }
}
