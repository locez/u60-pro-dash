use crate::platform::{HEIGHT, WIDTH};
use anyhow::{Context, Result, bail, ensure};
use evdev::{
    AbsoluteAxisCode as Abs, EventSummary, KeyCode as Key, SynchronizationCode as Syn,
    raw_stream::RawDevice,
};
use nix::{
    libc,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use slint::{
    LogicalPosition,
    platform::{PointerEventButton, WindowEvent},
};
use std::{
    fs::OpenOptions,
    os::{fd::AsFd, unix::fs::OpenOptionsExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Default)]
struct Contact {
    x: i32,
    y: i32,
    active: bool,
}
#[derive(Clone, Copy)]
pub struct Calibration {
    pub x: (i32, i32),
    pub y: (i32, i32),
    pub rotate: bool,
}
impl Calibration {
    pub fn map(self, x: i32, y: i32) -> LogicalPosition {
        fn scale(n: i32, (lo, hi): (i32, i32), size: usize) -> f32 {
            (((n.clamp(lo, hi) as i64 - lo as i64) * (size - 1) as i64) as f64
                / (hi as i64 - lo as i64) as f64) as f32
        }
        let x = scale(x, self.x, WIDTH);
        let y = scale(y, self.y, HEIGHT);
        if self.rotate {
            LogicalPosition::new((WIDTH - 1) as f32 - x, (HEIGHT - 1) as f32 - y)
        } else {
            LogicalPosition::new(x, y)
        }
    }
}

struct PowerKey {
    device: RawDevice,
    pressed: Option<Instant>,
}
pub struct Inputs {
    touch: RawDevice,
    power: Vec<PowerKey>,
    calibration: Calibration,
    contacts: [Contact; 16],
    slot: usize,
    selected: Option<usize>,
    position: LogicalPosition,
    dropped: bool,
    mt: bool,
}

pub enum Input {
    Pointer(WindowEvent),
    CancelPointer,
    Power,
}
fn open(path: &Path) -> Result<RawDevice> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    Ok(RawDevice::from_fd(file.into())?)
}

