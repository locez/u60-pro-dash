//! Saved configuration is the source of truth. Validate before applying it,
//! retain the controller credential on reload, and verify persistence before
//! reporting success. The service operation lock surrounds all writes.
use super::*;

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Cannot read saved proxy configuration".into()),
    }
}
fn revision(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
struct Candidate(PathBuf);
impl Drop for Candidate {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

impl MihomoService {
    pub(super) fn configuration_revision(&self) -> Result<Option<String>, String> {
        Ok(read_optional(&self.root.join("source.json"))?.map(|bytes| revision(&bytes)))
    }
    pub(super) fn configuration(&self) -> Result<Value, String> {
        let Some(bytes) = read_optional(&self.root.join("source.json"))? else {
            return Ok(json!({"configured":false,"config":"","revision":null}));
        };
        let source: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Saved configuration is invalid")?;
        let config =
            serde_yaml::to_string(&source).map_err(|_| "Cannot read saved configuration")?;
        Ok(json!({"configured":true,"config":config,"revision":revision(&bytes)}))
    }
    fn check_revision(&self, expected: Option<&Value>) -> Result<(), String> {
        if let Some(expected) = expected {
            let expected = if expected.is_null() {
                None
            } else {
                Some(
                    expected
                        .as_str()
                        .filter(|v| v.len() == 64 && v.bytes().all(|c| c.is_ascii_hexdigit()))
                        .ok_or("Invalid configuration revision")?
                        .to_string(),
                )
            };
            if self.configuration_revision()? != expected {
                return Err("Configuration changed since it was read. Load the current configuration before applying this draft".into());
            }
        }
        Ok(())
    }
    pub(super) fn apply_configuration(
        &self,
        text: &str,
        expected: Option<&Value>,
        validate_only: bool,
    ) -> Result<(), String> {
        self.check_revision(expected)?;
        if !self.root.join("engine").is_file() {
            return Err(
                "Install the proxy engine before validating or applying configuration".into(),
            );
        }
        let settings = self.settings.lock().unwrap().clone();
        let driver = engine::adapter(&settings.adapter)?;
        let source = driver.normalize(text)?;
        let (running, secret) = {
            let runtime = self.runtime.lock().unwrap();
            (runtime.child.is_some(), runtime.secret.clone())
        };
        let runtime = driver.runtime(
            &source,
            &settings,
            if running { &secret } else { "validation-only" },
        );
        let runtime = serde_json::to_vec(&runtime).map_err(|_| "Invalid runtime configuration")?;
        let candidate = Candidate(self.root.join("candidate.json"));
        private_write(&candidate.0, &runtime, 0o600)?;
        validation::run_prepared(
            driver.as_ref(),
            &self.root.join("engine"),
            &self.root,
            &candidate.0,
            false,
            !running,
        )?;
        if validate_only {
            return Ok(());
        }
        // An external editor may have changed the file during validation.
        self.check_revision(expected)?;
        let source = serde_json::to_vec(&source).map_err(|_| "Invalid configuration")?;
        commit(
            &self.root,
            &candidate.0,
            &source,
            running.then_some(runtime.as_slice()),
            |path| {
                self.controller(
                    reqwest::Method::PUT,
                    "/configs?force=true",
                    Some(json!({"path":path})),
                )
                .map(|_| ())
            },
        )
    }
    pub(super) fn prepare_geodata(
        &self,
        text: &str,
        expected: Option<&Value>,
    ) -> Result<(), String> {
        self.check_revision(expected)?;
        let settings = self.settings.lock().unwrap().clone();
        let driver = engine::adapter(&settings.adapter)?;
        let source = driver.normalize(text)?;
        let runtime = driver.runtime(&source, &settings, "validation-only");
        let candidate = Candidate(self.root.join("candidate.json"));
        private_write(
            &candidate.0,
            &serde_json::to_vec(&runtime).map_err(|_| "Invalid runtime configuration")?,
            0o600,
        )?;
        validation::run_prepared(
            driver.as_ref(),
            &self.root.join("engine"),
            &self.root,
            &candidate.0,
            true,
            true,
        )
    }
}

fn restore(path: &Path, bytes: Option<&[u8]>) -> Result<(), String> {
    match bytes {
        Some(bytes) => private_write(path, bytes, 0o600),
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("Cannot remove uncommitted configuration".into()),
        },
    }
}

fn commit(
    root: &Path,
    candidate: &Path,
    source: &[u8],
    runtime: Option<&[u8]>,
    mut reload: impl FnMut(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let source_path = root.join("source.json");
    let runtime_path = root.join("runtime.json");
    let previous_source = read_optional(&source_path)?;
    let previous_runtime = if runtime.is_some() {
        Some(
            fs::read(&runtime_path)
                .map_err(|_| "Cannot preserve the active configuration; reload cancelled")?,
        )
    } else {
        None
    };
    if runtime.is_some() && reload(candidate).is_err() {
        return Err(if reload(&runtime_path).is_ok() {
            "Reload failed; the previous configuration was restored"
        } else {
            "Reload failed and restoration could not be confirmed. The previous saved configuration is retained; check or restart this service"
        }.into());
    }
    let saved = (|| {
        private_write(&source_path, source, 0o600)?;
        if let Some(runtime) = runtime {
            private_write(&runtime_path, runtime, 0o600)?;
        }
        if fs::read(&source_path).ok().as_deref() != Some(source) {
            return Err("Saved configuration could not be verified".into());
        }
        Ok(())
    })();
    if saved.is_err() {
        let source_restored = restore(&source_path, previous_source.as_deref());
        let runtime_restored = previous_runtime
            .as_ref()
            .map_or(Ok(()), |previous| restore(&runtime_path, Some(previous)));
        let reapplied = if runtime.is_some() && runtime_restored.is_ok() {
            reload(&runtime_path)
        } else {
            Ok(())
        };
        return Err(if source_restored.is_ok() && runtime_restored.is_ok() && reapplied.is_ok() {
            "Configuration could not be saved; the previous configuration was restored"
        } else {
            "Configuration save failed and restoration could not be confirmed. Check the service and device storage"
        }.into());
    }
    saved
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("u60-config-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }
    #[test]
    fn reload_keeps_old_files_until_accepted_and_rolls_back_a_lost_reply() {
        let root = root("reload");
        fs::write(root.join("source.json"), b"old source").unwrap();
        fs::write(root.join("runtime.json"), b"old runtime").unwrap();
        let candidate = root.join("candidate.json");
        let mut calls = Vec::new();
        let error = commit(
            &root,
            &candidate,
            b"new source",
            Some(b"new runtime"),
            |path| {
                calls.push(path.to_path_buf());
                assert_eq!(fs::read(root.join("source.json")).unwrap(), b"old source");
                if path == candidate {
                    Err("reply lost".into())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert!(error.contains("previous configuration was restored"));
        assert_eq!(calls, [candidate.clone(), root.join("runtime.json")]);
        assert_eq!(fs::read(root.join("runtime.json")).unwrap(), b"old runtime");
        commit(
            &root,
            &candidate,
            b"new source",
            Some(b"new runtime"),
            |path| {
                assert_eq!(path, candidate);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(fs::read(root.join("source.json")).unwrap(), b"new source");
        assert_eq!(fs::read(root.join("runtime.json")).unwrap(), b"new runtime");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn saving_stopped_service_round_trips_and_rejects_stale_editors() {
        let root = root("stopped");
        let service = MihomoService::at(root.clone());
        assert_eq!(
            service.configuration().unwrap(),
            json!({"configured":false,"config":"","revision":null})
        );
        service.check_revision(Some(&Value::Null)).unwrap();
        let source = br#"{"proxies":[{"name":"Example","type":"socks5","server":"example.com","port":1080}]}"#;
        commit(&root, &root.join("candidate.json"), source, None, |_| {
            panic!("a stopped service must stay stopped")
        })
        .unwrap();
        let saved = service.configuration().unwrap();
        assert_eq!(saved["configured"], true);
        assert!(saved["config"].as_str().unwrap().contains("Example"));
        service.check_revision(Some(&saved["revision"])).unwrap();
        assert!(service.check_revision(Some(&Value::Null)).is_err());
        assert!(service
            .check_revision(Some(&json!("0".repeat(64))))
            .is_err());
        assert_eq!(
            fs::metadata(root.join("source.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(!root.join("runtime.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn validator_failure_keeps_saved_configuration_and_removes_candidate() {
        let root = root("invalid");
        let service = MihomoService::at(root.clone());
        private_write(&root.join("engine"), b"#!/bin/sh\nexit 1\n", 0o700).unwrap();
        fs::write(root.join("source.json"), b"{}").unwrap();
        let source = "proxies: [{name: Example, type: socks5, server: example.com, port: 1080}]";
        assert!(service.apply_configuration(source, None, false).is_err());
        assert_eq!(fs::read(root.join("source.json")).unwrap(), b"{}");
        assert!(!root.join("candidate.json").exists());
        private_write(&root.join("engine"), b"#!/bin/sh\nexit 0\n", 0o700).unwrap();
        service.apply_configuration(source, None, true).unwrap();
        assert_eq!(fs::read(root.join("source.json")).unwrap(), b"{}");
        assert!(!root.join("runtime.json").exists());
        assert!(!root.join("candidate.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
