use crate::{
    DataRow, FeatureAction, FeatureInput, MenuEntry, Meter, Palette, Reading, Screen, Trend,
    TrendPlot,
    config::{Preferences, parse_color, readable_accent},
};
use anyhow::{Result, bail};
use serde::Deserialize;
use serde_json::Value;
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
pub struct Catalog {
    pub groups: Vec<Group>,
    pub features: Vec<Feature>,
}
#[derive(Deserialize)]
pub struct Group {
    pub id: String,
    pub title: String,
    subtitle: String,
    root: String,
}
#[derive(Deserialize)]
pub struct Feature {
    pub id: String,
    pub title: String,
    group: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    rows: Vec<[String; 2]>,
    #[serde(default)]
    fields: Vec<Value>,
    #[serde(default)]
    items: Vec<Value>,
    #[serde(default)]
    actions: Vec<Value>,
    #[serde(default)]
    guide: Vec<String>,
}
impl Catalog {
    fn tab_for(&self, id: &str) -> i32 {
        let mut root = id;
        for _ in 0..=self.groups.len() {
            match root {
                "connect" => return 1,
                "manage" => return 2,
                _ => match self.groups.iter().find(|g| g.id == root) {
                    Some(group) => root = &group.root,
                    None => return 0,
                },
            }
        }
        0
    }
    pub fn load() -> Result<Self> {
        let mut catalog: Value = serde_json::from_str(include_str!("../assets/catalog.json"))?;
        // The catalog is authored preview copy, never live router data.
        fn localize(value: &mut Value) {
            match value {
                Value::String(text) => *text = crate::i18n::tr(text).into(),
                Value::Array(items) => items.iter_mut().for_each(localize),
                Value::Object(items) => {
                    for (key, value) in items {
                        if !matches!(
                            key.as_str(),
                            "id" | "root" | "group" | "type" | "tone" | "method" | "endpoint"
                        ) {
                            localize(value);
                        }
                    }
                }
                _ => {}
            }
        }
        localize(&mut catalog);
        Ok(serde_json::from_value(catalog)?)
    }
}

pub struct App {
    pub ui: Screen,
    pub state: Rc<RefCell<State>>,
    timer: slint::Timer,
}
#[derive(Clone, Copy)]
enum Editor {
    AgentLogin,
    ProbeTarget,
    Accent,
    Feature(usize),
    Confirmation,
}

pub struct State {
    agent: Option<crate::agent::Agent>,
    live_revision: u64,
    network: crate::agent::network::Network,
    network_rows: Vec<(String, String)>,
    clients: crate::agent::clients::Clients,
    wifi: crate::wifi_home::WifiHome,
    editor: Option<Editor>,
    features: crate::features::controller::Controller,
    feature_revision: u64,
    feature_qr_text: Option<zeroize::Zeroizing<String>>,
    choice_field: Option<usize>,
    metric_choice: Option<usize>,
    pub security: crate::security::ScreenLock,
    pub prefs: Preferences,
    prefs_path: Option<PathBuf>,
    prefs_dirty: bool,
    prefs_changed: Instant,
    prefs_baseline: Value,
    prefs_checked: Instant,
    prefs_stamp: Option<std::time::SystemTime>,
    home: crate::home::Summary,
    pub catalog: Catalog,
    history: Vec<String>,
    page: String,
    scenario: u8,
    pub last_input: Instant,
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(items)).into()
}
fn entry(id: &str, title: &str, subtitle: &str, tag: &str) -> MenuEntry {
    MenuEntry {
        id: id.into(),
        title: title.into(),
        subtitle: subtitle.into(),
        tag: tag.into(),
    }
}
fn reading(label: &str, value: &str) -> Reading {
    Reading {
        label: label.into(),
        value: value.into(),
    }
}
fn display(value: &Value) -> String {
    match value {
        Value::Null => "—".into(),
        Value::String(s) if s.is_empty() => crate::i18n::tr("未填写").into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => if *b {
            crate::i18n::tr("开启")
        } else {
            crate::i18n::tr("关闭")
        }
        .into(),
        Value::Array(a) => a.iter().map(display).collect::<Vec<_>>().join("、"),
        v => v.to_string(),
    }
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}

