//! Read-only path and TCP checks on the router, independent of destination use.
use ipnet::IpNet;
use serde_json::{Value, json};
use std::{
    net::SocketAddr,
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{net::TcpSocket, process::Command, time::timeout};

pub fn target(text: &str) -> anyhow::Result<SocketAddr> {
    let address: SocketAddr = text.trim().parse()?;
    anyhow::ensure!(
        !address.ip().is_loopback()
            && !address.ip().is_unspecified()
            && !address.ip().is_multicast()
            && address.port() != 0,
        "请输入目标 IP 和端口，例如 10.20.0.10:443 或 [fd7a::10]:443"
    );
    anyhow::ensure!(
        !matches!(address.ip(), std::net::IpAddr::V4(ip) if ip.is_broadcast()),
        "不支持广播地址"
    );
    Ok(address)
}
fn uses_tailscale(route: &str) -> bool {
    route
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|p| p == ["dev", "tailscale0"])
}
async fn output(program: &str, args: &[&str]) -> Option<String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    let response = timeout(Duration::from_secs(2), command.output())
        .await
        .ok()?
        .ok()?;
    (response.status.success() && response.stdout.len() <= 1024 * 1024)
        .then(|| String::from_utf8_lossy(&response.stdout).into_owned())
}
fn part<'a>(rule: &'a [&str], key: &str) -> Option<&'a str> {
    rule.windows(2)
        .find(|pair| pair[0] == key)
        .map(|pair| pair[1].trim_matches('"'))
}
fn owned(rule: &[&str]) -> bool {
    matches!(
        part(rule, "--comment"),
        Some("u60-tailscale-gateway" | "u60-home-gateway")
    )
}
fn configured_gateway(
    filter: &str,
    nat: &str,
    target: std::net::IpAddr,
) -> Option<(String, IpNet)> {
    let forwards = filter
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let translations = nat
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for rule in &forwards {
        if !owned(rule)
            || part(rule, "-A") != Some("FORWARD")
            || part(rule, "-j") != Some("ACCEPT")
            || part(rule, "-o") != Some("tailscale0")
        {
            continue;
        }
        let (Some(lan), Some(source), Some(destination)) =
            (part(rule, "-i"), part(rule, "-s"), part(rule, "-d"))
        else {
            continue;
        };
        let (Ok(source_net), Ok(destination_net)) =
            (source.parse::<IpNet>(), destination.parse::<IpNet>())
        else {
            continue;
        };
        if !destination_net.contains(&target) {
            continue;
        }
        let reverse = forwards.iter().any(|r| {
            owned(r)
                && part(r, "-A") == Some("FORWARD")
                && part(r, "-j") == Some("ACCEPT")
                && part(r, "-i") == Some("tailscale0")
                && part(r, "-o") == Some(lan)
                && part(r, "-s") == Some(destination)
                && part(r, "-d") == Some(source)
                && part(r, "--ctstate").is_some_and(|s| {
                    s.split(',').any(|s| s == "ESTABLISHED") && s.split(',').any(|s| s == "RELATED")
                })
        });
        let masquerade = translations.iter().any(|r| {
            owned(r)
                && part(r, "-A") == Some("POSTROUTING")
                && part(r, "-j") == Some("MASQUERADE")
                && part(r, "-o") == Some("tailscale0")
                && part(r, "-s") == Some(source)
                && part(r, "-d") == Some(destination)
        });
        if reverse && masquerade {
            return Some((lan.into(), source_net));
        }
    }
    None
}
async fn gateway_rules_present(address: SocketAddr) -> bool {
    let path = if address.is_ipv4() {
        "/proc/sys/net/ipv4/ip_forward"
    } else {
        "/proc/sys/net/ipv6/conf/all/forwarding"
    };
    if !std::fs::read_to_string(path).is_ok_and(|v| v.trim() == "1") {
        return false;
    }
    let save = if address.is_ipv4() {
        "iptables-save"
    } else {
        "ip6tables-save"
    };
    let Some(filter) = output(save, &["-t", "filter"]).await else {
        return false;
    };
    let Some(nat) = output(save, &["-t", "nat"]).await else {
        return false;
    };
    let Some((lan, network)) = configured_gateway(&filter, &nat, address.ip()) else {
        return false;
    };
    let Some(addresses) = output(
        "ip",
        &[
            "-o",
            if address.is_ipv4() { "-4" } else { "-6" },
            "addr",
            "show",
            "dev",
            &lan,
        ],
    )
    .await
    else {
        return false;
    };
    addresses
        .split_whitespace()
        .filter_map(|s| s.parse::<IpNet>().ok())
        .any(|n| n.trunc() == network.trunc())
}
pub async fn probe(address: SocketAddr) -> Value {
    let Some(route) = output(
        "ip",
        &[
            if address.is_ipv4() { "-4" } else { "-6" },
            "route",
            "get",
            &address.ip().to_string(),
        ],
    )
    .await
    else {
        return json!({"routed":null, "reachable":false, "error":"route"});
    };
    let routed = uses_tailscale(&route);
    if !routed {
        return json!({"routed":false, "reachable":false, "gateway_rules_present":false});
    }
    let socket = if address.is_ipv4() {
        TcpSocket::new_v4()
    } else {
        TcpSocket::new_v6()
    };
    let Ok(socket) = socket else {
        return json!({"routed":true,"reachable":false,"error":"socket"});
    };
    #[cfg(target_os = "linux")]
    if socket2::SockRef::from(&socket)
        .bind_device(Some(b"tailscale0"))
        .is_err()
    {
        return json!({"routed":true,"reachable":false,"error":"bind"});
    }
    #[cfg(not(target_os = "linux"))]
    return json!({"routed":true,"reachable":false,"error":"unsupported"});
    // Bind to the tunnel as well as checking the route, so a route lost between
    // the check and connect cannot send this probe through the cellular network.
    let start = Instant::now();
    let connection = timeout(Duration::from_secs(3), socket.connect(address)).await;
    let reachable = matches!(&connection, Ok(Ok(_)));
    let refused =
        matches!(&connection, Ok(Err(e)) if e.kind()==std::io::ErrorKind::ConnectionRefused);
    let elapsed = start.elapsed().as_millis() as u64;
    drop(connection);
    let forwarding = gateway_rules_present(address).await;
    json!({"routed":true, "reachable":reachable, "refused":refused, "gateway_rules_present":forwarding, "latency_ms":elapsed})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_routes_and_gateway_rules_are_not_tied_to_one_destination_network() {
        for value in [
            "10.20.0.10:443",
            "192.0.2.10:80",
            "100.64.0.1:22",
            "[fd7a:115c:a1e0::10]:80",
        ] {
            assert!(target(value).is_ok());
        }
        for value in [
            "127.0.0.1:80",
            "[::1]:80",
            "224.0.0.1:80",
            "10.20.0.10:0",
            "10.20.0.10:80; reboot",
        ] {
            assert!(target(value).is_err());
        }
        assert!(uses_tailscale("10.20.0.10 dev tailscale0 table 52"));
        assert!(!uses_tailscale("10.20.0.10 via 10.8.0.1 dev br-lan"));
        assert!(!uses_tailscale("10.20.0.10 dev tailscale01"));
        let filter = "-A FORWARD -i br-test -o tailscale0 -s 10.8.0.0/23 -d 172.20.0.0/16 -m comment --comment u60-tailscale-gateway -j ACCEPT\n-A FORWARD -i tailscale0 -o br-test -s 172.20.0.0/16 -d 10.8.0.0/23 -m conntrack --ctstate RELATED,ESTABLISHED -m comment --comment u60-tailscale-gateway -j ACCEPT";
        let nat = "-A POSTROUTING -s 10.8.0.0/23 -d 172.20.0.0/16 -o tailscale0 -m comment --comment u60-tailscale-gateway -j MASQUERADE";
        assert!(configured_gateway(filter, nat, "172.20.40.50".parse().unwrap()).is_some());
        assert!(configured_gateway(filter, nat, "192.168.1.10".parse().unwrap()).is_none());
        assert!(configured_gateway(filter, "", "172.20.40.50".parse().unwrap()).is_none());
        assert!(
            configured_gateway(
                &filter.replace("u60-tailscale-gateway", "unrelated"),
                nat,
                "172.20.40.50".parse().unwrap()
            )
            .is_none()
        );
    }
}
