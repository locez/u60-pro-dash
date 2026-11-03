//! Native feature contracts. Browser fixtures are never request bodies.
pub mod controller;
mod esim;
mod forwarding;
mod history;
mod lists;
mod messages;
mod network;
mod proxy;
mod radio;
mod scheduler;
mod services;
mod settings;
mod status;
mod wifi_share;

use crate::agent::{Error, Request};
use anyhow::{Result, ensure};
use reqwest::Method;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use zeroize::Zeroize;

pub type Data = BTreeMap<String, Result<Value, Error>>;
pub fn data<'a>(readings: &'a Data, key: &str) -> &'a Value {
    readings
        .get(key)
        .and_then(|v| v.as_ref().ok())
        .unwrap_or(&Value::Null)
}
pub fn text(value: &Value) -> String {
    match value {
        Value::String(v) => v.clone(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}
pub fn flag(value: &Value) -> Option<bool> {
    match text(value).as_str() {
        "true" | "1" | "on" | "enable" | "enabled" => Some(true),
        "false" | "0" | "off" | "disable" | "disabled" => Some(false),
        _ => None,
    }
}
pub fn shown(value: &Value) -> String {
    match value {
        Value::Null => crate::i18n::tr("未提供").into(),
        Value::Bool(v) => if *v {
            crate::i18n::tr("开启")
        } else {
            crate::i18n::tr("关闭")
        }
        .into(),
        Value::Array(v) => v.iter().map(shown).collect::<Vec<_>>().join("、"),
        Value::String(v) if v.is_empty() => crate::i18n::tr("未设置").into(),
        _ => text(value),
    }
}
#[derive(Clone)]
pub struct Read {
    pub key: String,
    pub request: Request,
}
impl Read {
    pub fn get(key: &str, path: &str) -> Self {
        Self {
            key: key.into(),
            request: Request::get(path),
        }
    }
    pub fn post(key: &str, path: &str, body: Value) -> Self {
        Self {
            key: key.into(),
            request: Request::json(Method::POST, path, body),
        }
    }
}
#[derive(Clone, Copy)]
pub enum Wire {
    String,
    Integer,
    Boolean,
    Strings,
}
#[derive(Clone)]
pub struct Choice {
    pub value: String,
    pub label: String,
}
pub struct Field {
    pub key: String,
    pub label: String,
    pub value: String,
    original: String,
    pub choices: Vec<Choice>,
    pub secret: bool,
    pub multiline: bool,
    pub available: bool,
    pub visible: bool,
    pub wire: Wire,
    pub required: bool,
    pub max: usize,
    pub range: Option<(i64, i64)>,
}
impl Field {
    pub fn text(key: &str, label: &str, value: &Value) -> Self {
        Self {
            key: key.into(),
            label: crate::i18n::tr(label).into(),
            value: text(value),
            original: text(value),
            choices: vec![],
            secret: false,
            multiline: false,
            available: !value.is_null(),
            visible: true,
            wire: Wire::String,
            required: false,
            max: 4096,
            range: None,
        }
    }
    pub fn number(key: &str, label: &str, value: &Value, min: i64, max: i64) -> Self {
        Self {
            wire: Wire::Integer,
            range: Some((min, max)),
            required: true,
            ..Self::text(key, label, value)
        }
    }
    pub fn boolean(key: &str, label: &str, value: &Value) -> Self {
        let normalized = flag(value).map(Value::Bool).unwrap_or(Value::Null);
        let mut f = Self::text(key, label, &normalized);
        f.wire = Wire::Boolean;
        f.choices = vec![
            Choice {
                value: "false".into(),
                label: crate::i18n::tr("关闭").into(),
            },
            Choice {
                value: "true".into(),
                label: crate::i18n::tr("开启").into(),
            },
        ];
        f
    }
    pub fn choice(key: &str, label: &str, value: &Value, options: &[(&str, &str)]) -> Self {
        let mut f = Self::text(key, label, value);
        f.choices = options
            .iter()
            .map(|(v, l)| Choice {
                value: (*v).into(),
                label: crate::i18n::tr(l).into(),
            })
            .collect();
        if f.available && !f.choices.iter().any(|c| c.value == f.value) {
            f.choices.insert(
                0,
                Choice {
                    value: f.value.clone(),
                    label: crate::i18n::format("当前：{0}", &[shown(value).to_string()]),
                },
            );
        }
        f
    }
    pub fn password(key: &str, label: &str, available: bool) -> Self {
        Self {
            secret: true,
            available,
            max: 1024,
            ..Self::text(key, label, &Value::String(String::new()))
        }
    }
    pub fn dirty(&self) -> bool {
        self.value != self.original
    }
    pub fn display(&self) -> String {
        if !self.available {
            return crate::i18n::tr("当前不可用").into();
        }
        if self.secret {
            return if self.value.is_empty() {
                crate::i18n::tr("留空保持不变")
            } else {
                crate::i18n::tr("已填写")
            }
            .into();
        }
        self.choices
            .iter()
            .find(|c| c.value == self.value)
            .map(|c| c.label.clone())
            .unwrap_or_else(|| {
                if self.value.is_empty() {
                    crate::i18n::tr("未填写").into()
                } else {
                    self.value.clone()
                }
            })
    }
    pub fn encode(&self) -> Result<Value> {
        ensure!(
            self.available,
            "{}",
            crate::i18n::format("{0}尚不可用，请先刷新。", std::slice::from_ref(&self.label))
        );
        ensure!(
            !self.required || !self.value.trim().is_empty(),
            "{}",
            crate::i18n::format("请填写{0}。", std::slice::from_ref(&self.label))
        );
        ensure!(
            self.value.len() <= self.max,
            "{}",
            crate::i18n::format(
                "{0}不能超过 {1} 字节。",
                &[self.label.to_string(), format!("{}", self.max)]
            )
        );
        if !self.choices.is_empty() {
            ensure!(
                self.choices.iter().any(|c| c.value == self.value),
                "{}",
                crate::i18n::format("请选择有效的{0}。", std::slice::from_ref(&self.label))
            );
        }
        if let Some((min, max)) = self.range {
            let n = self.value.parse::<i64>().map_err(|_| {
                anyhow::anyhow!(
                    "{}",
                    crate::i18n::format("{0}必须为整数。", std::slice::from_ref(&self.label))
                )
            })?;
            ensure!(
                (min..=max).contains(&n),
                "{}",
                crate::i18n::format(
                    "{0}必须为 {1}–{2}。",
                    &[
                        self.label.to_string(),
                        format!("{}", min),
                        format!("{}", max)
                    ]
                )
            );
        }
        Ok(match self.wire {
            Wire::String => Value::String(self.value.clone()),
            Wire::Integer => Value::from(self.value.parse::<i64>()?),
            Wire::Boolean => Value::Bool(self.value.parse::<bool>()?),
            Wire::Strings => Value::Array(
                self.value
                    .split([',', '\n'])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| Value::String(s.into()))
                    .collect(),
            ),
        })
    }
}
pub struct Item {
    pub id: String,
    pub title: String,
    pub subtitle: String,
}
pub struct Action {
    pub id: String,
    pub label: String,
    pub warning: String,
    pub enabled: bool,
    pub phrase: Option<String>,
}
impl Action {
    pub fn new(id: &str, label: &str, warning: &str) -> Self {
        Self {
            id: id.into(),
            label: crate::i18n::tr(label).into(),
            warning: crate::i18n::tr(warning).into(),
            enabled: true,
            phrase: None,
        }
    }
}
#[derive(Default)]
pub struct Page {
    pub title: Option<String>,
    pub qr: Option<String>,
    pub qr_caption: String,
    pub content: String,
    pub rows: Vec<(String, String)>,
    pub fields: Vec<Field>,
    pub items: Vec<Item>,
    pub actions: Vec<Action>,
    pub note: String,
}
impl Drop for Page {
    fn drop(&mut self) {
        if let Some(qr) = &mut self.qr {
            qr.zeroize();
        }
        for field in &mut self.fields {
            if field.secret {
                field.value.zeroize();
                field.original.zeroize();
            }
        }
    }
}
impl Page {
    pub fn row(&mut self, label: &str, value: &Value) {
        self.rows
            .push((crate::i18n::tr(label).into(), shown(value)));
    }
    pub fn field(&self, key: &str) -> Result<&Field> {
        self.fields
            .iter()
            .find(|f| f.key == key)
            .ok_or_else(|| anyhow::anyhow!("{}", crate::i18n::tr("缺少设置字段，请刷新。")))
    }
    pub fn value(&self, key: &str) -> Result<Value> {
        self.field(key)?.encode()
    }
    pub fn changed(&self, keys: &[&str]) -> Result<Value> {
        let mut map = Map::new();
        for key in keys {
            let f = self.field(key)?;
            if f.dirty() {
                map.insert((*key).into(), f.encode()?);
            }
        }
        Ok(Value::Object(map))
    }
    pub fn all(&self, keys: &[&str]) -> Result<Value> {
        keys.iter()
            .map(|key| Ok(((*key).into(), self.value(key)?)))
            .collect::<Result<Map<_, _>>>()
            .map(Value::Object)
    }
    pub fn dirty(&self) -> bool {
        self.fields.iter().any(Field::dirty)
    }
}
pub struct Plan {
    pub requests: Vec<Request>,
    pub summary: String,
    pub phrase: Option<String>,
    pub guards: Vec<Guard>,
    pub keep_draft: bool,
}
pub struct Guard {
    pub read: Read,
    pub pointers: Vec<String>,
    pub expected: Vec<Value>,
}
impl Guard {
    pub fn new(read: Read, original: &Value, pointers: &[&str]) -> Self {
        Self {
            read,
            pointers: pointers.iter().map(|p| (*p).into()).collect(),
            expected: pointers
                .iter()
                .map(|p| original.pointer(p).cloned().unwrap_or(Value::Null))
                .collect(),
        }
    }
    pub fn matches(&self, value: &Value) -> bool {
        self.pointers
            .iter()
            .zip(&self.expected)
            .all(|(p, expected)| value.pointer(p).unwrap_or(&Value::Null) == expected)
    }
}
impl Plan {
    pub fn many(requests: Vec<Request>) -> Self {
        Self {
            requests,
            summary: String::new(),
            phrase: None,
            guards: vec![],
            keep_draft: false,
        }
    }
    pub fn one(method: Method, path: &str, body: Value) -> Self {
        Self {
            requests: vec![Request::json(method, path, body)],
            summary: String::new(),
            phrase: None,
            guards: vec![],
            keep_draft: false,
        }
    }
}
pub fn reads(id: &str) -> Option<Vec<Read>> {
    if id == "history" {
        return Some(vec![]);
    }
    settings::reads(id)
        .or_else(|| status::reads(id))
        .or_else(|| network::reads(id))
        .or_else(|| lists::reads(id))
        .or_else(|| messages::reads(id))
        .or_else(|| radio::reads(id))
        .or_else(|| services::reads(id))
        .or_else(|| esim::reads(id))
        .or_else(|| forwarding::reads(id))
        .or_else(|| scheduler::reads(id))
        .or_else(|| wifi_share::reads(id))
        .or_else(|| proxy::reads(id))
}
pub fn page(id: &str, readings: &Data) -> Page {
    let mut result = feature_page(id, readings);
    if let Some(reads) = reads(id)
        && !reads.is_empty()
        && !reads.iter().any(|read| {
            readings
                .get(&read.key)
                .is_some_and(|v| v.as_ref().is_ok_and(|v| !v.is_null()))
        })
    {
        for action in &mut result.actions {
            action.enabled = false;
        }
        for field in &mut result.fields {
            field.available = false;
        }
        result.items.clear();
    }
    visibility(id, &mut result);
    result
}
fn feature_page(id: &str, readings: &Data) -> Page {
    if proxy::reads(id).is_some() {
        return proxy::page(id, readings);
    }
    if id == "wifi-share" {
        return wifi_share::page(readings);
    }
    if id == "history" {
        return history::page(readings);
    }
    if forwarding::reads(id).is_some() {
        return forwarding::page(id, readings);
    }
    if scheduler::reads(id).is_some() {
        return scheduler::page(id, readings);
    }
    if settings::reads(id).is_some() {
        settings::page(id, readings)
    } else if esim::reads(id).is_some() {
        esim::page(id, readings)
    } else if services::reads(id).is_some() {
        services::page(id, readings)
    } else if radio::reads(id).is_some() {
        radio::page(id, readings)
    } else if messages::reads(id).is_some() {
        messages::page(id, readings)
    } else if lists::reads(id).is_some() {
        lists::page(id, readings)
    } else if network::reads(id).is_some() {
        network::page(id, readings)
    } else {
        status::page(id, readings)
    }
}
/// Show only the inputs relevant to the current form choices; keep hidden
/// drafts intact, and retain original field indexes for keyboard callbacks.
pub fn visibility(id: &str, page: &mut Page) {
    let value = |key: &str| {
        page.fields
            .iter()
            .find(|f| f.key == key)
            .map(|f| f.value.clone())
            .unwrap_or_default()
    };
    let schedule = value("schedule");
    let custom = value("action") == "custom";
    let restore = value("restore") == "true";
    let filter = value("filter");
    for f in &mut page.fields {
        f.visible = match (id, f.key.as_str()) {
            ("scheduler", "time" | "days") => schedule == "recurring",
            ("scheduler", "at") => schedule == "once",
            ("scheduler", "restore_time") => restore,
            ("scheduler", "method" | "path" | "body" | "clear_body") => custom,
            ("scheduler", "restore_body" | "clear_restore_body") => custom && restore,
            ("sms-forward", "patterns") => {
                ["sender", "sender_and_content"].contains(&filter.as_str())
            }
            ("sms-forward", "keywords") => {
                ["content", "sender_and_content"].contains(&filter.as_str())
            }
            _ => true,
        };
    }
}
pub fn plan(id: &str, action: &str, page: &Page, readings: &Data) -> Result<Plan> {
    if proxy::reads(id).is_some() {
        return proxy::plan(id, action, page, readings);
    }
    if forwarding::reads(id).is_some() {
        return forwarding::plan(id, action, page, readings);
    }
    if scheduler::reads(id).is_some() {
        return scheduler::plan(id, action, page, readings);
    }
    if settings::reads(id).is_some() {
        settings::plan(id, action, page, readings)
    } else if esim::reads(id).is_some() {
        esim::plan(id, action, page, readings)
    } else if services::reads(id).is_some() {
        services::plan(id, action, page, readings)
    } else if radio::reads(id).is_some() {
        radio::plan(id, action, page, readings)
    } else if messages::reads(id).is_some() {
        messages::plan(id, action, page, readings)
    } else if lists::reads(id).is_some() {
        lists::plan(id, action, page, readings)
    } else if network::reads(id).is_some() {
        network::plan(id, action, page, readings)
    } else {
        status::plan(id, action, page, readings)
    }
}
pub fn selected(readings: &Data) -> Option<&str> {
    data(readings, "__selection").as_str()
}
pub fn qr_image(value: &str) -> Option<slint::Image> {
    use qrcodegen::{QrCode, QrCodeEcc};
    if value.len() > 2048 {
        return None;
    }
    let qr = QrCode::encode_text(value, QrCodeEcc::Medium).ok()?;
    let modules = qr.size() + 8;
    let scale = 256 / modules;
    if scale < 2 {
        return None;
    }
    let size = 256u32;
    let margin = (256 - modules * scale) / 2;
    let mut pixels = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(size, size);
    for (index, pixel) in pixels.make_mut_slice().iter_mut().enumerate() {
        let x = ((index as u32 % size) as i32 - margin) / scale - 4;
        let y = ((index as u32 / size) as i32 - margin) / scale - 4;
        let v = if qr.get_module(x, y) { 0 } else { 255 };
        *pixel = slint::Rgb8Pixel::new(v, v, v);
    }
    Some(slint::Image::from_rgb8(pixels))
}
pub fn topics(id: &str) -> &'static [&'static str] {
    match id {
        "signal-live" | "carriers" => &["signal"],
        "resources" => &["health"],
        "battery-details" => &["charger", "battery_info"],
        "traffic" => &["traffic"],
        "sim-info" => &["sim"],
        "wan" => &["wan"],
        "processes" => &["processes"],
        "esim-download" => &["esim_download"],
        "esim-profiles" => &["esim_switch"],
        "speedtest" => &["speedtest"],
        "usb" => &["usb", "charger"],
        "stk" => &["stk"],
        _ => &[],
    }
}
pub fn poll_seconds(id: &str) -> Option<u64> {
    match id {
        "battery-details" | "clients" | "sms-inbox" | "sms-forward-log" | "scheduler"
        | "esim-profiles" | "esim-receipts" => Some(10),
        "esim-download" => Some(3),
        "dns-cache" | "operators" | "neighbors" => Some(5),
        "signal-test" | "speedtest" | "phone" | "stk" => Some(2),
        "proxy-service" | "proxy-groups" | "tailscale-service" | "tailscale-version"
        | "agent-update" | "https" | "usb" => Some(5),
        "resources" | "wan" | "device-info" | "wifi-share" => Some(30),
        _ => None,
    }
}

