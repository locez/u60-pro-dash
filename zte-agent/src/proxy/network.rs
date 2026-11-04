//! Only hotspot ingress uses the owned routing table. Existing lower-priority
//! numbers (including the overlay network's table 52) are evaluated first.
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TUN_DEVICE: &str = "/sys/class/net/u60proxy0";

fn tunnel_ready(device: &std::path::Path) -> Result<bool, String> {
    let flags = |name| -> Result<Option<u32>, String> {
        match std::fs::read_to_string(device.join(name)) {
            Ok(value) => u32::from_str_radix(value.trim().trim_start_matches("0x"), 16)
                .map(Some)
                .map_err(|_| "Cannot read proxy tunnel flags".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err("Cannot inspect proxy tunnel".into()),
        }
    };
    // TUN appears asynchronously after the controller accepts requests. A
    // present but still-down interface is not ready to carry hotspot traffic.
    Ok(
        flags("tun_flags")?.is_some_and(|flags| flags & libc::IFF_TUN as u32 != 0)
            && flags("flags")?.is_some_and(|flags| flags & libc::IFF_UP as u32 != 0),
    )
}

fn wait_tunnel_at(
    device: &std::path::Path,
    timeout: Duration,
    mut alive: impl FnMut() -> bool,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        if !alive() {
            return Err("Proxy engine stopped before its tunnel became ready".into());
        }
        if tunnel_ready(device)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("Proxy tunnel u60proxy0 did not become ready within 20 seconds; inspect the service logs".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn wait_tunnel(alive: impl FnMut() -> bool) -> Result<(), String> {
    wait_tunnel_at(
        std::path::Path::new(TUN_DEVICE),
        Duration::from_secs(20),
        alive,
    )
}

// The service lease is inherited by both agent and supervisor. A separate
// independently opened flock serializes their network changes with each other.
pub fn locked<T>(
    root: &std::path::Path,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    use std::os::unix::{fs::OpenOptionsExt, io::AsRawFd};
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(root.join("network.lock"))
        .map_err(|_| "Cannot lock proxy network state")?;
    loop {
        // SAFETY: the descriptor remains live until action returns.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } == 0 {
            break;
        }
        if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return Err("Cannot lock proxy network state".into());
        }
    }
    action()
}

const TABLE: &str = "20260";
const IFACES: [&str; 3] = ["br-lan", "wlan0", "wlan2"];
const PRIVATE4: [&str; 8] = [
    "0.0.0.0/8",
    "10.0.0.0/8",
    "100.64.0.0/10",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "224.0.0.0/3",
];
const PRIVATE6: [&str; 4] = ["::1/128", "fc00::/7", "fe80::/10", "ff00::/8"];

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| format!("Cannot start network utility {program}"))?;
    let stderr = super::validation::capture(child.stderr.take().unwrap());
    let end = Instant::now() + Duration::from_secs(4);
    let result = loop {
        match child.try_wait() {
            Ok(Some(s)) => {
                break if s.success() {
                    Ok(())
                } else {
                    Err("command failed")
                };
            }
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("command timed out");
            }
        }
    };
    result.map_err(|reason| {
        let bytes = stderr
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or_default();
        let detail: String = String::from_utf8_lossy(&bytes)
            .chars()
            .filter(|c| !c.is_control())
            .take(800)
            .collect();
        format!(
            "{program} {}: {}",
            args.join(" "),
            if detail.is_empty() { reason } else { &detail }
        )
    })
}

fn parse_local_addresses(bytes: &[u8], family: &str) -> Result<Vec<String>, String> {
    let interfaces: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| "Cannot parse local interface addresses")?;
    let mut addresses = std::collections::BTreeSet::new();
    for interface in interfaces
        .as_array()
        .ok_or("Invalid local interface addresses")?
    {
        for addr in interface["addr_info"].as_array().into_iter().flatten() {
            let Some(address) = addr["local"]
                .as_str()
                .and_then(|s| s.parse::<std::net::IpAddr>().ok())
            else {
                continue;
            };
            if address.is_ipv4() == (family == "-4")
                && !address.is_loopback()
                && !address.is_unspecified()
            {
                addresses.insert(address.to_string());
            }
        }
    }
    Ok(addresses.into_iter().collect())
}
fn local_addresses(family: &str) -> Result<Vec<String>, String> {
    let output = Command::new("ip")
        .args([family, "-j", "address", "show"])
        .output()
        .map_err(|_| "Cannot inspect local interface addresses")?;
    if !output.status.success() {
        return Err("Cannot inspect local interface addresses".into());
    }
    parse_local_addresses(&output.stdout, family)
}
fn ip(family: &str, args: Vec<String>) -> Result<(), String> {
    let mut all = vec![family];
    all.extend(args.iter().map(String::as_str));
    run("ip", &all)
}
fn words(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_owned).collect()
}
fn rules(family: &str) -> Vec<Vec<String>> {
    let mut all = vec![];
    let private: &[&str] = if family == "-4" { &PRIVATE4 } else { &PRIVATE6 };
    for iface in IFACES {
        all.push(words(&format!(
            "priority 10020 iif {iface} lookup main suppress_prefixlength 0"
        )));
        for cidr in private {
            all.push(words(&format!(
                "priority 10030 iif {iface} to {cidr} lookup main"
            )));
        }
        all.push(words(&format!("priority 10070 iif {iface} lookup {TABLE}")));
    }
    all
}
fn fw(tool: &str, table: &str, args: &[&str]) -> Result<(), String> {
    let mut v = vec!["-w", "2", "-t", table];
    v.extend_from_slice(args);
    run(tool, &v)
}

