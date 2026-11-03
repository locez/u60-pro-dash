mod agent;
mod app;
mod config;
#[cfg(feature = "device")]
mod device;
mod features;
mod i18n;
mod platform;
mod security;

use anyhow::{Context, Result, bail};
use config::Preferences;
use slint::ComponentHandle;
use std::path::PathBuf;
slint::include_modules!();

#[derive(Default)]
struct Options {
    agent: Option<std::net::SocketAddr>,
    demo: bool,
    local_agent: bool,
    probe: bool,
    probe_features: bool,
    set_probe_target: Option<String>,
    check_target: Option<std::net::SocketAddr>,
    device: bool,
    service: bool,
    screenshot: Option<PathBuf>,
    prefs: Option<PathBuf>,
    page: Option<String>,
    dark: Option<bool>,
    language: Option<crate::i18n::Locale>,
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
            "--agent" => opts.agent = Some(value()?.parse()?),
            "--demo" => opts.demo = true,
            "--local-agent" => opts.local_agent = true,
            "--probe-agent" => opts.probe = true,
            "--probe-features" => opts.probe_features = true,
            "--set-probe-target" => opts.set_probe_target = Some(value()?),
            "--check-target" => opts.check_target = Some(agent::connectivity::target(&value()?)?),
            "--device" => opts.device = true,
            "--service" => opts.service = true,
            "--version" => {
                println!("u60-screen {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--screenshot" => opts.screenshot = Some(value()?.into()),
            "--prefs" => opts.prefs = Some(value()?.into()),
            "--page" => opts.page = Some(value()?),
            "--light" => opts.dark = Some(false),
            "--dark" => opts.dark = Some(true),
            "--language" => {
                opts.language = Some(
                    crate::i18n::Locale::parse(&value()?)
                        .context("language must be zh-Hans, zh-Hant, en, ja or ko")?,
                )
            }
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
                    "u60-screen — native Slint screen\n\n\
                    Desktop: u60-screen [--light|--dark] [--accent '#2563eb']\n\
                    Capture: u60-screen --screenshot FILE.png [--page FEATURE_ID]\n\
                    Device:  use scripts/device-trial.sh (guarded, 90 seconds by default)\n\n\
                    --prefs FILE          Load/save local preferences, never credentials\n\
                    --language LOCALE     zh-Hans, zh-Hant, en, ja or ko\n\
                    --rotate 0|180        Output orientation (device default: 180)\n\
                    --touch-rotate 0|180  Independent input calibration (default: 180)\n\
                    --touch /dev/input/eventN  Override Sitronix auto-detection\n\
                    --service             Persistent device session; requires project supervisor\n\
                    --seconds 5..600      Device trial deadline\n\n\
                    --agent HOST:PORT     Live agent through a loopback tunnel\n\
                    --demo                Use demonstration data, also on device\n\
                    --local-agent         Authenticate with the on-device agent process\n\
                    --probe-agent         Check login and live topics without taking the display\n\n\
                    --probe-features      Read implemented feature contracts; print counts only\n\n\
                    --set-probe-target IP:PORT  Save a target with --prefs; empty disables probing\n\
                    --check-target IP:PORT      Check this host's tunnel route and TCP port; no display\n\n\
                    Desktop: arrows navigate; Escape goes back; Space sleeps/wakes.\n\
                    Desktop defaults to demo; guarded device trials use live data."
                );
                std::process::exit(0);
            }
            _ => bail!("unknown option {arg}; use --help"),
        }
    }
    if opts.service && (!opts.device || opts.seconds.is_some()) {
        bail!("--service requires --device and cannot set --seconds");
    }
    if opts.device && opts.screenshot.is_some() {
        bail!("--device and --screenshot are mutually exclusive");
    }
    if opts.demo && (opts.agent.is_some() || opts.local_agent || opts.probe || opts.probe_features)
    {
        bail!("--demo cannot be combined with agent options");
    }
    let maintenance = opts.set_probe_target.is_some() || opts.check_target.is_some();
    if maintenance
        && (opts.device
            || opts.screenshot.is_some()
            || opts.probe
            || opts.probe_features
            || opts.agent.is_some()
            || opts.local_agent
            || opts.demo
            || (opts.set_probe_target.is_some() && opts.check_target.is_some()))
    {
        bail!("target configuration/checks cannot be combined with display or agent options");
    }
    if opts.set_probe_target.is_some() && opts.prefs.is_none() {
        bail!("--set-probe-target requires --prefs FILE");
    }
    Ok(opts)
}

