use serde::{Deserialize, Serialize};

const CONFIG_PATH: &str = "/data/local/tmp/tailscale_config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsConfig {
    pub enabled: bool,
    /// Set true after a successful enrollment, false on logout. Distinguishes
    /// "never enrolled" from "enrolled but key invalid" — the state file alone
    /// can't (tailscaled creates it before first login).
    #[serde(default)]
    pub enrolled: bool,
    #[serde(default = "default_hostname")]
    pub hostname: String,
    #[serde(default)]
    pub advertise_routes: Vec<String>,
    #[serde(default = "default_true")]
    pub hold_wakelock: bool,
    #[serde(default)]
    pub tailscale_ssh: bool,
    #[serde(default = "default_true")]
    pub accept_routes: bool,
    #[serde(default)]
    pub accept_dns: bool,
    #[serde(default)]
    pub exit_node: String,
    #[serde(default)]
    pub pinned_version: String,
}

fn default_hostname() -> String {
    "u60-pro".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for TsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            enrolled: false,
            hostname: default_hostname(),
            advertise_routes: Vec::new(),
            hold_wakelock: true,
            tailscale_ssh: false,
            accept_routes: true,
            accept_dns: false,
            exit_node: String::new(),
            pinned_version: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TsConfigPatch {
    pub hostname: Option<String>,
    pub advertise_routes: Option<Vec<String>>,
    pub hold_wakelock: Option<bool>,
    pub tailscale_ssh: Option<bool>,
    pub accept_routes: Option<bool>,
    pub accept_dns: Option<bool>,
    pub exit_node: Option<String>,
}

impl TsConfig {
    pub fn apply_patch(&mut self, patch: TsConfigPatch) -> Result<(), String> {
        if let Some(v) = patch.hostname {
            let v = v.trim().to_string();
            if v.is_empty()
                || v.len() > 63
                || !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                return Err("hostname must be 1-63 chars, alphanumeric or '-'".into());
            }
            self.hostname = v;
        }
        if let Some(v) = patch.advertise_routes {
            for route in &v {
                if !valid_cidr(route) {
                    return Err(format!("invalid CIDR route: {route}"));
                }
            }
            self.advertise_routes = v;
        }
        if let Some(v) = patch.hold_wakelock {
            self.hold_wakelock = v;
        }
        if let Some(v) = patch.accept_routes {
            self.accept_routes = v;
        }
        if let Some(v) = patch.accept_dns {
            self.accept_dns = v;
        }
        if let Some(v) = patch.exit_node {
            let v = v.trim();
            if !v.is_empty() && !tailnet_ip(v) {
                return Err("Exit node must be a Tailscale IP address".into());
            }
            self.exit_node = v.into();
        }
        if let Some(v) = patch.tailscale_ssh {
            self.tailscale_ssh = v;
        }
        Ok(())
    }
}

pub fn tailnet_ip(value: &str) -> bool {
    match value.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            let o = ip.octets();
            o[0] == 100 && (64..128).contains(&o[1])
        }
        Ok(std::net::IpAddr::V6(ip)) => ip.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
        _ => false,
    }
}

/// Minimal IPv4 CIDR validation (e.g. "192.168.0.0/24").
fn valid_cidr(s: &str) -> bool {
    let mut parts = s.splitn(2, '/');
    let (addr, prefix) = match (parts.next(), parts.next()) {
        (Some(a), Some(p)) => (a, p),
        _ => return false,
    };
    let prefix_ok = prefix.parse::<u8>().map(|p| p <= 32).unwrap_or(false);
    let octets: Vec<&str> = addr.split('.').collect();
    let addr_ok = octets.len() == 4 && octets.iter().all(|o| o.parse::<u8>().is_ok());
    prefix_ok && addr_ok
}

pub fn load() -> TsConfig {
    std::fs::read_to_string(CONFIG_PATH)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(config: &TsConfig) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let json = serde_json::to_string_pretty(config).map_err(|e| format!("serialize: {e}"))?;
    let temporary = format!("{CONFIG_PATH}.{}.new", std::process::id());
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| format!("stage config: {e}"))?;
    let result = (|| {
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary, CONFIG_PATH)
    })()
    .map_err(|e| format!("save config: {e}"));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