pub fn preflight() -> Result<(), String> {
    if std::path::Path::new(TUN_DEVICE).exists() {
        return Err("Proxy tunnel u60proxy0 is already in use".into());
    }
    for (family, tool) in [("-4", "iptables"), ("-6", "ip6tables")] {
        let output = Command::new("ip")
            .args([family, "route", "show", "table", TABLE])
            .output()
            .map_err(|_| "Cannot inspect proxy routing table")?;
        // iproute2 reports an unused table as absent on some firmware versions.
        if output.stdout.iter().any(|b| !b.is_ascii_whitespace()) {
            return Err("Proxy routing table is already in use".into());
        }
        let output = Command::new("ip")
            .args([family, "rule", "show"])
            .output()
            .map_err(|_| "Cannot inspect routing rules")?;
        if !output.status.success() {
            return Err("Cannot inspect routing rules".into());
        }
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line
                .split(':')
                .next()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .is_some_and(|p| [10020, 10030, 10070].contains(&p))
            {
                return Err("Proxy routing priorities are already in use".into());
            }
        }
        for (table, chain) in [
            ("filter", "U60_PROXY_IN"),
            ("filter", "U60_PROXY_FW"),
            ("nat", "U60_PROXY_DNS"),
        ] {
            fw(tool, table, &["-S"])?;
            if fw(tool, table, &["-S", chain]).is_ok() {
                return Err("Proxy firewall chains are already in use".into());
            }
        }
    }
    Ok(())
}

// Install the input guard before starting the DNS listener. Routing is added
// only once the engine's controller and TUN device are ready.
pub fn prepare() -> Result<(), String> {
    let result = (|| {
        for (family, tool) in [("-4", "iptables"), ("-6", "ip6tables")] {
            // B31 has no xt_addrtype match. Exact local addresses provide the
            // LOCAL destination exception before private-network DNS bypass.
            let local = local_addresses(family)?;
            for (table, chain) in [
                ("filter", "U60_PROXY_IN"),
                ("filter", "U60_PROXY_FW"),
                ("nat", "U60_PROXY_DNS"),
            ] {
                fw(tool, table, &["-N", chain])?;
            }
            for iface in IFACES {
                for proto in ["udp", "tcp"] {
                    fw(
                        tool,
                        "filter",
                        &[
                            "-A",
                            "U60_PROXY_IN",
                            "-i",
                            iface,
                            "-p",
                            proto,
                            "--dport",
                            "1053",
                            "-j",
                            "ACCEPT",
                        ],
                    )?;
                    for address in &local {
                        fw(
                            tool,
                            "nat",
                            &[
                                "-A",
                                "U60_PROXY_DNS",
                                "-i",
                                iface,
                                "-p",
                                proto,
                                "--dport",
                                "53",
                                "-d",
                                address,
                                "-j",
                                "REDIRECT",
                                "--to-ports",
                                "1053",
                            ],
                        )?;
                    }
                }
                fw(
                    tool,
                    "filter",
                    &[
                        "-A",
                        "U60_PROXY_FW",
                        "-i",
                        iface,
                        "-o",
                        "u60proxy0",
                        "-j",
                        "ACCEPT",
                    ],
                )?;
                fw(
                    tool,
                    "filter",
                    &[
                        "-A",
                        "U60_PROXY_FW",
                        "-i",
                        "u60proxy0",
                        "-o",
                        iface,
                        "-m",
                        "conntrack",
                        "--ctstate",
                        "ESTABLISHED,RELATED",
                        "-j",
                        "ACCEPT",
                    ],
                )?;
            }
            let private: &[&str] = if family == "-4" { &PRIVATE4 } else { &PRIVATE6 };
            for cidr in private {
                fw(
                    tool,
                    "nat",
                    &["-A", "U60_PROXY_DNS", "-d", cidr, "-j", "RETURN"],
                )?;
            }
            for iface in IFACES {
                for proto in ["udp", "tcp"] {
                    fw(
                        tool,
                        "nat",
                        &[
                            "-A",
                            "U60_PROXY_DNS",
                            "-i",
                            iface,
                            "-p",
                            proto,
                            "--dport",
                            "53",
                            "-j",
                            "REDIRECT",
                            "--to-ports",
                            "1053",
                        ],
                    )?;
                }
            }
            // DNS is listening on all local addresses for REDIRECT, but is
            // reachable only from the three hotspot interfaces and loopback.
            fw(
                tool,
                "filter",
                &["-A", "U60_PROXY_IN", "-i", "lo", "-j", "RETURN"],
            )?;
            for proto in ["udp", "tcp"] {
                fw(
                    tool,
                    "filter",
                    &[
                        "-A",
                        "U60_PROXY_IN",
                        "-p",
                        proto,
                        "--dport",
                        "1053",
                        "-j",
                        "DROP",
                    ],
                )?;
            }
            for (table, chain, parent) in [
                ("filter", "U60_PROXY_IN", "INPUT"),
                ("filter", "U60_PROXY_FW", "FORWARD"),
                ("nat", "U60_PROXY_DNS", "PREROUTING"),
            ] {
                fw(tool, table, &["-I", parent, "1", "-j", chain])?;
            }
        }
        Ok(())
    })();
    if result.is_err() {
        disable();
    }
    result
}

