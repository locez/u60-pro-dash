//! Bounded, in-memory samples from existing reads; no extra router requests.
use super::{Data, data, status::number};
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

const WINDOW: Duration = Duration::from_secs(120);
const MAX_POINTS: usize = 61;

#[derive(Clone, PartialEq, Eq)]
pub struct Chart {
    pub id: &'static str,
    pub source: &'static str,
    pub label: String,
    pub unit: &'static str,
    pub value: String,
    pub raw: Option<i64>,
    pub range: (i64, i64),
    pub path: String,
    pub upper: String,
    pub lower: String,
    pub samples: usize,
}
impl Chart {
    fn new(
        id: &'static str,
        source: &'static str,
        label: &str,
        unit: &'static str,
        value: Option<f64>,
        range: (i64, i64),
    ) -> Self {
        let value = value.filter(|v| v.is_finite() && v.abs() < 1e9);
        Self {
            id,
            source,
            label: label.into(),
            unit,
            value: value
                .map(|v| format!("{v:.1}"))
                .unwrap_or_else(|| "—".into()),
            raw: value.map(|v| (v * 1000.).round() as i64),
            range,
            path: String::new(),
            upper: String::new(),
            lower: String::new(),
            samples: 0,
        }
    }
}
pub fn charts(id: &str, readings: &Data) -> Vec<Chart> {
    let n = |key: &str, field: &str| number(&data(readings, key)[field]);
    match id {
        "resources" => {
            let memory = n("memory", "used_kb")
                .zip(n("memory", "total_kb"))
                .filter(|(used, total)| *total > 0. && *used >= 0. && used <= total)
                .map(|(used, total)| used / total * 100.);
            let thermal = data(readings, "thermal")["sensors"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| number(&s["temperature_c"]))
                .filter(|v| (-40.0..=150.0).contains(v))
                .reduce(f64::max);
            vec![
                Chart::new(
                    "cpu",
                    "cpu",
                    crate::i18n::tr("CPU 使用率"),
                    "%",
                    n("cpu", "overall").filter(|v| (0.0..=100.0).contains(v)),
                    (0, 100_000),
                ),
                Chart::new(
                    "memory",
                    "memory",
                    crate::i18n::tr("内存使用率"),
                    "%",
                    memory,
                    (0, 100_000),
                ),
                Chart::new(
                    "thermal",
                    "thermal",
                    crate::i18n::tr("最高温度"),
                    "°C",
                    thermal,
                    (0, 100_000),
                ),
            ]
        }
        "traffic" => vec![
            Chart::new(
                "download",
                "traffic",
                crate::i18n::tr("蜂窝下行速率"),
                "Mbps",
                n("traffic", "real_rx_speed")
                    .filter(|v| *v >= 0.)
                    .map(|v| v / 125_000.),
                (0, 1000),
            ),
            Chart::new(
                "upload",
                "traffic",
                crate::i18n::tr("蜂窝上行速率"),
                "Mbps",
                n("traffic", "real_tx_speed")
                    .filter(|v| *v >= 0.)
                    .map(|v| v / 125_000.),
                (0, 1000),
            ),
        ],
        "signal-live" => [
            ("nr", "nr5g_rsrp", "5G NR · RSRP"),
            ("lte", "lte_rsrp", "4G LTE · RSRP"),
        ]
        .into_iter()
        .map(|(id, key, label)| {
            Chart::new(
                id,
                "signal",
                label,
                "dBm",
                super::status::signal_metric(data(readings, "signal"), key, -160., -20.),
                (-140_000, -40_000),
            )
        })
        .collect(),
        "battery-details" => vec![
            Chart::new(
                "capacity",
                "battery",
                crate::i18n::tr("剩余电量"),
                "%",
                n("battery", "capacity").filter(|v| (0.0..=100.0).contains(v)),
                (0, 100_000),
            ),
            Chart::new(
                "battery-temp",
                "battery",
                crate::i18n::tr("电池温度"),
                "°C",
                n("battery", "temperature")
                    .map(|v| v / 10.)
                    .filter(|v| (-40.0..=100.0).contains(v)),
                (0, 60_000),
            ),
            Chart::new(
                "battery-current",
                "battery",
                crate::i18n::tr("电池电流"),
                "mA",
                n("battery", "current_ua").map(|v| v / 1000.),
                (-1000, 1000),
            ),
        ],
        _ => vec![],
    }
}
#[derive(Default)]
pub struct Telemetry {
    series: BTreeMap<&'static str, VecDeque<(Instant, Option<i64>)>>,
    recorded_at: Option<Instant>,
}
impl Telemetry {
    pub fn record(&mut self, charts: &[Chart], sources: &[String], now: Instant) -> bool {
        let mut changed = false;
        for chart in charts {
            if !sources.iter().any(|s| s == chart.source) {
                continue;
            }
            let series = self.series.entry(chart.id).or_default();
            if series.back().is_some_and(|(at, v)| {
                now.saturating_duration_since(*at) < Duration::from_secs(2)
                    && v.is_some() == chart.raw.is_some()
            }) {
                continue;
            }
            series.push_back((now, chart.raw));
            while series.len() > MAX_POINTS
                || series
                    .front()
                    .is_some_and(|(at, _)| now.saturating_duration_since(*at) > WINDOW)
            {
                series.pop_front();
            }
            changed = true;
        }
        if changed {
            self.recorded_at = Some(now);
        }
        changed
    }
    pub fn interrupt(&mut self) {
        for series in self.series.values_mut() {
            if series.back().is_some_and(|(_, v)| v.is_some()) {
                series.push_back((Instant::now(), None));
                while series.len() > MAX_POINTS {
                    series.pop_front();
                }
            }
        }
    }
    pub fn decorate(&self, charts: &mut [Chart], now: Instant) {
        // The graph advances on samples, not UI ticks. A value-only update
        // between samples must not move every other chart by a fraction of a
        // pixel and damage the whole page.
        let now = self.recorded_at.unwrap_or(now);
        for chart in charts {
            chart.path.clear();
            let mut min = chart.range.0;
            let mut max = chart.range.1;
            let points: Vec<_> = self
                .series
                .get(chart.id)
                .into_iter()
                .flatten()
                .filter(|(at, _)| now.saturating_duration_since(*at) <= WINDOW)
                .collect();
            for (_, v) in &points {
                if let Some(v) = v {
                    min = min.min(*v);
                    max = max.max(*v);
                }
            }
            chart.lower = axis(min);
            chart.upper = axis(max);
            chart.samples = points.iter().filter(|(_, v)| v.is_some()).count();
            let mut last = None;
            for (at, value) in points {
                let Some(value) = value else {
                    last = None;
                    continue;
                };
                let x = 260. * (1. - now.saturating_duration_since(*at).as_secs_f64() / 120.);
                let y = 58. - (*value - min) as f64 / (max - min).max(1) as f64 * 56.;
                // A missing or expired sample leaves a real gap. HTTP memory
                // reads are 30 s apart; other streams normally sample in 1–3 s.
                let gap = if chart.source == "memory" {
                    65
                } else if chart.source == "battery" {
                    25
                } else {
                    8
                };
                let connected = last
                    .is_some_and(|t| at.saturating_duration_since(t) <= Duration::from_secs(gap));
                use std::fmt::Write;
                let _ = write!(
                    chart.path,
                    "{} {x:.1} {y:.1} ",
                    if connected { "L" } else { "M" }
                );
                // Even a single sample is visible, without inventing history.
                if !connected {
                    let _ = write!(chart.path, "l 0.1 0 ");
                }
                last = Some(*at);
            }
        }
    }
}
fn axis(value: i64) -> String {
    let v = value as f64 / 1000.;
    if v.abs() >= 10. || v.fract() == 0. {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn history_is_bounded_and_never_samples_unread_memory_or_missing_values_as_zero() {
        let readings = Data::from([
            ("cpu".into(), Ok(json!({"overall":25}))),
            ("memory".into(), Ok(json!({"used_kb":50,"total_kb":100}))),
        ]);
        let mut chart = charts("resources", &readings);
        let mut history = Telemetry::default();
        let start = Instant::now();
        for i in 0..200 {
            history.record(&chart, &["cpu".into()], start + Duration::from_secs(i * 2));
        }
        assert_eq!(history.series["cpu"].len(), 61);
        assert!(!history.series.contains_key("memory"));
        history.decorate(&mut chart, start + Duration::from_secs(398));
        assert_eq!(chart[0].upper, "100");
        assert_eq!(chart[0].lower, "0");
        assert_eq!(chart[1].samples, 0);
        assert_eq!(charts("traffic", &Data::new())[0].raw, None);
        assert_eq!(
            charts(
                "resources",
                &Data::from([("memory".into(), Ok(json!({"used_kb":1,"total_kb":0})))])
            )[1]
            .raw,
            None
        );
    }
    #[test]
    fn charts_break_lines_at_missing_samples_and_keep_signed_axes() {
        let mut history = Telemetry::default();
        let now = Instant::now();
        let mut chart = vec![Chart::new(
            "test",
            "battery",
            "test",
            "mA",
            Some(-10.),
            (-1000, 1000),
        )];
        for (offset, value) in [(0, Some(-10_000)), (2, None), (4, Some(20_000))] {
            chart[0].raw = value;
            history.record(
                &chart,
                &["battery".into()],
                now + Duration::from_secs(offset),
            );
        }
        history.decorate(&mut chart, now + Duration::from_secs(4));
        assert_eq!(chart[0].lower, "-10");
        assert_eq!(chart[0].upper, "20");
        assert_eq!(chart[0].path.matches('M').count(), 2);
        assert!(!chart[0].path.contains('L'));
    }
    #[test]
    fn reading_changes_between_samples_do_not_move_or_append_to_the_plot() {
        let mut history = Telemetry::default();
        let now = Instant::now();
        let mut chart = vec![Chart::new(
            "cpu",
            "cpu",
            "CPU",
            "%",
            Some(25.),
            (0, 100_000),
        )];
        assert!(history.record(&chart, &["cpu".into()], now));
        history.decorate(&mut chart, now);
        let first_path = chart[0].path.clone();
        chart[0].raw = Some(30_000);
        assert!(!history.record(&chart, &["cpu".into()], now + Duration::from_millis(500)));
        history.decorate(&mut chart, now + Duration::from_millis(500));
        assert_eq!(chart[0].path, first_path);
        assert_eq!(chart[0].samples, 1);
        assert!(history.record(&chart, &["cpu".into()], now + Duration::from_secs(2)));
        history.decorate(&mut chart, now + Duration::from_secs(2));
        assert!(chart[0].path.contains('L'));
        assert_eq!(chart[0].samples, 2);
    }
    #[test]
    fn pausing_and_expired_observations_break_the_line() {
        let now = Instant::now();
        let mut history = Telemetry::default();
        let mut chart = vec![Chart::new(
            "cpu",
            "cpu",
            "CPU",
            "%",
            Some(25.),
            (0, 100_000),
        )];
        history.record(&chart, &["cpu".into()], now - Duration::from_secs(2));
        history.interrupt();
        history.record(&chart, &["cpu".into()], now + Duration::from_secs(2));
        history.decorate(&mut chart, now + Duration::from_secs(2));
        assert_eq!(chart[0].path.matches('M').count(), 2);
        assert!(!chart[0].path.contains('L'));
        history.record(&chart, &["cpu".into()], now + Duration::from_secs(130));
        history.decorate(&mut chart, now + Duration::from_secs(130));
        assert_eq!(chart[0].samples, 1);
        assert!(!chart[0].path.contains('L'));
    }
}