impl Inputs {
    pub fn open(touch_path: Option<&Path>, rotate: bool) -> Result<Self> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir("/dev/input")?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|s| s.to_string_lossy().starts_with("event"))
            })
            .collect();
        paths.sort();
        let mut touch = None;
        let mut power = vec![];
        for path in &paths {
            let Ok(device) = open(path) else {
                continue;
            };
            let is_touch =
                touch_path.map_or_else(|| device.name() == Some("sitronix_ts_i2c"), |p| p == path);
            if is_touch {
                touch = Some(device);
            } else if device
                .supported_keys()
                .is_some_and(|keys| keys.contains(Key::KEY_POWER))
            {
                power.push(PowerKey {
                    device,
                    pressed: None,
                });
            }
        }
        let Some(mut touch) = touch else {
            bail!("Sitronix touch input not found; use --touch /dev/input/eventN to override");
        };
        ensure!(
            !power.is_empty(),
            "power key not found; refuse a trial without a wake button"
        );
        let axes = touch
            .supported_absolute_axes()
            .context("touch has no absolute axes")?;
        let mt = axes.contains(Abs::ABS_MT_POSITION_X) && axes.contains(Abs::ABS_MT_POSITION_Y);
        ensure!(
            !mt || axes.contains(Abs::ABS_MT_SLOT),
            "MT protocol A is not supported; expected the U60 slot protocol"
        );
        let (x, y) = if mt {
            (Abs::ABS_MT_POSITION_X, Abs::ABS_MT_POSITION_Y)
        } else {
            (Abs::ABS_X, Abs::ABS_Y)
        };
        ensure!(
            axes.contains(x) && axes.contains(y),
            "touch X/Y axes missing"
        );
        let abs = touch.get_abs_state()?;
        let ax = abs[x.0 as usize];
        let ay = abs[y.0 as usize];
        ensure!(
            ax.maximum > ax.minimum && ay.maximum > ay.minimum,
            "invalid touch axis range"
        );
        if mt {
            ensure!(
                abs[Abs::ABS_MT_SLOT.0 as usize].maximum < 16
                    && abs[Abs::ABS_MT_SLOT.0 as usize].minimum == 0,
                "unsupported slot range"
            );
        }
        let calibration = Calibration {
            x: (ax.minimum, ax.maximum),
            y: (ay.minimum, ay.maximum),
            rotate,
        };
        // Only grab touch. Power remains available to the firmware for long press/shutdown.
        touch.grab().context("exclusive touch access")?;
        eprintln!(
            "input: {} x={}..{} y={}..{} touch_rotation={} power_keys={}",
            touch.name().unwrap_or("touch"),
            ax.minimum,
            ax.maximum,
            ay.minimum,
            ay.maximum,
            if rotate { 180 } else { 0 },
            power.len()
        );
        let contacts = [Contact {
            x: ax.value,
            y: ay.value,
            active: false,
        }; 16];
        Ok(Self {
            touch,
            power,
            calibration,
            contacts,
            slot: 0,
            selected: None,
            position: LogicalPosition::default(),
            dropped: false,
            mt,
        })
    }

    pub fn wait(&self, timeout: Duration) -> Result<()> {
        let mut fds = vec![PollFd::new(self.touch.as_fd(), PollFlags::POLLIN)];
        fds.extend(
            self.power
                .iter()
                .map(|p| PollFd::new(p.device.as_fd(), PollFlags::POLLIN)),
        );
        match poll(
            &mut fds,
            PollTimeout::try_from(timeout).unwrap_or(PollTimeout::MAX),
        ) {
            Err(nix::errno::Errno::EINTR) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => {
                ensure!(
                    !fds.iter().any(|f| f.revents().is_some_and(|r| r.intersects(
                        PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL
                    ))),
                    "input device disconnected"
                );
                Ok(())
            }
        }
    }

    pub fn read(&mut self) -> Result<Vec<Input>> {
        let mut output = Vec::new();
        let events = match self.touch.fetch_events() {
            Ok(events) => events.collect::<Vec<_>>(),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => vec![],
            Err(e) => return Err(e.into()),
        };
        for event in events {
            let summary = event.destructure();
            if matches!(
                summary,
                EventSummary::Synchronization(_, Syn::SYN_DROPPED, _)
            ) {
                self.selected = None;
                self.contacts.fill(Contact::default());
                self.dropped = true;
                output.push(Input::CancelPointer);
                continue;
            }
            if self.dropped {
                if matches!(
                    summary,
                    EventSummary::Synchronization(_, Syn::SYN_REPORT, _)
                ) {
                    self.dropped = false;
                    // Reset to the kernel's current slot, but require a fresh tracking ID / press.
                    self.slot = self.touch.get_abs_state()?[Abs::ABS_MT_SLOT.0 as usize]
                        .value
                        .clamp(0, 15) as usize;
                }
                continue;
            }
            match summary {
                EventSummary::AbsoluteAxis(_, Abs::ABS_MT_SLOT, v) if self.mt => {
                    self.slot = v as usize;
                }
                EventSummary::AbsoluteAxis(_, axis, v) if self.slot < self.contacts.len() => {
                    let c = &mut self.contacts[self.slot];
                    if (self.mt && axis == Abs::ABS_MT_POSITION_X)
                        || (!self.mt && axis == Abs::ABS_X)
                    {
                        c.x = v;
                    }
                    if (self.mt && axis == Abs::ABS_MT_POSITION_Y)
                        || (!self.mt && axis == Abs::ABS_Y)
                    {
                        c.y = v;
                    }
                    if self.mt && axis == Abs::ABS_MT_TRACKING_ID {
                        c.active = v >= 0;
                    }
                }
                EventSummary::Key(_, Key::BTN_TOUCH, v) if !self.mt => {
                    self.contacts[0].active = v != 0;
                }
                EventSummary::Synchronization(_, Syn::SYN_REPORT, _) => self.report(&mut output),
                _ => {}
            }
        }
        for power in &mut self.power {
            let events = match power.device.fetch_events() {
                Ok(events) => events.collect::<Vec<_>>(),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => vec![],
                Err(e) => return Err(e.into()),
            };
            for event in events {
                match event.destructure() {
                    EventSummary::Key(_, Key::KEY_POWER, 1) => {
                        power.pressed = Some(Instant::now());
                    }
                    EventSummary::Key(_, Key::KEY_POWER, 0) => {
                        if power
                            .pressed
                            .take()
                            .is_some_and(|time| time.elapsed() < Duration::from_millis(900))
                        {
                            output.push(Input::Power);
                        }
                    }
                    EventSummary::Synchronization(_, Syn::SYN_DROPPED, _) => power.pressed = None,
                    _ => {}
                }
            }
        }
        Ok(output)
    }

    fn report(&mut self, output: &mut Vec<Input>) {
        if let Some(slot) = self.selected
            && !self.contacts[slot].active
        {
            output.push(Input::Pointer(WindowEvent::PointerReleased {
                position: self.position,
                button: PointerEventButton::Left,
            }));
            self.selected = None;
            // Don't turn a second finger into a new tap on the same report.
            for c in &mut self.contacts {
                c.active = false;
            }
            return;
        }
        let Some(slot) = self
            .selected
            .or_else(|| self.contacts.iter().position(|c| c.active))
        else {
            return;
        };
        let position = self
            .calibration
            .map(self.contacts[slot].x, self.contacts[slot].y);
        if self.selected.is_none() {
            output.push(Input::Pointer(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            }));
        } else if self.position != position {
            output.push(Input::Pointer(WindowEvent::PointerMoved { position }));
        }
        self.position = position;
        self.selected = Some(slot);
    }
}

impl Drop for Inputs {
    fn drop(&mut self) {
        let _ = self.touch.ungrab();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calibrated_coordinates_match_render_rotation_without_double_rotation() {
        let c = Calibration {
            x: (-10, 1010),
            y: (20, 2020),
            rotate: true,
        };
        assert_eq!(c.map(-99, 0), LogicalPosition::new(319., 479.));
        assert_eq!(c.map(5000, 5000), LogicalPosition::new(0., 0.));
        assert_eq!(c.map(500, 1020), LogicalPosition::new(159.5, 239.5));
        assert_eq!(
            Calibration { rotate: false, ..c }.map(-10, 20),
            LogicalPosition::new(0., 0.)
        );
    }
}