impl State {
    fn show(&mut self, ui: &Screen, id: &str, push: bool) -> Result<()> {
        if id != self.page {
            self.metric_choice = None;
            self.feature_qr_text = None;
            self.wifi.suspend(ui);
            self.features.cancel();
            ui.set_feature_has_qr(false);
            ui.set_feature_qr(Default::default());
            ui.set_feature_qr_caption("".into());
            ui.set_choice_visible(false);
            ui.set_confirm_visible(false);
        }
        let title;
        let mut view = 1;
        let mut entries = Vec::new();
        let mut readings = Vec::new();
        let mut actions: Vec<SharedString> = Vec::new();
        let mut description = String::new();
        let mut note = String::new();
        let mut tab = ui.get_tab();
        match id {
            "account" if self.agent.is_some() => {
                title = crate::i18n::tr("后台连接").into();
                view = 6;
                tab = 2;
            }
            "overview" => {
                title = crate::i18n::tr("随身网络").to_string();
                view = 0;
                tab = 0;
            }
            "connect" | "manage" => {
                let (t, label) = match id {
                    "connect" => (1, crate::i18n::tr("连接")),
                    _ => (2, crate::i18n::tr("管理")),
                };
                tab = t;
                title = label.into();
                if tab == 2 && self.agent.is_none() {
                    entries.push(entry(
                        "demo-scenario",
                        crate::i18n::tr("演示场景"),
                        crate::i18n::tr("正常、目标离线、无网络、低电量"),
                        "",
                    ));
                }
                for g in &self.catalog.groups {
                    if g.root == id {
                        let count = self
                            .catalog
                            .features
                            .iter()
                            .filter(|f| f.group == g.id)
                            .count()
                            + self
                                .catalog
                                .groups
                                .iter()
                                .filter(|child| child.root == g.id)
                                .count();
                        entries.push(entry(&g.id, &g.title, &g.subtitle, &count.to_string()));
                    }
                }
                for f in self.catalog.features.iter().filter(|f| f.group == id) {
                    entries.insert(
                        usize::from(!entries.is_empty()),
                        entry(&f.id, &f.title, &f.description, ""),
                    );
                }
            }
            "screen-lock" => {
                title = crate::i18n::tr("锁屏与密码").into();
                view = 5;
                tab = 2;
            }
            "screen-prefs" => {
                title = crate::i18n::tr("屏幕与外观").into();
                view = 3;
                tab = 2;
            }
            "tailscale-network" => {
                title = crate::i18n::tr("连通性检测").into();
                view = 4;
                tab = self.catalog.tab_for("tailscale");
                self.network.refresh();
            }
            "demo-scenario" => {
                title = crate::i18n::tr("演示场景").into();
                for (index, (name, subtitle)) in [
                    (
                        crate::i18n::tr("正常连接"),
                        crate::i18n::tr("蜂窝与远端子网均可用"),
                    ),
                    (
                        crate::i18n::tr("子网路由节点离线"),
                        crate::i18n::tr("Tailscale 在线，远端网络不可达"),
                    ),
                    (
                        crate::i18n::tr("蜂窝断开"),
                        crate::i18n::tr("Tailscale 连接等待网络"),
                    ),
                    (
                        crate::i18n::tr("低电量"),
                        crate::i18n::tr("电量 14%，网络正常"),
                    ),
                ]
                .iter()
                .enumerate()
                {
                    entries.push(entry(
                        &format!("scenario:{index}"),
                        name,
                        subtitle,
                        if self.scenario as usize == index {
                            "✓"
                        } else {
                            ""
                        },
                    ));
                }
            }
            _ => {
                if let Some(g) = self.catalog.groups.iter().find(|g| g.id == id) {
                    title = g.title.clone();
                    tab = self.catalog.tab_for(&g.id);
                    for child in self.catalog.groups.iter().filter(|child| child.root == id) {
                        entries.push(entry(&child.id, &child.title, &child.subtitle, ""));
                    }
                    for f in &self.catalog.features {
                        if f.group == id {
                            entries.push(entry(
                                &f.id,
                                &f.title,
                                if f.description.is_empty() {
                                    crate::i18n::tr("查看功能")
                                } else {
                                    &f.description
                                },
                                "",
                            ));
                        }
                    }
                } else if let Some((feature, index)) =
                    id.strip_prefix("item:").and_then(|s| s.rsplit_once(':'))
                {
                    let f = self
                        .catalog
                        .features
                        .iter()
                        .find(|f| f.id == feature)
                        .ok_or_else(|| anyhow::anyhow!("unknown feature"))?;
                    let item = f
                        .items
                        .get(index.parse::<usize>()?)
                        .ok_or_else(|| anyhow::anyhow!("unknown item"))?;
                    title = text(item, "title").into();
                    view = 2;
                    description = text(item, "subtitle").into();
                    if let Some(rows) = item["rows"].as_array() {
                        for row in rows {
                            readings
                                .push(reading(row[0].as_str().unwrap_or(""), &display(&row[1])));
                        }
                    }
                    if let Some(body) = item["body"].as_str() {
                        note = body.into();
                    }
                } else if let Some(f) = self.catalog.features.iter().find(|f| f.id == id) {
                    title = f.title.clone();
                    view = 2;
                    if let Some(g) = self.catalog.groups.iter().find(|g| g.id == f.group) {
                        tab = self.catalog.tab_for(&g.id);
                    }
                    description = f.description.clone();
                    for [label, value] in &f.rows {
                        readings.push(reading(label, value));
                    }
                    for field in &f.fields {
                        let value = if text(field, "type") == "password" {
                            crate::i18n::tr("未显示").into()
                        } else {
                            display(&field["value"])
                        };
                        readings.push(reading(text(field, "label"), &value));
                    }
                    for (index, item) in f.items.iter().enumerate() {
                        entries.push(entry(
                            &format!("item:{}:{index}", f.id),
                            text(item, "title"),
                            text(item, "subtitle"),
                            "",
                        ));
                    }
                    for action in &f.actions {
                        actions.push(text(action, "label").into());
                    }
                    note = f.guide.join("\n\n");
                    if !f.actions.is_empty() || !f.fields.is_empty() {
                        if !note.is_empty() {
                            note.push_str("\n\n");
                        }
                        note.push_str(crate::i18n::tr("当前展示演示配置。设备读写尚未接入。完整表单交互可在浏览器原型中体验。"));
                    }
                    if self.agent.is_some() {
                        readings.clear();
                        entries.clear();
                        actions.clear();
                        if self.features.open(id) {
                            view = 7;
                            self.feature_revision = u64::MAX;
                            note.clear();
                        } else {
                            note = crate::i18n::tr("正在接入此功能，当前尚不能操作。").into();
                        }
                    } else if self.features.preview(id) {
                        view = 7;
                        self.feature_revision = u64::MAX;
                        readings.clear();
                        entries.clear();
                        actions.clear();
                        note.clear();
                    }
                } else {
                    bail!("unknown screen page: {id}");
                }
            }
        }
        if push && self.page != id {
            self.history.push(self.page.clone());
        }
        self.page = id.into();
        self.last_input = Instant::now();
        ui.set_message("".into());
        ui.set_tab(tab);
        ui.set_heading(title.into());
        ui.set_kicker(["STATUS", "CONNECT", "MANAGE"][tab as usize].into());
        ui.set_can_back(!matches!(id, "overview" | "connect" | "manage"));
        ui.set_view(view);
        ui.set_entries(model(entries));
        ui.set_readings(model(readings));
        ui.set_actions(model(actions));
        ui.set_description(description.into());
        ui.set_note(note.into());
        ui.set_page_revision(ui.get_page_revision().wrapping_add(1));
        if self.agent.is_none() {
            self.wifi.demo(
                ui,
                !self.security.locked && !ui.get_asleep() && !ui.get_pin_visible() && view == 0,
            );
            self.render_features(ui);
        }
        Ok(())
    }

    fn apply_preferences(&self, ui: &Screen) {
        ui.set_language(self.prefs.language.code().into());
        ui.set_follow_system(self.prefs.follow_system);
        ui.set_home_show_radio(self.prefs.home.show_radio);
        ui.set_home_show_wifi(self.prefs.home.show_wifi);
        ui.set_home_show_services(self.prefs.home.show_services);
        ui.set_home_show_tailscale(self.prefs.home.show_tailscale);
        ui.set_home_show_wireguard(self.prefs.home.show_wireguard);
        ui.set_home_show_mihomo(self.prefs.home.show_mihomo);
        ui.set_home_metric_choices(model(
            [
                &self.prefs.home.metric_1,
                &self.prefs.home.metric_2,
                &self.prefs.home.metric_3,
            ]
            .into_iter()
            .map(|key| crate::home::metric_label(key).into())
            .collect(),
        ));
        self.home.render(ui, &self.prefs.home, None);
        let rgb = parse_color(&self.prefs.accent).expect("validated preferences");
        let ink = readable_accent(rgb, self.prefs.dark);
        let palette = ui.global::<Palette>();
        palette.set_dark(self.prefs.dark);
        palette.set_accent(Color::from_rgb_u8(rgb[0], rgb[1], rgb[2]));
        palette.set_accent_ink(Color::from_rgb_u8(ink[0], ink[1], ink[2]));
        let on = crate::config::button_ink(rgb);
        palette.set_accent_on(Color::from_rgb_u8(on[0], on[1], on[2]));
        ui.set_accent_hex(self.prefs.accent.clone().into());
        ui.set_probe_target(self.prefs.probe_target.clone().into());
        ui.set_brightness(self.prefs.brightness.into());
        ui.set_sleep_label(if self.prefs.sleep_seconds == 0 {
            crate::i18n::tr("从不").into()
        } else {
            crate::i18n::format("{0} 秒", &[format!("{}", self.prefs.sleep_seconds)]).into()
        });
    }

