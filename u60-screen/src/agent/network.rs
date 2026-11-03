//! Presentation state keeps service status, path, TCP and forwarding separate.
use super::Error;
use serde_json::Value;
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

pub struct Network {
    pub last_refresh: Instant,
    status: Option<(Instant, Value)>,
    status_error: Option<Error>,
    probe: Option<(Instant, Value)>,
    probe_error: Option<Error>,
    pending: Option<(u64, SocketAddr)>,
    next_id: u64,
}
impl Default for Network {
    fn default() -> Self {
        Self {
            last_refresh: Instant::now() - Duration::from_secs(30),
            status: None,
            status_error: None,
            probe: None,
            probe_error: None,
            pending: None,
            next_id: 100,
        }
    }
}
impl Network {
    pub fn refresh(&mut self) {
        self.last_refresh = Instant::now() - Duration::from_secs(30);
    }
    pub fn reset_target(&mut self) {
        self.probe = None;
        self.pending = None;
        self.probe_error = None;
        self.refresh();
    }
    pub fn suspend(&mut self) {
        self.pending = None;
    }
    pub fn disconnected(&mut self) {
        self.status = None;
        self.probe = None;
        self.pending = None;
        self.refresh();
    }
    pub fn accept_status(&mut self, result: Result<Value, Error>) {
        match result {
            Ok(value) => {
                self.status_error = None;
                self.status = Some((Instant::now(), value));
            }
            Err(error) => {
                self.status = None;
                self.status_error = Some(error);
            }
        }
        if !self.online() {
            self.probe = None;
            self.pending = None;
        }
    }
    fn status(&self) -> Option<&Value> {
        self.status
            .as_ref()
            .filter(|(at, _)| at.elapsed() < Duration::from_secs(60))
            .map(|(_, v)| v)
    }
    pub fn online(&self) -> bool {
        self.status().is_some_and(|s| s["state"] == "healthy")
    }
    pub fn label(&self) -> &'static str {
        match self.status().and_then(|v| v["state"].as_str()) {
            Some("healthy") => crate::i18n::tr("已连接"),
            Some("not_installed") => crate::i18n::tr("尚未安装"),
            Some("disabled" | "paused") => crate::i18n::tr("已停止"),
            Some("needs_login" | "awaiting_setup") => crate::i18n::tr("需要登录 Tailscale"),
            Some("pending_approval") => crate::i18n::tr("等待设备批准"),
            Some(_) => crate::i18n::tr("正在连接"),
            None => {
                if self.status_error.is_some() {
                    crate::i18n::tr("状态查询失败")
                } else {
                    crate::i18n::tr("等待查询状态")
                }
            }
        }
    }
    pub fn begin_probe(&mut self, address: SocketAddr) -> Option<u64> {
        if !self.online() || self.pending.is_some() {
            return None;
        }
        self.next_id = self.next_id.wrapping_add(1).max(100);
        self.pending = Some((self.next_id, address));
        self.probe_error = None;
        Some(self.next_id)
    }
    pub fn accept_probe(&mut self, id: u64, target: &str, result: Result<Value, Error>) {
        let Some((wanted, address)) = self.pending else {
            return;
        };
        if wanted != id {
            return;
        }
        self.pending = None;
        if target.parse::<SocketAddr>().ok() != Some(address) || !self.online() {
            return;
        }
        match result {
            Ok(value) => {
                self.probe = Some((Instant::now(), value));
                self.probe_error = None;
            }
            Err(error) => {
                self.probe = None;
                self.probe_error = Some(error);
            }
        }
    }
    pub fn rows(&self, target: &str) -> Vec<(String, String)> {
        let mut rows = vec![("Tailscale".into(), self.label().into())];
        if let Some(status) = self.status() {
            rows.push((
                crate::i18n::tr("网关模式").into(),
                if status["gateway_mode"] == true {
                    crate::i18n::tr("已开启")
                } else {
                    crate::i18n::tr("未开启")
                }
                .into(),
            ));
        }
        let probe = self
            .probe
            .as_ref()
            .filter(|(at, _)| at.elapsed() < Duration::from_secs(60) && self.online());
        if let Some((at, result)) = probe {
            rows.push((
                crate::i18n::tr("目标路径").into(),
                match result["routed"].as_bool() {
                    Some(true) => crate::i18n::tr("经由 Tailscale"),
                    Some(false) => crate::i18n::tr("未使用 Tailscale"),
                    None => crate::i18n::tr("无法读取路由"),
                }
                .into(),
            ));
            rows.push((
                crate::i18n::tr("目标 TCP 服务").into(),
                if result["reachable"] == true {
                    crate::i18n::format(
                        "可连接 · {0} ms",
                        &[format!("{}", result["latency_ms"].as_u64().unwrap_or(0))],
                    )
                } else if result["routed"] != true {
                    crate::i18n::tr("未检测").into()
                } else if result["refused"] == true {
                    crate::i18n::tr("端口拒绝连接").into()
                } else if result.get("error").is_some() {
                    crate::i18n::tr("无法执行检测").into()
                } else {
                    crate::i18n::tr("未能连接").into()
                },
            ));
            rows.push((
                crate::i18n::tr("热点转发规则").into(),
                if result["gateway_rules_present"] == true {
                    crate::i18n::tr("已配置")
                } else {
                    crate::i18n::tr("未确认")
                }
                .into(),
            ));
            rows.push((
                crate::i18n::tr("检测时间").into(),
                crate::i18n::format("{0} 秒前", &[format!("{}", at.elapsed().as_secs())]),
            ));
        } else {
            let message = if target.is_empty() {
                crate::i18n::tr("尚未设置目标")
            } else if let Some(error) = &self.probe_error {
                error.message()
            } else if self.pending.is_some() {
                crate::i18n::tr("正在检测")
            } else if !self.online() {
                crate::i18n::tr("等待 Tailscale 连接")
            } else {
                crate::i18n::tr("尚未验证或结果已过期")
            };
            rows.push((crate::i18n::tr("连通性检测").into(), message.into()));
        }
        if let Some(error) = &self.status_error {
            rows.push((crate::i18n::tr("连接提示").into(), error.message().into()));
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn status_never_implies_reachability_and_old_target_results_are_discarded() {
        let mut network = Network::default();
        network.accept_status(Ok(json!({"state":"healthy","gateway_mode":true})));
        assert!(network.online());
        assert!(network.rows("").iter().any(|(_, v)| v == "尚未设置目标"));
        let target = "10.20.0.10:443";
        let old = network.begin_probe(target.parse().unwrap()).unwrap();
        network.reset_target();
        let current = network.begin_probe(target.parse().unwrap()).unwrap();
        let result =
            json!({"routed":true,"reachable":true,"gateway_rules_present":false,"latency_ms":42});
        network.accept_probe(old, target, Ok(result.clone()));
        assert!(network.probe.is_none());
        network.accept_probe(current, target, Ok(result));
        assert!(
            network
                .rows(target)
                .iter()
                .any(|(k, v)| k == "热点转发规则" && v == "未确认")
        );
        network.probe.as_mut().unwrap().0 -= Duration::from_secs(61);
        assert!(
            !network
                .rows(target)
                .iter()
                .any(|(k, _)| k == "目标 TCP 服务")
        );
        network.accept_status(Err(Error::Unavailable));
        assert!(!network.online());
        assert!(network.probe.is_none());
    }
}
