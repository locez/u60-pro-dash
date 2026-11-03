//! Independently owned services. There is no selected engine or global running flag.
use super::MihomoService;
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::Arc};

// New engines add their own implementation, settings and private storage here.
// This enum dispatches operations; it does not prescribe a shared configuration schema.
pub(super) enum Service {
    Mihomo(Arc<MihomoService>),
}
impl Service {
    pub fn logs(&self) -> Result<Value, String> {
        match self {
            Self::Mihomo(s) => s.logs(),
        }
    }
    pub fn status(&self) -> Value {
        match self {
            Self::Mihomo(s) => s.status(),
        }
    }
    pub fn auto_start(&self) {
        match self {
            Self::Mihomo(s) => s.auto_start(),
        }
    }
    pub fn change(&self, action: &str, body: &Value) -> Result<Value, String> {
        match self {
            Self::Mihomo(s) => s.change(action, body),
        }
    }
    pub fn config(&self) -> Result<Value, String> {
        match self {
            Self::Mihomo(s) => s.configuration(),
        }
    }
    pub fn groups(&self) -> Result<Value, String> {
        match self {
            Self::Mihomo(s) => super::engine::adapter("mihomo").and_then(|d| {
                s.controller(reqwest::Method::GET, d.groups_path(), None)
                    .map(|v| d.groups(&v))
            }),
        }
    }
}

pub struct ProxyServices {
    services: BTreeMap<&'static str, Service>,
}
impl ProxyServices {
    pub fn new() -> Self {
        // Retain the existing service directory so upgrades preserve its private data.
        Self {
            services: BTreeMap::from([("mihomo", Service::Mihomo(Arc::new(MihomoService::new())))]),
        }
    }
    pub(super) fn get(&self, id: &str) -> Option<&Service> {
        self.services.get(id)
    }
    pub(crate) fn legacy(&self) -> &Arc<MihomoService> {
        match self.get("mihomo").unwrap() {
            Service::Mihomo(service) => service,
        }
    }
    pub fn status(&self) -> Value {
        json!({"services": self.services.values().map(|m| m.status()).collect::<Vec<_>>()})
    }
    pub fn auto_start(&self) {
        for service in self.services.values() {
            service.auto_start();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_services_do_not_share_configuration_or_lifecycle_leases() {
        let root = std::env::temp_dir().join(format!("u60-services-{}", std::process::id()));
        let a = MihomoService::at(root.join("one"));
        let b = MihomoService::at(root.join("two"));
        a.prepare().unwrap();
        b.prepare().unwrap();
        a.lease().unwrap();
        b.lease().unwrap();
        assert!(a.runtime.lock().unwrap().lock.is_some());
        assert!(b.runtime.lock().unwrap().lock.is_some());
        a.stop();
        assert!(b.runtime.lock().unwrap().lock.is_some());
        assert!(!b.root.join("settings.json").exists());
        b.stop();
        std::fs::remove_dir_all(root).unwrap();
    }
}
