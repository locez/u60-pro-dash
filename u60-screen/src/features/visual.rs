//! Presentation for measured data. Settings and actions retain their contracts.
use super::{Data, Page, data, selected, shown, status::number};

#[derive(Default, PartialEq, Eq)]
pub struct Table {
    pub headings: [String; 3],
    pub rows: Vec<TableRow>,
    pub replaces_items: bool,
}
#[derive(PartialEq, Eq)]
pub struct TableRow {
    pub id: String,
    pub name: String,
    pub detail: String,
    pub first: String,
    pub second: String,
    pub target: String,
}
impl TableRow {
    fn detail(mut self, text: impl Into<String>) -> Self {
        self.detail = text.into();
        self
    }
}
#[derive(PartialEq, Eq)]
pub struct Meter {
    pub id: String,
    pub label: String,
    pub value: String,
    pub permille: i32,
}
fn num(v: &serde_json::Value, divisor: f64) -> String {
    number(v)
        .map(|n| format!("{:.1}", n / divisor))
        .unwrap_or_else(|| "—".into())
}
pub(super) fn row(
    id: impl Into<String>,
    name: impl Into<String>,
    first: String,
    second: String,
    target: impl Into<String>,
) -> TableRow {
    TableRow {
        id: id.into(),
        name: name.into(),
        detail: String::new(),
        first,
        second,
        target: target.into(),
    }
}
pub fn apply(id: &str, d: &Data, p: &mut Page) {
    match id {
        "usage" => {
            let settings = data(d, "settings");
            p.table.headings = [crate::i18n::tr("流量").into(), "↓ GB".into(), "↑ GB".into()];
            p.table.rows = [
                ("day", crate::i18n::tr("今日")),
                ("month", crate::i18n::tr("本月")),
                ("total", crate::i18n::tr("累计")),
            ]
            .into_iter()
            .map(|(key, label)| {
                row(
                    key,
                    label,
                    num(&settings[key]["rx_bytes"], 1e9),
                    num(&settings[key]["tx_bytes"], 1e9),
                    "",
                )
            })
            .collect();
            p.rows.clear();
        }
        "carriers" if selected(d).is_none() => {
            p.table.headings = [crate::i18n::tr("频段").into(), "PCI".into(), "MHz".into()];
            p.table.replaces_items = true;
            for (key, _, rows) in super::status::carriers(data(d, "signal")) {
                let parts: Vec<_> = key.split(':').collect();
                if parts.len() == 4 {
                    p.table.rows.push(
                        row(
                            key.clone(),
                            format!("{} {}", parts[0], parts[1]),
                            parts[2].into(),
                            rows[1].1.clone(),
                            key.clone(),
                        )
                        .detail(format!("ARFCN {}", parts[3])),
                    );
                }
            }
        }
        "sms-storage" => {
            let capacity = data(d, "capacity");
            for (id, label, used, total) in [
                (
                    "device",
                    crate::i18n::tr("设备"),
                    "sms_nvused_total",
                    "sms_nv_total",
                ),
                ("sim", "SIM", "sms_simused_total", "sms_sim_total"),
            ] {
                let ratio = number(&capacity[used])
                    .zip(number(&capacity[total]))
                    .filter(|(u, t)| *t > 0. && *u >= 0. && u <= t);
                p.meters.push(Meter {
                    id: id.into(),
                    label: label.into(),
                    value: ratio
                        .map(|(u, t)| format!("{:.0}%", u / t * 100.))
                        .unwrap_or_else(|| "—".into()),
                    permille: ratio.map_or(-1, |(u, t)| (u / t * 1000.).round() as i32),
                });
            }
        }
        "resources" => {
            if let Some(cores) = data(d, "cpu")["cores"].as_array() {
                p.meters = cores
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let value = number(v).filter(|n| (0.0..=100.0).contains(n));
                        Meter {
                            id: format!("cpu-{i}"),
                            label: crate::i18n::format("核心 {0}", &[(i + 1).to_string()]),
                            value: value
                                .map(|v| format!("{v:.0}%"))
                                .unwrap_or_else(|| "—".into()),
                            permille: value.map_or(-1, |v| (v * 10.).round() as i32),
                        }
                    })
                    .collect();
            }
        }
        "traffic" => {
            p.table.headings = [crate::i18n::tr("接口").into(), "↓ MB".into(), "↑ MB".into()];
            if let Some(list) = data(d, "interfaces").as_array() {
                p.table.rows = list
                    .iter()
                    .map(|v| {
                        row(
                            shown(&v["name"]),
                            shown(&v["name"]),
                            num(&v["rx_bytes"], 1e6),
                            num(&v["tx_bytes"], 1e6),
                            "",
                        )
                    })
                    .collect();
            }
        }
        "processes" if selected(d).is_none() => {
            p.table.headings = [crate::i18n::tr("进程").into(), "CPU %".into(), "MiB".into()];
            p.table.replaces_items = true;
            if let Some(list) = data(d, "processes")["processes"].as_array() {
                p.table.rows = list
                    .iter()
                    .map(|v| {
                        row(
                            shown(&v["pid"]),
                            shown(&v["name"]),
                            num(&v["cpu_pct"], 1.),
                            num(&v["rss_kb"], 1024.),
                            shown(&v["pid"]),
                        )
                        .detail(format!("PID {}", shown(&v["pid"])))
                    })
                    .collect();
            }
        }
        "clients" if selected(d).is_none() => {
            p.table.headings = [
                crate::i18n::tr("设备名称").into(),
                "↓ Mbps".into(),
                "↑ Mbps".into(),
            ];
            p.table.replaces_items = true;
            if let Some(list) = data(d, "clients")["devices"].as_array() {
                p.table.rows = list
                    .iter()
                    .filter(|v| v["mac"].is_string())
                    .map(|v| {
                        let name = v["name"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| v["mac"].as_str().unwrap_or("—"));
                        row(
                            shown(&v["mac"]),
                            name,
                            num(&v["traffic"]["download_bps"], 1e6),
                            num(&v["traffic"]["upload_bps"], 1e6),
                            shown(&v["mac"]),
                        )
                        .detail(format!(
                            "{} · {}",
                            shown(&v["ipv4"]),
                            shown(&v["band"])
                        ))
                    })
                    .collect();
            }
        }
        "neighbors" => {
            p.table.headings = [crate::i18n::tr("频段").into(), "PCI".into(), "dBm".into()];
            for (key, name) in [("nr", "NR"), ("lte", "LTE")] {
                if let Some(list) = data(d, key)["cell_list"].as_array() {
                    for (i, v) in list.iter().enumerate() {
                        p.table.rows.push(
                            row(
                                format!("{key}-{}-{}-{i}", shown(&v["pci"]), shown(&v["earfcn"])),
                                format!("{name} {}", shown(&v["band"])),
                                shown(&v["pci"]),
                                num(&v["rsrp"], 1.),
                                "",
                            )
                            .detail(format!("ARFCN {}", shown(&v["earfcn"]))),
                        );
                    }
                }
            }
            p.rows.clear();
        }
        _ => {}
    }
}
