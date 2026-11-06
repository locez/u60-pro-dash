use super::*;

impl MihomoService {
    pub(super) fn update_settings(self: &Arc<Self>, body: &Value) -> Result<(), String> {
        let previous = self.settings.lock().unwrap().clone();
        let mut next = previous.clone();
        for (key, target) in [("mode", &mut next.mode), ("capture", &mut next.capture)] {
            if let Some(v) = body.get(key) {
                *target = v.as_str().ok_or("Invalid proxy setting")?.into();
            }
        }
        if let Some(id) = body.get("adapter") {
            if !matches!(id.as_str(), Some("mihomo" | "compatible-v1")) {
                return Err("Manage each proxy service separately".into());
            }
        }
        if let Some(v) = body.get("mixed_port") {
            next.mixed_port = v
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or("Invalid mixed port")?;
        }
        if let Some(v) = body.get("ipv6") {
            next.ipv6 = v.as_bool().ok_or("Invalid IPv6 setting")?;
        }
        if let Some(v) = body.get("tcp_concurrent") {
            next.tcp_concurrent = v.as_bool().ok_or("Invalid TCP setting")?;
        }
        next.adapter = "mihomo".into();
        next.validate()?;
        let running = self.runtime.lock().unwrap().child.is_some();
        if !running {
            self.save(&next)?;
            *self.settings.lock().unwrap() = next;
            return Ok(());
        }
        let mode_only = previous.capture == next.capture
            && previous.mixed_port == next.mixed_port
            && previous.ipv6 == next.ipv6
            && previous.tcp_concurrent == next.tcp_concurrent;
        if mode_only && body["restart"] != true {
            if next.mode == previous.mode {
                return Ok(());
            }
            // The controller supports mode changes without replacing the process,
            // route ownership or active connections. Persist only after readback.
            let apply = |mode: &str| -> Result<(), String> {
                self.controller(
                    reqwest::Method::PATCH,
                    "/configs",
                    Some(json!({"mode":mode})),
                )?;
                let actual = self.controller(reqwest::Method::GET, "/configs", None)?;
                if actual["mode"] != mode {
                    return Err("The controller has not confirmed the requested mode".into());
                }
                Ok(())
            };
            if let Err(cause) = apply(&next.mode).and_then(|()| self.save(&next)) {
                return Err(if apply(&previous.mode).is_ok() {
                    format!(
                        "Mode change could not be applied; the previous mode was restored. {cause}"
                    )
                } else {
                    format!("Mode change could not be confirmed. Read the service state before retrying. {cause}")
                });
            }
            *self.settings.lock().unwrap() = next;
            return Ok(());
        }
        if body["restart"] != true {
            return Err(
                "Confirm save and restart to change runtime settings while this service is running"
                    .into(),
            );
        }
        if previous.mixed_port != next.mixed_port {
            let _port = std::net::TcpListener::bind(("127.0.0.1", next.mixed_port))
                .map_err(|_| "The requested proxy port is already in use")?;
        }
        // Reject invalid configurations before interrupting a working process.
        let work = geodata::Workspace::new(&self.root)?;
        let path = work.path.join("settings-check.json");
        let driver = engine::adapter(&next.adapter)?;
        private_write(
            &path,
            &serde_json::to_vec(&driver.runtime(&self.source()?, &next, "validation-only"))
                .unwrap(),
            0o600,
        )?;
        validation::run_prepared(
            driver.as_ref(),
            &self.root.join("engine"),
            &self.root,
            &path,
            false,
            false,
        )?;
        let lease = self
            .runtime
            .lock()
            .unwrap()
            .lock
            .as_ref()
            .ok_or("Service lease missing")?
            .try_clone()
            .map_err(|_| "Cannot retain service lease")?;
        let retain = || -> Result<(), String> {
            self.runtime.lock().unwrap().lock = Some(
                lease
                    .try_clone()
                    .map_err(|_| "Cannot retain service lease")?,
            );
            Ok(())
        };
        self.stop();
        retain()?;
        *self.settings.lock().unwrap() = next.clone();
        let result = self.start().and_then(|()| self.save(&next));
        if let Err(cause) = result {
            self.stop();
            *self.settings.lock().unwrap() = previous.clone();
            let saved = self.save(&previous);
            let restored = retain().and_then(|()| self.start());
            return Err(if saved.is_ok() && restored.is_ok() {
                format!("New settings could not be applied; the previous settings and service were restored. {cause}")
            } else {
                format!("New settings could not be applied; previous settings were retained but service recovery could not be confirmed. {cause}")
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a host agent build via U60_AGENT_TEST_BINARY; uses a fake engine"]
    fn running_settings_restart_and_restore_after_a_failed_start() {
        assert!(std::env::var_os("U60_AGENT_TEST_BINARY").is_some());
        let root = std::env::temp_dir().join(format!("u60-settings-live-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let service = Arc::new(MihomoService::at(root.clone()));
        struct Cleanup(Arc<MihomoService>, PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                self.0.stop();
                let _ = fs::remove_dir_all(&self.1);
            }
        }
        let _cleanup = Cleanup(Arc::clone(&service), root.clone());
        private_write(
            &root.join("source.json"),
            br#"{"proxies":[],"rules":["MATCH,DIRECT"]}"#,
            0o600,
        )
        .unwrap();
        private_write(
            &root.join("engine"),
            br#"#!/usr/bin/python3
import sys, json
from http.server import BaseHTTPRequestHandler, HTTPServer
if '-t' in sys.argv: sys.exit(0)
config = json.load(open(sys.argv[sys.argv.index('-f') + 1]))
if config['mode'] == 'global':
    print('fixture global start rejected', flush=True)
    sys.exit(1)
class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(json.dumps({'mode': config['mode']}).encode())
    def do_PATCH(self):
        value = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        config['mode'] = value['mode']
        self.send_response(204)
        self.end_headers()
    def log_message(self, *args): pass
class Server(HTTPServer): allow_reuse_address = True
Server(('127.0.0.1', 19092), Handler).serve_forever()
"#,
            0o700,
        )
        .unwrap();
        service.change("enable", &json!({})).unwrap();
        let first_pid = service.runtime.lock().unwrap().child.as_ref().unwrap().id();
        service
            .change("settings", &json!({"mode":"direct"}))
            .unwrap();
        assert_eq!(
            service.runtime.lock().unwrap().child.as_ref().unwrap().id(),
            first_pid
        );
        assert_eq!(service.status()["mode"], "direct");
        assert_eq!(MihomoService::at(root.clone()).status()["mode"], "direct");
        let ipv6 = service.settings.lock().unwrap().ipv6;
        assert!(service.change("settings", &json!({"ipv6":!ipv6})).is_err());
        service
            .change("settings", &json!({"mode":"direct","restart":true}))
            .unwrap();
        assert_ne!(
            service.runtime.lock().unwrap().child.as_ref().unwrap().id(),
            first_pid
        );
        assert_eq!(service.status()["mode"], "direct");
        assert_eq!(MihomoService::at(root.clone()).status()["mode"], "direct");
        let error = service
            .change("settings", &json!({"mode":"global","restart":true}))
            .unwrap_err();
        assert!(
            error.contains("previous settings and service were restored"),
            "{error}"
        );
        assert_eq!(service.status()["mode"], "direct");
        assert_eq!(service.status()["running"], true);
        assert_eq!(MihomoService::at(root.clone()).status()["mode"], "direct");
        assert!(service.logs().unwrap()["text"]
            .as_str()
            .unwrap()
            .contains("fixture global start rejected"));
    }
    #[test]
    fn stopped_settings_are_saved_without_starting_and_failed_writes_preserve_memory() {
        let root = std::env::temp_dir().join(format!("u60-settings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let service = Arc::new(MihomoService::at(root.clone()));
        service
            .change("settings", &json!({"capture":"hotspot"}))
            .unwrap();
        assert_eq!(service.status()["capture"], "hotspot");
        assert_eq!(service.status()["running"], false);
        assert_eq!(
            MihomoService::at(root.clone()).status()["capture"],
            "hotspot"
        );
        assert!(service
            .change("settings", &json!({"mixed_port":19092}))
            .is_err());
        fs::remove_file(root.join("settings.json")).unwrap();
        fs::create_dir(root.join("settings.json")).unwrap();
        assert!(service
            .change("settings", &json!({"capture":"local"}))
            .is_err());
        assert_eq!(service.status()["capture"], "hotspot");
        fs::remove_dir_all(root).unwrap();
    }
}
