//! SMS storage and SIM credential operations. No automatic credential attempts.
use super::*;
use serde_json::json;

pub fn reads(id: &str) -> Option<Vec<Read>> {
    match id {
        "sms-inbox" => Some(vec![sms_read(1), sms_read(0)]),
        "sms-compose" => Some(vec![Read::get("sim", "/api/sim/info")]),
        "sim-verify" | "sim-pin" | "sim-change-pin" | "sim-network-unlock" => Some(vec![
            Read::get("sim", "/api/sim/info"),
            Read::get("trials", "/api/sim/lock-trials"),
        ]),
        _ => None,
    }
}
fn sms_read(store: u8) -> Read {
    Read::post(
        if store == 1 { "internal" } else { "card" },
        "/api/sms/list",
        json!({"page":0,"data_per_page":500,"mem_store":store,"tags":10,"order_by":"order by id desc"}),
    )
}
fn messages(readings: &Data) -> Vec<&Value> {
    let mut out = vec![];
    let mut ids = std::collections::BTreeSet::new();
    for key in ["internal", "card"] {
        if let Some(list) = data(readings, key)["messages"].as_array() {
            for message in list {
                let id = text(&message["id"]);
                if digits(&id, 1, 20) && ids.insert(id) {
                    out.push(message);
                }
            }
        }
    }
    out.sort_by_key(|v| std::cmp::Reverse(text(&v["id"]).parse::<u64>().unwrap_or(0)));
    out
}
fn digits(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}
fn state(sim: &Value) -> String {
    let s = text(&sim["sim_states"]).to_lowercase();
    if s.is_empty() {
        match text(&sim["modem_main_state"]).to_lowercase().as_str() {
            "modem_waitpin" => "wait pin",
            "modem_waitpuk" => "wait puk",
            "modem_init_complete" => "sim ready",
            _ => "",
        }
        .into()
    } else {
        s
    }
}
fn tries(readings: &Data, id: &str) -> Option<i64> {
    let s = data(readings, "sim");
    super::status::number(if id == "sim-network-unlock" {
        &data(readings, "trials")["available_trials"]
    } else if id == "sim-verify" && state(s) == "wait puk" {
        &s["puknumber"]
    } else {
        &s["pinnumber"]
    })
    .filter(|v| *v >= 0. && v.fract() == 0.)
    .map(|n| n as i64)
}
fn credential(key: &str, label: &str, max: usize) -> Field {
    let mut f = Field::password(key, label, true);
    f.required = true;
    f.max = max;
    f
}
fn composer(p: &mut Page, number: &str) {
    let mut recipient = Field::text("number", "收件号码", &json!(number));
    recipient.required = true;
    recipient.max = 32;
    let mut message = Field::text("message", "短信内容", &json!(""));
    message.multiline = true;
    message.required = true;
    message.max = 6120;
    p.fields = vec![recipient, message];
    p.actions.push(Action::new("send","发送短信","将向所填号码发送此短信，可能产生套餐费用。长短信可能拆分多条计费。请求已接受不代表已送达，超时后请查看已发消息再决定是否重试。"));
    p.note =
        "机身软键盘当前支持 ASCII 输入。中文短信可在配套管理后台编写，本屏可完整阅读中文内容。"
            .into();
}
pub fn page(id: &str, readings: &Data) -> Page {
    let mut p = Page::default();
    let sim = data(readings, "sim");
    match id {
        "sms-inbox" => {
            let messages = messages(readings);
            if let Some(selected) = selected(readings) {
                let reply = selected.strip_prefix("reply:");
                let selected = reply.unwrap_or(selected);
                if let Some(message) = messages.iter().find(|v| text(&v["id"]) == selected) {
                    p.title = Some(if reply.is_some() {
                        "回复短信".into()
                    } else {
                        shown(&message["number"])
                    });
                    if reply.is_some() {
                        composer(&mut p, &text(&message["number"]));
                    } else {
                        p.row("号码", &message["number"]);
                        p.row("时间", &message["date"]);
                        p.row("存储", &message["mem_store"]);
                        p.content = content(&text(&message["content"]));
                        if p.content.is_empty() {
                            p.content = "（空短信）".into();
                        }
                        p.items.push(Item {
                            id: format!("reply:{selected}"),
                            title: "回复此号码".into(),
                            subtitle: "填写后确认发送".into(),
                        });
                        let tag = text(&message["tag"]);
                        if ["0", "1"].contains(&tag.as_str()) {
                            p.actions.push(Action::new(
                                if tag == "1" { "read" } else { "unread" },
                                if tag == "1" {
                                    "标记已读"
                                } else {
                                    "标记未读"
                                },
                                "只改变此短信的阅读标记。",
                            ));
                        }
                        p.actions.push(Action::new(
                            "delete",
                            "删除此短信",
                            "从设备或 SIM 中永久删除这条短信，无法撤销。",
                        ));
                    }
                } else {
                    p.note = "短信已不存在，请返回列表刷新。".into();
                }
            } else {
                p.row("已读取短信", &json!(messages.len()));
                p.items = messages
                    .iter()
                    .map(|v| Item {
                        id: text(&v["id"]),
                        title: format!(
                            "{}{}",
                            if text(&v["tag"]) == "1" { "● " } else { "" },
                            shown(&v["number"])
                        ),
                        subtitle: content(&text(&v["content"])).chars().take(80).collect(),
                    })
                    .collect();
                p.note =
                    "分别读取设备与 SIM 最近 500 条，按短信 ID 去重。点开不会自动标记已读。".into();
            }
        }
        "sms-compose" => composer(&mut p, ""),
        "sim-verify" | "sim-pin" | "sim-change-pin" | "sim-network-unlock" => {
            let s = state(sim);
            let count = tries(readings, id);
            let puk = id == "sim-verify" && s == "wait puk";
            p.row("SIM 状态", &json!(s));
            p.row(
                "剩余尝试次数",
                &count.map(Value::from).unwrap_or(Value::Null),
            );
            match id {
                "sim-verify" if puk => {
                    p.fields = vec![
                        credential("puk", "PUK（8 位数字）", 8),
                        credential("new_pin", "新 PIN（4–8 位）", 8),
                        credential("repeat_pin", "再次输入新 PIN", 8),
                    ];
                }
                "sim-verify" => p.fields.push(credential("pin", "SIM PIN（4–8 位）", 8)),
                "sim-pin" => {
                    p.fields = vec![
                        Field::boolean("enabled", "开机要求 SIM PIN", &sim["pin_status"]),
                        credential("pin", "当前 SIM PIN", 8),
                    ];
                }
                "sim-change-pin" => {
                    p.fields = vec![
                        credential("pin", "当前 SIM PIN", 8),
                        credential("new_pin", "新 PIN（4–8 位）", 8),
                        credential("repeat_pin", "再次输入新 PIN", 8),
                    ];
                }
                _ => p.fields.push(credential("nck", "运营商网络解锁码", 16)),
            }
            let mut action = Action::new(
                "apply",
                if puk {
                    "使用 PUK 解锁"
                } else {
                    "确认提交"
                },
                if puk {
                    "错误 PUK 会扣除次数，用尽后 SIM 永久失效。"
                } else if id == "sim-network-unlock" {
                    "错误解锁码会扣除次数，可能造成永久网络锁定。"
                } else {
                    "错误 PIN 会扣除次数，用尽后需要 PUK。SIM PIN 与本屏锁屏密码是两个独立的密码。"
                },
            );
            action.enabled = count.is_some_and(|n| n > 0)
                && match id {
                    "sim-verify" => ["wait pin", "wait puk"].contains(&s.as_str()),
                    "sim-network-unlock" => !sim.is_null(),
                    _ => s == "sim ready",
                };
            if count == Some(1) {
                action.warning = format!("这是最后一次尝试。{}", action.warning);
                action.phrase = Some("LAST".into());
            }
            p.actions.push(action);
            p.note="仅向设备提交你输入的凭证。剩余次数未知或为零时禁用；提交前重新核对 SIM 身份、状态和次数，失败后清空输入并重新读取。".into();
        }
        _ => {}
    }
    p
}
pub fn plan(id: &str, action: &str, p: &Page, readings: &Data) -> Result<Plan> {
    if action == "send" && ["sms-compose", "sms-inbox"].contains(&id) {
        let number = recipient(&text(&p.value("number")?))?;
        let message = text(&p.value("message")?);
        return Ok(Plan::one(
            Method::POST,
            "/api/sms/send",
            send_body(&number, &message, &timestamp())?,
        ));
    }
    if id == "sms-inbox" {
        let selected = selected(readings).ok_or_else(|| anyhow::anyhow!("请选择短信。"))?;
        ensure!(digits(selected, 1, 20), "短信标识无效。");
        ensure!(
            messages(readings)
                .iter()
                .any(|v| text(&v["id"]) == selected),
            "短信已变化。"
        );
        let body = match action {
            "delete" => json!({"id":format!("{selected};")}),
            "read" | "unread" => {
                json!({"id":format!("{selected};"),"tag":if action=="read"{0}else{1}})
            }
            _ => anyhow::bail!("操作不存在。"),
        };
        let mut plan = Plan::one(
            Method::POST,
            if action == "delete" {
                "/api/sms/delete"
            } else {
                "/api/sms/read"
            },
            body,
        );
        for store in [1, 0] {
            let read = sms_read(store);
            let d = data(readings, &read.key);
            if let Some((index, _)) = d["messages"].as_array().and_then(|v| {
                v.iter()
                    .enumerate()
                    .find(|(_, v)| text(&v["id"]) == selected)
            }) {
                plan.guards
                    .push(Guard::new(read, d, &[&format!("/messages/{index}")]));
                break;
            }
        }
        return Ok(plan);
    }
    ensure!(action == "apply", "操作不存在。");
    let sim = data(readings, "sim");
    let s = state(sim);
    ensure!(
        tries(readings, id).is_some_and(|n| n > 0),
        "无法确认剩余尝试次数，不能提交。"
    );
    let pin = |key: &str| -> Result<String> {
        let s = text(&p.value(key)?);
        ensure!(digits(&s, 4, 8), "PIN 必须为 4–8 位数字。");
        Ok(s)
    };
    let next = || -> Result<String> {
        let a = pin("new_pin")?;
        ensure!(a == pin("repeat_pin")?, "两次新 PIN 不一致。");
        Ok(a)
    };
    let (path, body) = match id {
        "sim-verify" if s == "wait puk" => {
            let puk = text(&p.value("puk")?);
            ensure!(digits(&puk, 8, 8), "PUK 必须为 8 位数字。");
            (
                "/api/sim/pin/verify",
                json!({"pin_num":next()?,"puk_num":puk,"pin_encode_flag":"0"}),
            )
        }
        "sim-verify" => {
            ensure!(s == "wait pin", "SIM 当前不需要 PIN 解锁。");
            (
                "/api/sim/pin/verify",
                json!({"pin_num":pin("pin")?,"puk_num":"","pin_encode_flag":"0"}),
            )
        }
        "sim-pin" => {
            ensure!(s == "sim ready", "请先解锁 SIM。");
            let enabled = p.value("enabled")?;
            ensure!(
                flag(&sim["pin_status"]) != enabled.as_bool(),
                "PIN 保护设置没有改变。"
            );
            (
                "/api/sim/pin/mode",
                json!({"pin_num_m":pin("pin")?,"pin_mode":if enabled==true{1}else{0},"pin_encode_flag":"0"}),
            )
        }
        "sim-change-pin" => {
            ensure!(s == "sim ready", "请先解锁 SIM。");
            (
                "/api/sim/pin/change",
                json!({"pin_num":pin("pin")?,"new_pin_num":next()?,"pin_encode_flag":"0"}),
            )
        }
        "sim-network-unlock" => {
            let nck = text(&p.value("nck")?);
            ensure!(digits(&nck, 1, 16), "解锁码需为最多 16 位数字。");
            ("/api/sim/unlock", json!({"nck":nck}))
        }
        _ => anyhow::bail!("操作不存在。"),
    };
    let mut plan = Plan::one(Method::POST, path, body);
    plan.guards.push(Guard::new(
        Read::get("sim", "/api/sim/info"),
        sim,
        &[
            "/sim_epoch",
            "/sim_iccid",
            "/sim_states",
            "/modem_main_state",
            "/pin_status",
            "/pinnumber",
            "/puknumber",
        ],
    ));
    if id == "sim-network-unlock" {
        plan.guards.push(Guard::new(
            Read::get("trials", "/api/sim/lock-trials"),
            data(readings, "trials"),
            &["/available_trials"],
        ));
    }
    Ok(plan)
}
pub fn recipient(value: &str) -> Result<String> {
    let number = value
        .trim()
        .chars()
        .filter(|c| !" ()-".contains(*c))
        .collect::<String>();
    ensure!(
        digits(number.strip_prefix('+').unwrap_or(&number), 1, 20),
        "请输入有效电话号码，可包含开头的国际区号 +。"
    );
    Ok(number)
}
pub fn content(raw: &str) -> String {
    let mut value = if !raw.is_empty()
        && raw.len().is_multiple_of(4)
        && raw.bytes().all(|b| b.is_ascii_hexdigit())
    {
        let units = raw
            .as_bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .filter_map(|v| u16::from_str_radix(std::str::from_utf8(v).ok()?, 16).ok())
            .collect::<Vec<_>>();
        String::from_utf16_lossy(&units)
    } else {
        raw.into()
    };
    value.retain(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'));
    value
}
fn send_body(number: &str, message: &str, time: &str) -> Result<Value> {
    ensure!(
        !message.trim().is_empty() && message.encode_utf16().count() <= 1530,
        "短信不能为空，最多 1530 个 UTF-16 字符单位。"
    );
    const GSM: &str = "@£$¥èéùìòÇ\nØø\rÅåΔ_ΦΓΛΩΠΨΣΘΞÆæßÉ !\"#¤%&'()*+,-./0123456789:;<=>?¡ABCDEFGHIJKLMNOPQRSTUVWXYZÄÖÑÜ§¿abcdefghijklmnopqrstuvwxyzäöñüà^{}\\[~]|€";
    Ok(
        json!({"number":number,"sms_time":time,"message_body":message.encode_utf16().map(|u|format!("{u:04X}")).collect::<String>(),"id":"-1","encode_type":if message.chars().all(|c|GSM.contains(c)){"GSM7_default"}else{"UNICODE"}}),
    )
}
fn timestamp() -> String {
    // Same local-clock format as the existing mobile/web SMS clients.
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&now, &mut tm) }.is_null() {
        return String::new();
    }
    format!(
        "{:02};{:02};{:02};{:02};{:02};{:02};{:+}",
        (tm.tm_year + 1900) % 100,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
        tm.tm_gmtoff / 3600
    )
}
pub fn checked(path: &str, value: &Value) -> Result<(), Error> {
    if path == "/api/sms/send" && !value["result"].is_null() && text(&value["result"]) != "3" {
        return Err(Error::Rejected(422));
    }
    if path.starts_with("/api/sim/") && text(&value["result"]).to_lowercase().contains("fail") {
        return Err(Error::Rejected(422));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sms_codec_roundtrips_cjk_and_surrogates_and_rejection_is_not_success() {
        let b = send_body("+15555550100", "中文🙂", "26;01;01;12;00;00;+8").unwrap();
        assert_eq!(content(b["message_body"].as_str().unwrap()), "中文🙂");
        assert_eq!(b["encode_type"], "UNICODE");
        assert_eq!(content("normal text"), "normal text");
        assert!(checked("/api/sms/send", &json!({"result":0})).is_err());
        assert!(checked("/api/sms/send", &json!({"result":3})).is_ok());
    }
    #[test]
    fn sim_last_attempt_requires_extra_confirmation_and_rechecks_identity() {
        let d = Data::from([(
            "sim".into(),
            Ok(
                json!({"sim_states":"wait puk","pinnumber":"0","puknumber":"1","sim_epoch":4,"sim_iccid":"test-card"}),
            ),
        )]);
        let mut p = page("sim-verify", &d);
        assert_eq!(p.actions[0].phrase.as_deref(), Some("LAST"));
        p.fields[0].value = "12345678".into();
        p.fields[1].value = "1234".into();
        p.fields[2].value = "1234".into();
        let plan = plan("sim-verify", "apply", &p, &d).unwrap();
        assert!(
            !plan.guards[0]
                .matches(&json!({"sim_states":"wait puk","puknumber":"0","sim_epoch":4}))
        );
        assert!(!page("sim-verify", &Data::new()).actions[0].enabled);
    }
}
