//! Unlocked overview sharing, backed by one bounded read-only controller.
use crate::{
    Screen,
    agent::{Agent, Error},
    features::{
        self, Data,
        controller::Controller,
        wifi_share::{self, HomeJoin},
    },
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub struct WifiHome {
    controller: Controller,
    revision: u64,
    current: Option<HomeJoin>,
    preferred: &'static str,
    revealed_until: Option<Instant>,
    sampled: Option<Instant>,
    demo_data: Data,
}
impl Default for WifiHome {
    fn default() -> Self {
        let mut controller = Controller::default();
        controller.open("wifi-home");
        Self {
            controller,
            revision: u64::MAX,
            current: None,
            preferred: "2g",
            revealed_until: None,
            sampled: None,
            demo_data: Data::from([(
                "wifi".into(),
                Ok(
                    json!({"wifi_onoff":"1","radio2_disabled":"0","radio5_disabled":"0","ssid_2g":"U60 Demo","ssid_5g":"U60 Demo","key_2g":"example-passphrase-2026","key_5g":"example-passphrase-2026","encryption_2g":"sae-mixed","encryption_5g":"sae-mixed","hidden_2g":"0","hidden_5g":"0"}),
                ),
            )]),
        }
    }
}
impl WifiHome {
    fn clear_display(&mut self, ui: &Screen) {
        self.current = None;
        self.sampled = None;
        self.revealed_until = None;
        ui.set_home_wifi_ready(false);
        ui.set_home_wifi_revealed(false);
        ui.set_home_wifi_protected(false);
        ui.set_home_wifi_name("".into());
        ui.set_home_wifi_key("".into());
        ui.set_home_wifi_band("".into());
        ui.set_home_wifi_switchable(false);
        ui.set_home_wifi_qr(Default::default());
        ui.set_home_wifi_qr_large(Default::default());
        ui.set_wifi_expanded(false);
        ui.set_home_wifi_message(crate::i18n::tr("正在读取 Wi-Fi…").into());
    }
    pub fn suspend(&mut self, ui: &Screen) {
        self.controller.suspend();
        self.revision = u64::MAX;
        self.clear_display(ui);
    }
    pub fn receive(&mut self, id: u64, result: Result<Value, Error>) -> bool {
        self.controller.receive(id, result)
    }
    fn apply(&mut self, ui: &Screen, result: Result<HomeJoin, &'static str>) {
        self.sampled = Some(Instant::now());
        match result {
            Ok(join) => {
                let changed = self.current.as_ref().is_none_or(|old| {
                    old.payload.as_str() != join.payload.as_str()
                        || old.band != join.band
                        || old.switchable != join.switchable
                });
                if changed {
                    self.revealed_until = None;
                    let small = features::qr_image_sized(&join.payload, 148);
                    let large = features::qr_image(&join.payload);
                    ui.set_home_wifi_ready(small.is_some() && large.is_some());
                    ui.set_home_wifi_qr(small.unwrap_or_default());
                    ui.set_home_wifi_qr_large(large.unwrap_or_default());
                    ui.set_home_wifi_name(join.ssid.clone().into());
                    ui.set_home_wifi_band(join.band.into());
                    ui.set_home_wifi_switchable(join.switchable);
                    ui.set_home_wifi_protected(join.protected);
                    ui.set_home_wifi_message("".into());
                    self.current = Some(join);
                }
            }
            Err(message) => {
                self.clear_display(ui);
                ui.set_home_wifi_message(message.into());
            }
        }
        self.mask_expired(ui);
    }
    fn mask_expired(&mut self, ui: &Screen) {
        let revealed = self
            .revealed_until
            .is_some_and(|until| Instant::now() < until);
        if !revealed {
            self.revealed_until = None;
        }
        ui.set_home_wifi_revealed(revealed);
        if let Some(join) = &self.current {
            ui.set_home_wifi_key(if !join.protected {
                crate::i18n::tr("无需密码").into()
            } else if revealed {
                join.password.as_str().into()
            } else {
                "********".into()
            });
        }
    }
    pub fn tick(&mut self, ui: &Screen, agent: &Agent, active: bool) {
        if !active {
            self.suspend(ui);
            return;
        }
        self.controller.tick(agent, true, true);
        if self
            .sampled
            .is_some_and(|at| at.elapsed() > Duration::from_secs(60))
        {
            self.clear_display(ui);
        }
        self.render_reading(ui);
        self.mask_expired(ui);
    }
    fn render_reading(&mut self, ui: &Screen) {
        // Fresh reads can carry unchanged data. The generic page revision
        // intentionally does not advance in that case, but expiry must renew.
        if !self.controller.busy() && self.revision != self.controller.read_revision {
            self.revision = self.controller.read_revision;
            self.apply(ui, wifi_share::home(&self.controller.data, self.preferred));
        }
    }
    pub fn demo(&mut self, ui: &Screen, active: bool) {
        if !active {
            self.suspend(ui);
            return;
        }
        if self.current.is_none() {
            self.apply(ui, wifi_share::home(&self.demo_data, self.preferred));
        }
        self.mask_expired(ui);
    }
    pub fn toggle_password(&mut self, ui: &Screen) {
        if !self.current.as_ref().is_some_and(|n| n.protected) || !ui.get_home_wifi_ready() {
            return;
        }
        self.revealed_until = if ui.get_home_wifi_revealed() {
            None
        } else {
            Some(Instant::now() + Duration::from_secs(15))
        };
        self.mask_expired(ui);
    }
    pub fn switch_band(&mut self, ui: &Screen) {
        let Some(join) = &self.current else { return };
        if !join.switchable {
            return;
        }
        self.preferred = if join.selection == "2g" { "5g" } else { "2g" };
        let data = if ui.get_demo() {
            &self.demo_data
        } else {
            &self.controller.data
        };
        self.apply(ui, wifi_share::home(data, self.preferred));
    }
    #[cfg(test)]
    pub fn fixture(&mut self, ui: &Screen, data: Data) {
        self.demo_data = data;
        self.current = None;
        self.demo(ui, true);
    }
    #[cfg(test)]
    pub fn poll_fixture(&mut self, ui: &Screen, wifi: Value) {
        self.controller.poll_fixture();
        self.controller.reply_fixture(Ok(wifi));
        self.render_reading(ui);
        assert!(
            self.sampled
                .is_some_and(|at| at.elapsed() < Duration::from_secs(1))
        );
        self.sampled = Some(Instant::now() - Duration::from_secs(61));
    }
    #[cfg(test)]
    pub fn expire_reveal(&mut self, ui: &Screen) {
        self.revealed_until = Some(Instant::now() - Duration::from_secs(1));
        self.mask_expired(ui);
    }
}
