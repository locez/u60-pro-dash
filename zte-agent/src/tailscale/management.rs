//! Kernel-mode Serve uses real listening sockets. Its port must differ from
//! the agent's wildcard listener, including across agent restarts.
use serde_json::{json, Value};

pub(super) const PORT: u16 = 9091;
const TARGET: &str = "127.0.0.1:9090";

fn commands(config: &Value) -> Result<Vec<Vec<&'static str>>, String> {
    if !config.is_object() || (!config["TCP"].is_null() && !config["TCP"].is_object()) {
        return Err("Invalid Tailscale Serve configuration".into());
    }
    let owned = json!({"TCPForward": TARGET});
    for port in ["9090", "9091"] {
        if let Some(handler) = config["TCP"].get(port) {
            if handler != &owned {
                return Err(format!("Tailscale Serve port {port} belongs to another handler; configuration was preserved"));
            }
        }
    }
    let mut result = Vec::new();
    if config["TCP"].get("9090").is_some() {
        result.push(vec!["serve", "--tcp=9090", "off"]);
    }
    if config["TCP"].get("9091").is_none() {
        result.push(vec![
            "serve",
            "--bg",
            "--yes",
            "--tcp=9091",
            "tcp://127.0.0.1:9090",
        ]);
    }
    Ok(result)
}

pub(super) fn configure() -> Result<(), String> {
    let raw = super::cli(&["serve", "status", "--json"], 8)?;
    let config: Value = serde_json::from_str(&raw)
        .map_err(|_| "Invalid Tailscale Serve configuration".to_string())?;
    for command in commands(&config)? {
        super::cli(&command, 10)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_only_the_owned_forwarder_and_is_idempotent() {
        let old = json!({"TCP":{"9090":{"TCPForward":TARGET},"8443":{"HTTPS":true}},"Web":{"example":{"Handlers":{}}}});
        let plan = commands(&old).unwrap();
        assert_eq!(plan[0], ["serve", "--tcp=9090", "off"]);
        assert_eq!(
            plan[1],
            [
                "serve",
                "--bg",
                "--yes",
                "--tcp=9091",
                "tcp://127.0.0.1:9090"
            ]
        );
        assert_eq!(commands(&json!({})).unwrap().len(), 1);
        assert!(commands(&json!({"TCP":{"9091":{"TCPForward":TARGET}}}))
            .unwrap()
            .is_empty());
        assert_ne!(PORT, 9090);
    }

    #[test]
    fn never_overwrites_user_handlers_or_partially_migrates_a_conflict() {
        for config in [
            json!({"TCP":{"9090":{"TCPForward":"127.0.0.1:8000"}}}),
            json!({"TCP":{"9090":{"TCPForward":TARGET},"9091":{"HTTPS":true}}}),
            json!({"TCP":{"9090":{"TCPForward":TARGET,"TerminateTLS":"example"}}}),
            json!({"TCP":[]}),
            Value::Null,
        ] {
            assert!(commands(&config).is_err());
        }
    }
}
