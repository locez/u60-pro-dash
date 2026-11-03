//! Management endpoints shared by startup, probes, and remote forwarding.
use std::{collections::BTreeSet, fs, net::SocketAddr, path::Path};

pub const HTTP_PORT: u16 = 2026;
pub const REMOTE_PORT: u16 = 2027;
pub const LEGACY_HTTP_PORT: u16 = 9090;
pub const LEGACY_REMOTE_PORT: u16 = 9091;
pub const PORT_LAYOUTS: [(u16, u16); 3] = [
    (LEGACY_HTTP_PORT, LEGACY_REMOTE_PORT),
    (20600, 20601),
    (HTTP_PORT, REMOTE_PORT),
];
const DEFAULT_BIND: &str = "0.0.0.0:2026";

fn effective_bind(value: Option<&str>, paired: bool) -> String {
    let value = value.unwrap_or(DEFAULT_BIND);
    if let Some(mut address) = value.parse::<SocketAddr>().ok().filter(|address| {
        paired
            && (address.ip().is_unspecified() || address.ip().is_loopback())
            && PORT_LAYOUTS.iter().any(|(port, _)| *port == address.port())
    }) {
        address.set_port(HTTP_PORT);
        return address.to_string();
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
    ports.retain(|port| PORT_LAYOUTS.iter().any(|(http, _)| port == http));
    if ports.len() != 1 {
        return Err("Expected one agent management listener on port 2026, 20600 or 9090".into());
    }
    Ok(*ports.first().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_environment_is_migrated_only_for_the_paired_service() {
        assert_eq!(effective_bind(None, false), "0.0.0.0:2026");
        assert_eq!(effective_bind(Some("0.0.0.0:9090"), true), "0.0.0.0:2026");
        assert_eq!(
            effective_bind(Some("127.0.0.1:9090"), true),
            "127.0.0.1:2026"
        );
        for legacy in [9090, 20600] {
            assert_eq!(
                effective_bind(Some(&format!("0.0.0.0:{legacy}")), true),
                "0.0.0.0:2026"
            );
            assert_eq!(
                effective_bind(Some(&format!("127.0.0.1:{legacy}")), true),
                "127.0.0.1:2026"
            );
            assert_eq!(
                effective_bind(Some(&format!("0.0.0.0:{legacy}")), false),
                format!("0.0.0.0:{legacy}")
            );
        }
        assert_eq!(
            effective_bind(Some("127.0.0.1:23000"), false),
            "127.0.0.1:23000"
        );
    }
    #[test]
    fn discovers_each_supported_generation_before_exporting_or_restoring() {
        let root = std::env::temp_dir().join(format!("u60-port-discovery-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("fd")).unwrap();
        fs::create_dir_all(root.join("net")).unwrap();
        std::os::unix::fs::symlink("socket:[101]", root.join("fd/8")).unwrap();
        for (port, _) in PORT_LAYOUTS {
            fs::write(
                root.join("net/tcp"),
                format!("0: 00000000:{port:04X} 00000000:0000 0A 0 0 0 0 0 101\n"),
            )
            .unwrap();
            assert_eq!(process_http_port(&root).unwrap(), port);
        }
        fs::write(
            root.join("net/tcp"),
            "0: 00000000:07EA 00000000:0000 0A 0 0 0 0 0 999\n",
        )
        .unwrap();
        assert!(process_http_port(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn discovery_requires_an_owned_listening_socket() {
        let sockets = BTreeSet::from(["101".into(), "102".into()]);
        let table = "0: 00000000:07EA 00000000:0000 0A 0 0 0 0 0 101\n1: 00000000:2382 00000000:0000 0A 0 0 0 0 0 999\n2: 00000000:2382 00000000:0000 01 0 0 0 0 0 102\n";
        assert_eq!(socket_ports(table, &sockets), BTreeSet::from([HTTP_PORT]));
    }
}
