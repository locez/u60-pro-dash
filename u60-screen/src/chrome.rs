//! Public screen chrome: no credentials retained, bounded plots, no sleep polling.
use crate::{
    Screen,
    agent::{Agent, Error, Request, Snapshot},
    i18n::Locale,
};
use serde_json::Value;
use std::{
    collections::VecDeque,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub struct Chrome {
    next_wifi: Instant,
    pending: Option<(u64, Instant)>,
    sequence: u64,
    wifi: Option<(bool, Instant)>,
    clock_key: Option<(u64, Locale)>,
    history: TrafficHistory,
}
impl Default for Chrome {
    fn default() -> Self {
        Self {
            next_wifi: Instant::now(),
            pending: None,
            sequence: 1 << 42,
            wifi: None,
            clock_key: None,
            history: Default::default(),
        }
    }
}
fn flag(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::String(value) if value == "1" || value == "0" => Some(value == "1"),
        Value::Number(value) if value.as_u64() == Some(0) || value.as_u64() == Some(1) => {
            Some(value.as_u64() == Some(1))
        }
        _ => None,
    }
}
fn wifi_enabled(value: &Value) -> Option<bool> {
    match flag(&value["wifi_onoff"]) {
        Some(false) => Some(false),
        Some(true) => match (
            flag(&value["radio2_disabled"]),
            flag(&value["radio5_disabled"]),
        ) {
            (Some(false), _) | (_, Some(false)) => Some(true),
            (Some(true), Some(true)) => Some(false),
            _ => None,
        },
        None => None,
    }
}
impl Chrome {
    pub fn receive(&mut self, id: u64, result: &Result<Value, Error>) -> bool {
        if self.pending.is_none_or(|(expected, _)| expected != id) {
            return false;
        }
        self.pending = None;
        if let Err(Error::Busy(seconds)) = result {
            self.next_wifi = Instant::now() + Duration::from_secs((*seconds).clamp(1, 5));
            return true;
        }
        // Only retain the public boolean. SSIDs and keys do not enter the lock model.
        self.wifi = result
            .as_ref()
            .ok()
            .and_then(wifi_enabled)
            .map(|value| (value, Instant::now()));
        true
    }
    pub fn clock(&mut self, ui: &Screen, locale: Locale) {
        if ui.get_asleep() || !ui.get_locked() || ui.get_demo() {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let key = (now / 60, locale);
        if self.clock_key == Some(key) {
            return;
        }
        self.clock_key = Some(key);
        let timestamp = now as libc::time_t;
        // SAFETY: initialized output storage, valid pointers and libc localtime_r
        // uses the device's configured timezone without global locale changes.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        let valid =
            !unsafe { libc::localtime_r(&timestamp, &mut tm) }.is_null() && tm.tm_year >= 124;
        if valid {
            ui.set_lock_clock(format!("{:02}:{:02}", tm.tm_hour, tm.tm_min).into());
            ui.set_lock_date(
                date_label(
                    tm.tm_mon as usize + 1,
                    tm.tm_mday,
                    tm.tm_wday as usize,
                    locale,
                )
                .into(),
            );
        } else {
            ui.set_lock_clock("—:—".into());
            ui.set_lock_date(crate::i18n::tr("设备时间未就绪").into());
        }
    }
    pub fn tick(&mut self, ui: &Screen, agent: &Agent, snapshot: &Snapshot) {
        let now = Instant::now();
        if ui.get_asleep() || ui.get_pin_visible() || !snapshot.authenticated {
            self.pending = None;
            self.wifi = None;
            self.next_wifi = now;
            self.history.pause(now);
            ui.set_wifi_enabled(false);
            return;
        }
        if self
            .pending
            .is_some_and(|(_, at)| now.duration_since(at) > Duration::from_secs(30))
        {
            self.pending = None;
            self.wifi = None;
        }
        if snapshot.status == crate::agent::Status::Live
            && self.pending.is_none()
            && now >= self.next_wifi
        {
            self.next_wifi = now + Duration::from_secs(30);
            self.sequence += 1;
            self.pending = Some((self.sequence, now));
            if let Err(e) = agent.call(self.sequence, Request::get("/api/wifi/radios")) {
                self.receive(self.sequence, &Err(e));
            }
        }
        ui.set_wifi_enabled(
            snapshot.status == crate::agent::Status::Live
                && self.wifi.is_some_and(|(enabled, at)| {
                    enabled && now.duration_since(at) < Duration::from_secs(65)
                }),
        );
        if !ui.get_locked() {
            self.history.pause(now);
            return;
        }
        let pair = snapshot
            .get("traffic")
            .filter(|_| {
                !snapshot.values.contains_key("operating")
                    || snapshot
                        .get("operating")
                        .is_some_and(|v| v["airplane"]["enabled"] == false)
            })
            .map(|v| [rate(&v["real_rx_speed"]), rate(&v["real_tx_speed"])])
            .unwrap_or([None; 2]);
        if self.history.record(now, pair) {
            let (paths, samples) = self.history.paths();
            ui.set_lock_download_path(paths[0].clone().into());
            ui.set_lock_upload_path(paths[1].clone().into());
            ui.set_lock_samples(samples as i32);
        }
    }
}
fn rate(value: &Value) -> Option<f64> {
    crate::agent::model::number(value)
        .filter(|v| *v >= 0. && *v <= 1e12)
        .map(|v| v / 125_000.)
}
fn date_label(month: usize, day: i32, weekday: usize, locale: Locale) -> String {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || weekday > 6 {
        return "—".into();
    }
    match locale {
        Locale::Simplified => format!(
            "{month} 月 {day} 日 · 星期{}",
            ["日", "一", "二", "三", "四", "五", "六"][weekday]
        ),
        Locale::Traditional => format!(
            "{month} 月 {day} 日 · 星期{}",
            ["日", "一", "二", "三", "四", "五", "六"][weekday]
        ),
        Locale::Japanese => format!(
            "{month}月{day}日 · {}曜日",
            ["日", "月", "火", "水", "木", "金", "土"][weekday]
        ),
        Locale::Korean => format!(
            "{month}월 {day}일 · {}요일",
            ["일", "월", "화", "수", "목", "금", "토"][weekday]
        ),
        Locale::English => format!(
            "{} {day} · {}",
            [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"
            ][month - 1],
            ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][weekday]
        ),
    }
}
#[derive(Default)]
struct TrafficHistory {
    points: VecDeque<(Instant, [Option<f64>; 2])>,
}
impl TrafficHistory {
    fn pause(&mut self, now: Instant) {
        if self
            .points
            .back()
            .is_some_and(|(_, pair)| pair.iter().any(Option::is_some))
        {
            self.record(now, [None; 2]);
        }
    }
    fn record(&mut self, now: Instant, pair: [Option<f64>; 2]) -> bool {
        if self.points.back().is_some_and(|(at, old)| {
            now.saturating_duration_since(*at) < Duration::from_secs(2)
                && old.map(|v| v.is_some()) == pair.map(|v| v.is_some())
        }) {
            return false;
        }
        self.points.push_back((now, pair));
        while self.points.len() > 61
            || self.points.front().is_some_and(|(at, _)| {
                now.saturating_duration_since(*at) > Duration::from_secs(120)
            })
        {
            self.points.pop_front();
        }
        true
    }
    fn paths(&self) -> ([String; 2], usize) {
        let Some((now, _)) = self.points.back() else {
            return (Default::default(), 0);
        };
        let max = self
            .points
            .iter()
            .flat_map(|(_, pair)| pair.iter().flatten())
            .copied()
            .fold(1., f64::max);
        let mut paths: [String; 2] = Default::default();
        for (index, path) in paths.iter_mut().enumerate() {
            let mut previous = None;
            for (at, pair) in &self.points {
                let Some(value) = pair[index] else {
                    previous = None;
                    continue;
                };
                let connected = previous.is_some_and(|last| {
                    at.saturating_duration_since(last) <= Duration::from_secs(8)
                });
                let x = 271. * (1. - now.saturating_duration_since(*at).as_secs_f64() / 120.);
                let y = 51. - value / max * 48.;
                use std::fmt::Write;
                let _ = write!(path, "{} {x:.1} {y:.1} ", if connected { "L" } else { "M" });
                if !connected {
                    path.push_str("l 0.1 0 ");
                }
                previous = Some(*at);
            }
        }
        (
            paths,
            self.points
                .iter()
                .filter(|(_, pair)| pair.iter().any(Option::is_some))
                .count(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wifi_requires_reported_radios_and_explicit_global_state() {
        for (value, expected) in [
            (serde_json::json!({}), None),
            (serde_json::json!({"wifi_onoff": "0"}), Some(false)),
            (
                serde_json::json!({"wifi_onoff": "1", "radio2_disabled":"1", "radio5_disabled":"0"}),
                Some(true),
            ),
            (
                serde_json::json!({"wifi_onoff": true, "radio2_disabled":1, "radio5_disabled":1}),
                Some(false),
            ),
        ] {
            assert_eq!(wifi_enabled(&value), expected);
        }
    }
    #[test]
    fn lock_history_is_bounded_uses_one_scale_and_breaks_across_sleep() {
        let mut h = TrafficHistory::default();
        let start = Instant::now();
        for i in 0..200 {
            h.record(start + Duration::from_secs(i * 2), [Some(100.), Some(50.)]);
        }
        assert_eq!(h.points.len(), 61);
        let (paths, count) = h.paths();
        assert_eq!(count, 61);
        assert!(paths[0].contains("271.0 3.0"));
        assert!(paths[1].contains("271.0 27.0"));
        assert!(!h.record(start + Duration::from_secs(399), [Some(100.), Some(50.)]));
        h.pause(start + Duration::from_secs(399));
        h.record(start + Duration::from_secs(401), [Some(1.), Some(2.)]);
        assert!(h.paths().0[0].matches("M ").count() >= 2);
        h.pause(start + Duration::from_secs(600));
        h.record(start + Duration::from_secs(601), [None, None]);
        assert!(h.paths().0.iter().all(String::is_empty));
        assert_eq!(h.paths().1, 0);
    }
    #[test]
    fn lock_date_follows_selected_language_without_host_locale() {
        assert_eq!(
            date_label(11, 6, 5, Locale::Simplified),
            "11 月 6 日 · 星期五"
        );
        assert_eq!(date_label(11, 6, 5, Locale::English), "Nov 6 · Fri");
        assert_eq!(date_label(11, 6, 5, Locale::Japanese), "11月6日 · 金曜日");
        assert_eq!(date_label(11, 6, 5, Locale::Korean), "11월 6일 · 금요일");
        assert_eq!(date_label(0, 6, 5, Locale::Traditional), "—");
    }
}
