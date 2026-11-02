//! Native feature contracts. Browser fixtures are never request bodies.
pub mod controller;
mod esim;
mod lists;
mod messages;
mod network;
mod radio;
mod services;
mod settings;
mod status;

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
        Value::Null => "未提供".into(),
        Value::Bool(v) => if *v { "开启" } else { "关闭" }.into(),
        Value::Array(v) => v.iter().map(shown).collect::<Vec<_>>().join("、"),
        Value::String(v) if v.is_empty() => "未设置".into(),
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
    pub wire: Wire,
    pub required: bool,
    pub max: usize,
    pub range: Option<(i64, i64)>,
}
impl Field {
    pub fn text(key: &str, label: &str, value: &Value) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value: text(value),
            original: text(value),
            choices: vec![],
            secret: false,
            multiline: false,
            available: !value.is_null(),
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
                label: "关闭".into(),
            },
            Choice {
                value: "true".into(),
                label: "开启".into(),
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
                label: (*l).into(),
            })
            .collect();
        if f.available && !f.choices.iter().any(|c| c.value == f.value) {
            f.choices.insert(
                0,
                Choice {
                    value: f.value.clone(),
                    label: format!("当前：{}", shown(value)),
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
            return "当前不可用".into();
        }
        if self.secret {
            return if self.value.is_empty() {
                "留空保持不变"
            } else {
                "已填写"
            }
            .into();
        }
        self.choices
            .iter()
            .find(|c| c.value == self.value)
            .map(|c| c.label.clone())
            .unwrap_or_else(|| {
                if self.value.is_empty() {
                    "未填写".into()
                } else {
                    self.value.clone()
                }
            })
    }
    pub fn encode(&self) -> Result<Value> {
        ensure!(self.available, "{}尚不可用，请先刷新。", self.label);
        ensure!(
            !self.required || !self.value.trim().is_empty(),
            "请填写{}。",
            self.label
        );
        ensure!(
            self.value.len() <= self.max,
            "{}不能超过 {} 字节。",
            self.label,
            self.max
        );
        if !self.choices.is_empty() {
            ensure!(
                self.choices.iter().any(|c| c.value == self.value),
                "请选择有效的{}。",
                self.label
            );
        }
        if let Some((min, max)) = self.range {
            let n = self
                .value
                .parse::<i64>()
                .map_err(|_| anyhow::anyhow!("{}必须为整数。", self.label))?;
            ensure!(
                (min..=max).contains(&n),
                "{}必须为 {}–{}。",
                self.label,
                min,
                max
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
            label: label.into(),
            warning: warning.into(),
            enabled: true,
            phrase: None,
        }
    }
}
#[derive(Default)]
pub struct Page {
    pub title: Option<String>,
    pub content: String,
    pub qr: Option<String>,
    pub rows: Vec<(String, String)>,
    pub fields: Vec<Field>,
    pub items: Vec<Item>,
    pub actions: Vec<Action>,
    pub note: String,
}
impl Drop for Page {
    fn drop(&mut self) {
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
        self.rows.push((label.into(), shown(value)));
    }
    pub fn field(&self, key: &str) -> Result<&Field> {
        self.fields
            .iter()
            .find(|f| f.key == key)
            .ok_or_else(|| anyhow::anyhow!("缺少设置字段，请刷新。"))
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
    settings::reads(id)
        .or_else(|| status::reads(id))
        .or_else(|| network::reads(id))
        .or_else(|| lists::reads(id))
        .or_else(|| messages::reads(id))
        .or_else(|| radio::reads(id))
        .or_else(|| services::reads(id))
        .or_else(|| esim::reads(id))
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
    result
}
fn feature_page(id: &str, readings: &Data) -> Page {
    if settings::reads(id).is_some() {
        settings::page(id, readings)
    } else if messages::reads(id).is_some() {
        messages::page(id, readings)
    } else if radio::reads(id).is_some() {
        radio::page(id, readings)
    } else if services::reads(id).is_some() {
        services::page(id, readings)
    } else if esim::reads(id).is_some() {
        esim::page(id, readings)
    } else if lists::reads(id).is_some() {
        lists::page(id, readings)
    } else if network::reads(id).is_some() {
        network::page(id, readings)
    } else {
        status::page(id, readings)
    }
}
pub fn plan(id: &str, action: &str, page: &Page, readings: &Data) -> Result<Plan> {
    if settings::reads(id).is_some() {
        settings::plan(id, action, page, readings)
    } else if messages::reads(id).is_some() {
        messages::plan(id, action, page, readings)
    } else if radio::reads(id).is_some() {
        radio::plan(id, action, page, readings)
    } else if services::reads(id).is_some() {
        services::plan(id, action, page, readings)
    } else if esim::reads(id).is_some() {
        esim::plan(id, action, page, readings)
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
        "clients" | "sms-inbox" | "esim-profiles" | "esim-receipts" => Some(10),
        "esim-download" => Some(3),
        "dns-cache" | "operators" | "neighbors" => Some(5),
        "signal-test" | "speedtest" | "phone" | "stk" => Some(2),
        "tailscale-service" | "tailscale-version" | "agent-update" | "https" | "usb" => Some(5),
        "resources" | "wan" | "device-info" => Some(30),
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
        // These reads can initialize a card session or resume queued receipts.
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
