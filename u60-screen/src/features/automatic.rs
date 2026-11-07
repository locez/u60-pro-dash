//! Ordinary settings commit when a choice or text edit completes. Commands
//! (sending, deleting, installing, PIN verification) remain explicit actions.
use super::*;
pub fn action(id: &str) -> Option<&'static str> {
    Some(match id {
        "quick-controls" | "mobile-modes" | "wifi2" | "wifi5" | "guest" | "charge"
        | "power-save" | "fast-boot" | "auto-sleep" | "ttl" | "oos" | "watchdog"
        | "tailscale-config" | "mobile-data" | "network-mode" | "usage" | "lan" | "firewall"
        | "nat" | "dmz" | "upnp" | "qos" | "vpn" | "dns-config" | "doh" | "usb" => "save",
        "proxy-service" => "settings",
        "proxy-groups" => "select",
        _ => return None,
    })
}
pub fn interruption(id: &str, page: &Page, readings: &Data) -> bool {
    match id {
        "quick-controls" | "mobile-modes" => page
            .fields
            .iter()
            .any(|f| f.key == "airplane" && f.dirty() && f.value == "true"),
        "proxy-service" => {
            data(readings, "status")["running"] == true
                && page.field("capture").is_ok_and(Field::dirty)
        }
        "charge" | "power-save" | "fast-boot" | "oos" | "watchdog" | "usage" | "proxy-groups" => {
            false
        }
        "tailscale-config" => page
            .fields
            .iter()
            .any(|f| f.dirty() && f.key != "hold_wakelock"),
        "usb" => page.field("mode").is_ok_and(Field::dirty),
        _ => true,
    }
}
