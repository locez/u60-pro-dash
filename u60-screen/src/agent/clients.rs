//! Count actual connected clients, not stale DHCP/ARP host hints.
use super::{Agent, Error, Request};
use serde_json::Value;
use std::time::{Duration, Instant};
pub struct Clients {
    next: Instant,
    pending: Option<(Instant, u64)>,
    sequence: u64,
    sample: Option<(Instant, usize)>,
}
impl Default for Clients {
    fn default() -> Self {
        Self {
            next: Instant::now(),
            pending: None,
            sequence: 1 << 31,
            sample: None,
        }
    }
}
impl Clients {
    pub fn tick(&mut self, agent: &Agent, active: bool) {
        if !active {
            self.pending = None;
            self.sample = None;
            self.next = Instant::now();
            return;
        }
        if self
            .pending
            .is_some_and(|(at, _)| at.elapsed() > Duration::from_secs(35))
        {
            self.pending = None;
            self.sample = None;
        }
        if self.pending.is_none() && Instant::now() >= self.next {
            self.sequence = self.sequence.saturating_add(1);
            let id = self.sequence;
            self.pending = Some((Instant::now(), id));
            self.next = Instant::now() + Duration::from_secs(10);
            if let Err(e) = agent.call(id, Request::get("/api/network/client-details")) {
                self.receive(id, Err(e));
            }
        }
    }
    pub fn receive(&mut self, id: u64, result: Result<Value, Error>) -> bool {
        if self.pending.is_none_or(|(_, expected)| expected != id) {
            return false;
        }
        self.pending = None;
        self.sample = result
            .as_ref()
            .ok()
            .filter(|v| v["inventory_complete"] == true)
            .and_then(|v| v["devices"].as_array())
            .map(|a| {
                (
                    Instant::now(),
                    a.iter().filter(|v| v["connected"] == true).count(),
                )
            });
        if let Err(Error::Busy(seconds)) = result {
            self.next = Instant::now() + Duration::from_secs(seconds.clamp(1, 5));
        }
        true
    }
    pub fn label(&self) -> String {
        self.sample
            .as_ref()
            .filter(|(at, _)| at.elapsed() < Duration::from_secs(30))
            .map(|(_, n)| n.to_string())
            .unwrap_or("—".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn host_hints_and_incomplete_inventory_are_not_a_zero_count() {
        let mut c = Clients {
            pending: Some((Instant::now(), 2)),
            ..Default::default()
        };
        c.receive(2, Ok(
            json!({"inventory_complete":true,"devices":[{"connected":true},{"connected":false}]}),
        ));
        assert_eq!(c.label(), "1");
        c.pending = Some((Instant::now(), 3));
        c.receive(
            3,
            Ok(json!({"hosts":{"old":{}},"inventory_complete":false,"devices":[]})),
        );
        assert_eq!(c.label(), "—");
        c.pending = Some((Instant::now(), 4));
        assert!(!c.receive(3, Ok(json!({"inventory_complete":true,"devices":[]}))));
        assert_eq!(c.pending.unwrap().1, 4);
    }
}
