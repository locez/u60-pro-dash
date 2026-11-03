//! Bounded observations for this screen process; never persistent personal logs.
use super::*;
use std::{collections::VecDeque, time::Instant};
pub struct History {
    start: Instant,
    last: BTreeMap<&'static str, String>,
    entries: VecDeque<(String, String)>,
}
impl Default for History {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            last: BTreeMap::new(),
            entries: VecDeque::new(),
        }
    }
}
impl History {
    pub fn observe(&mut self, s: &crate::agent::Snapshot) -> bool {
        if matches!(
            s.status,
            crate::agent::Status::Sleeping | crate::agent::Status::Paused
        ) {
            return false;
        }
        let mut values = vec![(crate::i18n::tr("后台连接"), s.status.label().to_string())];
        if let Some(signal) = s.get("signal")
            && let Some(kind) = signal["network_type"].as_str().filter(|s| !s.is_empty())
        {
            values.push((crate::i18n::tr("蜂窝网络类型"), kind.into()));
        }
        let mut changed = false;
        for (k, v) in values {
            if self.last.get(k) == Some(&v) {
                continue;
            }
            let prior = self.last.insert(k, v.clone());
            let seconds = self.start.elapsed().as_secs();
            self.entries.push_front((
                crate::i18n::format(
                    "运行 {0}分{1}秒 · {2}",
                    &[
                        format!("{}", seconds / 60),
                        format!("{:02}", seconds % 60),
                        k.to_string(),
                    ],
                ),
                prior.map(|old| format!("{old} → {v}")).unwrap_or(v),
            ));
            self.entries.truncate(100);
            changed = true;
        }
        changed
    }
    pub fn value(&self) -> Value {
        serde_json::json!(self.entries)
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
pub fn page(d: &Data) -> Page {
    let mut p = Page::default();
    if let Some(entries) = data(d, "__history").as_array() {
        p.rows = entries.iter().map(|e| (text(&e[0]), text(&e[1]))).collect();
    }
    p.note=crate::i18n::tr("记录本次屏幕程序运行期间观察到的变化，最多保留 100 条。熄屏时暂停订阅，唤醒后记录当前状态；后台连接中断并不等于蜂窝网络断开。重启屏幕程序会清空记录。").into();
    p.actions.push(Action::new(
        "clear-local",
        crate::i18n::tr("清空本次记录"),
        "",
    ));
    p
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_stream_samples_do_not_duplicate_history() {
        let mut h = History::default();
        let mut s = crate::agent::Snapshot::default();
        assert!(h.observe(&s));
        assert!(!h.observe(&s));
        for i in 0..110 {
            s.status = if i % 2 == 0 {
                crate::agent::Status::Live
            } else {
                crate::agent::Status::Reconnecting
            };
            h.observe(&s);
        }
        assert_eq!(h.entries.len(), 100);
        h.clear();
        assert_eq!(h.entries.len(), 0);
        assert!(!h.observe(&s));
    }
}
