use serde_json::{json, Value};

use crate::cache;
use crate::handlers::AppState;
use crate::ubus;

pub fn modem_data_get(state: &AppState) -> (u16, Value) {
    match wwan(state) {
        Ok(data) => (200, json!({"ok": true, "data": data})),
        Err(e) => (503, json!({"ok": false, "error": e})),
    }
}

/// `zwrt_data get_wwaniface` for the data call, through the shared cache.
pub(crate) fn wwan(state: &AppState) -> Result<Value, String> {
    let fetch = || ubus::call("zwrt_data", "get_wwaniface", Some(r#"{"cid":1}"#));
    state.cache.get_or_fetch(cache::MODEM_DATA, cache::MODEM_DATA_TTL, fetch)
}

/// The data daemon applies mode changes asynchronously, so after a local
/// write the modem/data entry is dropped and reads stay near-fresh for the
/// settle window (the apps run 2 s confirmation polls against it).
fn settle_modem_data(state: &AppState) {
    state
        .cache
        .invalidate_with_settle(cache::MODEM_DATA, cache::SETTLE_WINDOW);
}

/// A user-initiated modem change: the no-service policy and the connectivity
/// watchdog hold off for 2 min.
pub(crate) fn note_user_modem_change(state: &AppState) -> std::sync::MutexGuard<'_, ()> {
    let guard = state.esim.note_user_modem_change();
    let now = std::time::Instant::now();
    state.oos_policy.set_user_activity(now);
    state.conn_watchdog.set_user_activity(now);
    guard
}

/// POST /api/modem/online from the app: AT+CFUN=1 plus the user-activity
/// bookkeeping (the policies' own effects send AT+CFUN=1 directly and never
/// come through here).
pub fn modem_online_user(state: &AppState) -> (u16, Value) {
    let _command = note_user_modem_change(state);
    state.oos_policy.user_mode(true);
    crate::handlers::modem_online(state)
}

pub fn modem_data_set(state: &AppState, body: &[u8]) -> (u16, Value) {
    let parsed: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return (400, json!({"ok": false, "error": "invalid JSON"})),
    };
    let _command = note_user_modem_change(state);
    let result = ubus::call("zwrt_data", "set_wwaniface", Some(&parsed.to_string()));
    settle_modem_data(state);
    match result {
        Ok(data) => (200, json!({"ok": true, "data": data})),
        Err(e) => (503, json!({"ok": false, "error": e})),
    }
}

pub(crate) fn airplane_enabled(mode: &str) -> Option<bool> {
    match mode {
        "ONLINE" => Some(false),
        "LPM" | "OFFLINE" => Some(true),
        _ => None,
    }
}

