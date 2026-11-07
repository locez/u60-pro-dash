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
    due: BTreeMap<&'static str, Instant>,
    health: Value,
}
impl Default for Summary {
    fn default() -> Self {
        Self {
            pending: BTreeMap::new(),
            values: BTreeMap::new(),
            sequence: 1 << 40,
            due: BTreeMap::new(),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServiceState {
    Running,
    Stopped,
    Attention,
    Unknown,
}
impl ServiceState {
    fn tag(self) -> &'static str {
        match self {
            Self::Running => "on",
            Self::Stopped => "off",
            Self::Attention => "attention",
            Self::Unknown => "unknown",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Running => crate::i18n::tr("运行中"),
            Self::Stopped => crate::i18n::tr("已停止"),
            Self::Attention => crate::i18n::tr("需要处理"),
            Self::Unknown => crate::i18n::tr("状态未知"),
        }
    }
}
fn service_state(key: &str, value: &Value) -> ServiceState {
    if key == "tailscale" {
        return match value["state"].as_str() {
            Some("healthy") => ServiceState::Running,
            Some("disabled" | "paused" | "not_installed") => ServiceState::Stopped,
            Some("unknown" | "") | None => ServiceState::Unknown,
            Some(_) => ServiceState::Attention,
        };
    }
    let running = if key == "wireguard" {
        "active"
    } else {
        "running"
    };
    match value[running].as_bool() {
        Some(true) => ServiceState::Running,
        Some(false) if value["enabled"] == true => ServiceState::Attention,
        Some(false) => ServiceState::Stopped,
        None => ServiceState::Unknown,
    }
}
impl Summary {
    pub fn refresh(&mut self) {
        self.due.clear();
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
            Err(Error::Busy(seconds)) => {
                // The request worker serializes work. Retry only this read;
                // other due summaries can proceed before its retry deadline.
                self.due.insert(
                    key,
                    Instant::now() + Duration::from_secs(seconds.clamp(1, 5)),
                );
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
            self.due.clear();
            return;
        }
        if let Some((id, path)) = self.next_request(prefs, Instant::now()) {
            if let Err(e) = agent.call(id, Request::get(path)) {
                self.receive(id, Err(e));
            }
        }
    }
    fn next_request(&mut self, prefs: &Home, now: Instant) -> Option<(u64, &'static str)> {
        self.pending
            .retain(|_, (_, at)| now.saturating_duration_since(*at) < Duration::from_secs(35));
        if !self.pending.is_empty() {
            return None;
        }
        let metrics = [&prefs.metric_1, &prefs.metric_2, &prefs.metric_3];
        for (key, path, wanted) in [
            (
                "tailscale",
                "/api/tailscale/status",
                prefs.show_services && prefs.show_tailscale,
            ),
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
                self.due.remove(key);
                continue;
            }
            if self.due.get(key).is_some_and(|at| now < *at) {
                continue;
            }
            self.sequence += 1;
            let id = self.sequence;
            self.pending.insert(id, (key, now));
            self.due.insert(key, now + Duration::from_secs(30));
            return Some((id, path));
        }
        None
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
            let state = if demo {
                ServiceState::Running
            } else {
                service_state(key, self.value(key))
            };
            services.push(MenuEntry {
                id: id.into(),
                title: title.into(),
                subtitle: state.label().into(),
                tag: state.tag().into(),
            });
        }
        ui.set_home_services(crate::ui_model::reconcile(
            ui.get_home_services(),
            services,
            |r| r.id.clone(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn each_service_uses_its_live_state_not_installation_or_preference() {
        for (key, value, state) in [
            (
                "tailscale",
                json!({"state":"healthy"}),
                ServiceState::Running,
            ),
            (
                "tailscale",
                json!({"enabled":true,"state":"needs_login"}),
                ServiceState::Attention,
            ),
            (
                "tailscale",
                json!({"state":"disabled"}),
                ServiceState::Stopped,
            ),
            ("wireguard", json!({"active":true}), ServiceState::Running),
            (
                "wireguard",
                json!({"active":false,"enabled":false}),
                ServiceState::Stopped,
            ),
            (
                "wireguard",
                json!({"active":false,"enabled":true}),
                ServiceState::Attention,
            ),
            ("mihomo", json!({"running":true}), ServiceState::Running),
            (
                "mihomo",
                json!({"running":false,"enabled":true}),
                ServiceState::Attention,
            ),
            ("mihomo", json!({"running":false}), ServiceState::Stopped),
        ] {
            assert_eq!(service_state(key, &value), state);
        }
        for key in ["tailscale", "wireguard", "mihomo"] {
            assert_eq!(service_state(key, &Value::Null), ServiceState::Unknown);
            assert_eq!(
                service_state(key, &json!({"enabled":true,"installed":true})),
                ServiceState::Unknown
            );
        }
    }

    #[test]
    fn sequential_home_reads_retry_busy_without_starving_later_services() {
        let prefs = Home {
            metric_1: "none".into(),
            metric_2: "none".into(),
            metric_3: "none".into(),
            ..Default::default()
        };
        let mut summary = Summary::default();
        let now = Instant::now();
        let (ts, path) = summary.next_request(&prefs, now).unwrap();
        assert_eq!(path, "/api/tailscale/status");
        assert!(
            summary.next_request(&prefs, now).is_none(),
            "one request in flight"
        );
        summary.receive(ts, Err(Error::Busy(1)));
        let (wg, path) = summary.next_request(&prefs, now).unwrap();
        assert_eq!(path, "/api/vpn/wireguard");
        summary.receive(wg, Ok(json!({"active":false})));
        let (proxy, path) = summary.next_request(&prefs, now).unwrap();
        assert_eq!(path, "/api/proxy/mihomo/status");
        summary.receive(proxy, Ok(json!({"running":true})));
        assert_eq!(
            service_state("mihomo", summary.value("mihomo")),
            ServiceState::Running
        );
        let (retry, path) = summary
            .next_request(&prefs, now + Duration::from_secs(2))
            .unwrap();
        assert_eq!(path, "/api/tailscale/status");
        assert!(
            !summary.receive(ts, Ok(json!({"state":"disabled"}))),
            "late replies cannot replace a newer request"
        );
        summary.receive(retry, Ok(json!({"state":"healthy"})));
        assert_eq!(
            service_state("tailscale", summary.value("tailscale")),
            ServiceState::Running
        );
        summary.values.get_mut("tailscale").unwrap().1 -= Duration::from_secs(66);
        assert_eq!(
            service_state("tailscale", summary.value("tailscale")),
            ServiceState::Unknown
        );
        assert!(
            summary
                .next_request(&prefs, now + Duration::from_secs(3))
                .is_none()
        );
    }
}
