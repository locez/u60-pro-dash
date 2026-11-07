//! Device operating controls. Reads never change hardware. Writes are bounded,
//! verified, and restore the previous CPU policy if applying or persisting fails.
use crate::{
    events::{self, Topic},
    handlers::{self, AppState},
    ubus,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    Balanced,
    Performance,
    Saving,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Setting {
    id: String,
    governor: String,
    min_khz: u64,
    max_khz: u64,
}
struct Policy {
    current: Setting,
    governors: Vec<String>,
    frequencies: Vec<u64>,
    hardware_min: u64,
    hardware_max: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    version: u8,
    profile: Profile,
    baseline: Vec<Setting>,
}

pub struct Performance {
    cpu: PathBuf,
    thermal: PathBuf,
    saved: PathBuf,
    lock: Mutex<()>,
}
impl Default for Performance {
    fn default() -> Self {
        Self {
            cpu: "/sys/devices/system/cpu/cpufreq".into(),
            thermal: "/sys/class/thermal".into(),
            saved: "/data/u60-dash/performance.json".into(),
            lock: Mutex::new(()),
        }
    }
}
fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|s| s.trim().to_owned())
        .map_err(|_| {
            format!(
                "Cannot read {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )
        })
}
fn integer(path: &Path) -> Result<u64, String> {
    read(path)?
        .parse()
        .map_err(|_| "Invalid CPU policy value".into())
}
fn policies(base: &Path) -> Result<Vec<Policy>, String> {
    let mut dirs: Vec<_> = fs::read_dir(base)
        .map_err(|_| "CPU policy control is unavailable")?
        .filter_map(Result::ok)
        .filter(|p| {
            p.file_name()
                .to_string_lossy()
                .strip_prefix("policy")
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        })
        .collect();
    dirs.sort_by_key(|d| d.file_name());
    if dirs.is_empty() || dirs.len() > 16 {
        return Err("CPU policy control is unavailable".into());
    }
    dirs.iter()
        .map(|dir| {
            let root = dir.path();
            if read(&root.join("scaling_driver"))? != "qcom-cpufreq-hw" {
                return Err("This CPU driver has no supported performance profile".into());
            }
            let current = Setting {
                id: dir.file_name().to_string_lossy().into_owned(),
                governor: read(&root.join("scaling_governor"))?,
                min_khz: integer(&root.join("scaling_min_freq"))?,
                max_khz: integer(&root.join("scaling_max_freq"))?,
            };
            let governors = read(&root.join("scaling_available_governors"))?
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if ["schedutil", "performance", "conservative"]
                .iter()
                .any(|g| !governors.iter().any(|v| v == g))
            {
                return Err("Required CPU policies are unavailable".into());
            }
            let mut frequencies = read(&root.join("scaling_available_frequencies"))?
                .split_whitespace()
                .map(str::parse::<u64>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "Invalid CPU frequencies")?;
            frequencies.sort_unstable();
            frequencies.dedup();
            let p = Policy {
                current,
                governors,
                frequencies,
                hardware_min: integer(&root.join("cpuinfo_min_freq"))?,
                hardware_max: integer(&root.join("cpuinfo_max_freq"))?,
            };
            validate(&p.current, &p)?;
            if p.frequencies.is_empty()
                || p.frequencies
                    .iter()
                    .any(|f| *f < p.hardware_min || *f > p.hardware_max)
            {
                return Err("Invalid CPU frequencies".into());
            }
            Ok(p)
        })
        .collect()
}
fn validate(s: &Setting, p: &Policy) -> Result<(), String> {
    if s.id != p.current.id
        || !p.governors.contains(&s.governor)
        || s.min_khz < p.hardware_min
        || s.max_khz > p.hardware_max
        || s.min_khz > s.max_khz
    {
        return Err("CPU profile does not match this device".into());
    }
    Ok(())
}
fn targets(
    profile: Profile,
    baseline: &[Setting],
    current: &[Policy],
) -> Result<Vec<Setting>, String> {
    if baseline.len() != current.len() {
        return Err("CPU policy layout changed".into());
    }
    baseline
        .iter()
        .zip(current)
        .map(|(b, p)| {
            validate(b, p)?;
            let mut s = b.clone();
            match profile {
                Profile::Balanced => s.governor = "schedutil".into(),
                Profile::Performance => s.governor = "performance".into(),
                Profile::Saving => {
                    s.governor = "conservative".into();
                    let cap = b.max_khz.saturating_mul(70) / 100;
                    s.max_khz = p
                        .frequencies
                        .iter()
                        .copied()
                        .filter(|f| *f <= cap && *f >= b.min_khz)
                        .max()
                        .ok_or("A safe power-saving frequency is unavailable")?;
                }
            }
            validate(&s, p)?;
            Ok(s)
        })
        .collect()
}
fn thermal_limited(root: &Path) -> Option<bool> {
    let mut found = false;
    let mut missing = false;
    for d in fs::read_dir(root).ok()?.filter_map(Result::ok) {
        if !d
            .file_name()
            .to_string_lossy()
            .starts_with("cooling_device")
        {
            continue;
        }
        let Ok(kind) = read(&d.path().join("type")) else {
            continue;
        };
        if !["cpufreq-", "cpu-cluster", "cpu-hotplug"]
            .iter()
            .any(|prefix| kind.starts_with(prefix))
        {
            continue;
        }
        found = true;
        match integer(&d.path().join("cur_state")) {
            Ok(n) if n > 0 => return Some(true),
            Ok(_) => {}
            Err(_) => missing = true,
        }
    }
    (found && !missing).then_some(false)
}
impl Performance {
    fn load(&self) -> Result<Option<Saved>, String> {
        let raw = match fs::read(&self.saved) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("Cannot read saved performance profile".into()),
        };
        if raw.len() > 16_384 {
            return Err("Invalid saved performance profile".into());
        }
        let saved: Saved =
            serde_json::from_slice(&raw).map_err(|_| "Invalid saved performance profile")?;
        if saved.version != 1 {
            return Err("Unsupported performance profile version".into());
        }
        Ok(Some(saved))
    }
    fn store(&self, saved: &Saved) -> Result<(), String> {
        use std::os::unix::fs::OpenOptionsExt;
        let parent = self
            .saved
            .parent()
            .ok_or("Invalid performance state location")?;
        fs::create_dir_all(parent).map_err(|_| "Cannot create performance state directory")?;
        let temp = parent.join(format!(".performance-{}.tmp", std::process::id()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp)
                .map_err(|_| "Cannot prepare performance state")?;
            file.write_all(
                &serde_json::to_vec(saved).map_err(|_| "Cannot encode performance state")?,
            )
            .map_err(|_| "Cannot save performance state")?;
            file.sync_all()
                .map_err(|_| "Cannot synchronize performance state")?;
            fs::rename(&temp, &self.saved).map_err(|_| "Cannot replace performance state")?;
            Ok(())
        })();
        // A prior crash may leave this one harmless staging file; never use it as a profile.
        let _ = fs::remove_file(&temp);
        result
    }
    fn write(&self, desired: &[Setting]) -> Result<(), String> {
        for setting in desired {
            let base = self.cpu.join(&setting.id);
            let min = integer(&base.join("scaling_min_freq"))?;
            // Keep min <= max for every individual sysfs write, including rollback.
            let limits = if setting.max_khz < min {
                [
                    ("scaling_min_freq", setting.min_khz),
                    ("scaling_max_freq", setting.max_khz),
                ]
            } else {
                [
                    ("scaling_max_freq", setting.max_khz),
                    ("scaling_min_freq", setting.min_khz),
                ]
            };
            for (key, value) in limits {
                fs::write(base.join(key), value.to_string())
                    .map_err(|_| format!("Cannot apply {key}"))?;
            }
            fs::write(base.join("scaling_governor"), &setting.governor)
                .map_err(|_| "Cannot apply CPU governor")?;
        }
        let actual = policies(&self.cpu)?;
        if actual.iter().map(|p| &p.current).ne(desired.iter()) {
            return Err("CPU settings did not confirm the requested profile".into());
        }
        Ok(())
    }
    fn apply_locked(&self, profile: Profile, persist: bool) -> Result<Value, String> {
        let current = policies(&self.cpu)?;
        let previous: Vec<_> = current.iter().map(|p| p.current.clone()).collect();
        let baseline = match self.load()? {
            Some(saved) => saved.baseline,
            None => previous.clone(),
        };
        // Validate every policy before touching the first one.
        let desired = targets(profile, &baseline, &current)?;
        let next = Saved {
            version: 1,
            profile,
            baseline,
        };
        let result = self
            .write(&desired)
            .and_then(|_| if persist { self.store(&next) } else { Ok(()) });
        if let Err(error) = result {
            return match self.write(&previous) {
                Ok(()) => Err(format!("{error}; previous CPU settings restored")),
                Err(_) => Err(format!(
                    "{error}; CPU rollback could not be verified, check device status"
                )),
            };
        }
        self.status_locked()
    }
    pub fn apply(&self, profile: Profile) -> Result<Value, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "Performance control is unavailable")?;
        self.apply_locked(profile, true)
    }
    pub fn resume(&self) -> Result<(), String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "Performance control is unavailable")?;
        if let Some(saved) = self.load()? {
            self.apply_locked(saved.profile, false)?;
        }
        Ok(())
    }
    fn status_locked(&self) -> Result<Value, String> {
        let current = policies(&self.cpu)?;
        let saved = self.load()?;
        let actual: Vec<_> = current.iter().map(|p| p.current.clone()).collect();
        let (requested, effective) = if let Some(saved) = &saved {
            let expected = targets(saved.profile, &saved.baseline, &current)?;
            (
                Some(saved.profile),
                (expected == actual).then_some(saved.profile),
            )
        } else {
            (
                None,
                current
                    .iter()
                    .all(|p| p.current.governor == "schedutil")
                    .then_some(Profile::Balanced),
            )
        };
        let baseline = saved
            .as_ref()
            .map(|s| s.baseline.clone())
            .unwrap_or_else(|| actual.clone());
        let supported: Vec<_> = [Profile::Balanced, Profile::Performance, Profile::Saving]
            .into_iter()
            .filter(|profile| targets(*profile, &baseline, &current).is_ok())
            .collect();
        Ok(
            json!({"supported":true,"available":supported,"requested":requested,"effective":effective,
            "applied": requested.is_none() || requested == effective, "thermal_limited":thermal_limited(&self.thermal),"policies":actual,"error":null}),
        )
    }
    pub fn status(&self) -> Value {
        let result = self
            .lock
            .lock()
            .map_err(|_| "Performance control is unavailable".to_owned())
            .and_then(|_guard| self.status_locked());
        result.unwrap_or_else(|error| json!({"supported":false,"available":[],"requested":null,"effective":null,"applied":false,"thermal_limited":thermal_limited(&self.thermal),"policies":[],"error":error}))
    }
}
const BANDWIDTH: [(&str, &str); 6] = [
    ("normal", "Nomal_mode"),
    ("ai", "Ai_mode"),
    ("game", "Game_mode"),
    ("video", "Video_mode"),
    ("meeting", "Chat_mode"),
    ("live", "Live_mode"),
];
fn bandwidth() -> Result<&'static str, String> {
    let raw = handlers::read_uci_option("zwrt_smart_mng", "smart_qos", "mode")?;
    BANDWIDTH
        .iter()
        .find(|(_, v)| *v == raw)
        .map(|(mode, _)| *mode)
        .ok_or_else(|| "Unknown bandwidth strategy".into())
}
pub fn status(state: &AppState) -> (u16, Value) {
    let mode = handlers::read_uci_option("zte_nwinfo", "sys_info", "operate_mode");
    let airplane = mode.as_deref().ok().and_then(crate::modem_ext::airplane_enabled);
    let bandwidth = bandwidth();
    let rail = handlers::read_uci_option("zte_nwinfo", "sys_info", "hst_info")
        .ok()
        .and_then(|v| match v.as_str() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        });
    (
        200,
        json!({"ok":true,"data":{
            "airplane":{"enabled":airplane,"mode":mode.as_ref().ok(),"error":mode.err()},
            "bandwidth":{"supported":bandwidth.is_ok(),"mode":bandwidth.as_ref().ok(),"error":bandwidth.err()},
            "rail":{"active":rail},"performance":state.performance.status(),
        }}),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileRequest {
    profile: Profile,
}
pub fn set_performance(state: &AppState, body: &[u8]) -> (u16, Value) {
    let Ok(request) = serde_json::from_slice::<ProfileRequest>(body) else {
        return (
            400,
            json!({"ok":false,"error":"Invalid performance profile"}),
        );
    };
    let result = state.performance.apply(request.profile);
    events::touch(Topic::Operating);
    match result {
        Ok(data) => (200, json!({"ok":true,"data":data})),
        Err(error) => (503, json!({"ok":false,"error":error})),
    }
}
static QOS_LOCK: Mutex<()> = Mutex::new(());
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BandwidthRequest {
    mode: String,
}
pub fn set_bandwidth(_state: &AppState, body: &[u8]) -> (u16, Value) {
    let Ok(request) = serde_json::from_slice::<BandwidthRequest>(body) else {
        return (
            400,
            json!({"ok":false,"error":"Invalid bandwidth strategy"}),
        );
    };
    let Some((_, wire)) = BANDWIDTH.iter().find(|(id, _)| *id == request.mode) else {
        return (
            400,
            json!({"ok":false,"error":"Invalid bandwidth strategy"}),
        );
    };
    let result = (|| {
        let _guard = QOS_LOCK
            .lock()
            .map_err(|_| "Bandwidth control is unavailable")?;
        // Refuse unsupported firmware before attempting any mutation.
        bandwidth()?;
        ubus::call_bounded(
            "zwrt_smart_mng.api",
            "zte_smart_mng_set_qos_mode",
            Some(&json!({"qos_mode":wire}).to_string()),
            3,
        )?;
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            if bandwidth().is_ok_and(|mode| mode == request.mode) {
                return Ok(json!({"mode":request.mode}));
            }
            if Instant::now() >= until {
                return Err(
                    "Bandwidth change could not be verified; check current settings".to_string(),
                );
            }
            std::thread::sleep(Duration::from_millis(150));
        }
    })();
    events::touch(Topic::Operating);
    match result {
        Ok(data) => (200, json!({"ok":true,"data":data})),
        Err(error) => (503, json!({"ok":false,"error":error})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct FixtureDir(PathBuf);
    impl FixtureDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for FixtureDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (FixtureDir, Performance) {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = FixtureDir(
            std::env::temp_dir().join(format!("u60-performance-{}-{id}", std::process::id())),
        );
        fs::create_dir(&dir.0).unwrap();
        let base = dir.path().join("cpu/policy0");
        fs::create_dir_all(&base).unwrap();
        for (key, value) in [
            ("scaling_driver", "qcom-cpufreq-hw"),
            ("scaling_governor", "schedutil"),
            (
                "scaling_available_governors",
                "conservative powersave performance schedutil",
            ),
            (
                "scaling_available_frequencies",
                "691200 960000 1516800 2208000",
            ),
            ("scaling_min_freq", "691200"),
            ("scaling_max_freq", "2208000"),
            ("cpuinfo_min_freq", "691200"),
            ("cpuinfo_max_freq", "2208000"),
        ] {
            fs::write(base.join(key), value).unwrap();
        }
        let manager = Performance {
            cpu: dir.path().join("cpu"),
            thermal: dir.path().join("thermal"),
            saved: dir.path().join("state/performance.json"),
            lock: Mutex::new(()),
        };
        (dir, manager)
    }
    #[test]
    fn profiles_are_verified_and_balanced_restores_original_limits() {
        let (_dir, m) = fixture();
        assert_eq!(m.status()["effective"], "balanced");
        assert!(!m.saved.exists());
        let saving = m.apply(Profile::Saving).unwrap();
        assert_eq!(saving["policies"][0]["max_khz"], 1516800);
        assert_eq!(saving["policies"][0]["governor"], "conservative");
        let performance = m.apply(Profile::Performance).unwrap();
        assert_eq!(performance["effective"], "performance");
        assert_eq!(performance["policies"][0]["max_khz"], 2208000);
        assert_eq!(m.apply(Profile::Balanced).unwrap()["effective"], "balanced");
        assert_eq!(
            integer(&m.cpu.join("policy0/scaling_min_freq")).unwrap(),
            691200
        );
    }
    #[test]
    fn persistence_failure_rolls_hardware_back() {
        let (_dir, m) = fixture();
        m.apply(Profile::Balanced).unwrap();
        // Make atomic staging fail after the hardware was written, not during load.
        fs::write(
            m.saved
                .parent()
                .unwrap()
                .join(format!(".performance-{}.tmp", std::process::id())),
            "stale staging file",
        )
        .unwrap();
        assert!(
            m.apply(Profile::Saving)
                .unwrap_err()
                .contains("previous CPU settings restored")
        );
        assert_eq!(
            read(&m.cpu.join("policy0/scaling_governor")).unwrap(),
            "schedutil"
        );
        assert_eq!(
            integer(&m.cpu.join("policy0/scaling_max_freq")).unwrap(),
            2208000
        );
        assert_eq!(m.load().unwrap().unwrap().profile, Profile::Balanced);
    }
    #[test]
    fn external_change_is_not_reported_as_applied_and_resume_is_explicit() {
        let (_dir, m) = fixture();
        m.apply(Profile::Saving).unwrap();
        fs::write(m.cpu.join("policy0/scaling_governor"), "schedutil").unwrap();
        let s = m.status();
        assert_eq!(s["requested"], "saving");
        assert_eq!(s["applied"], false);
        assert!(s["effective"].is_null());
        assert_eq!(
            read(&m.cpu.join("policy0/scaling_governor")).unwrap(),
            "schedutil"
        );
        m.resume().unwrap();
        assert_eq!(m.status()["effective"], "saving");
    }
    #[test]
    fn corrupt_or_foreign_profile_cannot_write_sysfs() {
        let (_dir, m) = fixture();
        m.apply(Profile::Balanced).unwrap();
        let mut s = m.load().unwrap().unwrap();
        s.baseline[0].id = "../../outside".into();
        m.store(&s).unwrap();
        assert!(m.apply(Profile::Performance).is_err());
        assert_eq!(
            read(&m.cpu.join("policy0/scaling_governor")).unwrap(),
            "schedutil"
        );
        assert_eq!(m.status()["supported"], false);
        assert!(serde_json::from_str::<ProfileRequest>(r#"{"profile":"powersave"}"#).is_err());
    }
    #[test]
    fn thermal_status_uses_real_cooling_state_not_a_temperature_guess() {
        let (_dir, m) = fixture();
        assert!(m.status()["thermal_limited"].is_null());
        let cooling = m.thermal.join("cooling_device5");
        fs::create_dir_all(&cooling).unwrap();
        fs::write(cooling.join("type"), "cpufreq-cpu0").unwrap();
        fs::write(cooling.join("cur_state"), "1").unwrap();
        assert_eq!(m.status()["thermal_limited"], true);
        fs::write(cooling.join("cur_state"), "0").unwrap();
        assert_eq!(m.status()["thermal_limited"], false);
    }
}
