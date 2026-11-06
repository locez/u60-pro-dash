//! Visible-home summaries. Reads stop when hidden, locked or asleep. Display
//! preferences never change the corresponding service's operating state.
use crate::{
    MenuEntry, Screen,
    agent::{Agent, Error, Request, Snapshot},
    config::shared::Home,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
pub struct Summary {
    pending: BTreeMap<u64, (&'static str, Instant)>,
    values: BTreeMap<&'static str, (Value, Instant)>,
    sequence: u64,
    next: Instant,
    health: Value,
}
impl Default for Summary {
    fn default() -> Self {
        Self {
            pending: BTreeMap::new(),
            values: BTreeMap::new(),
            sequence: 1 << 40,
            next: Instant::now(),
            health: Value::Null,
        }
    }
}
pub fn metric_label(key: &str) -> &str {
    match key {
        "clients" => crate::i18n::tr("在线设备"),
        "month" => crate::i18n::tr("本月流量"),
        "temperature" => crate::i18n::tr("温度"),
        "cpu" => "CPU",
        "memory" => crate::i18n::tr("内存"),
        "uptime" => crate::i18n::tr("运行时间"),
        "battery" => crate::i18n::tr("电量"),
        _ => crate::i18n::tr("隐藏"),
    }
}
impl Summary {
    pub fn refresh(&mut self) {
        self.next = Instant::now();
    }
    fn value(&self, key: &str) -> &Value {
        self.values
            .get(key)
            .filter(|(_, at)| at.elapsed() < Duration::from_secs(65))
            .map(|(v, _)| v)
            .unwrap_or(&Value::Null)
    }
    pub fn receive(&mut self, id: u64, result: Result<Value, Error>) -> bool {
        let Some((key, _)) = self.pending.remove(&id) else {
            return false;
        };
        match result {
            Ok(v) => {
                self.values.insert(key, (v, Instant::now()));
            }
            Err(_) => {
                self.values.remove(key);
            }
        }
        true
    }
    pub fn tick(&mut self, agent: &Agent, active: bool, prefs: &Home, snapshot: &Snapshot) {
        self.health = snapshot.get("health").cloned().unwrap_or(Value::Null);
        if !active {
            self.pending.clear();
            self.values.clear();
            self.next = Instant::now();
            return;
        }
        self.pending
            .retain(|_, (_, at)| at.elapsed() < Duration::from_secs(35));
        if !self.pending.is_empty() || Instant::now() < self.next {
            return;
        }
        self.next = Instant::now() + Duration::from_secs(30);
        let metrics = [&prefs.metric_1, &prefs.metric_2, &prefs.metric_3];
        for (key, path, wanted) in [
            (
                "usage",
                "/api/data-usage",
                metrics.iter().any(|k| k.as_str() == "month"),
            ),
            (
                "memory",
                "/api/memory",
                metrics.iter().any(|k| k.as_str() == "memory"),
            ),
            (
                "system",
                "/api/device/system",
                metrics.iter().any(|k| k.as_str() == "uptime"),
            ),
            (
                "wireguard",
                "/api/vpn/wireguard",
                prefs.show_services && prefs.show_wireguard,
            ),
            (
                "mihomo",
                "/api/proxy/mihomo/status",
                prefs.show_services && prefs.show_mihomo,
            ),
        ] {
            if !wanted {
                self.values.remove(key);
                continue;
            }
            self.sequence += 1;
            let id = self.sequence;
            self.pending.insert(id, (key, Instant::now()));
            if let Err(e) = agent.call(id, Request::get(path)) {
                self.receive(id, Err(e));
            }
        }
    }
    pub fn render(&self, ui: &Screen, prefs: &Home, _snapshot: Option<&Snapshot>) {
        use crate::agent::model::number;
        let demo = ui.get_demo();
        let val = |key: &str| -> String {
            if demo {
                return match key {
                    "clients" => "6",
                    "month" => "62.8 GB",
                    "temperature" => "39°C",
                    "cpu" => "14%",
                    "memory" => "32%",
                    "uptime" => "2h 18m",
                    "battery" => "82%",
                    _ => "—",
                }
                .into();
            }
            match key {
                "clients" => ui.get_client_count().to_string(),
                "battery" => ui.get_battery().to_string(),
                "cpu" => number(&self.health["cpu"]["overall"])
                    .map(|n| format!("{n:.0}%"))
                    .unwrap_or("—".into()),
                "temperature" => self.health["thermal"]["sensors"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|s| number(&s["temperature_c"]))
                    .filter(|n| (-40.0..=150.).contains(n))
                    .reduce(f64::max)
                    .map(|n| format!("{n:.0}°C"))
                    .unwrap_or("—".into()),
                "month" => {
                    let v = self.value("usage");
                    number(&v["month"]["rx_bytes"])
                        .zip(number(&v["month"]["tx_bytes"]))
                        .map(|(a, b)| crate::agent::model::bytes(a + b))
                        .unwrap_or("—".into())
                }
                "memory" => {
                    let v = self.value("memory");
                    number(&v["used_kb"])
                        .zip(number(&v["total_kb"]))
                        .filter(|(_, n)| *n > 0.)
                        .map(|(a, b)| format!("{:.0}%", a / b * 100.))
                        .unwrap_or("—".into())
                }
                "uptime" => number(&self.value("system")["uptime"])
                    .filter(|n| *n >= 0.)
                    .map(|n| format!("{}h {}m", n as u64 / 3600, n as u64 / 60 % 60))
                    .unwrap_or("—".into()),
                _ => "—".into(),
            }
        };
        let metrics = [&prefs.metric_1, &prefs.metric_2, &prefs.metric_3]
            .into_iter()
            .enumerate()
            .filter(|(_, k)| k.as_str() != "none")
            .map(|(i, k)| MenuEntry {
                id: i.to_string().into(),
                title: metric_label(k).into(),
                subtitle: val(k).into(),
                tag: k.as_str().into(),
            })
            .collect();
        ui.set_home_metrics(crate::ui_model::reconcile(
            ui.get_home_metrics(),
            metrics,
            |r| r.id.clone(),
        ));
        let mut services = Vec::new();
        for (key, title, show, id) in [
            ("tailscale", "Tailscale", prefs.show_tailscale, "tailscale"),
            (
                "wireguard",
                "WireGuard",
                prefs.show_wireguard,
                "wireguard-service",
            ),
            ("mihomo", "Mihomo", prefs.show_mihomo, "mihomo"),
        ] {
            if !show || !prefs.show_services {
                continue;
            }
            let active = if key == "tailscale" {
                ui.get_tailnet_online()
            } else if key == "wireguard" {
                self.value(key)["active"] == true
            } else {
                self.value(key)["running"] == true
            };
            services.push(MenuEntry {
                id: id.into(),
                title: title.into(),
                subtitle: "".into(),
                tag: if active { "on" } else { "unknown" }.into(),
            });
        }
        ui.set_home_services(crate::ui_model::reconcile(
            ui.get_home_services(),
            services,
            |r| r.id.clone(),
        ));
    }
}