    pub fn sync_security(&mut self, ui: &Screen) {
        if self.security.locked
            || self.security.mode != crate::security::Mode::Hold
            || ui.get_asleep()
        {
            self.metric_choice = None;
            self.wifi.suspend(ui);
            self.feature_qr_text = None;
            self.close_editor(ui);
            ui.set_feature_has_qr(false);
            ui.set_feature_qr(Default::default());
            ui.set_feature_qr_caption("".into());
            ui.set_choice_visible(false);
            ui.set_confirm_visible(false);
            ui.set_confirm_word("".into());
        }
        use crate::security::Mode;
        let s = &self.security;
        ui.set_locked(s.locked);
        ui.set_pin_enabled(s.enabled());
        ui.set_pin_visible(s.mode != Mode::Hold);
        ui.set_pin_busy(s.busy());
        ui.set_pin_title(
            match s.mode {
                Mode::Unlock => crate::i18n::tr("解锁屏幕"),
                Mode::New | Mode::Confirm => crate::i18n::tr("设置解锁密码"),
                Mode::Change => crate::i18n::tr("修改解锁密码"),
                Mode::Disable => crate::i18n::tr("关闭密码解锁"),
                Mode::Hold => crate::i18n::tr("屏幕锁"),
            }
            .into(),
        );
        ui.set_pin_message(s.message.clone().into());
        ui.set_pin_masked("●".repeat(s.input.len()).into());
        ui.set_pin_remaining(s.remaining() as i32);
        ui.set_hold_progress(s.progress);
        if self.agent.is_none() {
            self.wifi.demo(
                ui,
                !ui.get_locked() && !ui.get_asleep() && !ui.get_pin_visible() && ui.get_view() == 0,
            );
        }
    }

    fn persist(&mut self, ui: &Screen) {
        self.last_input = Instant::now();
        self.home.refresh();
        self.apply_preferences(ui);
        self.prefs_dirty = self.prefs_path.is_some();
        self.prefs_changed = Instant::now();
    }

    fn flush_preferences(&mut self, ui: &Screen) {
        if !self.prefs_dirty {
            return;
        }
        self.prefs_dirty = false;
        let language = self.prefs.language;
        if let Some(path) = &self.prefs_path {
            let patch = crate::config::shared::diff(
                &self.prefs_baseline,
                &serde_json::to_value(&self.prefs).unwrap(),
            );
            match crate::config::shared::patch(path, &patch) {
                Ok(value) => {
                    self.prefs =
                        serde_json::from_value(value.clone()).expect("validated preferences");
                    self.prefs_baseline = value;
                    self.prefs_stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok();
                    self.apply_preferences(ui);
                }
                Err(error) => {
                    eprintln!("preferences: {error}");
                    self.prefs = serde_json::from_value(self.prefs_baseline.clone())
                        .expect("previous preferences");
                    self.apply_preferences(ui);
                    ui.set_message(crate::i18n::tr("设置未能保存，已恢复原值。").into());
                }
            }
        }
        if language != self.prefs.language {
            crate::i18n::select(self.prefs.language);
            let _ = slint::select_bundled_translation(self.prefs.language.code());
            self.catalog = Catalog::load().expect("embedded catalog");
            self.live_revision = u64::MAX;
            self.feature_revision = u64::MAX;
            let page = self.page.clone();
            let _ = self.show(ui, &page, false);
        }
    }
    fn poll_preferences(&mut self, ui: &Screen) {
        if self.prefs_checked.elapsed() < Duration::from_secs(2)
            || self.prefs_dirty
            || ui.get_editor_visible()
            || ui.get_confirm_visible()
            || ui.get_choice_visible()
        {
            return;
        }
        self.prefs_checked = Instant::now();
        let Some(path) = &self.prefs_path else {
            return;
        };
        let stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if stamp == self.prefs_stamp {
            return;
        }
        self.prefs_stamp = stamp;
        let Ok(prefs) = Preferences::load(path) else {
            return;
        };
        let changed_language = prefs.language != self.prefs.language;
        self.prefs_baseline = serde_json::to_value(&prefs).unwrap();
        self.prefs = prefs;
        self.home.refresh();
        if changed_language {
            crate::i18n::select(self.prefs.language);
            let _ = slint::select_bundled_translation(self.prefs.language.code());
            self.catalog = Catalog::load().expect("embedded catalog");
            self.live_revision = u64::MAX;
            self.feature_revision = u64::MAX;
            let page = self.page.clone();
            let _ = self.show(ui, &page, false);
        }
        self.apply_preferences(ui);
    }
    fn apply_scenario(&self, ui: &Screen) {
        if self.agent.is_some() {
            return;
        }
        ui.set_network_online(self.scenario != 2);
        ui.set_carrier_label(
            if self.scenario == 2 {
                crate::i18n::tr("无服务")
            } else {
                crate::i18n::tr("演示运营商")
            }
            .into(),
        );
        ui.set_tailnet_online(self.scenario != 2);
        ui.set_battery(if self.scenario == 3 { "14%" } else { "82%" }.into());
        ui.set_battery_level(if self.scenario == 3 { 14 } else { 82 });
        ui.set_charging(false);
        ui.set_external_power(false);
        ui.set_bypass_power(false);
        ui.set_power_label(crate::i18n::tr("电池供电").into());
        ui.set_signal_level(if self.scenario == 2 { 0 } else { 3 });
        ui.set_network_badge(if self.scenario == 2 { "" } else { "5G" }.into());
        ui.set_download(if self.scenario == 2 { "—" } else { "128.6" }.into());
        ui.set_upload(if self.scenario == 2 { "—" } else { "16.4" }.into());
        ui.set_signal_strength(if self.scenario == 2 { "—" } else { "−86" }.into());
        ui.set_network_mode(
            if self.scenario == 2 {
                crate::i18n::tr("离线")
            } else {
                "5G"
            }
            .into(),
        );
        ui.set_radio_label(
            if self.scenario == 2 {
                crate::i18n::tr("等待蜂窝网络")
            } else {
                "5G SA · n78"
            }
            .into(),
        );
        ui.set_tailnet_status(
            if self.scenario == 2 {
                crate::i18n::tr("等待网络")
            } else {
                crate::i18n::tr("已连接")
            }
            .into(),
        );
        ui.set_tailnet_readings(model(vec![
            reading(
                "Tailscale",
                if self.scenario == 2 {
                    crate::i18n::tr("等待网络")
                } else {
                    crate::i18n::tr("已连接")
                },
            ),
            reading(
                crate::i18n::tr("远端子网"),
                if self.scenario == 0 || self.scenario == 3 {
                    crate::i18n::tr("演示：已验证可达")
                } else {
                    crate::i18n::tr("演示：不可达")
                },
            ),
        ]));
    }

