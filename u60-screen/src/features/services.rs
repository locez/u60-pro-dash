//! Service lifecycle and system actions. Commands are only built after screen confirmation.
use super::network::first;
use super::*;
use serde_json::json;
pub fn reads(id: &str) -> Option<Vec<Read>> {
    let paths: &[(&str, &str)] = match id {
        "tailscale-service" | "tailscale-version" => &[("status", "/api/tailscale/status")],
        "usb" => &[
            ("status", "/api/usb/status"),
            ("charger", "/api/device/charger"),
        ],
        "https" => &[("status", "/api/https")],
        "agent-update" => &[("status", "/api/update/status")],
        "live-activity" => &[("status", "/api/live-activity")],
        "reboot" | "factory-reset" => &[("status", "/api/device")],
        "phone" => &[("status", "/api/call/status")],
        "ussd" => &[("sim", "/api/sim/info")],
        "stk" => &[("status", "/api/stk/menu")],
        _ => return None,
    };
    Some(paths.iter().map(|(k, p)| Read::get(k, p)).collect())
}
fn action(p: &mut Page, id: &str, label: &str, warning: &str, enabled: bool) {
    let mut a = Action::new(id, label, warning);
    a.enabled = enabled;
    p.actions.push(a);
}
pub fn page(id: &str, readings: &Data) -> Page {
    let mut p = Page::default();
    let d = data(readings, "status");
    let available = !d.is_null();
    match id {
        "tailscale-service" => {
            for (k, l) in [
                ("installed", "已安装"),
                ("enabled", "服务启用"),
                ("daemon_running", "后台运行"),
                ("backend_state", "连接状态"),
                ("tailscale_ips", "Tailscale 地址"),
                ("management_port", "远程 Web 端口"),
                ("health", "健康提示"),
            ] {
                p.row(l, &d[k]);
            }
            let login = first(d, &["login_url"]);
            let login = if login.is_null() {
                &data(readings, "last_result")["login_url"]
            } else {
                login
            };
            if available
                && d["backend_state"] != "Running"
                && d["enabled"] == true
                && let Some(url) = login.as_str().filter(|s| !s.is_empty())
                && let Ok(u) = reqwest::Url::parse(url)
                && u.scheme() == "https"
                && u.username().is_empty()
                && u.password().is_none()
            {
                p.qr = Some(url.into());
                p.note = "用手机扫描授权链接，完成账号登录。授权后本页会读取实际连接状态。".into();
            }
            p.fields.push(Field::password(
                "auth_key",
                "使用认证密钥登录（可选）",
                available,
            ));
            let ready = available && d["installed"] == true && d["updating"] != true;
            let needs_login =
                d["enrolled"] != true || d["backend_state"] == "NeedsLogin" || d["expired"] == true;
            for (a, l, w, en) in [
                ("enable", "启用 Tailscale", "启动 Tailscale 服务。", ready),
                (
                    "login-url",
                    "生成手机授权二维码",
                    "生成此次设备登录的授权链接；扫码后在手机完成登录。",
                    ready && d["enabled"] == true && needs_login,
                ),
                (
                    "setup",
                    "使用认证密钥登录",
                    "将输入的认证密钥提交给本机 Tailscale，加入对应网络。",
                    ready && needs_login,
                ),
                (
                    "disable",
                    "停止 Tailscale",
                    "中断经由 Tailscale 的远程连接与子网访问。",
                    ready,
                ),
                (
                    "logout",
                    "退出 Tailscale 登录",
                    "移除此机登录状态，需要重新授权才能加入 Tailscale。",
                    ready,
                ),
            ] {
                action(&mut p, a, l, w, en);
            }
        }
        "tailscale-version" => {
            for (k, l) in [
                ("client_version", "当前版本"),
                ("pinned_version", "固定版本"),
                ("updating", "更新执行中"),
            ] {
                p.row(l, &d[k]);
            }
            p.row("上次安装版本", &d["last_update"]["version"]);
            p.row("上次安装成功", &d["last_update"]["ok"]);
            p.fields = vec![
                Field::text("version", "目标版本", &json!("1.104.4")),
                Field::text("sha256", "ARM64 安装包 SHA-256", &json!("")),
            ];
            p.fields[1].required = true;
            p.fields[1].max = 64;
            action(
                &mut p,
                "update",
                "下载并安装指定版本",
                "从 Tailscale 官方 stable 仓库下载 ARM64 tgz，核对 SHA-256 后安装。更新期间隧道中断，使用移动流量。",
                available && d["updating"] != true,
            );
            p.note="校验值对应 tailscale_<版本>_arm64.tgz 完整安装包。请从官方发行信息核对，不能填二进制文件的校验值。".into();
        }
        "usb" => {
            p.row("线缆 CC 状态", &d["typec_cc"]);
            p.fields = vec![
                Field::choice(
                    "mode",
                    "USB 模式",
                    &d["mode"],
                    &[
                        ("none", "停用 USB 数据"),
                        ("rndis", "RNDIS 网络"),
                        ("ecm", "ECM 网络"),
                        ("mtp", "MTP 文件传输"),
                        ("debug", "ADB 调试模式"),
                    ],
                ),
                Field::boolean(
                    "powerbank",
                    "反向供电",
                    &data(readings, "charger")["otg_powerbank_state"],
                ),
            ];
            action(
                &mut p,
                "save",
                "应用 USB 设置",
                "切换模式可能断开 USB 网络和 ADB。调试模式会向 USB 连接的电脑提供 root 访问。反向供电使用本机电池。",
                available,
            );
        }
        "https" => {
            for (k, l) in [
                ("enabled", "HTTPS 启用"),
                ("listening", "正在监听"),
                ("ca_name", "证书颁发者"),
                ("ca_sha256", "证书 SHA-256 指纹"),
                ("not_after", "证书到期 / Unix 秒"),
                ("clock_verified", "系统时间已验证"),
                ("urls", "HTTPS 地址"),
            ] {
                p.row(l, &d[k]);
            }
            action(
                &mut p,
                "on",
                "开启 HTTPS",
                "生成并启用本机 HTTPS 证书。需要在手机或电脑安装并信任对应证书。",
                available,
            );
            action(
                &mut p,
                "renew",
                "更新 HTTPS 证书",
                "使用现有本机证书颁发者续签。系统时间必须正确。",
                available && d["enabled"] == true,
            );
            action(
                &mut p,
                "reset",
                "重置 HTTPS",
                "关闭 HTTPS 并移除证书；已信任旧证书的设备需要移除旧信任，后续重新安装。",
                available,
            );
            if let Some(a) = p.actions.last_mut() {
                a.phrase = Some("RESET".into());
            }
            p.note = "在管理后台下载公开 CA 证书。安装时与本屏显示的 SHA-256 指纹核对。".into();
        }
        "agent-update" => {
            for (k, l) in [
                ("phase", "更新阶段"),
                ("signing_configured", "发行签名可验证"),
                ("bootstrap_ready", "恢复机制就绪"),
                ("active_slot", "运行槽位"),
                ("trial_in_progress", "健康检查中"),
            ] {
                p.row(l, &d[k]);
            }
            for (k, l) in [
                ("current", "当前"),
                ("available", "可用"),
                ("staged", "已校验暂存"),
            ] {
                p.row(&format!("{l}版本"), &d[k]["version"]);
            }
            p.rows.push((
                "下载进度".into(),
                format!(
                    "{} / {}",
                    super::status::quantity(&d["progress"]["received"], 1e6, "MB"),
                    super::status::quantity(&d["progress"]["total"], 1e6, "MB")
                ),
            ));
            let s = &d["settings"];
            p.fields = vec![
                Field::choice(
                    "channel",
                    "发行通道",
                    &s["channel"],
                    &[("stable", "稳定版"), ("beta", "测试版")],
                ),
                Field::boolean("automatic_checks", "自动检查", &s["automatic_checks"]),
                Field::boolean("automatic_downloads", "自动下载", &s["automatic_downloads"]),
            ];
            action(
                &mut p,
                "settings",
                "保存更新偏好",
                "测试版可能不稳定。自动下载可能使用移动流量，安装仍需手动确认。",
                available,
            );
            action(
                &mut p,
                "check",
                "检查已签名发行版",
                "连接配置的发行源检查更新。",
                d["signing_configured"] == true,
            );
            action(
                &mut p,
                "download",
                "下载并校验更新",
                "下载可用发行版至备用槽位，使用移动流量。",
                d["signing_configured"] == true && !d["available"].is_null(),
            );
            action(
                &mut p,
                "install",
                "安装暂存的后台版本",
                "重启后台并运行健康检查，屏幕会暂时断开。更新包可能不包含本 fork 的定制功能，请核对来源和版本。",
                d["bootstrap_ready"] == true
                    && !d["staged"].is_null()
                    && d["trial_in_progress"] != true,
            );
            action(
                &mut p,
                "rollback",
                "回退后台版本",
                "切换至上一份经过校验的后台，当前会话暂时断开。",
                d["bootstrap_ready"] == true
                    && d["rollback_available"] == true
                    && d["trial_in_progress"] != true,
            );
            p.note =
                "开发构建若没有发行验签公钥，则不能使用在线更新；本页会据实际能力禁用对应操作。"
                    .into();
        }
        "live-activity" => {
            for (k, l) in [
                ("enabled", "推送服务可用"),
                ("registered", "iPhone 已绑定"),
                ("last_accepted_at", "上次推送被接受"),
                ("last_error", "上次错误"),
            ] {
                p.row(l, &d[k]);
            }
            action(
                &mut p,
                "unbind",
                "取消当前绑定",
                "停止向当前 iPhone 实时活动发送状态。之后可在手机重新绑定。",
                available && d["registered"] == true,
            );
            p.note="实时活动需要在 iPhone 配套客户端授权与创建，本屏可查看状态并取消绑定。设备无需在此输入推送密钥。".into();
        }
        "reboot" | "factory-reset" => {
            p.row("设备", &d["hostname"]);
            let reset = id == "factory-reset";
            let mut a = Action::new(
                "execute",
                if reset {
                    "恢复出厂设置"
                } else {
                    "重启设备"
                },
                if reset {
                    "恢复原厂设置会删除网络配置，并可能影响 ADB、自定义后台和当前访问方式。请先在电脑保存所需配置。"
                } else {
                    "重启整台 U60，所有网络和当前会话会暂时中断。"
                },
            );
            a.enabled = available;
            a.phrase = Some(if reset { "RESET" } else { "REBOOT" }.into());
            p.actions.push(a);
        }
        "phone" => {
            if let Some(calls) = d["calls"].as_array() {
                for c in calls {
                    p.rows.push((
                        shown(&c["number"]),
                        format!(
                            "通话 {} · {}",
                            shown(&c["id"]),
                            match text(&c["stat"]).as_str() {
                                "0" => "通话中",
                                "1" => "保持",
                                "2" => "正在拨号",
                                "3" => "对方响铃",
                                "4" => "来电",
                                "5" => "呼叫等待",
                                _ => "状态未知",
                            }
                        ),
                    ));
                }
            }
            p.fields = vec![
                Field::text("number", "电话号码", &json!("")),
                Field::text("digits", "DTMF 按键", &json!("")),
            ];
            for (a, l) in [
                ("dial", "拨号"),
                ("answer", "接听"),
                ("hangup", "挂断"),
                ("mute", "静音"),
                ("unmute", "取消静音"),
                ("dtmf", "发送按键音"),
            ] {
                action(
                    &mut p,
                    a,
                    l,
                    "操作作用于蜂窝通话，拨号和接听可能产生运营商费用。音频能力取决于固件与所连接的音频设备。",
                    available,
                );
            }
        }
        "ussd" => {
            p.fields
                .push(Field::text("code", "服务代码 / 回复", &json!("")));
            let r = data(readings, "last_result");
            if !r.is_null() {
                p.content = text(&r["response"]);
                p.row("会话有效", &r["session_active"]);
            }
            for (a, l) in [
                ("send", "发送服务代码"),
                ("respond", "回复当前会话"),
                ("cancel", "结束 USSD 会话"),
            ] {
                action(
                    &mut p,
                    a,
                    l,
                    "向运营商提交 USSD 指令，某些服务会变更套餐或产生费用。请核对代码和当前提示。",
                    !data(readings, "sim").is_null()
                        && (a != "respond" || r["session_active"] == true),
                );
            }
        }
        "stk" => {
            p.row("工具菜单状态", &d["status"]);
            p.row("菜单标题", &d["title"]);
            p.content = text(&d["text"]);
            if p.content.is_empty() {
                p.note = text(&d["reason"]);
            }
            let ready = d["busy"] != true && d["supported"] == true && d["command_id"].is_string();
            if let Some(items) = d["items"].as_array() {
                let mut f = Field::text("item_id", "菜单选项", &json!(""));
                f.wire = Wire::Integer;
                f.required = true;
                f.choices = items
                    .iter()
                    .map(|i| Choice {
                        value: text(&i["id"]),
                        label: shown(&i["label"]),
                    })
                    .collect();
                f.available = ready && !f.choices.is_empty();
                if !f.choices.is_empty() {
                    p.fields.push(f);
                    action(
                        &mut p,
                        "select",
                        "打开菜单选项",
                        "按所选菜单向 SIM 应用发送操作；部分服务可能变更套餐或计费。",
                        ready,
                    );
                }
            }
            if d["type"] == "input" {
                if d["input"]["yes_no"] == true {
                    p.fields
                        .push(Field::boolean("yes", "回复选择", &json!(false)));
                } else {
                    let mut f = Field::text("text", "回复", &json!(""));
                    f.secret = d["input"]["hidden"] == true;
                    f.max = 1024;
                    p.fields.push(f);
                }
                action(
                    &mut p,
                    "respond",
                    "提交回复",
                    "将回复提交给当前 SIM 工具会话。",
                    ready,
                );
            } else if d["type"] == "display" {
                action(
                    &mut p,
                    "respond",
                    "已阅读",
                    "确认当前 SIM 工具提示。",
                    ready,
                );
            }
            for (a, l) in [("back", "返回上级菜单"), ("cancel", "结束工具会话")] {
                action(
                    &mut p,
                    a,
                    l,
                    "操作当前 SIM 工具会话。",
                    ready && d["root_menu"] != true,
                );
            }
        }
        _ => {}
    }
    p
}
pub fn plan(id: &str, action: &str, p: &Page, readings: &Data) -> Result<Plan> {
    let d = data(readings, "status");
    let post = |path: &str, body: Value| Plan::one(Method::POST, path, body);
    match id {
        "tailscale-service" => {
            ensure!(
                ["enable", "disable", "login-url", "setup", "logout"].contains(&action),
                "操作不存在。"
            );
            let body = if action == "setup" {
                let key = text(&p.value("auth_key")?);
                ensure!(
                    key.starts_with("tskey-auth-") && !key.chars().any(char::is_whitespace),
                    "请输入有效 Tailscale 认证密钥。"
                );
                json!({"auth_key":key})
            } else {
                json!({})
            };
            Ok(post(&format!("/api/tailscale/{action}"), body))
        }
        "tailscale-version" => {
            ensure!(action == "update", "操作不存在。");
            let version = text(&p.value("version")?)
                .trim_start_matches('v')
                .to_string();
            let parts = version.split('.').collect::<Vec<_>>();
            ensure!(
                parts.len() == 3
                    && parts
                        .iter()
                        .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())),
                "版本格式例如 1.104.4。"
            );
            let sha = text(&p.value("sha256")?);
            ensure!(
                sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
                "SHA-256 必须为 64 位十六进制字符。"
            );
            Ok(post(
                "/api/tailscale/update",
                json!({"version":version,"sha256":sha.to_lowercase()}),
            ))
        }
        "usb" => {
            ensure!(action == "save" && p.dirty(), "没有需要保存的更改。");
            let mut r = vec![];
            if p.field("powerbank")?.dirty() {
                ensure!(
                    d["typec_cc"].as_str().is_some_and(|s| s != "no_cc"),
                    "请先接入 USB-C 线缆。"
                );
                r.push(Request::json(
                    Method::PUT,
                    "/api/usb/powerbank",
                    json!({"state":if p.value("powerbank")?==true{1}else{0}}),
                ));
            }
            if p.field("mode")?.dirty() {
                r.push(Request::json(
                    Method::PUT,
                    "/api/usb/mode",
                    p.all(&["mode"])?,
                ));
            }
            Ok(Plan::many(r))
        }
        "https" => {
            ensure!(["on", "renew", "reset"].contains(&action), "操作不存在。");
            Ok(post(
                &format!("/api/https/{action}"),
                json!({"confirm":true}),
            ))
        }
        "agent-update" => {
            if action == "settings" {
                return Ok(Plan::one(
                    Method::PUT,
                    "/api/update/settings",
                    p.all(&["channel", "automatic_checks", "automatic_downloads"])?,
                ));
            }
            ensure!(
                ["check", "download", "install", "rollback"].contains(&action),
                "操作不存在。"
            );
            let mut plan = post(&format!("/api/update/{action}"), json!({"confirm":true}));
            if ["install", "rollback"].contains(&action) {
                plan.guards.push(Guard::new(
                    Read::get("status", "/api/update/status"),
                    d,
                    &[
                        "/staged",
                        "/active_slot",
                        "/rollback_available",
                        "/trial_in_progress",
                    ],
                ));
            }
            Ok(plan)
        }
        "live-activity" if action == "unbind" => {
            Ok(Plan::one(Method::DELETE, "/api/live-activity", json!({})))
        }
        "reboot" | "factory-reset" if action == "execute" => {
            Ok(post(&format!("/api/device/{id}"), json!({})))
        }
        "phone" => {
            let (action, body) = match action {
                "dial" => (
                    "dial",
                    json!({"number":super::messages::recipient(&text(&p.value("number")?))?}),
                ),
                "dtmf" => {
                    let digits = text(&p.value("digits")?);
                    ensure!(
                        !digits.is_empty()
                            && digits.len() <= 32
                            && digits.chars().all(|c| "0123456789*#ABCD".contains(c)),
                        "DTMF 只能包含 0–9、*、#、A–D，最多 32 位。"
                    );
                    ("dtmf", json!({"digits":digits}))
                }
                "mute" | "unmute" => ("mute", json!({"enabled":action=="mute"})),
                "answer" | "hangup" => (action, json!({})),
                _ => anyhow::bail!("操作不存在。"),
            };
            let mut plan = post(&format!("/api/call/{action}"), body);
            if action != "dial" {
                ensure!(
                    d["calls"].as_array().is_some_and(|calls| !calls.is_empty()),
                    "当前没有可操作的通话，请刷新。"
                );
                plan.guards.push(Guard::new(
                    Read::get("status", "/api/call/status"),
                    d,
                    &["/calls"],
                ));
                plan.summary = d["calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| format!("通话 {}：{}", shown(&c["id"]), shown(&c["number"])))
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            Ok(plan)
        }
        "ussd" => {
            ensure!(
                ["send", "respond", "cancel"].contains(&action),
                "操作不存在。"
            );
            let body = if action == "cancel" {
                json!({})
            } else {
                let code = text(&p.value("code")?);
                ensure!(
                    !code.is_empty()
                        && code.len() <= 182
                        && code.chars().all(|c| "0123456789*#+".contains(c)),
                    "服务代码只能包含数字、*、#、+，最多 182 位。"
                );
                if action == "send" {
                    json!({"code":code})
                } else {
                    ensure!(
                        data(readings, "last_result")["session_active"] == true,
                        "没有可回复的当前会话。"
                    );
                    json!({"reply":code})
                }
            };
            Ok(post(&format!("/api/ussd/{action}"), body))
        }
        "stk" => {
            ensure!(
                ["select", "respond", "cancel", "back"].contains(&action),
                "操作不存在。"
            );
            ensure!(
                d["command_id"].is_string() && d["busy"] != true,
                "菜单已变化或正在执行。"
            );
            let mut body = json!({"command_id":d["command_id"]});
            if action == "select" {
                body["item_id"] = p.value("item_id")?;
            }
            if action == "respond" && d["type"] == "input" {
                if d["input"]["yes_no"] == true {
                    body["yes"] = p.value("yes")?;
                } else {
                    let value = p.value("text")?;
                    let text = text(&value);
                    let count = text.chars().count() as u64;
                    ensure!(
                        d["input"]["min"]
                            .as_u64()
                            .zip(d["input"]["max"].as_u64())
                            .is_some_and(|(min, max)| (min..=max).contains(&count)),
                        "回复长度不符合 SIM 要求。"
                    );
                    ensure!(
                        d["input"]["digits_only"] != true
                            || text.chars().all(|c| "0123456789*#+".contains(c)),
                        "当前 SIM 请求仅接受数字、*、#、+。"
                    );
                    body["text"] = value;
                }
            }
            Ok(post(&format!("/api/stk/{action}"), body))
        }
        _ => anyhow::bail!("操作不存在。"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn updates_require_real_checksum_and_preserve_typed_confirmation() {
        let d = Data::from([("status".into(), Ok(json!({"installed":true})))]);
        let mut p = page("tailscale-version", &d);
        assert!(plan("tailscale-version", "update", &p, &d).is_err());
        p.fields[1].value = "ab".repeat(32);
        let b = plan("tailscale-version", "update", &p, &d)
            .unwrap()
            .requests[0]
            .body
            .clone()
            .unwrap();
        assert_eq!(b["version"], "1.104.4");
        assert_eq!(
            page("factory-reset", &d).actions[0].phrase.as_deref(),
            Some("RESET")
        );
        assert!(
            page("agent-update", &Data::new())
                .actions
                .iter()
                .all(|a| !a.enabled)
        );
    }
}
