//! The replaceable configuration/controller adapter. Lifecycle and network
//! ownership stay in the service, not in a particular engine implementation.
use super::CONFIG_LIMIT;
use serde::{Deserialize, Serialize};
type Settings = MihomoSettings;
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

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MihomoSettings {
    pub adapter: String,
    pub mode: String,
    pub mixed_port: u16,
    pub ipv6: bool,
    pub tcp_concurrent: bool,
    pub capture: String,
    pub enabled: bool,
}
impl Default for MihomoSettings {
    fn default() -> Self {
        Self {
            adapter: "mihomo".into(),
            mode: "rule".into(),
            mixed_port: 7890,
            ipv6: true,
            tcp_concurrent: false,
            capture: "local".into(),
            enabled: false,
        }
    }
}
impl MihomoSettings {
    pub(super) fn validate(&self) -> Result<(), String> {
        adapter(&self.adapter)?;
        if self.mixed_port < 1024 || [1053, 9090, 9091, 9443, 19092].contains(&self.mixed_port) {
            return Err(
                "Choose an unused local proxy port above 1023, excluding router management ports"
                    .into(),
            );
        }
        if !["rule", "global", "direct"].contains(&self.mode.as_str()) {
            return Err("Choose rule, global or direct mode".into());
        }
        if !["local", "hotspot"].contains(&self.capture.as_str()) {
            return Err("Choose local or hotspot traffic".into());
        }
        Ok(())
    }
}

pub(super) fn check_listeners(settings: &Settings) -> Result<(), String> {
    // Check the real resources, never stop another proxy or use a global service lock.
    let mut tcp = Vec::new();
    for port in [settings.mixed_port, 19092, 1053] {
        tcp.push(std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|_| format!("Port {port} is already in use; change this service's port or resolve the conflict"))?);
    }
    let _dns = std::net::UdpSocket::bind(("127.0.0.1", 1053))
        .map_err(|_| "DNS port 1053 is already in use")?;
    Ok(())
}

pub struct Mihomo;
pub fn adapter(id: &str) -> Result<Box<dyn Engine>, String> {
    match id {
        "mihomo" | "compatible-v1" => Ok(Box::new(Mihomo)),
        _ => Err("Unsupported proxy adapter".into()),
    }
}