    fn update_agent(&mut self, ui: &Screen) {
        let wifi_active = !ui.get_asleep()
            && !self.security.locked
            && !ui.get_pin_visible()
            && self.page == "overview"
            && ui.get_view() == 0
            && self.prefs.home.show_wifi;
        let Some(agent) = &mut self.agent else {
            self.wifi.demo(ui, wifi_active);
            self.home.render(ui, &self.prefs.home, None);
            return;
        };
        let mut topics = if ui.get_asleep() {
            vec![]
        } else {
            ["signal", "traffic", "battery", "charger"]
                .map(String::from)
                .to_vec()
        };
        if !ui.get_asleep() && !self.security.locked && ui.get_view() == 7 {
            topics.extend(
                crate::features::topics(&self.features.id)
                    .iter()
                    .map(|s| (*s).into()),
            );
        }
        if !ui.get_asleep()
            && !self.security.locked
            && ui.get_view() == 0
            && [
                &self.prefs.home.metric_1,
                &self.prefs.home.metric_2,
                &self.prefs.home.metric_3,
            ]
            .iter()
            .any(|v| matches!(v.as_str(), "cpu" | "temperature"))
        {
            topics.push("health".into());
        }
        agent.watch(topics);
        let snapshot = agent.snapshot();
        self.features.observe(&snapshot);
        if snapshot.revision != self.live_revision {
            self.live_revision = snapshot.revision;
            let dashboard = crate::agent::model::Dashboard::read(&snapshot);
            ui.set_connection_label(snapshot.status.label().into());
            ui.set_connection_live(snapshot.status == crate::agent::Status::Live);
            ui.set_carrier_label(dashboard.carrier.into());
            ui.set_network_mode(dashboard.network.into());
            ui.set_radio_label(dashboard.radio.into());
            ui.set_signal_strength(dashboard.rsrp.into());
            ui.set_network_online(dashboard.online);
            ui.set_battery(dashboard.battery.into());
            ui.set_battery_level(dashboard.battery_level);
            ui.set_charging(dashboard.power == crate::agent::model::Power::Charging);
            ui.set_external_power(dashboard.power.external());
            ui.set_bypass_power(dashboard.power == crate::agent::model::Power::Bypass);
            ui.set_power_label(dashboard.power.label().into());
            ui.set_signal_level(dashboard.signal_level);
            ui.set_network_badge(dashboard.network_badge.into());
            ui.set_download(dashboard.download.into());
            ui.set_upload(dashboard.upload.into());
            ui.set_traffic_total(dashboard.total.into());
        }
        let active = !ui.get_asleep() && !self.security.locked;
        if !active {
            self.network.suspend();
        }
        if !snapshot.authenticated || snapshot.status != crate::agent::Status::Live {
            self.network.disconnected();
        }
        let on_network = self.page == "tailscale-network";
        let wifi_active =
            wifi_active && snapshot.authenticated && snapshot.status == crate::agent::Status::Live;
        if !wifi_active {
            self.wifi.suspend(ui);
        }
        for reply in agent.replies() {
            if self.home.receive(reply.id, reply.result.clone()) {
                continue;
            }
            if self.wifi.receive(reply.id, reply.result.clone()) {
                continue;
            }
            if self.clients.receive(reply.id, reply.result.clone()) {
                continue;
            }
            if self.features.receive(reply.id, reply.result.clone()) {
                continue;
            }
            if reply.id == 1 {
                // Replies from a previous login or a sleeping screen do not
                // establish fresh service status for the active session.
                if !active
                    || !snapshot.authenticated
                    || snapshot.status != crate::agent::Status::Live
                {
                    continue;
                }
                self.network.accept_status(reply.result);
                if on_network
                    && let Ok(address) =
                        crate::agent::connectivity::target(&self.prefs.probe_target)
                    && let Some(id) = self.network.begin_probe(address)
                    && let Err(error) = agent.probe_target(id, address)
                {
                    self.network
                        .accept_probe(id, &self.prefs.probe_target, Err(error));
                }
            } else {
                self.network
                    .accept_probe(reply.id, &self.prefs.probe_target, reply.result);
            }
        }
        if active
            && snapshot.authenticated
            && snapshot.status == crate::agent::Status::Live
            && on_network
            && self.network.last_refresh.elapsed() >= Duration::from_secs(30)
        {
            self.network.last_refresh = Instant::now();
            if let Err(error) = agent.call(1, crate::agent::Request::get("/api/tailscale/status")) {
                self.network.accept_status(Err(error));
            }
        }
        self.wifi.tick(ui, agent, wifi_active);
        self.clients.tick(
            agent,
            active && snapshot.authenticated && self.page == "overview",
        );
        ui.set_client_count(self.clients.label().into());
        self.features.editing =
            ui.get_editor_visible() || ui.get_choice_visible() || ui.get_confirm_visible();
        self.features
            .tick(agent, active && ui.get_view() == 7, snapshot.authenticated);
        if active && ui.get_view() == 7 {
            self.features.stream(&snapshot);
        }
        self.home.tick(
            agent,
            active
                && snapshot.authenticated
                && snapshot.status == crate::agent::Status::Live
                && ui.get_view() == 0,
            &self.prefs.home,
            &snapshot,
        );
        self.render_features(ui);
        ui.set_tailnet_status(self.network.label().into());
        ui.set_tailnet_online(self.network.online());
        self.home.render(ui, &self.prefs.home, Some(&snapshot));
        let rows = self.network.rows(&self.prefs.probe_target);
        if rows != self.network_rows {
            self.network_rows = rows.clone();
            ui.set_tailnet_readings(crate::ui_model::reconcile(
                ui.get_tailnet_readings(),
                rows.into_iter().map(|(k, v)| reading(&k, &v)).collect(),
                |row| row.label.clone(),
            ));
        }
        ui.set_probe_note(crate::i18n::tr("仅在此页亮屏解锁时检测，每 30 秒更新，结果有效期 60 秒。TCP 检测从 U60 发起；热点客户端仍需实际访问验证。").into());
    }

