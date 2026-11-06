//! On-demand diagnostics. These do not alter routes or enroll the device.
use super::*;
use std::io::{Seek, SeekFrom};
use std::net::{IpAddr, SocketAddr, TcpStream};

fn output(args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("ip");
    cmd.args(args);
    let result = run_with_timeout(cmd, 3)?;
    if !result.status.success() || result.stdout.len() > 1024 * 1024 {
        return Err("Unable to read the kernel routing table".into());
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}
fn word<'a>(words: &'a [&str], key: &str) -> Option<&'a str> {
    words.windows(2).find(|w| w[0] == key).map(|w| w[1])
}
fn routes(text: &str) -> Vec<Value> {
    text.lines().filter_map(|line| {
        let words: Vec<_> = line.split_whitespace().collect();
        if word(&words, "dev") != Some("tailscale0") { return None; }
        let destination = *words.first()?;
        if destination != "default" && destination.parse::<ipnet::IpNet>().is_err()
            && destination.parse::<IpAddr>().is_err() { return None; }
        Some(json!({"destination":destination,"interface":"tailscale0","source":word(&words,"src"),"table":word(&words,"table")}))
    }).take(2048).collect()
}
pub fn network(_state: &AppState) -> (u16, Value) {
    let result = (|| {
        let mut entries = routes(&output(&["-4", "route", "show", "table", "all"])?);
        if let Ok(ipv6) = output(&["-6", "route", "show", "table", "all"]) {
            entries.extend(routes(&ipv6));
        }
        Ok(json!({"routes":entries,"gateway_mode":gateway_mode()}))
    })();
    reply(result)
}
fn target(value: &str) -> Result<SocketAddr, String> {
    let a: SocketAddr = value
        .trim()
        .parse()
        .map_err(|_| "Use an IP address and TCP port")?;
    let ip = a.ip().to_canonical();
    if a.port() == 0
        || ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || matches!(ip,IpAddr::V4(ip) if ip.is_broadcast())
    {
        return Err("Use a unicast destination outside the router loopback".into());
    }
    Ok(a)
}
pub fn probe(_state: &AppState, body: &[u8]) -> (u16, Value) {
    let parsed: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return (400, json!({"ok":false,"error":"invalid JSON"})),
    };
    let address = match target(parsed["target"].as_str().unwrap_or("")) {
        Ok(a) => a,
        Err(e) => return (400, json!({"ok":false,"error":e})),
    };
    let result = (|| {
        let ip = address.ip().to_string();
        let route = output(&[
            if address.is_ipv4() { "-4" } else { "-6" },
            "route",
            "get",
            &ip,
        ])?;
        let words: Vec<_> = route.split_whitespace().collect();
        let interface = word(&words, "dev");
        let via_tailscale = interface == Some("tailscale0");
        let started = Instant::now();
        let reachable = TcpStream::connect_timeout(&address, Duration::from_secs(3)).is_ok();
        Ok(
            json!({"target":address.to_string(),"route":interface,"via_tailscale":via_tailscale,
            "reachable":reachable,"latency_ms":if reachable{Some(started.elapsed().as_millis())}else{None},
            "gateway_ready":Value::Null,"scope":"router"}),
        )
    })();
    reply(result)
}
fn redact(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if [
        "tskey-",
        "privkey:",
        "authkey",
        "authorization:",
        "machinekey",
        "private key",
    ]
    .iter()
    .any(|key| lower.contains(key))
    {
        return "[credential-bearing log entry omitted]".into();
    }
    line.split_whitespace()
        .map(|word| {
            if word.contains("https://") || word.contains("http://") {
                "[URL]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(4096)
        .collect()
}
pub fn logs(_state: &AppState) -> (u16, Value) {
    let result = (|| {
        let mut file = match std::fs::File::open(LOG_FILE) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(json!({"text":"","truncated":false}))
            }
            Err(_) => return Err("Cannot read the service log".into()),
        };
        let len = file.metadata().map_err(|_| "Cannot read log size")?.len();
        let start = len.saturating_sub(96 * 1024);
        file.seek(SeekFrom::Start(start))
            .map_err(|_| "Cannot read log tail")?;
        let mut bytes = Vec::new();
        file.take(96 * 1024)
            .read_to_end(&mut bytes)
            .map_err(|_| "Cannot read service log")?;
        let text = String::from_utf8_lossy(&bytes)
            .lines()
            .skip(usize::from(start > 0))
            .map(redact)
            .collect::<Vec<_>>()
            .join("\n");
        Ok(json!({"text":text,"truncated":start>0}))
    })();
    reply(result)
}
fn reply(result: Result<Value, String>) -> (u16, Value) {
    match result {
        Ok(data) => (200, json!({"ok":true,"data":data})),
        Err(error) => (503, json!({"ok":false,"error":error})),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn route_inventory_excludes_other_interfaces_and_route_types() {
        let got = routes("192.0.2.0/24 dev tailscale0 table 52\n10.0.0.0/24 dev bridge0\nunreachable default dev lo table 52\n100.64.0.1 dev tailscale0 table 52\n");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0]["destination"], "192.0.2.0/24");
        assert_eq!(got[1]["table"], "52");
    }
    #[test]
    fn probe_requires_explicit_unicast_endpoint_and_logs_hide_credentials() {
        for bad in [
            "127.0.0.1:80",
            "[::ffff:127.0.0.1]:80",
            "0.0.0.0:80",
            "224.0.0.1:80",
            "255.255.255.255:80",
            "192.0.2.2:0",
            "example.com:80",
        ] {
            assert!(target(bad).is_err(), "{bad}");
        }
        assert!(target("192.0.2.2:443").is_ok());
        assert!(!redact("login https://login.example/auth/private-path").contains("private-path"));
        assert!(!redact("authkey=tskey-test-fixture").contains("tskey-"));
        assert_eq!(redact("netcheck: udp=true"), "netcheck: udp=true");
    }
}
