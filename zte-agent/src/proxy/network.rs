//! Only hotspot ingress uses the owned routing table. Existing lower-priority
//! numbers (including the overlay network's table 52) are evaluated first.
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| format!("Cannot start network utility {program}"))?;
    let end = Instant::now() + Duration::from_secs(4);
    loop {
        match child.try_wait() {
            Ok(Some(s)) => {
                return if s.success() {
                    Ok(())
                } else {
                    Err(format!("Network utility {program} refused a rule"))
                }
            }
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Network utility timed out".into());
            }
        }
    }
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
            fw(tool, table, &["-L"])?;
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
                            "-m",
                            "addrtype",
                            "--dst-type",
                            "LOCAL",
                            "-j",
                            "REDIRECT",
                            "--to-ports",
                            "1053",
                        ],
                    )?;
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