    fn render_features(&mut self, ui: &Screen) {
        if ui.get_view() != 7 {
            return;
        }
        ui.set_feature_busy(self.features.blocking());
        ui.set_feature_refreshing(self.features.busy());
        ui.set_feature_can_refresh(self.features.can_refresh());
        ui.set_feature_notice(self.features.notice.clone().into());
        if self.feature_revision == self.features.revision {
            return;
        }
        self.feature_revision = self.features.revision;
        let page = &self.features.page;
        let old_entries = ui.get_entries();
        let offset = ui.get_content_offset();
        let list_top = ui.get_feature_list_top();
        let step = ui.get_feature_entry_step();
        // Keep the same visible record at the same offset when new messages
        // arrive above it. At the top, show arrivals normally.
        let anchor = (offset < -list_top && step > 0.)
            .then(|| {
                let first = ((-offset - list_top) / step) as usize;
                (first..old_entries.row_count()).find_map(|index| {
                    let old = old_entries.row_data(index)?;
                    let id = old.id.strip_prefix("feature-item:")?;
                    let next = page.items.iter().position(|item| item.id == id)?;
                    Some((index, next))
                })
            })
            .flatten();
        if self.feature_qr_text.as_ref().map(|v| v.as_str()) != page.qr.as_deref() {
            let qr = page.qr.as_deref().and_then(crate::features::qr_image);
            ui.set_feature_has_qr(qr.is_some());
            ui.set_feature_qr(qr.unwrap_or_default());
            self.feature_qr_text = page.qr.clone().map(zeroize::Zeroizing::new);
        }
        ui.set_feature_qr_caption(page.qr_caption.clone().into());
        ui.set_feature_content(page.content.clone().into());
        if let Some(title) = &page.title {
            ui.set_heading(title.clone().into());
        } else if let Some(f) = self
            .catalog
            .features
            .iter()
            .find(|f| f.id == self.features.id)
        {
            ui.set_heading(f.title.clone().into());
        }
        let trends = ui.get_trends();
        ui.set_trends(crate::ui_model::reconcile(
            trends.clone(),
            page.charts
                .iter()
                .map(|c| {
                    let old = trends.iter().find(|t| t.id == c.id).unwrap_or_default();
                    Trend {
                        id: c.id.into(),
                        label: c.label.clone().into(),
                        unit: c.unit.into(),
                        reading: crate::ui_model::reconcile(
                            old.reading,
                            vec![c.value.clone().into()],
                            |_| 0,
                        ),
                        graph: crate::ui_model::reconcile(
                            old.graph,
                            vec![TrendPlot {
                                path: c.path.clone().into(),
                                upper: c.upper.clone().into(),
                                lower: c.lower.clone().into(),
                                samples: c.samples as i32,
                            }],
                            |_| 0,
                        ),
                    }
                })
                .collect(),
            |c| c.id.clone(),
        ));
        ui.set_meters(crate::ui_model::reconcile(
            ui.get_meters(),
            page.meters
                .iter()
                .map(|m| Meter {
                    id: m.id.clone().into(),
                    label: m.label.clone().into(),
                    value: m.value.clone().into(),
                    fraction: m.permille as f32 / 1000.,
                })
                .collect(),
            |m| m.id.clone(),
        ));
        ui.set_data_name(page.table.headings[0].clone().into());
        ui.set_data_first(page.table.headings[1].clone().into());
        ui.set_data_second(page.table.headings[2].clone().into());
        ui.set_data_replaces_items(page.table.replaces_items);
        ui.set_data_rows(crate::ui_model::reconcile(
            ui.get_data_rows(),
            page.table
                .rows
                .iter()
                .map(|r| DataRow {
                    id: r.id.clone().into(),
                    name: r.name.clone().into(),
                    detail: r.detail.clone().into(),
                    first: r.first.clone().into(),
                    second: r.second.clone().into(),
                    target: r.target.clone().into(),
                })
                .collect(),
            |r| r.id.clone(),
        ));
        ui.set_readings(crate::ui_model::reconcile(
            ui.get_readings(),
            page.rows.iter().map(|(k, v)| reading(k, v)).collect(),
            |row| row.label.clone(),
        ));
        ui.set_feature_fields(crate::ui_model::reconcile(
            ui.get_feature_fields(),
            page.fields
                .iter()
                .enumerate()
                .filter(|(_, f)| f.visible)
                .map(|(i, f)| FeatureInput {
                    index: i as i32,
                    title: f.label.clone().into(),
                    subtitle: f.display().into(),
                    tag: if f.dirty() {
                        crate::i18n::tr("已改")
                    } else {
                        ""
                    }
                    .into(),
                })
                .collect(),
            |row| row.index,
        ));
        ui.set_feature_actions(crate::ui_model::reconcile(
            ui.get_feature_actions(),
            page.actions
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    Some(a.id.as_str()) != crate::features::automatic::action(&self.features.id)
                })
                .map(|(index, a)| FeatureAction {
                    id: a.id.clone().into(),
                    label: a.label.clone().into(),
                    enabled: a.enabled,
                    index: index as i32,
                })
                .collect(),
            |row| row.id.clone(),
        ));
        ui.set_entries(crate::ui_model::reconcile(
            ui.get_entries(),
            page.items
                .iter()
                .map(|item| {
                    entry(
                        &format!("feature-item:{}", item.id),
                        &item.title,
                        &item.subtitle,
                        "",
                    )
                })
                .collect(),
            |row| row.id.clone(),
        ));
        if let Some((old, next)) = anchor {
            ui.set_content_offset(offset - (next as f32 - old as f32) * step);
        }
        ui.set_note(page.note.clone().into());
    }
    fn edit_feature(&mut self, ui: &Screen, index: usize) {
        self.features.pause_background();
        if self.features.busy() {
            return;
        }
        let Some(field) = self.features.page.fields.get(index) else {
            return;
        };
        if !field.available {
            ui.set_message(crate::i18n::tr("此设置当前不可用，请先刷新。").into());
            return;
        }
        if !field.choices.is_empty() {
            self.choice_field = Some(index);
            ui.set_choice_title(field.label.clone().into());
            ui.set_choices(model(
                field
                    .choices
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        entry(
                            &i.to_string(),
                            &c.label,
                            "",
                            if c.value == field.value { "●" } else { "" },
                        )
                    })
                    .collect(),
            ));
            ui.set_choice_visible(true);
        } else {
            self.editor = Some(Editor::Feature(index));
            ui.set_editor_title(field.label.clone().into());
            ui.set_editor_hint(
                if field.secret {
                    crate::i18n::tr("输入新值；留空保持不变。")
                } else {
                    crate::i18n::tr("完成输入后自动应用；需要中断服务时会先提示。")
                }
                .into(),
            );
            ui.set_editor_value(field.value.clone().into());
            ui.set_editor_secret(field.secret);
            ui.set_editor_multiline(field.multiline);
            ui.set_editor_layout(if field.range.is_some() { 1 } else { 0 });
            ui.set_editor_error("".into());
            ui.set_editor_visible(true);
        }
    }
    fn close_editor(&self, ui: &Screen) {
        ui.set_editor_visible(false);
        ui.set_editor_value("".into());
        ui.set_editor_error("".into());
    }
    fn apply_feature_edit(&mut self, ui: &Screen) {
        if let Err(error) = self.features.auto_apply() {
            ui.set_message(error.to_string().into());
        }
        if let Some(plan) = &self.features.confirmation {
            ui.set_confirm_body(plan.summary.clone().into());
            ui.set_confirm_phrase(plan.phrase.clone().unwrap_or_default().into());
            ui.set_confirm_word("".into());
            ui.set_confirm_visible(true);
        }
        self.render_features(ui);
    }
    fn edit(&mut self, ui: &Screen, kind: &str) {
        if ui.get_confirm_visible() && kind != "confirmation" {
            return;
        }
        let (purpose, title, hint, value, secret, layout) = match kind {
            "confirmation" => (
                Editor::Confirmation,
                crate::i18n::tr("确认操作"),
                crate::i18n::tr("输入页面提示的确认文字。"),
                String::new(),
                false,
                0,
            ),
            "agent-login" => (
                Editor::AgentLogin,
                crate::i18n::tr("后台密码"),
                crate::i18n::tr("输入已有的后台密码。"),
                String::new(),
                true,
                0,
            ),
            "probe-target" => (
                Editor::ProbeTarget,
                crate::i18n::tr("检测目标"),
                crate::i18n::tr("IP:端口 或 [IPv6]:端口；留空关闭检测。"),
                self.prefs.probe_target.clone(),
                false,
                1,
            ),
            "accent" => (
                Editor::Accent,
                crate::i18n::tr("自定义颜色"),
                crate::i18n::tr("输入六位 HEX 颜色，例如 #2563EB。"),
                self.prefs.accent.clone(),
                false,
                1,
            ),
            _ => return,
        };
        self.editor = Some(purpose);
        ui.set_editor_title(title.into());
        ui.set_editor_hint(hint.into());
        ui.set_editor_value(value.into());
        ui.set_editor_secret(secret);
        ui.set_editor_multiline(false);
        ui.set_editor_layout(layout);
        ui.set_editor_error("".into());
        ui.set_editor_visible(true);
    }
    fn submit_editor(&mut self, ui: &Screen) {
        if !ui.get_editor_visible() {
            return;
        }
        let value = zeroize::Zeroizing::new(ui.get_editor_value().to_string());
        let feature_edit = matches!(self.editor, Some(Editor::Feature(_)));
        match self.editor {
            Some(Editor::AgentLogin) => {
                if value.is_empty() || value.len() > 1024 {
                    ui.set_editor_error(
                        crate::i18n::tr("密码不能为空，长度不能超过 1024 字节。").into(),
                    );
                    return;
                }
                if let Some(agent) = &self.agent
                    && let Err(error) = agent.login(value)
                {
                    ui.set_editor_error(error.message().into());
                    return;
                }
            }
            Some(Editor::ProbeTarget) => {
                let target = value.trim();
                if !target.is_empty() && crate::agent::connectivity::target(target).is_err() {
                    ui.set_editor_error(
                        crate::i18n::tr("请输入有效目标，例如 10.20.0.10:443。").into(),
                    );
                    return;
                }
                self.prefs.probe_target = target.into();
                self.network.reset_target();
                self.persist(ui);
            }
            Some(Editor::Accent) => {
                if parse_color(value.trim()).is_err() {
                    ui.set_editor_error(
                        crate::i18n::tr("请输入六位 HEX 颜色，例如 #2563EB。").into(),
                    );
                    return;
                }
                self.prefs.accent = value.trim().into();
                self.persist(ui);
            }
            Some(Editor::Feature(index)) => {
                if let Err(error) = self.features.edit(index, value.to_string()) {
                    ui.set_editor_error(error.to_string().into());
                    return;
                }
                self.render_features(ui);
            }
            Some(Editor::Confirmation) => {
                if value.as_str() != ui.get_confirm_phrase().as_str() {
                    ui.set_editor_error(crate::i18n::tr("确认文字不匹配。").into());
                    return;
                }
                ui.set_confirm_word(value.to_string().into());
            }
            None => return,
        }
        self.editor = None;
        self.close_editor(ui);
        if feature_edit {
            self.apply_feature_edit(ui);
        }
    }
}

