//! Kernel-mode Serve owns sockets separate from the agent HTTP listener.
use crate::management::{HTTP_PORT, LEGACY_HTTP_PORT, LEGACY_REMOTE_PORT, REMOTE_PORT};
use serde_json::{json, Value};

pub(super) fn remote_port(http_port: u16) -> u16 {
    if http_port == LEGACY_HTTP_PORT {
        LEGACY_REMOTE_PORT
    } else {
        REMOTE_PORT
    }
}

fn owned(handler: &Value) -> bool {
    [LEGACY_HTTP_PORT, HTTP_PORT]
        .iter()
        .any(|port| handler == &json!({"TCPForward":format!("127.0.0.1:{port}")}))
}

fn commands(config: &Value, http_port: u16) -> Result<Vec<Vec<String>>, String> {
    if !config.is_object() || (!config["TCP"].is_null() && !config["TCP"].is_object()) {
        return Err("Invalid Tailscale Serve configuration".into());
    }
    let remote = remote_port(http_port);
    if remote == http_port {
        return Err("Tailscale remote management port conflicts with the agent listener".into());
    }
    let target = format!("127.0.0.1:{http_port}");
    let desired = json!({"TCPForward": target});
    // Check every conflicting destination before planning any removal.
    for port in [http_port, remote] {
        if let Some(handler) = config["TCP"].get(port.to_string()) {
            if !owned(handler) && handler != &desired {
                return Err(format!("Tailscale Serve port {port} belongs to another handler; configuration was preserved"));
            }
        }
    }
    let mut result = Vec::new();
    for port in [LEGACY_HTTP_PORT, LEGACY_REMOTE_PORT, HTTP_PORT, REMOTE_PORT] {
        if let Some(handler) = config["TCP"].get(port.to_string()) {
            if owned(handler) && (port != remote || handler != &desired) {
                result.push(vec!["serve".into(), format!("--tcp={port}"), "off".into()]);
            }
        }
    }
    if config["TCP"].get(remote.to_string()) != Some(&desired) {
        result.push(vec![
            "serve".into(),
            "--bg".into(),
            "--yes".into(),
            format!("--tcp={remote}"),
            format!("tcp://{target}"),
        ]);
    }
    Ok(result)
}

pub(super) fn configure(http_port: u16) -> Result<(), String> {
    let raw = super::cli(&["serve", "status", "--json"], 8)?;
    let config: Value = serde_json::from_str(&raw)
        .map_err(|_| "Invalid Tailscale Serve configuration".to_string())?;
    for command in commands(&config, http_port)? {
        super::cli(&command.iter().map(String::as_str).collect::<Vec<_>>(), 10)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrates_both_legacy_layouts_and_supports_original_restoration() {
        for old_port in ["9090", "9091"] {
            let old =
                json!({"TCP":{old_port:{"TCPForward":"127.0.0.1:9090"},"8443":{"HTTPS":true}}});
            let plan = commands(&old, HTTP_PORT).unwrap();
            assert_eq!(plan[0], ["serve", &format!("--tcp={old_port}"), "off"]);
            assert_eq!(
                plan[1],
                [
                    "serve",
                    "--bg",
                    "--yes",
                    "--tcp=20601",
                    "tcp://127.0.0.1:20600"
                ]
            );
        }
        let current = json!({"TCP":{"20601":{"TCPForward":"127.0.0.1:20600"}}});
        assert!(commands(&current, HTTP_PORT).unwrap().is_empty());
        let restore = commands(&current, LEGACY_HTTP_PORT).unwrap();
        assert_eq!(restore[0], ["serve", "--tcp=20601", "off"]);
        assert_eq!(
            restore[1],
            [
                "serve",
                "--bg",
                "--yes",
                "--tcp=9091",
                "tcp://127.0.0.1:9090"
            ]
        );
        assert_eq!(commands(&json!({}), HTTP_PORT).unwrap().len(), 1);
    }
    #[test]
    fn old_ports_can_belong_to_other_services_but_new_conflicts_are_refused() {
        let unrelated =
            json!({"TCP":{"9090":{"TCPForward":"127.0.0.1:8000"},"9091":{"HTTPS":true}}});
        assert_eq!(commands(&unrelated, HTTP_PORT).unwrap().len(), 1);
        for config in [
            json!({"TCP":{"20601":{"HTTPS":true},"9091":{"TCPForward":"127.0.0.1:9090"}}}),
            json!({"TCP":{"20600":{"TCPForward":"127.0.0.1:8000"}}}),
            json!({"TCP":{"20601":{"TCPForward":"127.0.0.1:20600","TerminateTLS":"example"}}}),
            json!({"TCP":[]}),
            Value::Null,
        ] {
            assert!(commands(&config, HTTP_PORT).is_err());
        }
    }
}