pub fn modem_airplane(state: &AppState, body: &[u8]) -> (u16, Value) {
    let parsed: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return (400, json!({"ok":false,"error":"invalid JSON"})),
    };
    let target = match parsed["operate_mode"].as_str() {
        Some(mode @ ("ONLINE" | "LPM" | "OFFLINE")) => mode,
        _ => return (400, json!({"ok":false,"error":"operate_mode must be ONLINE, LPM or OFFLINE"})),
    };
    let _command = note_user_modem_change(state);
    let response = if target == "ONLINE" {
        // Firmware's ubus ONLINE does not recover from LPM. Preserve the known AT path.
        state.oos_policy.user_mode(true);
        crate::handlers::modem_online(state)
    } else {
        let result = ubus::call_bounded("zte_nwinfo_api", "nwinfo_set_mode", Some(&json!({"operate_mode":target}).to_string()), 3);
        settle_modem_data(state);
        match result {
            Ok(_) => {
                state.oos_policy.user_mode(false);
                (200, json!({"ok":true}))
            }
            Err(error) => (503, json!({"ok":false,"error":error})),
        }
    };
    // Mark the request's outcome on both old and new live projections.
    crate::events::touch(crate::events::Topic::Modem);
    crate::events::touch(crate::events::Topic::Operating);
    if response.0 >= 400 { return response; }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        if let Ok(mode) = crate::handlers::read_uci_option("zte_nwinfo", "sys_info", "operate_mode") {
            // B31 reports OFFLINE after a successful LPM request (CFUN=0).
            // Confirm the radio state and return the actual firmware value.
            if airplane_enabled(&mode) == airplane_enabled(target) {
                crate::events::touch(crate::events::Topic::Modem);
                crate::events::touch(crate::events::Topic::Operating);
                // ONLINE confirms the radio, not registration or remote reachability.
                return (200, json!({"ok":true,"data":{"operate_mode":mode}}));
            }
        }
        if std::time::Instant::now() >= deadline {
            return (503, json!({"ok":false,"error":"Radio mode change could not be verified; check current mobile network status"}));
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

#[cfg(test)]
mod radio_mode_tests {
    use super::airplane_enabled;

    #[test]
    fn firmware_offline_is_a_valid_lpm_readback_but_unknown_is_not() {
        assert_eq!(airplane_enabled("LPM"), Some(true));
        assert_eq!(airplane_enabled("OFFLINE"), airplane_enabled("LPM"));
        assert_eq!(airplane_enabled("ONLINE"), Some(false));
        for mode in ["", "RESET", "LOW_POWER", "unknown"] {
            assert_eq!(airplane_enabled(mode), None);
        }
    }
}

pub fn modem_network_mode_set(state: &AppState, body: &[u8]) -> (u16, Value) {
    let parsed: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return (400, json!({"ok": false, "error": "invalid JSON"})),
    };
    let _command = note_user_modem_change(state);
    let result = ubus::call(
        "zte_nwinfo_api",
        "nwinfo_set_netselect",
        Some(&parsed.to_string()),
    );
    settle_modem_data(state);
    match result {
        Ok(data) => {
            match crate::dns_control::network_preference_changed(state) {
                Ok(()) => eprintln!("[dns] network preference changed: caches and sessions reset"),
                Err(error) => {
                    eprintln!("[dns] network preference changed: immediate reset failed: {error}")
                }
            }
            (200, json!({"ok": true, "data": data}))
        }
        Err(e) => (503, json!({"ok": false, "error": e})),
    }
}

pub fn modem_scan(state: &AppState) -> (u16, Value) {
    let _command = note_user_modem_change(state);
    match ubus::call("zte_nwinfo_api", "nwinfo_manual_scan", Some("{}")) {
        Ok(data) => (200, json!({"ok": true, "data": data})),
        Err(e) => (503, json!({"ok": false, "error": e})),
    }
}

pub fn modem_scan_status(_state: &AppState) -> (u16, Value) {
    match ubus::call("zte_nwinfo_api", "nwinfo_m_netselect_status", Some("{}")) {
        Ok(data) => (200, json!({"ok": true, "data": data})),
        Err(e) => (503, json!({"ok": false, "error": e})),
    }
}

pub fn modem_scan_results(_state: &AppState) -> (u16, Value) {
    match ubus::call("zte_nwinfo_api", "nwinfo_m_netselect_contents", Some("{}")) {
        Ok(data) => (200, json!({"ok": true, "data": data})),
        Err(e) => (503, json!({"ok": false, "error": e})),
    }
}

pub fn modem_register(state: &AppState, body: &[u8]) -> (u16, Value) {
    let parsed: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return (400, json!({"ok": false, "error": "invalid JSON"})),
    };
    let _command = note_user_modem_change(state);
    match ubus::call("zte_nwinfo_api", "nwinfo_manual_register", Some(&parsed.to_string())) {
        Ok(data) => (200, json!({"ok": true, "data": data})),
        Err(e) => (503, json!({"ok": false, "error": e})),
    }
}

pub fn modem_register_result(_state: &AppState) -> (u16, Value) {
    match ubus::call("zte_nwinfo_api", "nwinfo_m_netselect_result", Some("{}")) {
        Ok(data) => (200, json!({"ok": true, "data": data})),
        Err(e) => (503, json!({"ok": false, "error": e})),
    }
}

