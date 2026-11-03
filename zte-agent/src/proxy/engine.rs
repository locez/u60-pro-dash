//! The replaceable configuration/controller adapter. Lifecycle and network
//! ownership stay in the service, not in a particular engine implementation.
use super::{Settings, CONFIG_LIMIT};
use serde_json::{json, Value};

pub trait Engine: Send + Sync {
    fn command(
        &self,
        binary: &std::path::Path,
        root: &std::path::Path,
        config: &std::path::Path,
        check: bool,
    ) -> std::process::Command;
    fn readiness_path(&self) -> &'static str;
    fn groups_path(&self) -> &'static str;
    fn selection(&self, group: &str, choice: &str) -> (String, Value);
    fn normalize(&self, text: &str) -> Result<Value, String>;
    fn runtime(&self, source: &Value, settings: &Settings, secret: &str) -> Value;
    fn groups(&self, value: &Value) -> Value;
}

pub struct Compatible;
pub fn adapter(id: &str) -> Result<Box<dyn Engine>, String> {
    match id {
        "compatible-v1" => Ok(Box::new(Compatible)),
        _ => Err("Unsupported proxy adapter".into()),
    }
}

impl Engine for Compatible {
    fn command(
        &self,
        binary: &std::path::Path,
        root: &std::path::Path,
        config: &std::path::Path,
        check: bool,
    ) -> std::process::Command {
        let mut command = std::process::Command::new(binary);
        if check {
            command.arg("-t");
        }
        command.arg("-d").arg(root).arg("-f").arg(config);
        command
    }
    fn readiness_path(&self) -> &'static str {
        "/version"
    }
    fn groups_path(&self) -> &'static str {
        "/proxies"
    }
    fn selection(&self, group: &str, choice: &str) -> (String, Value) {
        let mut url = reqwest::Url::parse("http://127.0.0.1/proxies/").unwrap();
        url.path_segments_mut().unwrap().pop_if_empty().push(group);
        (url.path().into(), json!({"name":choice}))
    }
    fn normalize(&self, text: &str) -> Result<Value, String> {
        if text.is_empty() || text.len() > CONFIG_LIMIT {
            return Err("Configuration must be 1–512 KiB".into());
        }
        let yaml: serde_yaml::Value =
            serde_yaml::from_str(text).map_err(|_| "Invalid YAML configuration")?;
        let source = serde_json::to_value(yaml)
            .map_err(|_| "Configuration must use string keys and plain values")?;
        let object = source
            .as_object()
            .ok_or("Configuration must be an object")?;
        if serde_json::to_vec(&source)
            .map_err(|_| "Invalid configuration")?
            .len()
            > CONFIG_LIMIT
        {
            return Err("Expanded configuration exceeds 512 KiB".into());
        }
        // Only routing data crosses the boundary. The service owns all ports,
        // controller credentials, TUN settings and writable provider paths.
        let mut clean = json!({});
        for key in [
            "proxies",
            "proxy-groups",
            "proxy-providers",
            "rule-providers",
            "rules",
            "hosts",
        ] {
            if let Some(v) = object.get(key) {
                clean[key] = v.clone();
            }
        }
        for key in ["proxies", "proxy-groups", "rules"] {
            if !clean[key].is_null() && !clean[key].is_array() {
                return Err(format!("{key} must be a list"));
            }
        }
        if clean["proxies"].as_array().is_none_or(Vec::is_empty)
            && clean["proxy-providers"]
                .as_object()
                .is_none_or(serde_json::Map::is_empty)
        {
            return Err("Configuration needs at least one proxy or remote provider".into());
        }
        for key in ["proxy-providers", "rule-providers"] {
            if clean[key].is_null() {
                continue;
            }
            let providers = clean[key]
                .as_object_mut()
                .ok_or("Providers must be a mapping")?;
            if providers.len() > 64 {
                return Err("At most 64 providers per kind".into());
            }
            for (index, provider) in providers.values_mut().enumerate() {
                let entry = provider.as_object_mut().ok_or("Invalid provider")?;
                if entry.get("type").and_then(Value::as_str) != Some("http") {
                    return Err("Only HTTPS remote providers are supported".into());
                }
                super::https_url(
                    entry
                        .get("url")
                        .and_then(Value::as_str)
                        .ok_or("Provider URL required")?,
                )?;
                entry.insert(
                    "path".into(),
                    json!(format!("./providers/{key}-{index}.yaml")),
                );
                // Prevent an imported header from sending controller credentials.
                entry.remove("proxy");
            }
        }
        fn check(v: &Value, depth: usize) -> Result<(), String> {
            if depth > 32 {
                return Err("Configuration nesting is too deep".into());
            }
            match v {
                Value::Object(o) => {
                    for (key, value) in o {
                        if [
                            "script",
                            "command",
                            "exec",
                            "external-ui",
                            "external-controller",
                            "secret",
                        ]
                        .contains(&key.as_str())
                        {
                            return Err(
                                "Executable and controller options are not allowed in routing data"
                                    .into(),
                            );
                        }
                        if ["private-key", "certificate", "ca"].contains(&key.as_str())
                            && value
                                .as_str()
                                .is_some_and(|s| !s.is_empty() && !s.starts_with("-----BEGIN "))
                        {
                            return Err("Certificate file references are not allowed".into());
                        }
                        check(value, depth + 1)?;
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        check(v, depth + 1)?;
                    }
                }
                _ => {}
            }
            Ok(())
        }
        check(&clean, 0)?;
        Ok(clean)
    }
    fn runtime(&self, source: &Value, settings: &Settings, secret: &str) -> Value {
        let mut v = source.clone();
        v["mode"] = json!(settings.mode);
        v["mixed-port"] = json!(7890);
        v["bind-address"] = json!("127.0.0.1");
        v["allow-lan"] = json!(false);
        v["external-controller"] = json!("127.0.0.1:19092");
        v["secret"] = json!(secret);
        v["log-level"] = json!("silent");
        v["ipv6"] = json!(true);
        v["profile"] = json!({"store-selected":true,"store-fake-ip":false});
        v["tun"] = json!({"enable":settings.capture=="hotspot","device":"u60proxy0","stack":"gvisor","auto-route":false,"auto-redirect":false,"auto-detect-interface":true,"strict-route":false,"mtu":1400,"dns-hijack":["any:53","tcp://any:53"]});
        v["dns"] = json!({"enable":true,"listen":if settings.capture=="hotspot" {"[::]:1053"}else{"127.0.0.1:1053"},"ipv6":true,"enhanced-mode":"redir-host","use-hosts":true,"default-nameserver":["1.1.1.1","8.8.8.8"],"nameserver":["https://cloudflare-dns.com/dns-query","https://dns.google/dns-query"],"proxy-server-nameserver":["1.1.1.1","8.8.8.8"]});
        v
    }
    fn groups(&self, value: &Value) -> Value {
        let mut groups = Vec::new();
        if let Some(proxies) = value["proxies"].as_object() {
            for (name, p) in proxies {
                if let Some(all) = p["all"].as_array() {
                    groups.push(json!({"name":name,"kind":p["type"],"selected":p["now"],"selectable":p["type"]=="Selector","choices":all.iter().filter_map(Value::as_str).take(1024).collect::<Vec<_>>()}));
                }
            }
        }
        json!(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_configuration_cannot_own_listeners_routes_or_paths() {
        let a = Compatible;
        let source=a.normalize("proxies: [{name: Example, type: socks5, server: example.com, port: 1080}]\nexternal-controller: 0.0.0.0:9090\nsecret: unsafe\ntun: {auto-route: true}\nproxy-providers: {remote: {type: http, url: 'https://example.com/config', path: /etc/passwd}}\n").unwrap();
        let runtime = a.runtime(&source, &Settings::default(), "private-controller");
        assert_eq!(runtime["external-controller"], "127.0.0.1:19092");
        assert_eq!(runtime["tun"]["auto-route"], false);
        assert_eq!(runtime["secret"], "private-controller");
        assert_eq!(
            runtime["proxy-providers"]["remote"]["path"],
            "./providers/proxy-providers-0.yaml"
        );
        assert!(a
            .normalize("proxies: [{name: x, private-key: /etc/shadow}]")
            .is_err());
        assert!(a
            .normalize(
                "proxies: [{name: x}]\nproxy-providers: {x: {type: file, path: /etc/shadow}}"
            )
            .is_err());
        assert!(adapter("unknown").is_err());
    }
}