fn main() -> Result<()> {
    let opts = options()?;
    if let Some(target) = opts.check_target {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let result = runtime.block_on(agent::connectivity::probe(target));
        println!("{result}");
        return Ok(());
    }
    let agent = (!opts.demo
        && (opts.device
            || opts.agent.is_some()
            || opts.local_agent
            || opts.probe
            || opts.probe_features))
        .then(|| agent::Config {
            address: opts
                .agent
                .unwrap_or_else(|| "127.0.0.1:9090".parse().unwrap()),
            session_file: opts.prefs.as_ref().map(|p| {
                p.with_file_name(".screen-private")
                    .join("agent-session.private.json")
            }),
            local_credentials: opts.device || opts.local_agent,
        });
    if opts.probe {
        return agent::probe(agent.expect("probe configuration"));
    }
    if opts.probe_features {
        return features::probe(agent.expect("feature probe configuration"));
    }
    let mut prefs = match &opts.prefs {
        Some(path) if path.exists() => Preferences::load(path)
            .context("invalid screen preferences; correct or rename the file")?,
        _ => Preferences::default(),
    };
    if let Some(dark) = opts.dark {
        prefs.dark = dark;
    }
    if let Some(language) = opts.language {
        prefs.language = language;
    }
    if let Some(accent) = opts.accent {
        prefs.accent = accent;
    }
    prefs.validate()?;
    if let Some(target) = opts.set_probe_target {
        prefs.probe_target = target.trim().into();
        prefs.save(opts.prefs.as_deref().expect("validated preferences path"))?;
        println!("probe target saved to local preferences; no connection attempted");
        return Ok(());
    }
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
    if let Some(config) = agent {
        app.connect(config)?;
    }
    if let Some(page) = &opts.page
        && page != "lock"
    {
        app.open(page)?;
    }
    if let Some(path) = opts.screenshot {
        // Offline demo captures can show pages without a hold gesture. A supplied
        // password configuration still remains locked; --device cannot use this path.
        if opts.page.as_deref() != Some("lock") && !app.state.borrow().security.enabled() {
            app.state.borrow_mut().security.locked = false;
            app.state.borrow().sync_security(&app.ui);
        }
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
            if opts.service {
                None
            } else {
                Some(opts.seconds.unwrap_or(90))
            },
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
        assert!(app.ui.get_locked());
        tap(120., 450.);
        assert_eq!(app.ui.get_tab(), 0, "navigation must be gated while locked");
        let hold = || {
            let position = LogicalPosition::new(160., 355.);
            window.dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
            std::thread::sleep(std::time::Duration::from_millis(1010));
            window.dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
        };
        hold();
        assert!(!app.ui.get_locked());
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
        app.feature_fixture("charge",serde_json::json!({"charge_limit_enabled":true,"charge_limit":80,"hysteresis":5,"charging_stopped":false}));
        assert_eq!(app.ui.get_feature_fields().row_count(), 4);
        app.ui.invoke_feature_edit(0);
        assert!(app.ui.get_choice_visible());
        platform::capture(&window, false);
        tap(140., 90.);
        assert!(!app.ui.get_choice_visible());
        assert_eq!(
            app.ui.get_feature_fields().row_data(0).unwrap().subtitle,
            "关闭"
        );
        app.ui.invoke_feature_action(0);
        assert!(app.ui.get_confirm_visible());
        assert!(app.ui.get_confirm_body().contains("关闭"));
        app.ui.invoke_confirm_cancel();
        assert!(!app.ui.get_confirm_visible());
        assert!(!app.ui.get_feature_busy());
        // Exercise real feature rendering without a device or network writes.
        let fixture = |pairs: Vec<(&str, serde_json::Value)>| {
            pairs.into_iter().map(|(k, v)| (k.into(), Ok(v))).collect()
        };
        let save = |name: &str| -> Result<()> {
            if let Ok(dir) = std::env::var("U60_SCREEN_TEST_IMAGES") {
                let dir = std::path::Path::new(&dir);
                std::fs::create_dir_all(dir)?;
                platform::save_png(&dir.join(name), &platform::capture(&window, false))?;
            }
            Ok(())
        };
        app.feature_data_fixture("sms-inbox", fixture(vec![
            ("internal",serde_json::json!({"messages":[{"id":1,"number":"+15555550100","date":"2026-11-03 08:00","tag":1,"mem_store":1,"content":"测试短信：中文正文应清楚显示。\n这是第二行，带有网络状态提示。"}]})),
            ("card",serde_json::json!({"messages":[]})),("__selection",serde_json::json!("1")),
        ]));
        assert!(app.ui.get_feature_content().contains("中文正文"));
        save("live-sms.png")?;
        app.ui.invoke_open("feature-item:reply:1".into());
        app.ui.invoke_feature_edit(1);
        assert!(app.ui.get_editor_multiline());
        app.ui.set_editor_value("Hello\nsecond line".into());
        save("live-multiline.png")?;
        app.ui.invoke_editor_submit();
        assert!(!app.ui.get_editor_visible());
        app.ui.invoke_feature_action(0);
        assert!(
            app.ui.get_confirm_body().contains("+15555550100"),
            "default reply recipient must be reviewed"
        );
        save("live-confirmation.png")?;
        app.ui.invoke_confirm_cancel();
        app.feature_data_fixture("tailscale-service",fixture(vec![("status",serde_json::json!({"installed":true,"enabled":true,"enrolled":false,"backend_state":"NeedsLogin","login_url":"https://example.com/authorize/demonstration"}))]));
        assert!(app.ui.get_feature_has_qr());
        save("live-tailscale-qr.png")?;
        app.feature_data_fixture("proxy-service", fixture(vec![("status",serde_json::json!({"installed":true,"version":"1.0.0","configured":true,"running":true,"capturing":true,"mode":"rule","capture":"hotspot"}))]));
        assert_eq!(app.ui.get_feature_fields().row_count(), 0);
        // Keep branded service UI out of published screenshot artifacts.
        app.feature_data_fixture("proxy-groups", fixture(vec![
            ("status",serde_json::json!({"running":true})),
            ("groups",serde_json::json!([{"name":"示例路由组","selected":"示例节点 A","choices":["示例节点 A","示例节点 B"],"selectable":true}])),
            ("__selection",serde_json::json!("示例路由组")),
        ]));
        assert_eq!(app.ui.get_feature_fields().row_count(), 1);
        // Proxy interaction is verified without exporting branded screenshots.
        app.feature_data_fixture("wifi-share", fixture(vec![
            ("wifi",serde_json::json!({
                "wifi_onoff":"1","radio2_disabled":"0","radio5_disabled":"0",
                "ssid_2g":"U60 Demo;测试","ssid_5g":"Example 5G",
                "key_2g":"Demo:pass;with,quotes\"and\\slashes-2026","key_5g":"example-passphrase",
                "encryption_2g":"sae-mixed","encryption_5g":"sae","hidden_2g":"0","hidden_5g":"1"
            })),
            ("guest",serde_json::json!({"ssid":"Example Guest","disabled_2g":"1","disabled_5g":"1","encryption":"none","hidden":"0"})),
        ]));
        assert_eq!(app.ui.get_entries().row_count(), 3);
        save("live-wifi-share-networks.png")?;
        app.ui.invoke_open("feature-item:2g".into());
        assert!(app.ui.get_feature_has_qr());
        assert_eq!(app.ui.get_feature_qr_caption(), "U60 Demo;测试");
        save("live-wifi-share-qr.png")?;
        app.ui.invoke_lock_now();
        assert!(!app.ui.get_feature_has_qr());
        assert!(app.ui.get_feature_qr_caption().is_empty());
        assert_eq!(app.ui.get_feature_qr().size().width, 0);
        hold();
        app.feature_data_fixture(
            "esim-profiles",
            crate::features::Data::from([
                ("chip".into(), Err(crate::agent::Error::EsimUnavailable)),
                (
                    "inventory".into(),
                    Err(crate::agent::Error::EsimUnavailable),
                ),
                ("switch".into(), Ok(serde_json::json!({"state":"idle"}))),
            ]),
        );
        assert!(app.ui.get_feature_content().contains("普通运营商 SIM"));
        save("live-esim-unavailable.png")?;
        app.feature_data_fixture("esim-download",fixture(vec![("inventory",serde_json::json!({"agent_instance":12})),("download",serde_json::json!({"state":"confirm","started_at":1234,"provider":"演示运营商","name":"示例套餐","iccid":"8900000000000000000"}))]));
        app.ui.invoke_feature_action(0);
        assert!(app.ui.get_confirm_body().contains("示例套餐"));
        assert!(app.ui.get_confirm_body().contains("8900000000000000000"));
        save("live-esim-confirmation.png")?;
        app.ui.invoke_confirm_cancel();
        app.feature_data_fixture(
            "scheduler",
            fixture(vec![
                ("jobs", serde_json::json!([])),
                ("__selection", serde_json::json!("new")),
            ]),
        );
        assert_eq!(app.ui.get_feature_fields().row_count(), 7);
        save("live-scheduler.png")?;
        let action_field = app.ui.get_feature_fields().row_data(5).unwrap();
        assert_eq!(
            action_field.index, 6,
            "hidden fields must not shift input targets"
        );
        app.ui.invoke_feature_edit(action_field.index);
        app.ui.invoke_choose(5);
        assert_eq!(app.ui.get_feature_fields().row_count(), 11);
        assert!(
            app.ui
                .get_feature_fields()
                .iter()
                .any(|f| f.title == "请求参数 JSON")
        );
        app.ui.set_demo(true);
        app.open("overview")?;
        app.ui.invoke_edit("probe-target".into());
        platform::capture(&window, false);
        // Touch-key events must reach TextInput without triggering navigation.
        tap(22., 258.);
        assert_eq!(app.ui.get_editor_value(), "1");
        app.ui.invoke_editor_submit();
        assert!(app.ui.get_editor_visible());
        assert!(!app.ui.get_editor_error().is_empty());
        assert!(app.state.borrow().prefs.probe_target.is_empty());
        app.ui.set_editor_value("10.20.0.10:443".into());
        app.ui
            .invoke_editor_key(slint::SharedString::from(slint::platform::Key::End));
        app.ui
            .invoke_editor_key(slint::SharedString::from(slint::platform::Key::Backspace));
        assert_eq!(app.ui.get_editor_value(), "10.20.0.10:44");
        app.ui.invoke_editor_key("3".into());
        if let Ok(path) = std::env::var("U60_SCREEN_TEST_IMAGES") {
            let path = std::path::Path::new(&path);
            std::fs::create_dir_all(path)?;
            app.ui.set_editor_error("".into());
            platform::save_png(
                &path.join("target-editor.png"),
                &platform::capture(&window, false),
            )?;
        }
        app.ui.invoke_editor_submit();
        assert!(!app.ui.get_editor_visible());
        assert_eq!(app.state.borrow().prefs.probe_target, "10.20.0.10:443");
        app.ui.invoke_edit("agent-login".into());
        app.ui.set_editor_value("ephemeral test input".into());
        app.ui.invoke_navigate(2);
        assert_eq!(
            app.ui.get_heading(),
            "随身网络",
            "editor blocks underlying navigation"
        );
        app.ui.invoke_power();
        assert!(app.ui.get_asleep());
        assert!(!app.ui.get_editor_visible());
        assert!(
            app.ui.get_editor_value().is_empty(),
            "locking discards editor secrets"
        );
        assert!(platform::capture(&window, false).iter().all(|p| p.0 == 0));
        tap(100., 100.);
        assert!(!app.ui.get_asleep());
        assert!(app.ui.get_locked(), "waking must not unlock");
        hold();
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
        app.ui.set_message("".into());
        // Switching language updates authored labels immediately, while stable
        // field keys, literal SSIDs, preferences and the lock remain intact.
        app.open("screen-prefs")?;
        for (code, heading) in [
            ("en", "Screen & appearance"),
            ("zh-Hant", "螢幕與外觀"),
            ("ja", "画面と外観"),
            ("ko", "화면 및 테마"),
            ("zh-Hans", "屏幕与外观"),
        ] {
            app.ui.invoke_set_language(code.into());
            assert_eq!(app.ui.get_language(), code);
            assert_eq!(app.ui.get_heading(), heading);
            assert_eq!(app.state.borrow().prefs.accent, "#d97706");
            assert_eq!(app.state.borrow().prefs.probe_target, "10.20.0.10:443");
            assert!(!app.ui.get_locked());
            platform::capture(&window, false);
            save(&format!("language-{code}.png"))?;
        }
        app.open("screen-lock")?;
        app.ui.invoke_pin_setup(false);
        platform::capture(&window, false);
        if let Ok(path) = std::env::var("U60_SCREEN_TEST_IMAGES") {
            let path = std::path::Path::new(&path);
            std::fs::create_dir_all(path)?;
            platform::save_png(
                &path.join("pin-pad.png"),
                &platform::capture(&window, false),
            )?;
        }
        let type_pin = || {
            for (x, y) in [
                (60., 205.),
                (160., 205.),
                (260., 205.),
                (60., 263.),
                (160., 263.),
                (260., 263.),
            ] {
                tap(x, y);
            }
            tap(260., 379.);
        };
        let finish_password = || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while app.state.borrow().security.busy() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(10));
                slint::platform::update_timers_and_animations();
            }
            assert!(!app.state.borrow().security.busy());
            platform::capture(&window, false);
        };
        type_pin();
        assert!(app.state.borrow().security.mode == crate::security::Mode::Confirm);
        type_pin();
        finish_password();
        assert!(app.ui.get_pin_enabled());
        assert!(!app.ui.get_pin_visible());
        app.ui.invoke_lock_now();
        platform::capture(&window, false);
        hold();
        assert!(app.ui.get_locked());
        assert!(app.ui.get_pin_visible());
        platform::capture(&window, false);
        app.ui.invoke_navigate(1);
        assert_eq!(
            app.ui.get_heading(),
            "锁屏与密码",
            "locked callbacks must not navigate"
        );
        type_pin();
        finish_password();
        assert!(!app.ui.get_locked());
        app.ui.invoke_power();
        assert!(
            app.ui.get_locked(),
            "sleep must discard the unlocked session"
        );
        Ok(())
    }
}
