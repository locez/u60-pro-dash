//! Shared mode controls. Writes go through the normal fresh-read, confirmation
//! and readback controller; the lock surface never exposes these callbacks.
use super::*;
use serde_json::json;

pub fn reads(id: &str) -> Option<Vec<Read>> {
    matches!(id, "quick-controls" | "mobile-modes" | "power-save")
        .then(|| vec![Read::get("operating", "/api/device/operating")])
}
pub fn profile_name(value: &str) -> &'static str {
    match value {
        "balanced" => crate::i18n::tr("均衡"),
        "performance" => crate::i18n::tr("性能"),
        "saving" => crate::i18n::tr("省电"),
        _ => crate::i18n::tr("状态未知"),
    }
}
pub fn bandwidth_name(value: &str) -> &'static str {
    match value {
        "normal" => crate::i18n::tr("极速"),
        "ai" => crate::i18n::tr("智能"),
        "game" => crate::i18n::tr("游戏"),
        "video" => crate::i18n::tr("视频"),
        "meeting" => crate::i18n::tr("会议"),
        "live" => crate::i18n::tr("直播"),
        _ => crate::i18n::tr("状态未知"),
    }
}
pub fn page(id: &str, readings: &Data) -> Page {
    let mut p = Page::default();
    let d = data(readings, "operating");
    if id != "power-save" {
        p.fields.push(Field::boolean(
            "airplane",
            "飞行模式",
            &d["airplane"]["enabled"],
        ));
    }
    if id != "mobile-modes" {
        let current = d["performance"]
            .get("requested")
            .filter(|v| !v.is_null())
            .unwrap_or(&d["performance"]["effective"]);
        let mut field = Field::choice(
            "profile",
            "性能模式",
            current,
            &[
                ("balanced", "均衡"),
                ("performance", "性能"),
                ("saving", "省电"),
            ],
        );
        field.available = d["performance"]["supported"] == true;
        field.choices.retain(|choice| {
            d["performance"]["available"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v == &choice.value))
        });
        p.fields.push(field);
    }
    if id == "mobile-modes" {
        let mut field = Field::choice(
            "bandwidth",
            "应用带宽分配",
            &d["bandwidth"]["mode"],
            &[
                ("normal", "极速"),
                ("ai", "智能"),
                ("game", "游戏"),
                ("video", "视频"),
                ("meeting", "会议"),
                ("live", "直播"),
            ],
        );
        field.available = d["bandwidth"]["supported"] == true;
        p.fields.push(field);
    }
    let mut action = Action::new("save", "应用设置", "");
    action.enabled = d.is_object();
    p.actions.push(action);
    p
}
pub fn plan(_id: &str, action: &str, page: &Page, readings: &Data) -> Result<Plan> {
    ensure!(action == "save", "Unknown operating action");
    let changed: Vec<_> = page.fields.iter().filter(|f| f.dirty()).collect();
    ensure!(changed.len() == 1, "Change one operating setting at a time");
    let field = changed[0];
    let value = field.encode()?;
    let (method, path, body, pointer) = match field.key.as_str() {
        "airplane" => (
            Method::POST,
            "/api/modem/airplane",
            json!({"operate_mode":if value == true {"LPM"} else {"ONLINE"}}),
            "/airplane/enabled",
        ),
        "profile" => (
            Method::PUT,
            "/api/device/performance",
            json!({"profile":value}),
            "/performance/effective",
        ),
        "bandwidth" => (
            Method::PUT,
            "/api/modem/bandwidth",
            json!({"mode":value}),
            "/bandwidth/mode",
        ),
        _ => anyhow::bail!("Unknown operating setting"),
    };
    let mut plan = Plan::one(method, path, body);
    plan.guards.push(Guard::new(
        Read::get("operating", "/api/device/operating"),
        data(readings, "operating"),
        &[pointer],
    ));
    if field.key == "airplane" && value == true {
        plan.summary = crate::i18n::tr("蜂窝数据、通话和短信将断开，Wi-Fi、USB 和本地管理继续可用。通过蜂窝网络的远程连接会断开。").into();
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> Data {
        [("operating".into(), Ok(json!({"airplane":{"enabled":false},"bandwidth":{"supported":true,"mode":"normal"},"performance":{"supported":true,"available":["balanced","performance","saving"],"requested":null,"effective":"balanced"}})))].into()
    }
    #[test]
    fn only_entering_flight_requires_confirmation_and_recovery_uses_verified_endpoint() {
        let d = state();
        let mut p = page("quick-controls", &d);
        p.fields[0].value = "true".into();
        assert!(automatic::interruption("quick-controls", &p, &d));
        let enter = plan("quick-controls", "save", &p, &d).unwrap();
        assert_eq!(enter.requests[0].path, "/api/modem/airplane");
        assert!(!enter.summary.is_empty());
        let mut off = d.clone();
        off.get_mut("operating").unwrap().as_mut().unwrap()["airplane"] = json!({"enabled":true,"mode":"LOW_POWER"});
        let mut p = page("mobile-modes", &off);
        assert!(p.fields[0].available, "LOW_POWER keeps flight recovery available");
        p.fields[0].value = "false".into();
        assert!(!automatic::interruption("mobile-modes", &p, &off));
        let recover = plan("mobile-modes", "save", &p, &off).unwrap();
        assert_eq!(recover.requests[0].path, "/api/modem/airplane");
        assert_eq!(
            recover.requests[0].body.as_ref().unwrap()["operate_mode"],
            "ONLINE"
        );
    }
    #[test]
    fn performance_and_bandwidth_are_separate_single_writes() {
        let d = state();
        let mut p = page("power-save", &d);
        p.fields[0].value = "saving".into();
        let plan = plan("power-save", "save", &p, &d).unwrap();
        assert_eq!(plan.requests.len(), 1);
        assert_eq!(plan.requests[0].path, "/api/device/performance");
        assert!(!automatic::interruption("power-save", &p, &d));
        let mut changed = d["operating"].as_ref().unwrap().clone();
        changed["performance"]["effective"] = json!("performance");
        assert!(!plan.guards[0].matches(&changed));
        let mut p = page("mobile-modes", &d);
        p.fields[1].value = "game".into();
        assert_eq!(
            super::plan("mobile-modes", "save", &p, &d)
                .unwrap()
                .requests[0]
                .path,
            "/api/modem/bandwidth"
        );
        assert!(!automatic::interruption("mobile-modes", &p, &d));
    }
}
