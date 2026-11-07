//! Clearly labeled offline preview samples; never an agent request body.
use super::Data;
use serde_json::{Value, json};
pub fn timeline(id: &str) -> Option<Vec<Data>> {
    if matches!(id, "quick-controls" | "mobile-modes" | "power-save") {
        return Some(vec![[ ("operating".into(), Ok(json!({
            "airplane":{"enabled":false}, "bandwidth":{"supported":true,"mode":"normal"}, "rail":{"active":false},
            "performance":{"supported":true,"available":["balanced","performance","saving"],"requested":null,"effective":"balanced","applied":true,"thermal_limited":false}
        }))) ].into()]);
    }
    if !matches!(
        id,
        "resources" | "traffic" | "signal-live" | "battery-details"
    ) {
        return None;
    }
    Some((0..=60).map(|i| {
        let wave=(i as f64 / 6.).sin();
        let pairs:Vec<(&str,Value)>=match id {
            "resources"=>vec![
                ("cpu",json!({"overall":28.+wave*12.,"cores":[18.,32.,49.,16.]})),
                ("memory",json!({"total_kb":1048576,"used_kb":450000.+wave*8000.,"available_kb":598576.-wave*8000.})),
                ("thermal",json!({"sensors":[{"name":"CPU","temperature_c":41.+wave*2.},{"name":"Modem","temperature_c":38.}]})),
                ("system",json!({"uptime":7200})),
            ],
            "traffic"=>vec![
                ("traffic",json!({"real_rx_speed":(45.+wave*25.)*125000.,"real_tx_speed":(8.+wave*3.)*125000.,"real_rx_bytes":2_300_000_000u64,"real_tx_bytes":180_000_000})),
                ("interfaces",json!([{"name":"rmnet0","rx_bytes":2_300_000_000u64,"tx_bytes":180_000_000},{"name":"bridge0","rx_bytes":170_000_000,"tx_bytes":2_200_000_000u64}])),
            ],
            "signal-live"=>vec![("signal",json!({"network_type":"SA","network_provider":"China Mobile","nr5g_rsrp":-86.+wave*5.,"lte_rsrp":-96.+wave*3.,"nr5g_rsrq":-9,"nr5g_snr":21,"nr5g_rssi":-62,"lte_rsrq":-12,"lte_snr":13,"lte_rssi":-70,"nr5g_action_band":"n78","wan_active_band":"B3","nr5g_pci":123,"lte_pci":321,"nr5g_action_channel":627264,"wan_active_channel":1650}))],
            _=>vec![
                ("battery",json!({"capacity":80.,"status":"Charging","temperature":310.+wave*5.,"current_ua":400_000.+wave*100_000.,"voltage_uv":4_100_000})),
                ("charger",json!({"charger_connect":1,"charge_status":1,"direct_power_supply_mode":"disable"})),
            ],
        };
        pairs.into_iter().map(|(k,v)|(k.into(),Ok(v))).collect()
    }).collect())
}
