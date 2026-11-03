//! Management endpoints shared by startup, probes, and remote forwarding.
use std::{collections::BTreeSet, fs, net::SocketAddr, path::Path};

pub const HTTP_PORT: u16 = 20600;
pub const REMOTE_PORT: u16 = 20601;
pub const LEGACY_HTTP_PORT: u16 = 9090;
pub const LEGACY_REMOTE_PORT: u16 = 9091;
const DEFAULT_BIND: &str = "0.0.0.0:20600";

fn effective_bind(value: Option<&str>, paired: bool) -> String {
    let value = value.unwrap_or(DEFAULT_BIND);
    if paired {
        match value {
            "0.0.0.0:9090" => return DEFAULT_BIND.into(),
            "127.0.0.1:9090" => return "127.0.0.1:20600".into(),
            _ => {}
        }
    }
    value.into()
}

pub fn bind() -> String {
    effective_bind(
        std::env::var("ZTE_AGENT_BIND").ok().as_deref(),
        crate::deployment::paired(),
    )
}

pub fn local_address() -> Result<SocketAddr, String> {
    let mut address: SocketAddr = bind().parse().map_err(|_| "Invalid agent bind address")?;
    if address.ip().is_unspecified() {
        address.set_ip(if address.is_ipv6() {
            "::1".parse().unwrap()
        } else {
            "127.0.0.1".parse().unwrap()
        });
    }
    Ok(address)
}

fn socket_ports(table: &str, inodes: &BTreeSet<String>) -> BTreeSet<u16> {
    table
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.get(3) != Some(&"0A") || !inodes.contains(*fields.get(9)?) {
                return None;
            }
            let (_, port) = fields.get(1)?.rsplit_once(':')?;
            u16::from_str_radix(port, 16).ok()
        })
        .collect()
}

// Probe only a listening socket belonging to the already-identified agent.
// During upgrades its executable may use either the old or the new default.
pub fn process_http_port(process: &Path) -> Result<u16, String> {
    let inodes = fs::read_dir(process.join("fd"))
        .map_err(|_| "Cannot inspect agent sockets")?
        .flatten()
        .filter_map(|entry| {
            let target = fs::read_link(entry.path()).ok()?;
            target
                .to_str()?
                .strip_prefix("socket:[")?
                .strip_suffix(']')
                .map(str::to_owned)
        })
        .collect();
    let mut ports = BTreeSet::new();
    for table in ["tcp", "tcp6"] {
        if let Ok(text) = fs::read_to_string(process.join("net").join(table)) {
            ports.extend(socket_ports(&text, &inodes));
        }
    }
    ports.retain(|port| [HTTP_PORT, LEGACY_HTTP_PORT].contains(port));
    if ports.len() != 1 {
        return Err("Expected one agent management listener on port 20600 or legacy 9090".into());
    }
    Ok(*ports.first().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_environment_is_migrated_only_for_the_paired_service() {
        assert_eq!(effective_bind(None, false), "0.0.0.0:20600");
        assert_eq!(effective_bind(Some("0.0.0.0:9090"), true), "0.0.0.0:20600");
        assert_eq!(
            effective_bind(Some("127.0.0.1:9090"), true),
            "127.0.0.1:20600"
        );
        assert_eq!(effective_bind(Some("0.0.0.0:9090"), false), "0.0.0.0:9090");
        assert_eq!(
            effective_bind(Some("127.0.0.1:23000"), false),
            "127.0.0.1:23000"
        );
    }
    #[test]
    fn discovery_requires_an_owned_listening_socket() {
        let sockets = BTreeSet::from(["101".into(), "102".into()]);
        let table = "0: 00000000:5078 00000000:0000 0A 0 0 0 0 0 101\n1: 00000000:2382 00000000:0000 0A 0 0 0 0 0 999\n2: 00000000:2382 00000000:0000 01 0 0 0 0 0 102\n";
        assert_eq!(socket_ports(table, &sockets), BTreeSet::from([HTTP_PORT]));
    }
}
