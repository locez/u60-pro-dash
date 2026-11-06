mod display;
mod input;
use crate::app::App;
use anyhow::{Context, Result, ensure};
use slint::platform::software_renderer::MinimalSoftwareWindow;
use slint::{
    ComponentHandle, LogicalPosition,
    platform::{PointerEventButton, WindowEvent},
};
use std::{
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

struct Backlight {
    path: PathBuf,
    original: String,
    max: u32,
    last: Option<u32>,
}
impl Backlight {
    fn open(path: &Path) -> Result<Self> {
        let original = fs::read_to_string(path)?;
        let max = fs::read_to_string(path.with_file_name("max_brightness"))?
            .trim()
            .parse::<u32>()?;
        ensure!(max > 0, "invalid backlight maximum");
        Ok(Self {
            path: path.into(),
            original,
            max,
            last: None,
        })
    }
    fn set(&mut self, percent: i32) -> Result<()> {
        let value = (self.max as u64 * percent.clamp(0, 100) as u64 / 100) as u32;
        if self.last != Some(value) {
            fs::write(&self.path, value.to_string())?;
            self.last = Some(value);
        }
        Ok(())
    }
}
impl Drop for Backlight {
    fn drop(&mut self) {
        let _ = fs::write(&self.path, &self.original);
    }
}

pub fn run(
    app: &App,
    window: Rc<MinimalSoftwareWindow>,
    seconds: Option<u64>,
    rotate: bool,
    touch_rotate: bool,
    touch: Option<&Path>,
) -> Result<()> {
    validate_session(seconds, std::env::var("U60_SCREEN_HANDOFF").ok().as_deref())?;
    let mut heartbeat = (seconds.is_none()).then(Heartbeat::new);
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, stop.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, stop.clone())?;
    let mut inputs = input::Inputs::open(touch, touch_rotate)?;
    let mut backlight =
        Backlight::open(Path::new("/sys/class/leds/led:lcd/brightness")).context("backlight")?;
    let mut display = display::Display::open(Path::new("/dev/dri/card0"))?;
    app.ui.show()?;
    let start = Instant::now();
    let mut first = true;
    let mut asleep = false;
    let mut wake_touch = false;
    display.run(&window, rotate, || {
        if stop.load(Ordering::Relaxed)
            || seconds.is_some_and(|limit| start.elapsed() >= Duration::from_secs(limit))
        {
            return Ok(false);
        }
        if !first {
            if let Some(heartbeat) = &mut heartbeat {
                heartbeat.tick()?;
            }
            let deadline = slint::platform::duration_until_next_timer_update()
                .unwrap_or(Duration::from_millis(500));
            // Cap waits for signal/deadline response. No fixed-rate redraw while idle.
            let timeout = if app.ui.window().has_active_animations() {
                Duration::from_millis(16)
            } else {
                deadline.min(Duration::from_millis(500))
            };
            inputs.wait(timeout)?;
        }
        first = false;
        for event in inputs.read()? {
            app.activity();
            match event {
                input::Input::Power => {
                    app.ui.invoke_power();
                }
                input::Input::CancelPointer => {
                    window.dispatch_event(WindowEvent::PointerMoved {
                        position: LogicalPosition::new(-1., -1.),
                    });
                    window.dispatch_event(WindowEvent::PointerReleased {
                        position: LogicalPosition::new(-1., -1.),
                        button: PointerEventButton::Left,
                    });
                    window.dispatch_event(WindowEvent::PointerExited);
                    wake_touch = false;
                }
                input::Input::Pointer(event) => {
                    if app.ui.get_asleep() {
                        app.ui.set_asleep(false);
                        wake_touch = true;
                    }
                    if wake_touch {
                        if matches!(event, WindowEvent::PointerReleased { .. }) {
                            wake_touch = false;
                        }
                    } else {
                        window.dispatch_event(event);
                    }
                }
            }
        }
        slint::platform::update_timers_and_animations();
        if app.ui.get_asleep() != asleep {
            asleep = app.ui.get_asleep();
            window.request_redraw();
            eprintln!("screen: {}", if asleep { "asleep" } else { "awake" });
        }
        backlight.set(if asleep { 0 } else { app.ui.get_brightness() })?;
        Ok(true)
    })?;
    app.ui.hide()?;
    eprintln!("screen: session ended normally");
    Ok(())
}

fn validate_session(seconds: Option<u64>, guard: Option<&str>) -> Result<()> {
    match seconds {
        Some(seconds) => ensure!(
            guard == Some("guarded") && (5..=600).contains(&seconds),
            "device trials require a recovery guard and 5..600 seconds"
        ),
        None => ensure!(
            guard == Some("service"),
            "persistent screen requires the project supervisor"
        ),
    }
    Ok(())
}
struct Heartbeat {
    last: Option<Instant>,
    sequence: u64,
}
impl Heartbeat {
    fn new() -> Self {
        Self {
            last: None,
            sequence: 0,
        }
    }
    fn tick(&mut self) -> Result<()> {
        if self
            .last
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
        {
            return Ok(());
        }
        self.sequence += 1;
        let root = Path::new("/tmp/u60-dash-run");
        fs::write(root.join("heartbeat.new"), self.sequence.to_string())
            .context("screen watchdog heartbeat")?;
        fs::rename(root.join("heartbeat.new"), root.join("heartbeat"))?;
        self.last = Some(Instant::now());
        Ok(())
    }
}
#[cfg(test)]
mod session_tests {
    #[test]
    fn persistent_mode_requires_its_guard_and_trials_remain_bounded() {
        assert!(super::validate_session(None, Some("service")).is_ok());
        assert!(super::validate_session(None, Some("guarded")).is_err());
        assert!(super::validate_session(Some(90), Some("guarded")).is_ok());
        assert!(super::validate_session(Some(601), Some("guarded")).is_err());
        assert!(super::validate_session(Some(0), Some("service")).is_err());
    }
}
