use super::Snapshot;
use serde_json::Value;

pub fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse().ok())
        .filter(|v| v.is_finite())
}
pub fn text(value: &Value) -> String {
    match value {
        Value::String(s) if !s.is_empty() => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => if *b { "开启" } else { "关闭" }.into(),
        _ => "—".into(),
    }
}
pub fn bytes(value: f64) -> String {
    if value >= 1e9 {
        format!("{:.2} GB", value / 1e9)
    } else if value >= 1e6 {
        format!("{:.1} MB", value / 1e6)
    } else {
        format!("{:.1} KB", value / 1e3)
    }
}
pub struct Dashboard {
    pub battery: String,
    pub network: String,
    pub radio: String,
    pub rsrp: String,
    pub online: bool,
    pub download: String,
    pub upload: String,
    pub total: String,
    pub clients: String,
}
impl Dashboard {
    pub fn read(state: &Snapshot) -> Self {
        let null = Value::Null;
        let signal = state.get("signal").unwrap_or(&null);
        let traffic = state.get("traffic").unwrap_or(&null);
        let battery = state.get("battery").unwrap_or(&null);
        let rsrp = |key: &str| number(&signal[key]).filter(|n| (-160.0..=-20.0).contains(n));
        let nr = rsrp("nr5g_rsrp");
        let lte = rsrp("lte_rsrp");
        let mode = signal["network_type"].as_str().unwrap_or("");
        let network = match mode {
            "SA" | "NR5G-SA" | "5G_SA" => "5G SA",
            "ENDC" | "LTE-NSA" | "NSA" => "5G NSA",
            "LTE" => "4G",
            "NO_SERVICE" => "无服务",
            "LIMITED_SERVICE_LTE" | "LIMITED_SERVICE_SA" => "有限服务",
            "" => "—",
            other => other,
        }
        .to_string();
        let band = if nr.is_some() {
            text(&signal["nr5g_action_band"])
        } else {
            text(&signal["wan_active_band"])
        };
        let radio = if band == "—" {
            network.clone()
        } else {
            format!("{network} · {band}")
        };
        let rate = |key: &str| {
            number(&traffic[key])
                .filter(|v| *v >= 0.)
                .map(|v| format!("{:.1}", v * 8. / 1e6))
                .unwrap_or("—".into())
        };
        Self {
            battery: number(&battery["capacity"])
                .filter(|v| (0.0..=100.0).contains(v))
                .map(|v| format!("{v:.0}%"))
                .unwrap_or("—".into()),
            online: nr.or(lte).is_some() && !mode.contains("NO_SERVICE"),
            rsrp: nr.or(lte).map(|v| format!("{v:.0}")).unwrap_or("—".into()),
            network,
            radio,
            download: rate("real_rx_speed"),
            upload: rate("real_tx_speed"),
            total: number(&traffic["real_rx_bytes"])
                .zip(number(&traffic["real_tx_bytes"]))
                .map(|(rx, tx)| bytes(rx + tx))
                .unwrap_or("—".into()),
            clients: state
                .get("clients")
                .and_then(|v| v["hosts"].as_object())
                .map(|v| v.len().to_string())
                .unwrap_or("—".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Sample, Status};
    use serde_json::json;
    #[test]
    fn live_values_handle_units_unknowns_failed_reads_and_stale_snapshots() {
        let mut state = Snapshot {
            status: Status::Live,
            packet_at: Some(std::time::Instant::now()),
            ..Default::default()
        };
        for (topic, data) in [
            ("battery", json!({"capacity":"0"})),
            (
                "signal",
                json!({"network_type":"LTE-NSA", "nr5g_rsrp":"-87", "nr5g_action_band":"n78"}),
            ),
            (
                "traffic",
                json!({"real_rx_speed":"1250000", "real_tx_speed":0, "real_rx_bytes":1000000, "real_tx_bytes":2000000}),
            ),
        ] {
            state.values.insert(
                topic.into(),
                Sample {
                    data,
                    age_ms: 0,
                    seq: 1,
                    failed: false,
                },
            );
        }
        let model = Dashboard::read(&state);
        assert_eq!(
            (
                model.battery.as_str(),
                model.download.as_str(),
                model.upload.as_str()
            ),
            ("0%", "10.0", "0.0")
        );
        assert_eq!(model.radio, "5G NSA · n78");
        state.values.get_mut("signal").unwrap().data["nr5g_rsrp"] = json!(9999);
        assert_eq!(Dashboard::read(&state).rsrp, "—");
        state.values.get_mut("battery").unwrap().failed = true;
        assert_eq!(Dashboard::read(&state).battery, "—");
        state.values.get_mut("traffic").unwrap().age_ms = 10_000;
        assert_eq!(Dashboard::read(&state).download, "—");
        state.status = Status::Reconnecting;
        assert_eq!(Dashboard::read(&state).network, "—");
    }
}