/// Run only declared reads, without taking over the display or printing values.
pub fn probe(config: crate::agent::Config) -> Result<()> {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let mut agent = crate::agent::Agent::start(config)?;
    agent.watch(vec!["battery".into()]);
    let deadline = Instant::now() + Duration::from_secs(15);
    while !agent.snapshot().authenticated && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
    ensure!(
        agent.snapshot().authenticated,
        "feature probe could not authenticate"
    );
    let mut request_id = 1u64 << 48;
    for feature in crate::app::Catalog::load()?.features {
        // Loading an eSIM inventory can resume queued operator receipts in
        // the existing agent. Keep the unattended probe strictly observational.
        if feature.id.starts_with("esim-") || feature.id == "stk" {
            println!("{}: skipped (interactive SIM session)", feature.id);
            continue;
        }
        let Some(reads) = reads(&feature.id) else {
            continue;
        };
        let mut values = Data::new();
        for read in reads {
            ensure!(
                read.request.method == Method::GET
                    || (read.request.method == Method::POST
                        && ["/api/device/power-save", "/api/sms/list"]
                            .contains(&read.request.path.as_str())),
                "undeclared read operation"
            );
            request_id += 1;
            agent
                .call(request_id, read.request)
                .map_err(|_| anyhow::anyhow!("feature read queue unavailable"))?;
            let deadline = Instant::now() + Duration::from_secs(35);
            let result = loop {
                if let Some(reply) = agent.replies().into_iter().find(|r| r.id == request_id) {
                    break reply.result;
                }
                if Instant::now() >= deadline {
                    break Err(Error::Unavailable);
                }
                thread::sleep(Duration::from_millis(50));
            };
            values.insert(read.key, result);
        }
        let page = page(&feature.id, &values);
        println!(
            "{}: reads_ok={}/{} editable_fields={}/{} actions={}",
            feature.id,
            values.values().filter(|v| v.is_ok()).count(),
            values.len(),
            page.fields.iter().filter(|f| f.available).count(),
            page.fields.len(),
            page.actions.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod coverage {
    use super::*;
    #[test]
    fn every_catalog_entry_has_a_live_contract_or_an_explicit_local_flow() {
        let catalog = crate::app::Catalog::load().unwrap();
        let local = [
            "screen-prefs",
            "screen-lock",
            "account",
            "tailscale-network",
        ];
        for feature in catalog.features {
            assert!(
                reads(&feature.id).is_some() || local.contains(&feature.id.as_str()),
                "missing live page: {}",
                feature.id
            );
            if let Some(reads) = reads(&feature.id) {
                let failed = reads
                    .iter()
                    .map(|r| (r.key.clone(), Err(Error::Unsupported)))
                    .collect();
                let p = page(&feature.id, &failed);
                if !reads.is_empty() {
                    assert!(
                        p.actions.iter().all(|a| !a.enabled),
                        "write without device data: {}",
                        feature.id
                    );
                }
                for read in reads {
                    assert!(
                        read.request.method == Method::GET
                            || ["/api/sms/list", "/api/device/power-save"]
                                .contains(&read.request.path.as_str()),
                        "unattended mutation"
                    );
                }
            }
        }
    }
}
