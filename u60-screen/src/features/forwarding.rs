//! Forwarding rules retain opaque credentials until explicitly replaced.
use super::*;
use serde_json::json;
const TYPES: &[(&str, &str)] = &[
    ("telegram", "Telegram"),
    ("webhook", "Webhook"),
    ("sms", "短信"),
    ("ntfy", "ntfy"),
    ("discord", "Discord"),
    ("slack", "Slack"),
];
pub fn reads(id: &str) -> Option<Vec<Read>> {
    match id {
        "sms-forward" => Some(vec![Read::get("config", "/api/sms/forward/config")]),
        "sms-forward-log" => Some(vec![
            Read::get("log", "/api/sms/forward/log"),
            Read::get("config", "/api/sms/forward/config"),
        ]),
        _ => None,
    }
}
fn rule<'a>(d: &'a Data, id: &str) -> Option<&'a Value> {
    data(d, "config")["config"]["rules"]
        .as_array()?
        .iter()
        .find(|v| text(&v["id"]) == id)
}
fn log_id(v: &Value) -> String {
    format!(
        "{}:{}:{}",
        text(&v["timestamp"]),
        text(&v["sms_id"]),
        text(&v["rule_id"])
    )
}
fn log_entry(d: &Data) -> Option<(usize, &Value)> {
    let id = selected(d)?;
    let mut matches = data(d, "log")
        .as_array()?
        .iter()
        .enumerate()
        .filter(|(_, v)| log_id(v) == id);
    let found = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(found)
    }
}
fn list_text(v: &Value) -> Value {
    json!(
        v.as_array()
            .map(|a| a.iter().map(text).collect::<Vec<_>>().join(","))
            .unwrap_or_default()
    )
}
fn optional(v: &Value) -> Value {
    json!(text(v))
}
fn destination_label(destination: &Value) -> String {
    let origin = |key: &str| {
        reqwest::Url::parse(&text(&destination[key]))
            .ok()
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_else(|| crate::i18n::tr("地址不可用").into())
    };
    match text(&destination["type"]).as_str() {
        "telegram" => format!("Telegram · {}", shown(&destination["chat_id"])),
        "sms" => crate::i18n::format(
            "短信 · {0}",
            &[shown(&destination["forward_number"]).to_string()],
        ),
        "ntfy" => crate::i18n::format(
            "{0} · 主题 {1}",
            &[
                origin("url").to_string(),
                shown(&destination["topic"]).to_string(),
            ],
        ),
        "webhook" => format!("{} · {}", origin("url"), shown(&destination["method"])),
        "discord" | "slack" => format!(
            "{} · {}",
            shown(&destination["type"]),
            origin("webhook_url")
        ),
        _ => crate::i18n::tr("未提供").into(),
    }
}
pub fn page(id: &str, d: &Data) -> Page {
    let mut p = Page::default();
    let envelope = data(d, "config");
    let config = &envelope["config"];
    if id == "sms-forward-log" {
        if selected(d).is_some() {
            if let Some((_, v)) = log_entry(d) {
                p.title = Some(format!(
                    "{} · {}",
                    shown(&v["rule_name"]),
                    shown(&v["sender"])
                ));
                for (k, l) in [
                    ("sender", crate::i18n::tr("发送方")),
                    ("date", crate::i18n::tr("短信时间")),
                    ("rule_name", crate::i18n::tr("转发规则")),
                    ("destination_type", crate::i18n::tr("目的地类型")),
                    ("success", crate::i18n::tr("转发成功")),
                    ("pending", crate::i18n::tr("等待处理")),
                ] {
                    p.row(l, &v[k]);
                }
                p.content = text(&v["content"]);
                if !v["error"].is_null() {
                    p.note = crate::i18n::tr("目的地未接受此次转发，请检查配置或发送测试。").into();
                }
                if let Some(current) = rule(d, &text(&v["rule_id"])) {
                    p.row(crate::i18n::tr("当前规则名称"), &current["name"]);
                    p.rows.push((
                        crate::i18n::tr("当前接收目的地").into(),
                        destination_label(&current["destination"]),
                    ));
                    let mut a = Action::new(
                        "retry",
                        crate::i18n::tr("重试此次转发"),
                        crate::i18n::tr(
                            "向规则当前配置的目的地重新发送这条短信；若目的地已收到但未返回成功，可能收到重复内容。短信目的地可能收费。",
                        ),
                    );
                    a.enabled = v["pending"] != true
                        && v["success"] == false
                        && envelope["revision"].is_u64();
                    p.actions.push(a);
                } else {
                    p.note = crate::i18n::tr("原规则已不存在，不能重试。").into();
                }
            } else {
                p.note = crate::i18n::tr("记录已变化或存在重名标识，请返回刷新后重新选择。").into();
            }
        } else {
            if let Some(log) = data(d, "log").as_array() {
                p.items = log
                    .iter()
                    .map(|v| Item {
                        id: log_id(v),
                        title: format!("{} · {}", shown(&v["rule_name"]), shown(&v["sender"])),
                        subtitle: format!(
                            "{} · {}",
                            if v["pending"] == true {
                                crate::i18n::tr("等待")
                            } else if v["success"] == true {
                                crate::i18n::tr("成功")
                            } else {
                                crate::i18n::tr("失败")
                            },
                            shown(&v["content_preview"])
                        ),
                    })
                    .collect();
                p.rows
                    .push((crate::i18n::tr("记录数").into(), log.len().to_string()));
                p.actions.push(Action::new(
                    "clear",
                    crate::i18n::tr("清空已完成记录"),
                    crate::i18n::tr(
                        "清除已结束的转发记录；后台仍保留待处理的转发，清空不会取消发送。",
                    ),
                ));
            }
            p.note = crate::i18n::tr("点选失败记录可以核对目标规则并重试。").into();
        }
        return p;
    }
    let Some(selection) = selected(d) else {
        p.fields = vec![
            Field::boolean("enabled", crate::i18n::tr("自动转发"), &config["enabled"]),
            Field::number(
                "poll_interval_secs",
                crate::i18n::tr("检查间隔（秒）"),
                &config["poll_interval_secs"],
                10,
                86400,
            ),
            Field::boolean(
                "mark_read_after_forward",
                crate::i18n::tr("成功后标记已读"),
                &config["mark_read_after_forward"],
            ),
            Field::boolean(
                "delete_after_forward",
                crate::i18n::tr("成功后删除原短信"),
                &config["delete_after_forward"],
            ),
        ];
        if let Some(rules) = config["rules"].as_array() {
            p.items = rules
                .iter()
                .filter(|r| r["id"].is_u64())
                .map(|r| Item {
                    id: text(&r["id"]),
                    title: shown(&r["name"]),
                    subtitle: format!(
                        "{} · {}",
                        shown(&r["enabled"]),
                        shown(&r["destination"]["type"])
                    ),
                })
                .collect();
            p.items.push(Item {
                id: "new".into(),
                title: crate::i18n::tr("新增转发规则").into(),
                subtitle: crate::i18n::tr("选择目的地类型").into(),
            });
            p.actions.push(Action::new("config",crate::i18n::tr("保存转发设置"),crate::i18n::tr("自动转发会向已启用规则的目的地发送收到的短信；开启删除后，成功转发的原短信会被删除。")));
        }
        return p;
    };
    if selection == "new" {
        p.title = Some(crate::i18n::tr("选择转发目的地").into());
        p.items = TYPES
            .iter()
            .map(|(key, label)| Item {
                id: format!("new:{key}"),
                title: (*label).into(),
                subtitle: crate::i18n::tr("创建规则").into(),
            })
            .collect();
        return p;
    }
    let empty = json!({"name":"","enabled":true,"filter":{"type":"all"},"destination":{}});
    let current = if selection.starts_with("new:") {
        Some(&empty)
    } else {
        rule(d, selection)
    };
    let Some(current) = current else {
        p.note = crate::i18n::tr("规则已不存在，请返回刷新。").into();
        return p;
    };
    let kind = selection
        .strip_prefix("new:")
        .unwrap_or_else(|| current["destination"]["type"].as_str().unwrap_or(""));
    if !TYPES.iter().any(|(k, _)| *k == kind) {
        p.note = crate::i18n::tr("此目的地类型暂不支持，请使用网页管理。").into();
        return p;
    }
    p.title = Some(if selection.starts_with("new:") {
        crate::i18n::format("新增 {0} 规则", &[kind.to_string()])
    } else {
        shown(&current["name"])
    });
    p.fields = vec![
        Field::text("name", crate::i18n::tr("规则名称"), &current["name"]),
        Field::boolean("enabled", crate::i18n::tr("启用规则"), &current["enabled"]),
        Field::choice(
            "filter",
            crate::i18n::tr("匹配条件"),
            &current["filter"]["type"],
            &[
                ("all", crate::i18n::tr("全部短信")),
                ("sender", crate::i18n::tr("发送方匹配")),
                ("content", crate::i18n::tr("内容包含关键词")),
                (
                    "sender_and_content",
                    crate::i18n::tr("发送方及内容同时匹配"),
                ),
            ],
        ),
        Field::text(
            "patterns",
            crate::i18n::tr("发送方模式（逗号分隔）"),
            &list_text(&current["filter"]["patterns"]),
        ),
        Field::text(
            "keywords",
            crate::i18n::tr("关键词（逗号分隔）"),
            &list_text(&current["filter"]["keywords"]),
        ),
    ];
    p.fields[0].required = true;
    p.fields[0].max = 80;
    let dest = &current["destination"];
    if !dest["type"].is_null() {
        p.rows.push((
            crate::i18n::tr("已保存目的地").into(),
            destination_label(dest),
        ));
    }
    let secret = |key, label| Field::password(key, label, true);
    match kind {
        "telegram" => p.fields.extend([
            secret("bot_token", crate::i18n::tr("Bot Token（留空保留）")),
            Field::text(
                "chat_id",
                crate::i18n::tr("接收方 Chat ID"),
                &optional(&dest["chat_id"]),
            ),
            Field::boolean(
                "silent",
                crate::i18n::tr("静音通知"),
                &json!(dest["silent"] == true),
            ),
        ]),
        "webhook" => {
            p.fields.extend([
                secret("url", crate::i18n::tr("Webhook URL（留空保留）")),
                Field::choice(
                    "method",
                    crate::i18n::tr("请求方式"),
                    &json!(dest["method"].as_str().unwrap_or("POST")),
                    &[("POST", "POST"), ("GET", "GET"), ("PUT", "PUT")],
                ),
                secret(
                    "headers",
                    crate::i18n::tr("请求头：每行 Name: Value（留空保留）"),
                ),
                Field::boolean(
                    "clear_headers",
                    crate::i18n::tr("清除现有请求头"),
                    &json!(false),
                ),
            ]);
            p.fields
                .iter_mut()
                .find(|f| f.key == "headers")
                .unwrap()
                .multiline = true;
        }
        "sms" => p.fields.push(Field::text(
            "forward_number",
            crate::i18n::tr("接收短信的号码"),
            &optional(&dest["forward_number"]),
        )),
        "ntfy" => p.fields.extend([
            Field::text(
                "url",
                crate::i18n::tr("ntfy 服务地址"),
                &json!(dest["url"].as_str().unwrap_or("https://ntfy.sh")),
            ),
            Field::text(
                "topic",
                crate::i18n::tr("通知主题"),
                &optional(&dest["topic"]),
            ),
            secret("token", crate::i18n::tr("访问令牌（留空保留）")),
            Field::boolean(
                "clear_token",
                crate::i18n::tr("清除现有令牌"),
                &json!(false),
            ),
        ]),
        _ => p.fields.push(secret(
            "webhook_url",
            crate::i18n::tr("Webhook URL（留空保留）"),
        )),
    }
    p.actions.push(Action::new(
        "save",
        crate::i18n::tr("保存规则"),
        crate::i18n::tr(
            "后续匹配的短信将按此规则转发，可能包含验证码与私人内容。短信转发会产生运营商费用。",
        ),
    ));
    p.actions.push(Action::new(
        "test",
        crate::i18n::tr("发送测试"),
        crate::i18n::tr("立即向本页填写的目的地发送一条测试消息；无需先保存，短信目的地可能收费。"),
    ));
    if !selection.starts_with("new:") {
        p.actions.push(Action::new(
            "toggle",
            if current["enabled"] == true {
                crate::i18n::tr("停用规则")
            } else {
                crate::i18n::tr("启用规则")
            },
            crate::i18n::tr("切换此规则的启用状态。"),
        ));
        p.actions.push(Action::new(
            "delete",
            crate::i18n::tr("删除规则"),
            crate::i18n::tr("删除此转发规则；已进入处理流程的消息可能仍会完成。"),
        ));
    }
    p.note = crate::i18n::tr(
        "凭据不回显；留空保留现有值。发送方模式支持 * 通配符；条件未用到的列表可留空。",
    )
    .into();
    p
}
fn url(value: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(value)
        .map_err(|_| anyhow::anyhow!("{}", crate::i18n::tr("请填写有效的服务地址。")))?;
    ensure!(
        ["http", "https"].contains(&parsed.scheme())
            && parsed.host_str().is_some()
            && parsed.username().is_empty()
            && parsed.password().is_none(),
        "{}",
        crate::i18n::tr("地址必须为 HTTP(S)，账号凭据请使用令牌或请求头。")
    );
    Ok(())
}
fn secret(p: &Page, old: &Value, key: &str) -> Result<Value> {
    let v = p.value(key)?;
    Ok(if text(&v).is_empty() {
        old[key].clone()
    } else {
        v
    })
}
fn destination(p: &Page, old: &Value, kind: &str) -> Result<Value> {
    let mut d = json!({"type":kind});
    match kind {
        "telegram" => {
            let token = secret(p, old, "bot_token")?;
            let chat = p.value("chat_id")?;
            ensure!(
                !text(&token).is_empty() && !text(&chat).is_empty(),
                "{}",
                crate::i18n::tr("请填写 Bot Token 和接收方 Chat ID。")
            );
            d["bot_token"] = token;
            d["chat_id"] = chat;
            d["silent"] = p.value("silent")?;
        }
        "webhook" => {
            d["url"] = secret(p, old, "url")?;
            url(&text(&d["url"]))?;
            d["method"] = p.value("method")?;
            let raw = text(&p.value("headers")?);
            d["headers"] =
                if p.value("clear_headers")? == true {
                    ensure!(
                        raw.is_empty(),
                        "{}",
                        crate::i18n::tr("清除请求头时请不要同时填写新请求头。")
                    );
                    json!([])
                } else if raw.is_empty() {
                    old["headers"]
                        .as_array()
                        .cloned()
                        .map(Value::Array)
                        .unwrap_or(json!([]))
                } else {
                    let mut headers = vec![];
                    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
                        let (name, value) = line.split_once(':').ok_or_else(|| {
                            anyhow::anyhow!("{}", crate::i18n::tr("每行请求头必须为 Name: Value。"))
                        })?;
                        ensure!(
                            !name.is_empty()
                                && name.bytes().all(|c| c.is_ascii_alphanumeric()
                                    || b"!#$%&'*+-.^_`|~".contains(&c))
                                && !value.chars().any(char::is_control),
                            "{}",
                            crate::i18n::tr("请求头名称或值无效。")
                        );
                        headers.push(json!({"name":name,"value":value.trim()}));
                    }
                    json!(headers)
                };
        }
        "sms" => {
            let n = text(&p.value("forward_number")?).replace([' ', '-'], "");
            ensure!(
                (1..=20).contains(&n.trim_start_matches('+').len())
                    && n.trim_start_matches('+')
                        .bytes()
                        .all(|b| b.is_ascii_digit())
                    && !n.starts_with("++"),
                "{}",
                crate::i18n::tr("请填写有效的短信号码。")
            );
            d["forward_number"] = json!(n);
        }
        "ntfy" => {
            d["url"] = p.value("url")?;
            url(&text(&d["url"]))?;
            d["topic"] = p.value("topic")?;
            let topic = text(&d["topic"]);
            ensure!(
                !topic.is_empty()
                    && topic.len() <= 128
                    && topic
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)),
                "{}",
                crate::i18n::tr("通知主题限字母、数字、下划线和连字符。")
            );
            let token = secret(p, old, "token")?;
            ensure!(
                p.value("clear_token")? != true || p.field("token")?.value.is_empty(),
                "{}",
                crate::i18n::tr("清除令牌时请不要同时填写新令牌。")
            );
            d["token"] = if p.value("clear_token")? == true || text(&token).is_empty() {
                Value::Null
            } else {
                token
            };
        }
        "discord" | "slack" => {
            d["webhook_url"] = secret(p, old, "webhook_url")?;
            url(&text(&d["webhook_url"]))?;
        }
        _ => anyhow::bail!("{}", crate::i18n::tr("不支持的目的地类型。")),
    }
    Ok(d)
}
pub fn plan(id: &str, action: &str, p: &Page, d: &Data) -> Result<Plan> {
    let envelope = data(d, "config");
    let mut result = if id == "sms-forward-log" {
        match action {
            "clear" => Plan::one(Method::POST, "/api/sms/forward/log/clear", json!({})),
            "retry" => {
                let (index, v) = log_entry(d).ok_or_else(|| {
                    anyhow::anyhow!("{}", crate::i18n::tr("记录已变化，请重新选择。"))
                })?;
                ensure!(
                    v["success"] == false && v["pending"] != true,
                    "{}",
                    crate::i18n::tr("此记录不可重试。")
                );
                ensure!(
                    envelope["revision"].is_u64() && rule(d, &text(&v["rule_id"])).is_some(),
                    "{}",
                    crate::i18n::tr("无法核对当前规则，请刷新。")
                );
                let mut plan = Plan::one(
                    Method::POST,
                    "/api/sms/forward/retry",
                    json!({"index":index,"expected":{"sms_id":v["sms_id"],"rule_id":v["rule_id"],"timestamp":v["timestamp"]},"expected_rule_revision":envelope["revision"]}),
                );
                plan.guards.push(Guard::new(
                    Read::get("log", "/api/sms/forward/log"),
                    data(d, "log"),
                    &[&format!("/{index}")],
                ));
                plan
            }
            _ => anyhow::bail!("{}", crate::i18n::tr("不支持的日志操作。")),
        }
    } else if action == "config" {
        let body = p.changed(&[
            "enabled",
            "poll_interval_secs",
            "mark_read_after_forward",
            "delete_after_forward",
        ])?;
        ensure!(
            !body.as_object().unwrap().is_empty(),
            "{}",
            crate::i18n::tr("没有需要保存的更改。")
        );
        Plan::one(Method::PUT, "/api/sms/forward/config", body)
    } else {
        let selection =
            selected(d).ok_or_else(|| anyhow::anyhow!("{}", crate::i18n::tr("请先选择规则。")))?;
        let old = rule(d, selection).unwrap_or(&Value::Null);
        let kind = selection
            .strip_prefix("new:")
            .unwrap_or_else(|| old["destination"]["type"].as_str().unwrap_or(""));
        match action {
            "delete" | "toggle" => {
                ensure!(old["id"].is_u64(), "{}", crate::i18n::tr("规则不存在。"));
                if action == "delete" {
                    Plan::one(
                        Method::DELETE,
                        "/api/sms/forward/rules",
                        json!({"id":old["id"]}),
                    )
                } else {
                    Plan::one(
                        Method::PUT,
                        "/api/sms/forward/rules/toggle",
                        json!({"id":old["id"],"enabled":old["enabled"]!=true}),
                    )
                }
            }
            "save" | "test" => {
                let dest = destination(p, &old["destination"], kind)?;
                if action == "test" {
                    let mut plan = Plan::one(
                        Method::POST,
                        "/api/sms/forward/test",
                        json!({"destination":dest}),
                    );
                    plan.keep_draft = true;
                    plan
                } else {
                    let filter_type = text(&p.value("filter")?);
                    let mut filter = json!({"type":filter_type});
                    for (k, needed) in [
                        (
                            "patterns",
                            ["sender", "sender_and_content"].contains(&filter_type.as_str()),
                        ),
                        (
                            "keywords",
                            ["content", "sender_and_content"].contains(&filter_type.as_str()),
                        ),
                    ] {
                        if needed {
                            let values = text(&p.value(k)?)
                                .split(',')
                                .map(str::trim)
                                .filter(|v| !v.is_empty())
                                .map(str::to_owned)
                                .collect::<Vec<_>>();
                            ensure!(
                                !values.is_empty(),
                                "{}",
                                crate::i18n::tr("匹配条件需要填写对应的发送方模式或关键词。")
                            );
                            filter[k] = json!(values);
                        }
                    }
                    let mut body = json!({"name":p.value("name")?,"enabled":p.value("enabled")?,"filter":filter,"destination":dest});
                    let method = if selection.starts_with("new:") {
                        Method::POST
                    } else {
                        ensure!(old["id"].is_u64(), "{}", crate::i18n::tr("规则不存在。"));
                        body["id"] = old["id"].clone();
                        Method::PUT
                    };
                    Plan::one(method, "/api/sms/forward/rules", body)
                }
            }
            _ => anyhow::bail!("{}", crate::i18n::tr("不支持的规则操作。")),
        }
    };
    if let Some(destination) = result
        .requests
        .first()
        .and_then(|r| r.body.as_ref())
        .and_then(|b| b.get("destination"))
    {
        result.summary = crate::i18n::format(
            "接收目的地：{0}",
            &[destination_label(destination).to_string()],
        );
    } else if action == "retry"
        && let Some((_, entry)) = log_entry(d)
        && let Some(current) = rule(d, &text(&entry["rule_id"]))
    {
        result.summary = crate::i18n::format(
            "当前接收目的地：{0}",
            &[destination_label(&current["destination"]).to_string()],
        );
    }
    result.guards.push(Guard::new(
        Read::get("config", "/api/sms/forward/config"),
        envelope,
        &["/revision", "/config"],
    ));
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retry_binds_log_identity_and_current_rule_revision() {
        let d = Data::from([
            (
                "config".into(),
                Ok(
                    json!({"revision":7,"config":{"rules":[{"id":3,"name":"Example","destination":{"type":"sms"}}]}}),
                ),
            ),
            (
                "log".into(),
                Ok(
                    json!([{"timestamp":99,"sms_id":2,"rule_id":3,"success":false,"pending":false}]),
                ),
            ),
            ("__selection".into(), Ok(json!("99:2:3"))),
        ]);
        let p = page("sms-forward-log", &d);
        let plan = plan("sms-forward-log", "retry", &p, &d).unwrap();
        assert_eq!(
            plan.requests[0].body.as_ref().unwrap()["expected_rule_revision"],
            7
        );
        assert_eq!(
            plan.requests[0].body.as_ref().unwrap()["expected"]["sms_id"],
            2
        );
        assert_eq!(plan.guards.len(), 2);
    }
    #[test]
    fn editing_rule_preserves_opaque_credentials_and_test_does_not_save() {
        let d = Data::from([
            (
                "config".into(),
                Ok(
                    json!({"revision":1,"config":{"rules":[{"id":1,"name":"Example","enabled":true,"filter":{"type":"all"},"destination":{"type":"telegram","bot_token":"test-placeholder","chat_id":"123","silent":false}}]}}),
                ),
            ),
            ("__selection".into(), Ok(json!("1"))),
        ]);
        let p = page("sms-forward", &d);
        assert!(p.field("bot_token").unwrap().value.is_empty());
        let plan = plan("sms-forward", "test", &p, &d).unwrap();
        assert_eq!(plan.requests.len(), 1);
        assert_eq!(plan.requests[0].path, "/api/sms/forward/test");
        assert_eq!(
            plan.requests[0].body.as_ref().unwrap()["destination"]["bot_token"],
            "test-placeholder"
        );
        assert!(plan.keep_draft);
        assert!(plan.summary.contains("123"));
        assert!(!plan.summary.contains("test-placeholder"));
        assert_eq!(
            destination_label(
                &json!({"type":"webhook","url":"https://example.com/private-token?key=secret","method":"POST"})
            ),
            "https://example.com · POST"
        );
    }
}
