//! A small independent cleanup process. EOF on its private socket means that
//! the owning agent stopped or died; it retains the same flock until cleanup
//! finishes, so a replacement agent cannot race it or inherit stale DNS rules.
use std::{
    fs::File,
    os::fd::OwnedFd,
    os::unix::net::UnixStream,
    path::Path,
    process::{Child, Command, Stdio},
};

pub fn launch(root: &Path, adapter: &str, lease: &File) -> Result<(UnixStream, Child), String> {
    let (owner, child) =
        UnixStream::pair().map_err(|_| "Cannot create proxy supervisor channel")?;
    let stdin: OwnedFd = child.into();
    let executable = std::env::current_exe().map_err(|_| "Cannot locate agent executable")?;
    #[cfg(test)]
    let executable = std::env::var_os("U60_AGENT_TEST_BINARY")
        .map(std::path::PathBuf::from)
        .unwrap_or(executable);
    let child = Command::new(executable)
        .arg("--proxy-watchdog")
        .arg(root)
        .arg(adapter)
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(
            lease.try_clone().map_err(|_| "Cannot retain proxy lease")?,
        ))
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Cannot start proxy cleanup supervisor")?;
    Ok((owner, child))
}

pub fn cli(args: &[String]) -> i32 {
    let Some(root) = args.first().map(Path::new) else {
        return 2;
    };
    let Some(adapter) = args.get(1).and_then(|a| super::engine::adapter(a).ok()) else {
        return 2;
    };
    let mut command = adapter.command(
        &root.join("engine"),
        root,
        &root.join("runtime.json"),
        false,
    );
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    use std::os::unix::process::CommandExt;
    // Here the parent is this dedicated process's main thread, which lives
    // until the engine is reaped (not a transient HTTP blocking-pool thread).
    unsafe {
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut engine = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            cleanup(root);
            return 1;
        }
    };
    let mut failed = false;
    let stdout = super::logs::capture(root, engine.stdout.take().unwrap());
    let stderr = super::logs::capture(root, engine.stderr.take().unwrap());
    let mut fd = libc::pollfd {
        fd: 0,
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        match engine.try_wait() {
            Ok(None) => {}
            _ => {
                failed = true;
                break;
            }
        }
        // SAFETY: fd points to one live pollfd; the inherited socket is fd 0.
        let result = unsafe { libc::poll(&mut fd, 1, 500) };
        if result > 0 {
            break;
        }
        if result < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            failed = true;
            break;
        }
    }
    let _ = engine.kill();
    let _ = engine.wait();
    let _ = stdout.recv_timeout(std::time::Duration::from_secs(1));
    let _ = stderr.recv_timeout(std::time::Duration::from_secs(1));
    cleanup(root);
    if failed {
        1
    } else {
        0
    }
}

pub fn cleanup(root: &Path) {
    let _ = super::network::locked(root, || {
        if root.join("capture-owned").exists() {
            super::network::disable();
            let _ = std::fs::remove_file(root.join("capture-owned"));
        }
        Ok(())
    });
    let _ = std::fs::remove_file(root.join("runtime.json"));
}