impl App {
    pub fn new(prefs: Preferences, prefs_path: Option<PathBuf>) -> Result<Self> {
        crate::i18n::select(prefs.language);
        let ui = Screen::new()?;
        crate::material::install(&ui)?;
        slint::select_bundled_translation(prefs.language.code())?;
        let state = Rc::new(RefCell::new(State {
            agent: None,
            live_revision: u64::MAX,
            network: crate::agent::network::Network::default(),
            clients: Default::default(),
            wifi: Default::default(),
            network_rows: Vec::new(),
            editor: None,
            features: crate::features::controller::Controller::default(),
            feature_revision: u64::MAX,
            feature_qr_text: None,
            choice_field: None,
            metric_choice: None,
            security: crate::security::ScreenLock::load(prefs_path.as_ref().map(|p| {
                p.with_file_name(".screen-private")
                    .join("screen-lock.private.json")
            }))?,
            prefs_baseline: serde_json::to_value(&prefs).unwrap(),
            prefs_checked: Instant::now(),
            prefs_stamp: prefs_path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok()),
            home: Default::default(),
            prefs,
            prefs_path,
            prefs_dirty: false,
            prefs_changed: Instant::now(),
            catalog: Catalog::load()?,
            history: vec![],
            page: "overview".into(),
            scenario: 0,
            last_input: Instant::now(),
        }));
        state.borrow().apply_preferences(&ui);
        state.borrow().apply_scenario(&ui);
        state.borrow_mut().show(&ui, "overview", false)?;
        state.borrow_mut().sync_security(&ui);
        // Weak component handles avoid keeping the UI alive through its own callbacks.
        macro_rules! bind {
            ($method:ident, |$s:ident, $u:ident $(, $arg:ident)*| $body:block) => {{
                let weak = ui.as_weak(); let state = state.clone();
                ui.$method(move |$($arg),*| {
                    if let Some($u) = weak.upgrade() {
                        let mut $s = state.borrow_mut();
                        if ($s.security.locked || $s.security.mode != crate::security::Mode::Hold)
                            && !matches!(stringify!($method), "on_power" | "on_activity" | "on_hold_start" | "on_hold_tick" | "on_hold_end" | "on_hold_cancel" | "on_pin_digit" | "on_pin_backspace" | "on_pin_submit" | "on_pin_cancel" | "on_pin_key") {
                            return;
                        }
                        if $u.get_editor_visible() && !matches!(stringify!($method), "on_power" | "on_activity" | "on_editor_submit" | "on_editor_cancel") { return; }
                        if $u.get_choice_visible() && !matches!(stringify!($method), "on_power" | "on_activity" | "on_choose" | "on_choice_cancel") { return; }
                        if $u.get_confirm_visible() && !matches!(stringify!($method), "on_power" | "on_activity" | "on_confirm_accept" | "on_confirm_cancel" | "on_edit" | "on_editor_submit" | "on_editor_cancel") { return; }
                        $s.last_input = Instant::now();
                        $body
                        $s.sync_security(&$u);
                    }
                });
            }};
        }
        bind!(on_navigate, |s, u, tab| {
            if !(0..3).contains(&tab) {
                return;
            }
            if u.get_asleep() {
                u.set_asleep(false);
                return;
            }
            s.history.clear();
            let _ = s.show(&u, ["overview", "connect", "manage"][tab as usize], false);
        });
        bind!(on_open, |s, u, id| {
            if let Some(id) = id.strip_prefix("feature-item:") {
                if let Err(error) = s.features.select(id) {
                    u.set_message(error.to_string().into());
                } else {
                    u.set_page_revision(u.get_page_revision().wrapping_add(1));
                }
                s.render_features(&u);
                return;
            }
            if let Some(n) = id
                .strip_prefix("scenario:")
                .and_then(|s| s.parse::<u8>().ok())
                .filter(|n| *n < 4)
            {
                s.scenario = n;
                s.apply_scenario(&u);
                let _ = s.show(&u, "overview", false);
                s.history.clear();
            } else if let Err(error) = s.show(&u, &id, true) {
                u.set_message(error.to_string().into());
            }
        });
        bind!(on_back, |s, u| {
            if u.get_wifi_expanded() {
                u.set_wifi_expanded(false);
                return;
            }
            if !u.get_message().is_empty() {
                u.set_message("".into());
                return;
            }
            if u.get_view() == 7 && s.features.back() {
                if let Some(feature) = s.catalog.features.iter().find(|f| f.id == s.features.id) {
                    u.set_heading(feature.title.clone().into());
                }
                s.render_features(&u);
                u.set_page_revision(u.get_page_revision().wrapping_add(1));
                return;
            }
            if let Some(id) = s.history.pop() {
                let _ = s.show(&u, &id, false);
            } else {
                let _ = s.show(&u, "overview", false);
            }
        });
        bind!(on_action, |_s, u, _index| {
            u.set_message(
                crate::i18n::tr(
                    "这项业务操作尚未接入设备。当前仅预览页面，不会更改网络或发送指令。",
                )
                .into(),
            );
        });
        bind!(on_refresh, |s, _u| {
            if let Some(agent) = &s.agent {
                let _ = agent.retry();
            }
            s.network.refresh();
        });
        bind!(on_wifi_toggle_password, |s, u| {
            s.wifi.toggle_password(&u);
        });
        bind!(on_wifi_switch_band, |s, u| {
            s.wifi.switch_band(&u);
        });
        bind!(on_wifi_expand, |_s, u| {
            if u.get_home_wifi_ready() {
                u.set_wifi_expanded(true);
            }
        });
        bind!(on_wifi_close, |_s, u| {
            u.set_wifi_expanded(false);
        });
        bind!(on_feature_edit, |s, u, index| {
            if let Ok(index) = usize::try_from(index) {
                s.edit_feature(&u, index);
            }
        });
        bind!(on_choose, |s, u, index| {
            if let Some(slot) = s.metric_choice.take() {
                let options = [
                    "none",
                    "clients",
                    "month",
                    "temperature",
                    "cpu",
                    "memory",
                    "uptime",
                    "battery",
                ];
                let Some(selected) = usize::try_from(index).ok().and_then(|i| options.get(i))
                else {
                    return;
                };
                let mut keys = [
                    s.prefs.home.metric_1.clone(),
                    s.prefs.home.metric_2.clone(),
                    s.prefs.home.metric_3.clone(),
                ];
                if *selected != "none" {
                    if let Some(other) = keys
                        .iter()
                        .enumerate()
                        .position(|(i, v)| i != slot && v == selected)
                    {
                        keys[other] = keys[slot].clone();
                    }
                }
                keys[slot] = (*selected).into();
                [
                    s.prefs.home.metric_1,
                    s.prefs.home.metric_2,
                    s.prefs.home.metric_3,
                ] = keys;
                u.set_choice_visible(false);
                s.persist(&u);
                return;
            }
            if let Some(field) = s.choice_field
                && let Ok(index) = usize::try_from(index)
                && let Some(choice) = s
                    .features
                    .page
                    .fields
                    .get(field)
                    .and_then(|f| f.choices.get(index))
            {
                let value = choice.value.clone();
                if let Err(error) = s.features.edit(field, value) {
                    u.set_message(error.to_string().into());
                }
            }
            s.choice_field = None;
            u.set_choice_visible(false);
            s.apply_feature_edit(&u);
        });
        bind!(on_choice_cancel, |s, u| {
            s.metric_choice = None;
            s.choice_field = None;
            u.set_choice_visible(false);
        });
        bind!(on_feature_action, |s, u, index| {
            if let Ok(index) = usize::try_from(index) {
                match s.features.prepare(index) {
                    Ok(()) => {
                        if let Some(plan) = &s.features.confirmation {
                            u.set_confirm_body(plan.summary.clone().into());
                            u.set_confirm_phrase(plan.phrase.clone().unwrap_or_default().into());
                            u.set_confirm_word("".into());
                            u.set_confirm_visible(true);
                        }
                    }
                    Err(error) => u.set_message(error.to_string().into()),
                }
            }
        });
        bind!(on_feature_refresh, |s, u| {
            s.features.refresh_preserving_draft();
            s.render_features(&u);
        });
        bind!(on_confirm_accept, |s, u| {
            if let Err(error) = s.features.confirm(&u.get_confirm_word()) {
                u.set_message(error.to_string().into());
            }
            u.set_confirm_visible(false);
            u.set_confirm_word("".into());
        });
        bind!(on_confirm_cancel, |s, u| {
            s.features.cancel_confirmation();
            u.set_confirm_visible(false);
            u.set_confirm_word("".into());
            s.render_features(&u);
        });
        bind!(on_edit, |s, u, kind| {
            s.edit(&u, &kind);
        });
        bind!(on_editor_submit, |s, u| {
            s.submit_editor(&u);
        });
        bind!(on_editor_cancel, |s, u| {
            s.editor = None;
            s.close_editor(&u);
        });
        {
            let weak = ui.as_weak();
            let state = state.clone();
            ui.on_editor_key(move |key| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                {
                    let mut s = state.borrow_mut();
                    if s.security.locked || ui.get_asleep() || !ui.get_editor_visible() {
                        return;
                    }
                    s.last_input = Instant::now();
                }
                // Dispatch outside the State borrow: TextInput can call activity.
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: key.clone() });
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: key });
            });
        }
        bind!(on_agent_forget, |s, u| {
            if let Some(agent) = &s.agent
                && let Err(error) = agent.forget()
            {
                u.set_message(error.message().into());
            }
        });
        bind!(on_theme, |s, u, dark| {
            s.prefs.dark = dark;
            s.prefs.follow_system = false;
            s.persist(&u);
        });
        bind!(on_system_theme, |s, u| {
            s.prefs.follow_system = true;
            s.persist(&u);
        });
        bind!(on_home_toggle, |s, u, key| {
            let field = match key.as_str() {
                "radio" => &mut s.prefs.home.show_radio,
                "wifi" => &mut s.prefs.home.show_wifi,
                "services" => &mut s.prefs.home.show_services,
                "tailscale" => &mut s.prefs.home.show_tailscale,
                "wireguard" => &mut s.prefs.home.show_wireguard,
                "mihomo" => &mut s.prefs.home.show_mihomo,
                _ => return,
            };
            *field = !*field;
            s.persist(&u);
        });
        bind!(on_home_metric, |s, u, index| {
            s.choice_field = None;
            let key = match index {
                0 => &s.prefs.home.metric_1,
                1 => &s.prefs.home.metric_2,
                2 => &s.prefs.home.metric_3,
                _ => return,
            };
            let options = [
                "none",
                "clients",
                "month",
                "temperature",
                "cpu",
                "memory",
                "uptime",
                "battery",
            ];
            u.set_choices(model(
                options
                    .iter()
                    .map(|item| {
                        entry(
                            item,
                            crate::home::metric_label(item),
                            "",
                            if *item == key { "✓" } else { "" },
                        )
                    })
                    .collect(),
            ));
            s.metric_choice = Some(index as usize);
            u.set_choice_title(crate::i18n::tr("快捷指标").into());
            u.set_choice_visible(true);
        });
        bind!(on_set_language, |s, u, code| {
            let Some(locale) = crate::i18n::Locale::parse(&code) else {
                return;
            };
            if locale == s.prefs.language {
                return;
            }
            if let Err(error) = slint::select_bundled_translation(locale.code()) {
                u.set_message(error.to_string().into());
                return;
            }
            crate::i18n::select(locale);
            s.prefs.language = locale;
            s.catalog = Catalog::load().expect("validated embedded catalog");
            s.persist(&u);
            s.live_revision = u64::MAX;
            s.feature_revision = u64::MAX;
            s.network.refresh();
            if s.agent.is_none() {
                s.apply_scenario(&u);
            }
            let page = s.page.clone();
            let _ = s.show(&u, &page, false);
            // Language selection is a discrete preference change. Save now so
            // an immediate restart keeps it, and report a failed save after
            // refreshing the page (show clears the previous message).
            s.flush_preferences(&u);
        });
        bind!(on_accent, |s, u, color| {
            match parse_color(&color) {
                Ok(_) => {
                    s.prefs.accent = color.to_string();
                    s.persist(&u);
                }
                Err(_) => {
                    u.set_message(crate::i18n::tr("请输入六位十六进制颜色，例如 #2563EB。").into())
                }
            }
        });
        bind!(on_set_brightness, |s, u, value| {
            s.prefs.brightness = value.clamp(10, 100) as u8;
            s.persist(&u);
        });
        bind!(on_cycle_sleep, |s, u| {
            let options = [0, 15, 30, 60, 120];
            s.prefs.sleep_seconds = options[(options
                .iter()
                .position(|v| *v == s.prefs.sleep_seconds)
                .unwrap_or(0)
                + 1)
                % options.len()];
            s.persist(&u);
        });
        bind!(on_power, |s, u| {
            if !u.get_asleep() {
                s.security.lock();
                s.features.suspend();
            }
            u.set_asleep(!u.get_asleep());
        });
        bind!(on_hold_start, |s, _u| {
            s.security.hold_start();
        });
        bind!(on_hold_tick, |s, _u| {
            s.security.hold_tick();
        });
        bind!(on_hold_end, |s, _u, inside| {
            s.security.hold_release(inside);
        });
        bind!(on_hold_cancel, |s, _u| {
            s.security.hold_cancel();
        });
        bind!(on_pin_setup, |s, _u, disable| {
            s.security.setup(disable);
        });
        bind!(on_pin_digit, |s, _u, n| {
            s.security.digit(n);
        });
        bind!(on_pin_key, |s, _u, key| {
            if key.len() == 1
                && let Ok(n) = key.parse::<i32>()
            {
                s.security.digit(n);
            }
        });
        bind!(on_pin_backspace, |s, _u| {
            s.security.backspace();
        });
        bind!(on_pin_submit, |s, _u| {
            s.security.submit();
        });
        bind!(on_pin_cancel, |s, _u| {
            s.security.cancel();
        });
        bind!(on_lock_now, |s, _u| {
            s.security.lock();
            s.features.suspend();
        });
        bind!(on_activity, |_s, _u| {});
        bind!(on_scenario, |s, u| {
            s.scenario = (s.scenario + 1) % 4;
            s.apply_scenario(&u);
        });
        ui.on_exit(|| {
            let _ = slint::quit_event_loop();
        });
        let timer = slint::Timer::default();
        let weak = ui.as_weak();
        let timer_state = state.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(200),
            move || {
                if let Some(ui) = weak.upgrade() {
                    let mut s = timer_state.borrow_mut();
                    s.security.poll();
                    if s.prefs_dirty && s.prefs_changed.elapsed() >= Duration::from_millis(500) {
                        s.flush_preferences(&ui);
                    }
                    s.poll_preferences(&ui);
                    if s.prefs.sleep_seconds != 0
                        && s.last_input.elapsed().as_secs() >= s.prefs.sleep_seconds as u64
                    {
                        if !ui.get_asleep() {
                            s.security.lock();
                        }
                        ui.set_asleep(true);
                    }
                    s.sync_security(&ui);
                    s.update_agent(&ui);
                }
            },
        );
        Ok(Self { ui, state, timer })
    }
    pub fn open(&self, page: &str) -> Result<()> {
        let page = match page {
            "tailscale-home" => "tailscale-network",
            "network-menu" => "connect",
            "apps" | "device-settings" => "manage",
            _ => page,
        };
        self.state.borrow_mut().show(&self.ui, page, true)
    }
    pub fn connect(&self, config: crate::agent::Config) -> Result<()> {
        let mut state = self.state.borrow_mut();
        state.agent = Some(crate::agent::Agent::start(config)?);
        self.ui.set_demo(false);
        self.ui.set_tailnet_online(false);
        self.ui
            .set_tailnet_status(crate::i18n::tr("等待查询连接状态").into());
        self.ui.set_tailnet_readings(model(vec![]));
        state.update_agent(&self.ui);
        Ok(())
    }
    #[cfg(test)]
    pub fn wifi_fixture(&self, data: crate::features::Data) {
        self.ui.set_demo(true);
        self.state.borrow_mut().wifi.fixture(&self.ui, data);
    }
    #[cfg(test)]
    pub fn wifi_expire_reveal(&self) {
        self.state.borrow_mut().wifi.expire_reveal(&self.ui);
    }
    #[cfg(test)]
    pub fn wifi_poll_fixture(&self, wifi: Value) {
        self.state.borrow_mut().wifi.poll_fixture(&self.ui, wifi);
    }
    #[cfg(test)]
    pub fn feature_fixture(&self, id: &str, value: Value) {
        let mut state = self.state.borrow_mut();
        state.features.seed(id, value);
        state.feature_revision = u64::MAX;
        self.ui.set_view(7);
        state.render_features(&self.ui);
    }
    #[cfg(test)]
    pub fn feature_data_fixture(&self, id: &str, data: crate::features::Data) {
        let mut state = self.state.borrow_mut();
        state.features.seed_data(id, data);
        state.feature_revision = u64::MAX;
        self.ui.set_view(7);
        self.ui.set_demo(false);
        self.ui
            .set_page_revision(self.ui.get_page_revision().wrapping_add(1));
        state.render_features(&self.ui);
    }
    #[cfg(test)]
    pub fn feature_poll_start(&self) {
        let mut state = self.state.borrow_mut();
        state.features.poll_fixture();
        state.render_features(&self.ui);
    }
    #[cfg(test)]
    pub fn feature_reply(&self, value: Value) {
        let mut state = self.state.borrow_mut();
        state.features.reply_fixture(Ok(value));
        state.render_features(&self.ui);
    }
    #[cfg(test)]
    pub fn feature_stream_fixture(&self, snapshot: &crate::agent::Snapshot) {
        let mut state = self.state.borrow_mut();
        state.features.stream(snapshot);
        state.render_features(&self.ui);
    }
    #[cfg(feature = "device")]
    pub fn activity(&self) {
        self.state.borrow_mut().last_input = Instant::now();
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.timer.stop();
        self.state.borrow_mut().flush_preferences(&self.ui);
    }
}