impl Engine for Mihomo {
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
        let mut yaml: serde_yaml::Value = serde_yaml::from_str(text).map_err(|error| {
            error.location().map_or_else(
                || "Invalid YAML configuration".to_string(),
                |at| {
                    format!(
                        "Invalid YAML configuration at line {}, column {}",
                        at.line(),
                        at.column()
                    )
                },
            )
        })?;
        // serde_yaml resolves aliases but does not apply YAML merge keys by
        // default. Expand them before converting to JSON or inherited group
        // fields (especially `type`) never reach the engine.
        yaml.apply_merge()
            .map_err(|_| "Invalid YAML merge mapping")?;
        let source = serde_json::to_value(yaml)
            .map_err(|_| "Configuration must use string keys and plain values")?;
        let object = source
            .as_object()
            .ok_or("Expected a mihomo YAML/JSON configuration. Use a compatible subscription format, not a plain node list or HTML page")?;
        if serde_json::to_vec(&source)
            .map_err(|_| "Invalid configuration")?
            .len()
            > CONFIG_LIMIT
        {
            return Err("Expanded configuration exceeds 512 KiB".into());
        }
        // Native engine configuration stays native. Only device-owned sockets,
        // routing and filesystem integration are overridden by the runtime.
        let mut clean = source.clone();
        for key in [
            "port",
            "socks-port",
            "redir-port",
            "tproxy-port",
            "listeners",
            "external-controller",
            "external-controller-tls",
            "external-controller-unix",
            "external-controller-pipe",
            "external-ui",
            "external-ui-url",
            "external-ui-name",
            "secret",
            "authentication",
            "allow-lan",
            "bind-address",
            "interface-name",
            "routing-mark",
            "tunnels",
            "tun",
            "experimental",
            "tls",
            "external-controller-cors",
        ] {
            clean.as_object_mut().unwrap().remove(key);
        }
        if object.contains_key("script") {
            return Err("Script execution is not supported on this router".into());
        }
        // Preserve user DNS choices while the device owns listening and private-domain routing.
        if let Some(dns) = clean.get_mut("dns").and_then(Value::as_object_mut) {
            for key in ["listen", "enable", "use-system-hosts"] {
                dns.remove(key);
            }
            dns.insert("use-system-hosts".into(), json!(false));
        }
        if clean.get("dns").is_some_and(|v| !v.is_object())
            || clean["dns"]
                .get("nameserver-policy")
                .is_some_and(|v| !v.is_object())
        {
            return Err("DNS configuration must be a mapping".into());
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
        for key in ["mode", "mixed-port", "ipv6", "tcp-concurrent"] {
            clean.as_object_mut().unwrap().remove(key);
        }
        check(&clean, 0)?;
        Ok(clean)
    }
    fn runtime(&self, source: &Value, settings: &Settings, secret: &str) -> Value {
        let mut v = source.clone();
        v["mode"] = json!(settings.mode);
        v["mixed-port"] = json!(settings.mixed_port);
        v["bind-address"] = json!("127.0.0.1");
        v["allow-lan"] = json!(false);
        v["external-controller"] = json!("127.0.0.1:19092");
        v["secret"] = json!(secret);
        if v.get("log-level").is_none() {
            v["log-level"] = json!("silent");
        }
        v["ipv6"] = json!(settings.ipv6);
        v["tcp-concurrent"] = json!(settings.tcp_concurrent);
        if v.get("profile").is_none() {
            v["profile"] = json!({"store-selected":true,"store-fake-ip":false});
        }
        v["tun"] = json!({"enable":settings.capture=="hotspot","device":"u60proxy0","stack":"gvisor","auto-route":false,"auto-redirect":false,"auto-detect-interface":true,"strict-route":false,"mtu":1400,"dns-hijack":["any:53","tcp://any:53"]});
        let dns = v
            .as_object_mut()
            .unwrap()
            .entry("dns")
            .or_insert_with(|| json!({}));
        let dns = dns.as_object_mut().unwrap();
        dns.insert("enable".into(), json!(true));
        dns.insert(
            "listen".into(),
            json!(if settings.capture == "hotspot" {
                "[::]:1053"
            } else {
                "127.0.0.1:1053"
            }),
        );
        dns.entry("ipv6").or_insert(json!(settings.ipv6));
        dns.entry("enhanced-mode").or_insert(json!("redir-host"));
        dns.entry("use-hosts").or_insert(json!(true));
        dns.entry("default-nameserver")
            .or_insert(json!(["1.1.1.1", "8.8.8.8"]));
        dns.entry("nameserver").or_insert(json!([
            "https://cloudflare-dns.com/dns-query",
            "https://dns.google/dns-query"
        ]));
        dns.entry("proxy-server-nameserver")
            .or_insert(json!(["1.1.1.1", "8.8.8.8"]));
        let policy = dns.entry("nameserver-policy").or_insert(json!({}));
        if let Some(policy) = policy.as_object_mut() {
            policy.insert("+.home.arpa".into(), json!(["udp://127.0.0.1:53"]));
            policy.insert("+.lan".into(), json!(["udp://127.0.0.1:53"]));
        }

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
    fn native_yaml_merges_preserve_group_defaults_and_explicit_overrides() {
        let source = Mihomo
            .normalize(
                r#"
proxies: [{name: Example HK, type: anytls, server: 192.0.2.1, port: 443, password: example-only}]
url-test: &common
  type: url-test
  include-all-proxies: true
  url: https://example.com/generate_204
  interval: 300
  lazy: true
proxy-groups:
  - name: HK
    <<: *common
    interval: 600
    filter: HK
  - name: Auto
    <<: [*common, {tolerance: 50}]
rules: ['MATCH,Auto']
"#,
            )
            .unwrap();
        assert_eq!(source["proxy-groups"][0]["type"], "url-test");
        assert_eq!(source["proxy-groups"][0]["interval"], 600);
        assert_eq!(source["proxy-groups"][1]["interval"], 300);
        assert_eq!(source["proxy-groups"][1]["tolerance"], 50);
        assert!(source["proxy-groups"][0].get("<<").is_none());
        let serialized = serde_json::to_string(&source).unwrap();
        assert_eq!(Mihomo.normalize(&serialized).unwrap(), source);
        assert!(Mihomo
            .normalize("proxies: []\nproxy-groups: [{<<: invalid}]")
            .unwrap_err()
            .contains("merge"));
    }
    #[test]
    fn imported_configuration_cannot_own_listeners_routes_or_paths() {
        let a = Mihomo;
        let source=a.normalize("proxies: [{name: Example, type: socks5, server: example.com, port: 1080}]\nexternal-controller: 0.0.0.0:9090\nsecret: unsafe\ntun: {auto-route: true}\nproxy-providers: {remote: {type: http, url: 'https://example.com/config', path: /etc/passwd}}\n").unwrap();
        let runtime = a.runtime(&source, &Settings::default(), "private-controller");
        assert_eq!(runtime["external-controller"], "127.0.0.1:19092");
        assert_eq!(runtime["tun"]["auto-route"], false);
        assert_eq!(runtime["secret"], "private-controller");
        assert_eq!(
            runtime["dns"]["nameserver-policy"]["+.home.arpa"][0],
            "udp://127.0.0.1:53"
        );
        assert_eq!(runtime["dns"]["listen"], "127.0.0.1:1053");
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
    #[test]
    fn preserves_native_dns_sniffer_and_protocol_options() {
        let a = Mihomo;
        let source = a.normalize("proxies: [{name: Example, type: socks5, server: example.com, port: 1080}]\ndns: {nameserver: ['https://resolver.example/dns-query'], enhanced-mode: fake-ip, fake-ip-filter: ['*.example']}\nsniffer: {enable: true, sniff: {TLS: {ports: [443]}}}\nunified-delay: true\n").unwrap();
        let runtime = a.runtime(&source, &Settings::default(), "private");
        assert_eq!(runtime["dns"]["nameserver"], source["dns"]["nameserver"]);
        assert_eq!(runtime["dns"]["enhanced-mode"], "fake-ip");
        assert_eq!(runtime["sniffer"], source["sniffer"]);
        assert_eq!(runtime["unified-delay"], true);
    }
    #[test]
    fn legacy_adapter_settings_load_without_disabling_service() {
        let s: Settings =
            serde_json::from_value(json!({"adapter":"compatible-v1","enabled":true})).unwrap();
        assert!(s.validate().is_ok());
        assert!(s.enabled);
        assert_eq!(s.mixed_port, 7890);
    }
}
