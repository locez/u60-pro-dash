mod app;
mod config;
#[cfg(feature = "device")]
mod device;
mod platform;

use anyhow::{Context, Result, bail};
use config::Preferences;
use slint::ComponentHandle;
use std::path::PathBuf;
slint::include_modules!();

#[derive(Default)]
struct Options {
    device: bool,
    screenshot: Option<PathBuf>,
    prefs: Option<PathBuf>,
    page: Option<String>,
    dark: Option<bool>,
    accent: Option<String>,
    rotate: Option<bool>,
    touch_rotate: Option<bool>,
    touch: Option<PathBuf>,
    seconds: Option<u64>,
}
fn options() -> Result<Options> {
    let mut opts = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .with_context(|| format!("missing value after {arg}"))
        };
        match arg.as_str() {
            "--device" => opts.device = true,
            "--screenshot" => opts.screenshot = Some(value()?.into()),
            "--prefs" => opts.prefs = Some(value()?.into()),
            "--page" => opts.page = Some(value()?),
            "--light" => opts.dark = Some(false),
            "--dark" => opts.dark = Some(true),
            "--accent" => opts.accent = Some(value()?),
            "--rotate" | "--touch-rotate" => {
                let rotate = match value()?.as_str() {
                    "0" => false,
                    "180" => true,
                    _ => bail!("rotation must be 0 or 180"),
                };
                if arg == "--rotate" {
                    opts.rotate = Some(rotate);
                } else {
                    opts.touch_rotate = Some(rotate);
                }
            }
            "--touch" => opts.touch = Some(value()?.into()),
            "--seconds" => opts.seconds = Some(value()?.parse()?),
            "--help" | "-h" => {
                println!(
                    "u60-screen — native Slint screen (demonstration data)\n\n\
                    Desktop: u60-screen [--light|--dark] [--accent '#2563eb']\n\
                    Capture: u60-screen --screenshot FILE.png [--page FEATURE_ID]\n\
                    Device:  use scripts/device-trial.sh (guarded, 90 seconds by default)\n\n\
                    --prefs FILE          Load/save local appearance, never credentials\n\
                    --rotate 0|180        Output orientation (device default: 180)\n\
                    --touch-rotate 0|180  Independent input calibration (default: 180)\n\
                    --touch /dev/input/eventN  Override Sitronix auto-detection\n\
                    --seconds 5..600      Device trial deadline\n\n\
                    Desktop: arrows navigate; Escape goes back; Space sleeps/wakes.\n\
                    Native business actions are not connected to the agent yet."
                );
                std::process::exit(0);
            }
            _ => bail!("unknown option {arg}; use --help"),
        }
    }
    if opts.device && opts.screenshot.is_some() {
        bail!("--device and --screenshot are mutually exclusive");
    }
    Ok(opts)
}

fn main() -> Result<()> {
    let opts = options()?;
    let mut prefs = match &opts.prefs {
        Some(path) if path.exists() => Preferences::load(path)
            .context("invalid screen preferences; correct or rename the file")?,
        _ => Preferences::default(),
    };
    if let Some(dark) = opts.dark {
        prefs.dark = dark;
    }
    if let Some(accent) = opts.accent {
        prefs.accent = accent;
    }
    prefs.validate()?;
    let software = if opts.device || opts.screenshot.is_some() {
        Some(platform::install()?)
    } else {
        #[cfg(feature = "desktop")]
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("software".into())
            .select()?;
        #[cfg(not(feature = "desktop"))]
        bail!(
            "desktop feature not compiled; use --device through the trial script or --screenshot"
        );
        #[allow(unreachable_code)]
        None
    };
    let app = app::App::new(prefs, opts.prefs)?;
    if let Some(page) = &opts.page {
        app.open(page)?;
    }
    if let Some(path) = opts.screenshot {
        app.ui.show()?;
        slint::platform::update_timers_and_animations();
        let pixels = platform::capture(
            software.as_ref().expect("software platform"),
            opts.rotate.unwrap_or(false),
        );
        platform::save_png(&path, &pixels)?;
        println!("{} (320x480 RGB565 rendering)", path.display());
        return Ok(());
    }
    if opts.device {
        #[cfg(feature = "device")]
        return device::run(
            &app,
            software.expect("software platform"),
            opts.seconds.unwrap_or(90),
            opts.rotate.unwrap_or(true),
            opts.touch_rotate.unwrap_or(true),
            opts.touch.as_deref(),
        );
        #[cfg(not(feature = "device"))]
        bail!("rebuild with --no-default-features --features device");
    }
    app.ui.run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::{
        LogicalPosition, Model,
        platform::{PointerEventButton, WindowEvent},
    };
    #[test]
    fn native_pages_touch_themes_sleep_and_rotation() -> Result<()> {
        let window = platform::install()?;
        let private = tempfile::tempdir()?;
        let app = app::App::new(
            Preferences::default(),
            Some(private.path().join("appearance.json")),
        )?;
        app.ui.show()?;
        slint::platform::update_timers_and_animations();
        let initial = platform::capture(&window, false);
        assert_eq!(initial.len(), 320 * 480);
        assert!(initial.iter().any(|p| p.0 != initial[0].0));
        let tap = |x, y| {
            let position = LogicalPosition::new(x, y);
            window.dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
            window.dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
        };
        tap(120., 450.);
        assert_eq!(app.ui.get_tab(), 1);
        assert_eq!(app.ui.get_heading(), "网络");
        platform::capture(&window, false);
        tap(140., 114.);
        assert_eq!(app.ui.get_heading(), "Wi-Fi 与局域网");
        assert!(app.ui.get_entries().row_count() >= 5);
        let ids = app
            .state
            .borrow()
            .catalog
            .features
            .iter()
            .map(|f| f.id.clone())
            .collect::<Vec<_>>();
        for id in &ids {
            app.open(id)?;
            let pixels = platform::capture(&window, false);
            assert!(pixels.iter().any(|p| p.0 != pixels[0].0), "blank page {id}");
        }
        assert!(
            ids.contains(&"wireguard".to_string()) && ids.contains(&"agent-update".to_string())
        );
        app.open("overview")?;
        app.ui.invoke_theme(false);
        let light = platform::capture(&window, false);
        assert_ne!(light[0].0, initial[0].0);
        app.ui.invoke_accent("#d97706".into());
        assert_eq!(app.state.borrow().prefs.accent, "#d97706");
        app.ui.invoke_power();
        assert!(app.ui.get_asleep());
        assert!(platform::capture(&window, false).iter().all(|p| p.0 == 0));
        tap(100., 100.);
        assert!(!app.ui.get_asleep());
        assert_eq!(app.ui.get_heading(), "随身网络");
        // Damage and input rotations are independent. The capture rotation must
        // invert the entire pixel buffer, including asymmetric text and corners.
        let upright = platform::capture(&window, false);
        let rotated = platform::capture(&window, true);
        // Fractional glyph origins get different subpixel antialiasing after
        // rotation; geometry and the great majority of pixels must still agree.
        let changed = upright
            .iter()
            .rev()
            .zip(rotated)
            .filter(|(a, b)| a.0 != b.0)
            .count();
        assert!(
            changed < upright.len() / 100,
            "rotation changed {changed} pixels"
        );
        app.ui.invoke_action(0);
        assert!(app.ui.get_message().contains("尚未接入"));
        Ok(())
    }
}