#[cfg(test)]
mod navigation_tests {
    use super::*;
    #[test]
    fn all_existing_capabilities_match_the_approved_product_roots() {
        let c = Catalog::load().unwrap();
        let agreed: Vec<Value> =
            serde_json::from_str(include_str!("../../design/product-ui/src/features.json"))
                .unwrap();
        assert_eq!(agreed.len(), 82);
        assert_eq!(c.features.len(), agreed.len());
        for expected in agreed {
            let id = expected["id"].as_str().unwrap();
            let feature = c
                .features
                .iter()
                .find(|f| f.id == id)
                .expect("preserved feature ID");
            let tab = match expected["screen"].as_str().unwrap() {
                "状态" => 0,
                "连接" => 1,
                "管理" => 2,
                _ => panic!("unknown root"),
            };
            assert_eq!(c.tab_for(&feature.group), tab, "{id}");
        }
    }
    #[test]
    fn network_services_and_nested_engines_stay_on_network_tab() {
        let c = Catalog::load().unwrap();
        for id in ["wireguard-service", "tailscale", "proxy", "mihomo"] {
            assert_eq!(c.tab_for(id), 1);
        }
        for id in [
            "tailscale-service",
            "tailscale-config",
            "tailscale-network",
            "tailscale-version",
            "wireguard",
            "proxy-service",
            "proxy-groups",
        ] {
            let f = c.features.iter().find(|f| f.id == id).unwrap();
            assert_eq!(c.tab_for(&f.group), 1);
        }
    }
}