pub fn activate() -> Result<(), String> {
    if !tunnel_ready(std::path::Path::new(TUN_DEVICE))? {
        return Err("Proxy tunnel u60proxy0 is not ready; routing was not activated".into());
    }
    for family in ["-4", "-6"] {
        ip(
            family,
            words(&format!("route add default dev u60proxy0 table {TABLE}")),
        )?;
        for rule in rules(family) {
            let mut args = words("rule add");
            args.extend(rule);
            ip(family, args)?;
        }
    }
    Ok(())
}

pub fn disable() {
    for (family, tool) in [("-4", "iptables"), ("-6", "ip6tables")] {
        for rule in rules(family) {
            let mut args = words("rule del");
            args.extend(rule);
            let _ = ip(family, args);
        }
        let _ = ip(
            family,
            words(&format!("route del default dev u60proxy0 table {TABLE}")),
        );
        for (table, chain, parent) in [
            ("filter", "U60_PROXY_IN", "INPUT"),
            ("filter", "U60_PROXY_FW", "FORWARD"),
            ("nat", "U60_PROXY_DNS", "PREROUTING"),
        ] {
            let _ = fw(tool, table, &["-D", parent, "-j", chain]);
            let _ = fw(tool, table, &["-F", chain]);
            let _ = fw(tool, table, &["-X", chain]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_tunnel_must_be_up_before_hotspot_routes_can_be_activated() {
        let root = std::env::temp_dir().join(format!("u60-tunnel-ready-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (send, receive) = std::sync::mpsc::channel();
        let device = root.clone();
        let waiter = std::thread::spawn(move || {
            send.send(wait_tunnel_at(&device, Duration::from_secs(3), || true))
                .unwrap();
        });
        // A responsive controller does not imply that the interface exists.
        assert!(receive.recv_timeout(Duration::from_millis(150)).is_err());
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("tun_flags"), "0x1001\n").unwrap();
        std::fs::write(root.join("flags"), "0x1000\n").unwrap();
        assert!(receive.recv_timeout(Duration::from_millis(150)).is_err());
        std::fs::write(root.join("flags"), "0x1001\n").unwrap();
        receive
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        waiter.join().unwrap();
        assert!(wait_tunnel_at(&root, Duration::ZERO, || false)
            .unwrap_err()
            .contains("stopped"));
        std::fs::remove_file(root.join("flags")).unwrap();
        assert!(wait_tunnel_at(&root, Duration::ZERO, || true)
            .unwrap_err()
            .contains("did not become ready"));
        std::fs::write(root.join("flags"), "0x1001\n").unwrap();
        std::fs::write(root.join("tun_flags"), "0x1002\n").unwrap();
        assert!(!tunnel_ready(&root).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn local_dns_addresses_are_normalized_deduplicated_and_family_specific() {
        let addresses = br#"[{"addr_info":[{"local":"127.0.0.1"},{"local":"192.0.2.1"},{"local":"192.0.2.1"},{"local":"2001:db8::1"},{"local":"fe80::1"},{"local":"::1"},{"local":"bad; command"}]}]"#;
        assert_eq!(
            parse_local_addresses(addresses, "-4").unwrap(),
            ["192.0.2.1"]
        );
        assert_eq!(
            parse_local_addresses(addresses, "-6").unwrap(),
            ["2001:db8::1", "fe80::1"]
        );
        assert!(parse_local_addresses(b"invalid", "-4").is_err());
    }
    #[test]
    fn routes_are_ingress_only_and_follow_existing_overlay_rules() {
        for f in ["-4", "-6"] {
            for r in rules(f) {
                assert!(r
                    .windows(2)
                    .any(|p| p[0] == "iif" && IFACES.contains(&p[1].as_str())));
                let priority = r[1].parse::<u32>().unwrap();
                assert!(priority > 5270 && priority < 32766);
                assert!(!r
                    .iter()
                    .any(|s| s == "52" || s == "tailscale0" || s == "rmnet_data0"));
            }
        }
        assert!(rules("-4")
            .iter()
            .any(|r| r.iter().any(|x| x == "100.64.0.0/10")));
    }
}
