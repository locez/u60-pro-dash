use crate::{
    MenuEntry, Palette, Reading, Screen,
    config::{Preferences, parse_color, readable_accent},
};
use anyhow::{Result, bail};
use serde::Deserialize;
use serde_json::Value;
use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};
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
    pub fn load() -> Result<Self> {
        Ok(serde_json::from_str(include_str!(
            "../assets/catalog.json"
        ))?)
    }
}

pub struct App {
    pub ui: Screen,
    pub state: Rc<RefCell<State>>,
    timer: slint::Timer,
}
pub struct State {
    agent: Option<crate::agent::Agent>,
    live_revision: u64,
    last_tailscale: Instant,
    pub security: crate::security::ScreenLock,
    pub prefs: Preferences,
    prefs_path: Option<PathBuf>,
    prefs_dirty: bool,
    prefs_changed: Instant,
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
        Value::String(s) if s.is_empty() => "未填写".into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => if *b { "开启" } else { "关闭" }.into(),
        Value::Array(a) => a.iter().map(display).collect::<Vec<_>>().join("、"),
        v => v.to_string(),
    }
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}

impl State {
    fn show(&mut self, ui: &Screen, id: &str, push: bool) -> Result<()> {
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
                title = "后台连接".into();
                view = 6;
                tab = 3;
            }
            "overview" => {
                title = "随身网络".to_string();
                view = 0;
                tab = 0;
            }
            "network-menu" | "apps" | "device-settings" => {
                let (t, label) = match id {
                    "network-menu" => (1, "网络"),
                    "apps" => (2, "应用"),
                    _ => (3, "设备"),
                };
                tab = t;
                title = label.into();
                if tab == 3 {
                    if self.agent.is_some() {
                        entries.push(entry("account", "后台连接", "连接状态与登录", ""));
                    }
                    entries.push(entry(
                        "screen-lock",
                        "锁屏与密码",
                        "长按解锁与可选密码保护",
                        "",
                    ));
                    entries.push(entry(
                        "screen-prefs",
                        "屏幕与外观",
                        "明暗、配色、亮度与熄屏",
                        "",
                    ));
                    if self.agent.is_none() {
                        entries.push(entry(
                            "demo-scenario",
                            "演示场景",
                            "正常、回家离线、无网络、低电量",
                            "",
                        ));
                    }
                }
                for g in &self.catalog.groups {
                    if g.root == id {
                        let count = self
                            .catalog
                            .features
                            .iter()
                            .filter(|f| f.group == g.id)
                            .count();
                        entries.push(entry(&g.id, &g.title, &g.subtitle, &count.to_string()));
                    }
                }
            }
            "screen-lock" => {
                title = "锁屏与密码".into();
                view = 5;
                tab = 3;
            }
            "screen-prefs" => {
                title = "屏幕与外观".into();
                view = 3;
                tab = 3;
            }
            "tailscale-home" => {
                title = "家庭内网".into();
                view = 4;
                tab = 2;
            }
            "demo-scenario" => {
                title = "演示场景".into();
                for (index, (name, subtitle)) in [
                    ("正常连接", "蜂窝与家庭子网均可用"),
                    ("家庭节点离线", "Tailscale 在线，家庭内网不可达"),
                    ("蜂窝断开", "回家通道等待网络"),
                    ("低电量", "电量 14%，网络正常"),
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
                    tab = match g.root.as_str() {
                        "network-menu" => 1,
                        "apps" => 2,
                        _ => 3,
                    };
                    for f in &self.catalog.features {
                        if f.group == id {
                            entries.push(entry(
                                &f.id,
                                &f.title,
                                if f.description.is_empty() {
                                    "查看功能"
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
                        tab = match g.root.as_str() {
                            "network-menu" => 1,
                            "apps" => 2,
                            _ => 3,
                        };
                    }
                    description = f.description.clone();
                    for [label, value] in &f.rows {
                        readings.push(reading(label, value));
                    }
                    for field in &f.fields {
                        let value = if text(field, "type") == "password" {
                            "未显示".into()
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
                        note.push_str("当前展示演示配置。设备读写尚未接入。完整表单交互可在浏览器原型中体验。");
                    }
                    if self.agent.is_some() {
                        readings.clear();
                        entries.clear();
                        actions.clear();
                        note = "正在接入此功能，当前尚不能操作。".into();
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
        ui.set_kicker(
            ["01 / OVERVIEW", "02 / NETWORK", "03 / APPS", "04 / DEVICE"][tab as usize].into(),
        );
        ui.set_can_back(!matches!(
            id,
            "overview" | "network-menu" | "apps" | "device-settings"
        ));
        ui.set_view(view);
        ui.set_entries(model(entries));
        ui.set_readings(model(readings));
        ui.set_actions(model(actions));
        ui.set_description(description.into());
        ui.set_note(note.into());
        ui.set_page_revision(ui.get_page_revision().wrapping_add(1));
        Ok(())
    }

    fn apply_preferences(&self, ui: &Screen) {
        let rgb = parse_color(&self.prefs.accent).expect("validated preferences");
        let ink = readable_accent(rgb, self.prefs.dark);
        let palette = ui.global::<Palette>();
        palette.set_dark(self.prefs.dark);
        palette.set_accent(Color::from_rgb_u8(rgb[0], rgb[1], rgb[2]));
        palette.set_accent_ink(Color::from_rgb_u8(ink[0], ink[1], ink[2]));
        let on = crate::config::button_ink(rgb);
        palette.set_accent_on(Color::from_rgb_u8(on[0], on[1], on[2]));
        ui.set_accent_hex(self.prefs.accent.clone().into());
        ui.set_brightness(self.prefs.brightness.into());
        ui.set_sleep_label(if self.prefs.sleep_seconds == 0 {
            "从不".into()
        } else {
            format!("{} 秒", self.prefs.sleep_seconds).into()
        });
    }

    pub fn sync_security(&self, ui: &Screen) {
        use crate::security::Mode;
        let s = &self.security;
        ui.set_locked(s.locked);
        ui.set_pin_enabled(s.enabled());
        ui.set_pin_visible(s.mode != Mode::Hold);
        ui.set_pin_busy(s.busy());
        ui.set_pin_title(
            match s.mode {
                Mode::Unlock => "解锁屏幕",
                Mode::New | Mode::Confirm => "设置解锁密码",
                Mode::Change => "修改解锁密码",
                Mode::Disable => "关闭密码解锁",
                Mode::Hold => "屏幕锁",
            }
            .into(),
        );
        ui.set_pin_message(s.message.clone().into());
        ui.set_pin_masked("●".repeat(s.input.len()).into());
        ui.set_pin_remaining(s.remaining() as i32);
        ui.set_hold_progress(s.progress);
    }

    fn persist(&mut self, ui: &Screen) {
        self.last_input = Instant::now();
        self.apply_preferences(ui);
        self.prefs_dirty = self.prefs_path.is_some();
        self.prefs_changed = Instant::now();
    }

    fn flush_preferences(&mut self, ui: &Screen) {
        if !self.prefs_dirty {
            return;
        }
        self.prefs_dirty = false;
        if let Some(path) = &self.prefs_path
            && let Err(error) = self.prefs.save(path)
        {
            eprintln!("preferences: {error:#}");
            ui.set_message("设置已生效，但未能保存到偏好文件。".into());
        }
    }
    fn apply_scenario(&self, ui: &Screen) {
        if self.agent.is_some() {
            return;
        }
        ui.set_network_online(self.scenario != 2);
        ui.set_home_online(self.scenario == 0 || self.scenario == 3);
        ui.set_battery(if self.scenario == 3 { "14%" } else { "82%" }.into());
        ui.set_download(if self.scenario == 2 { "—" } else { "128.6" }.into());
        ui.set_upload(if self.scenario == 2 { "—" } else { "16.4" }.into());
        ui.set_signal_strength(if self.scenario == 2 { "—" } else { "−86" }.into());
        ui.set_network_mode(if self.scenario == 2 { "离线" } else { "5G" }.into());
        ui.set_radio_label(
            if self.scenario == 2 {
                "等待蜂窝网络"
            } else {
                "5G SA · n78"
            }
            .into(),
        );
        ui.set_home_status(
            if self.scenario == 0 || self.scenario == 3 {
                "家庭内网可达"
            } else {
                "家庭内网未连通"
            }
            .into(),
        );
        ui.set_home_readings(model(vec![
            reading(
                "Tailscale",
                if self.scenario == 2 {
                    "等待网络"
                } else {
                    "已连接"
                },
            ),
            reading(
                "家庭子网",
                if self.scenario == 0 || self.scenario == 3 {
                    "演示：已验证可达"
                } else {
                    "演示：不可达"
                },
            ),
        ]));
    }

    fn update_agent(&mut self, ui: &Screen) {
        let Some(agent) = &mut self.agent else {
            return;
        };
        let mut topics = if ui.get_asleep() {
            vec![]
        } else {
            ["signal", "traffic", "battery"].map(String::from).to_vec()
        };
        if !ui.get_asleep() && !self.security.locked {
            topics.push("clients".into());
        }
        agent.watch(topics);
        let snapshot = agent.snapshot();
        if snapshot.revision != self.live_revision {
            self.live_revision = snapshot.revision;
            let dashboard = crate::agent::model::Dashboard::read(&snapshot);
            ui.set_connection_label(snapshot.status.label().into());
            ui.set_network_mode(dashboard.network.into());
            ui.set_radio_label(dashboard.radio.into());
            ui.set_signal_strength(dashboard.rsrp.into());
            ui.set_network_online(dashboard.online);
            ui.set_battery(dashboard.battery.into());
            ui.set_download(dashboard.download.into());
            ui.set_upload(dashboard.upload.into());
            ui.set_traffic_total(dashboard.total.into());
            ui.set_client_count(dashboard.clients.into());
        }
        if snapshot.authenticated
            && !ui.get_asleep()
            && !self.security.locked
            && matches!(self.page.as_str(), "overview" | "tailscale-home")
            && self.last_tailscale.elapsed() >= Duration::from_secs(30)
        {
            self.last_tailscale = Instant::now();
            let _ = agent.call(1, crate::agent::Request::get("/api/tailscale/status"));
        }
        for reply in agent.replies() {
            if reply.id == 1 {
                match reply.result {
                    Ok(data) => {
                        let text = match data["state"].as_str() {
                            Some("healthy") => "Tailscale 在线 · 内网待验证",
                            Some("not_installed") => "尚未安装 Tailscale",
                            Some("disabled" | "paused") => "Tailscale 已停止",
                            Some("needs_login" | "awaiting_setup") => "Tailscale 需要登录",
                            Some("pending_approval") => "等待设备批准",
                            _ => "Tailscale 正在连接",
                        };
                        ui.set_home_status(text.into());
                        ui.set_home_online(false);
                        ui.set_home_readings(model(vec![
                            reading("Tailscale", text),
                            reading(
                                "热点网关",
                                if data["gateway_mode"] == true {
                                    "已开启"
                                } else {
                                    "未开启"
                                },
                            ),
                            reading("家庭服务", "尚未验证"),
                        ]));
                    }
                    Err(error) => {
                        ui.set_home_status(error.message().into());
                        ui.set_home_online(false);
                    }
                }
            }
        }
    }
}

impl App {
    pub fn new(prefs: Preferences, prefs_path: Option<PathBuf>) -> Result<Self> {
        let ui = Screen::new()?;
        let state = Rc::new(RefCell::new(State {
            agent: None,
            live_revision: u64::MAX,
            last_tailscale: Instant::now() - Duration::from_secs(30),
            security: crate::security::ScreenLock::load(prefs_path.as_ref().map(|p| {
                p.with_file_name(".screen-private")
                    .join("screen-lock.private.json")
            }))?,
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
        state.borrow().sync_security(&ui);
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
                        $s.last_input = Instant::now();
                        $body
                        $s.sync_security(&$u);
                    }
                });
            }};
        }
        bind!(on_navigate, |s, u, tab| {
            if !(0..4).contains(&tab) {
                return;
            }
            if u.get_asleep() {
                u.set_asleep(false);
                return;
            }
            s.history.clear();
            let _ = s.show(
                &u,
                ["overview", "network-menu", "apps", "device-settings"][tab as usize],
                false,
            );
        });
        bind!(on_open, |s, u, id| {
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
            if !u.get_message().is_empty() {
                u.set_message("".into());
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
                "这项业务操作尚未接入设备。当前仅预览页面，不会更改网络或发送指令。".into(),
            );
        });
        bind!(on_refresh, |s, _u| {
            if let Some(agent) = &s.agent {
                let _ = agent.retry();
            }
            s.last_tailscale = Instant::now() - Duration::from_secs(30);
        });
        bind!(on_agent_login, |s, u, password| {
            if let Some(agent) = &s.agent {
                if password.is_empty() {
                    u.set_message("请输入后台密码。".into());
                } else if let Err(error) =
                    agent.login(zeroize::Zeroizing::new(password.to_string()))
                {
                    u.set_message(error.message().into());
                }
            }
        });
        bind!(on_agent_forget, |s, u| {
            if let Some(agent) = &s.agent
                && let Err(error) = agent.forget()
            {
                u.set_message(error.message().into());
            }
        });
        bind!(on_theme, |s, u, dark| {
            s.prefs.dark = dark;
            s.persist(&u);
        });
        bind!(on_accent, |s, u, color| {
            match parse_color(&color) {
                Ok(_) => {
                    s.prefs.accent = color.to_string();
                    s.persist(&u);
                }
                Err(_) => u.set_message("请输入六位十六进制颜色，例如 #2563EB。".into()),
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
        self.state.borrow_mut().show(&self.ui, page, true)
    }
    pub fn connect(&self, config: crate::agent::Config) -> Result<()> {
        let mut state = self.state.borrow_mut();
        state.agent = Some(crate::agent::Agent::start(config)?);
        self.ui.set_demo(false);
        self.ui.set_home_online(false);
        self.ui.set_home_status("等待查询连接状态".into());
        self.ui.set_home_readings(model(vec![]));
        state.update_agent(&self.ui);
        Ok(())
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
