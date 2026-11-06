//! Refresh is a page contract, not a button and timer added to every screen.
use super::{Data, data, flag, selected, text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Policy {
    Entry,
    Stream,
    Poll(u64),
    Task,
    Local,
}

fn policy(id: &str) -> Option<Policy> {
    use Policy::*;
    Some(match id {
        "screen-prefs" | "screen-lock" | "account" | "history" | "tailscale-network" => Local,
        "signal-live" | "carriers" | "processes" | "usb" => Stream,
        "battery-details" | "clients" | "sms-inbox" | "sms-forward-log" => Poll(10),
        "proxy-service" | "proxy-groups" | "tailscale-service" | "phone" => Poll(5),
        "resources" | "wan" | "traffic" | "sim-info" | "wifi-share" | "wifi-home" => Poll(30),
        "operators" | "signal-test" | "speedtest" | "tailscale-version" | "agent-update"
        | "esim-profiles" | "esim-download" | "stk" => Task,
        "wifi2" | "wifi5" | "guest" | "lan" | "mobile-data" | "network-mode" | "apn" | "usage"
        | "oos" | "watchdog" | "nr-bands" | "lte-bands" | "nr-lock" | "lte-lock" | "neighbors"
        | "stc" | "firewall" | "nat" | "dmz" | "upnp" | "ports" | "qos" | "vpn" | "ttl"
        | "domain" | "dns-config" | "doh" | "dns-cache" | "tailscale-config" | "sms-compose"
        | "sms-storage" | "sms-forward" | "sim-verify" | "sim-pin" | "sim-change-pin"
        | "sim-network-unlock" | "esim-receipts" | "ussd" | "lan-test" | "at-terminal"
        | "scheduler" | "live-activity" | "charge" | "power-save" | "fast-boot" | "auto-sleep"
        | "device-info" | "https" | "reboot" | "factory-reset" | "backup" | "wireguard" => Entry,
        _ => return None,
    })
}

pub(super) fn paused(id: &str, readings: &Data) -> bool {
    // Reading a message, composing a reply, or reviewing a selected record
    // must not become a moving target as a list is refreshed behind it.
    (selected(readings).is_some() && matches!(id, "sms-inbox" | "sms-forward-log" | "proxy-groups"))
        || (id.starts_with("esim-") && !super::esim::can_poll(readings))
}

pub fn poll_seconds(id: &str, readings: &Data) -> Option<u64> {
    if paused(id, readings) {
        return None;
    }
    match policy(id)? {
        Policy::Poll(seconds) => Some(seconds),
        Policy::Task => {
            let active = match id {
                "operators" => ["manual_selecting", "scanning", "running", "1"]
                    .contains(&text(&data(readings, "scan")["m_netselect_status"]).as_str()),
                "signal-test" => {
                    flag(&data(readings, "progress")["running"]) == Some(true)
                        && super::status::number(&data(readings, "progress")["progress"])
                            .is_none_or(|p| p < 100.)
                }
                "speedtest" => ["latency", "download", "upload"]
                    .contains(&text(&data(readings, "progress")["phase"]).as_str()),
                "tailscale-version" => data(readings, "status")["updating"] == true,
                "agent-update" => {
                    ["checking", "downloading", "installing", "rolling_back"]
                        .contains(&text(&data(readings, "status")["phase"]).as_str())
                        || data(readings, "status")["trial_in_progress"] == true
                }
                "esim-profiles" => {
                    data(readings, "inventory")["stale"] == true
                        || data(readings, "inventory")["updating"] == true
                        || ["reloading", "card_confirmed", "modem_confirmed"]
                            .contains(&text(&data(readings, "switch")["state"]).as_str())
                }
                "esim-download" => ["running", "confirm", "cancelling"]
                    .contains(&text(&data(readings, "download")["state"]).as_str()),
                "stk" => data(readings, "status")["busy"] == true,
                _ => false,
            };
            active.then_some(3)
        }
        _ => None,
    }
}

/// Firmware may acknowledge a command before reporting the task as running.
/// Observe briefly after an explicit action; never repeat the action itself.
pub(super) fn settle_seconds(id: &str) -> Option<u64> {
    if id == "neighbors" {
        Some(30)
    } else if policy(id) == Some(Policy::Task) {
        Some(15)
    } else {
        None
    }
}

/// Refresh is useful for explicitly inspected snapshots or external changes.
/// Scans, version checks, reconnection and testing retain their named actions.
pub fn manual_refresh(id: &str, readings: &Data) -> bool {
    if selected(readings).is_some() {
        return false;
    }
    matches!(
        id,
        "dns-cache" | "sms-storage" | "esim-profiles" | "esim-receipts"
    )
}

pub(super) fn poll_read(id: &str, key: &str) -> bool {
    match id {
        "sim-info" => key != "imei",
        "speedtest" => key == "progress",
        "esim-profiles" => key != "chip",
        "esim-download" => key == "download",
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn every_page_has_a_reviewed_refresh_policy() {
        let catalog = crate::app::Catalog::load().unwrap();
        for feature in &catalog.features {
            assert!(
                policy(&feature.id).is_some(),
                "missing refresh policy: {}",
                feature.id
            );
        }
        assert_eq!(
            catalog
                .features
                .iter()
                .filter(|f| manual_refresh(&f.id, &Data::new()))
                .count(),
            4
        );
        if let Ok(path) = std::env::var("U60_REFRESH_AUDIT") {
            let lines = catalog
                .features
                .iter()
                .map(|f| {
                    format!(
                        "| `{}` | {} | {:?} | {} |",
                        f.id,
                        f.title,
                        policy(&f.id).unwrap(),
                        if manual_refresh(&f.id, &Data::new()) {
                            "保留"
                        } else {
                            "移除"
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write(path, lines).unwrap();
        }
    }
    #[test]
    fn message_details_and_idle_tasks_do_not_poll() {
        let mut data = Data::new();
        assert_eq!(poll_seconds("sms-inbox", &data), Some(10));
        data.insert("__selection".into(), Ok(json!("reply:12")));
        assert_eq!(poll_seconds("sms-inbox", &data), None);
        assert!(!manual_refresh("sms-inbox", &data));
        data.clear();
        for id in [
            "wifi2",
            "sms-compose",
            "device-info",
            "scheduler",
            "https",
            "signal-test",
            "speedtest",
            "tailscale-version",
        ] {
            assert_eq!(poll_seconds(id, &data), None, "{id}");
        }
        data.insert(
            "progress".into(),
            Ok(json!({"running":true,"phase":"download"})),
        );
        assert_eq!(poll_seconds("signal-test", &data), Some(3));
        assert_eq!(poll_seconds("speedtest", &data), Some(3));
        data.insert(
            "progress".into(),
            Ok(json!({"running":false,"phase":"complete"})),
        );
        assert_eq!(poll_seconds("speedtest", &data), None);
        assert!(!poll_read("speedtest", "servers"));
    }
}
