#!/usr/bin/env python3
"""Build and deploy this fork's agent and native screen as a recoverable pair."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import shutil
import struct
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parents[2]
REMOTE = "/data/u60-dash"
TARGET = "aarch64-unknown-linux-musl"
FILES = ("zte-agent", "u60-screen", "service.sh", "manage.sh")
MARKER = "__U60_DASH_RC__"
# Audited predecessor: same release/lock/recovery protocol; only the exporter
# fallback changes. Unknown or locally modified managers still require restore.
COMPATIBLE_MANAGERS = {
    "e8e7bc1d9c6167686b6f0f4669eae81e903d3f1fe7dec73ac14923ab1694299d",
    "152743cd07d21704f57cc27d03126e35c7d8e4dc71f2c9ce7922c135fc674dcb",
}

class Failure(RuntimeError):
    pass

def shell(command):
    return "sh -c " + shlex.quote(command) + '; code=$?; printf "\\n' + MARKER + '%s\\n" "$code"'

def decode(result):
    match = re.search(r"\n" + MARKER + r"(\d+)\s*$", result.stdout)
    if result.returncode or not match:
        detail = (result.stderr or "").strip()[-400:]
        raise Failure("设备连接中断或没有返回执行结果；先运行 status，确认结果后再重试。" + ("\n" + detail if detail else ""))
    output = result.stdout[:match.start()].replace("\r\n", "\n").strip()
    if int(match.group(1)):
        raise Failure("设备操作失败：" + output[-1200:])
    return output

class Connection:
    def __init__(self, args):
        self.ssh = args.ssh
        if self.ssh:
            if self.ssh.startswith("-") or not re.fullmatch(r"[A-Za-z0-9_.:@\[\]-]+", self.ssh):
                raise Failure("SSH 地址无效；使用 root@设备地址。")
            self.command = ["ssh", "-p", str(args.port), "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", self.ssh]
        else:
            self.command = ["adb"] + (["-s", args.serial] if args.serial else [])
    def remote(self, command, timeout=30):
        argv = self.command + ([shell(command)] if self.ssh else ["shell", shell(command)])
        try:
            return decode(subprocess.run(argv, text=True, capture_output=True, timeout=timeout))
        except subprocess.TimeoutExpired:
            raise Failure("设备操作超时，先运行 status 检查；不会自动重发。") from None
    def push(self, source, destination):
        if self.ssh:
            with open(source, "rb") as stream:
                proc = subprocess.run(self.command + ["umask 077; cat > " + shlex.quote(destination)], stdin=stream, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
        else:
            proc = subprocess.run(self.command + ["push", str(source), destination], capture_output=True, timeout=120)
        if proc.returncode:
            raise Failure("传输失败：" + Path(source).name)
        expected = digest(source)
        actual = self.remote("sha256sum " + shlex.quote(destination)).split()[0]
        if actual != expected:
            raise Failure("SHA-256 不匹配：" + Path(source).name)

def digest(path):
    with open(path, "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()

def elf(path):
    data = Path(path).read_bytes()
    if len(data) < 64 or data[:6] != b"\x7fELF\x02\x01" or struct.unpack_from("<H", data, 18)[0] != 183:
        raise Failure(f"{Path(path).name} 必须是静态 Linux ARM64 ELF。")
    offset = struct.unpack_from("<Q", data, 32)[0]
    size, count = struct.unpack_from("<HH", data, 54)
    if size < 56 or not 0 < count < 256 or offset + size * count > len(data):
        raise Failure("ELF 程序头无效。")
    if any(struct.unpack_from("<I", data, offset + size * i)[0] == 3 for i in range(count)):
        raise Failure("拒绝依赖动态加载器的设备产物。")

def build(args):
    subprocess.run([sys.executable, str(REPO / "scripts/i18n/build.py"), "--check"], cwd=REPO, check=True)
    npm = shutil.which("npm")
    if not npm:
        raise Failure("缺少 Node.js/npm；Web 静态资源需要 Node.js 22 或更新版本构建。")
    if not (REPO / "u60-web/node_modules/.bin/vite").is_file():
        subprocess.run([npm, "ci"] + (["--offline"] if args.offline else []), cwd=REPO / "u60-web", check=True)
    subprocess.run([npm, "run", "build"], cwd=REPO / "u60-web", check=True)
    env = os.environ.copy()
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip()
    # build.rs watches this variable; Cargo otherwise reuses the old embedded
    # commit when only Git refs changed since the previous build.
    env["ZTE_AGENT_COMMIT"] = revision
    root = Path(env.get("U60_BUILD_ROOT", Path.home() / ".local/share/u60-build"))
    toolchain = root / "rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin"
    cargo = env.get("CARGO") or (str(toolchain / "cargo") if (toolchain / "cargo").is_file() else shutil.which("cargo"))
    if not cargo:
        raise Failure("缺少 Cargo；请先安装 Rust 与 ARM64 musl 标准库。")
    if (root / "cargo").is_dir():
        env.setdefault("CARGO_HOME", str(root / "cargo"))
    if (toolchain / "rustc").is_file():
        env.setdefault("RUSTC", str(toolchain / "rustc"))
    linker = env.get("CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER") or shutil.which("aarch64-linux-musl-gcc") or shutil.which("aarch64-unknown-linux-gnu-gcc")
    if not linker:
        raise Failure("缺少 ARM64 交叉链接器；设置 CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER。")
    env["CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER"] = linker
    env.setdefault("CC_aarch64_unknown_linux_musl", linker)
    common = [cargo, "build", "--locked", "--release", "--target", TARGET]
    if args.offline:
        common.append("--offline")
    destination = Path(args.artifacts).resolve()
    destination.mkdir(parents=True, exist_ok=True)
    for name, extra, target_dir in [
        ("zte-agent", ["-p", "zte-agent"], REPO / "target"),
        ("u60-screen", ["--manifest-path", "u60-screen/Cargo.toml", "--no-default-features", "--features", "device"], REPO / "target/u60-screen"),
    ]:
        print(f"构建 {name}…", flush=True)
        subprocess.run(common + extra, cwd=REPO, env={**env, "CARGO_TARGET_DIR": str(target_dir)}, check=True)
        artifact = target_dir / TARGET / "release" / name
        elf(artifact)
        shutil.copyfile(artifact, destination / name)
    for name in ("service.sh", "manage.sh"):
        shutil.copyfile(REPO / "scripts/dash" / name, destination / name)
    hashes = {name: digest(destination / name) for name in FILES}
    pair_id = hashlib.sha256(json.dumps(hashes, sort_keys=True).encode()).hexdigest()[:20]
    if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip() != revision:
        raise Failure("构建期间 Git 版本发生变化；请重新构建以保证版本信息一致。")
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=REPO, text=True).strip())
    manifest = {"format": 1, "id": pair_id, "source_commit": revision, "source_dirty": dirty, "sha256": hashes}
    (destination / "release.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (destination / "manifest.sha256").write_text("".join(f"{value}  {name}\n" for name, value in hashes.items()))
    print(f"已构建两个程序，版本组 {pair_id}\n产物：{destination}")

def package(args):
    root = Path(args.artifacts).resolve()
    try:
        manifest = json.loads((root / "release.json").read_text())
        hashes = manifest["sha256"]
        if set(hashes) != set(FILES) or manifest["format"] != 1:
            raise ValueError
        for name in FILES:
            if hashes[name] != digest(root / name):
                raise ValueError
        identity = hashlib.sha256(json.dumps(hashes, sort_keys=True).encode()).hexdigest()[:20]
        if manifest["id"] != identity or (root / "manifest.sha256").read_text() != "".join(f"{hashes[n]}  {n}\n" for n in FILES):
            raise ValueError
        for name in ("zte-agent", "u60-screen"):
            elf(root / name)
    except (OSError, ValueError, KeyError, TypeError):
        raise Failure("部署包缺失或校验失败；请运行 bash dash.sh build。") from None
    return root, identity

def check(connection):
    script = """
set -eu
test "$(id -u)" = 0
test "$(uname -m)" = aarch64
test -x /data/zte-agent
test -s /data/local/tmp/start_zte_agent.sh
test -c /dev/dri/card0
test -r /sys/class/leds/led:lcd/brightness
test -x /etc/init.d/zte_topsw_devui
test ! -e /tmp/u60-native-screen-trial
test ! -s /data/open-u60/update/pending
grep -qF /data/local/tmp/start_zte_agent.sh /etc/rc.local
for tool in flock sha256sum readlink nohup ubus; do command -v "$tool" >/dev/null; done
# Exercise the BusyBox-compatible flags, using a read-only descriptor.
exec 8</data/local/tmp/start_zte_agent.sh
flock -s -n 8
flock -u 8
exec 8<&-
ubus call zwrt_topsw_daemon.sync get_sync_info '{}' | grep -q '"noSyncModuleName"[[:space:]]*:[[:space:]]*"sync success"'
if test -e /data/u60-dash; then test -d /data/u60-dash && test ! -L /data/u60-dash; fi
printf 'device=root ARM64; boot=ready; recovery=available\n'
if test -L /data/zte-agent; then printf 'existing_layout=linked\n'; else printf 'existing_layout=direct\n'; fi
df -Pk /data | tail -1 | awk '{print "free_kib=" $4}'
"""
    result = connection.remote(script)
    free = re.search(r"free_kib=(\d+)", result)
    if not free or int(free.group(1)) < 150 * 1024:
        raise Failure("设备 /data 至少需要 150 MiB 空闲空间，用于新旧版本与回退。")
    print(result, flush=True)
    return result

def inactive_manager_condition():
    return (
        f"test ! -e /tmp/u60-dash-run && test ! -e {REMOTE}/pending "
        f"&& test ! -e {REMOTE}/enabled "
        f"&& (test ! -e {REMOTE}/current || test \"$(cat {REMOTE}/current)\" = none) "
        f"&& test -s /data/local/tmp/start_zte_agent.sh "
        f"&& ! grep -qF {REMOTE}/manage.sh /data/local/tmp/start_zte_agent.sh"
    )

def replace_inactive_manager(connection, source):
    # Recheck under the operation lock: another installer may have started
    # between preflight and upload. Never replace the lock file itself.
    connection.remote(f"""
set -eu
exec 9>{REMOTE}/operation.lock
flock -n 9 || {{ echo 'Deployment lock unavailable; retry after the active operation finishes.'; exit 1; }}
if ! ({inactive_manager_condition()}); then
    echo 'The project manager is active; run bash dash.sh restore before replacing it.'
    exit 1
fi
cp {shlex.quote(source)} {REMOTE}/manage.sh.new
chmod 700 {REMOTE}/manage.sh.new
mv -f {REMOTE}/manage.sh.new {REMOTE}/manage.sh
""")

def replace_compatible_manager(connection, source, expected):
    if expected not in COMPATIBLE_MANAGERS:
        raise Failure("部署管理器不在已验证的兼容版本中。")
    connection.remote(f"""
set -eu
exec 9>{REMOTE}/operation.lock
flock -n 9 || {{ echo 'Deployment operation is active.'; exit 1; }}
test ! -e {REMOTE}/pending || {{ echo 'Wait for the active deployment trial.'; exit 1; }}
actual=$(sha256sum {REMOTE}/manage.sh); actual=${{actual%% *}}
test "$actual" = {shlex.quote(expected)} || {{ echo 'Deployment manager changed; retry after checking its version.'; exit 1; }}
sh -n {shlex.quote(source)}
cp -p {REMOTE}/manage.sh {REMOTE}/manage.sh.previous
cp {shlex.quote(source)} {REMOTE}/manage.sh.new
chmod 700 {REMOTE}/manage.sh.new
sync
mv -f {REMOTE}/manage.sh.new {REMOTE}/manage.sh
""")

def install(connection, args):
    root, identity = package(args)
    check(connection)
    connection.remote(f"umask 077; mkdir -p {REMOTE}/releases; chmod 700 {REMOTE} {REMOTE}/releases")
    existing = connection.remote(f"if test -s {REMOTE}/manage.sh; then sha256sum {REMOTE}/manage.sh; fi")
    compatible = None
    if existing and existing.split()[0] != digest(root / "manage.sh"):
        inactive = connection.remote(f"if {inactive_manager_condition()}; then echo restored; else echo active; fi")
        if inactive != "restored" and existing.split()[0] in COMPATIBLE_MANAGERS:
            compatible = existing.split()[0]
        elif inactive != "restored":
            raise Failure("设备部署管理器版本不同；请先运行 bash dash.sh restore，再安装新版本。原备份不会被覆盖。")
        else:
            existing = ""
    destination = f"{REMOTE}/releases/{identity}"
    present = connection.remote(f"if test -d {destination}; then echo yes; else echo no; fi")
    if present == "no":
        staging = destination + ".incoming-" + secrets.token_hex(6)
        connection.remote(f"test ! -e {staging} && mkdir {staging}")
        try:
            for name in (*FILES, "release.json", "manifest.sha256"):
                connection.push(root / name, staging + "/" + name)
            connection.remote(f"cd {staging} && sha256sum -c manifest.sha256 >/dev/null && chmod 700 zte-agent u60-screen service.sh manage.sh && sh -n service.sh && sh -n manage.sh && test ! -e {destination} && mv {staging} {destination}")
        finally:
            try:
                connection.remote(f"rm -rf {staging}")
            except (Failure, OSError, subprocess.SubprocessError):
                print("未能清理未激活的上传目录：" + staging, file=sys.stderr)
    if compatible:
        replace_compatible_manager(connection, destination + "/manage.sh", compatible)
    elif not existing:
        replace_inactive_manager(connection, destination + "/manage.sh")
    print("文件已传输并校验；开始部署两个程序。30 秒健康观察通过后自动确认。", flush=True)
    operation = f'sh {REMOTE}/manage.sh install {identity}; result=$?; printf "%s\\n" "$result" >{REMOTE}/install.exit.new; mv {REMOTE}/install.exit.new {REMOTE}/install.exit'
    connection.remote(f"rm -f {REMOTE}/install.exit; nohup sh -c {shlex.quote(operation)} >{REMOTE}/install.log 2>&1 </dev/null & sleep 1")
    deadline = time.monotonic() + 240
    while time.monotonic() < deadline:
        time.sleep(3)
        state = connection.remote(f"sh {REMOTE}/manage.sh status; if test -f {REMOTE}/install.exit; then printf 'install_exit='; cat {REMOTE}/install.exit; fi")
        exit_code = re.search(r"^install_exit=(\d+)$", state, re.MULTILINE)
        if exit_code and exit_code.group(1) != "0":
            raise Failure("安装管理器未完成；设备端原因见 /data/u60-dash/install.log。\n" + state)
        if f"selected={identity}" in state and "trial=none" in state and "screen_frame=ready" in state and "agent=running" in state:
            print(state)
            print("两个程序已部署并通过健康检查。", flush=True)
            return
        if "was rolled back" in state or "Recovery requires attention" in state:
            raise Failure(state)
    raise Failure("健康观察尚未成功；运行 bash dash.sh status 查看状态。设备会按自己的恢复流程处理，不会自动重复安装。")

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["build", "check", "install", "status", "start", "stop", "rollback", "restore", "help"], nargs="?", default="help")
    parser.add_argument("--ssh", help="root@设备地址；默认使用 ADB")
    parser.add_argument("--port", type=int, default=2222)
    parser.add_argument("--serial", default=os.environ.get("ANDROID_SERIAL"), help="ADB 设备序列号")
    parser.add_argument("--artifacts", default=str(REPO / "target/dash"))
    parser.add_argument("--no-build", action="store_true", help="安装已经通过 manifest 校验的部署包")
    parser.add_argument("--offline", action="store_true", help="Cargo 离线构建")
    args = parser.parse_args(argv)
    if not 1 <= args.port <= 65535:
        parser.error("SSH 端口无效")
    if args.command == "help":
        parser.print_help()
        print("\n常用：bash dash.sh check；bash dash.sh install；bash dash.sh status\nstop：恢复原厂屏幕，保留后台；start：重新启动两个程序。\nrollback：恢复上一组；restore：恢复首次安装前的后台及启动文件。")
        return 0
    if args.command == "build":
        build(args)
        return 0
    connection = Connection(args)
    if args.command == "check":
        check(connection)
    elif args.command == "install":
        if not args.no_build:
            build(args)
        install(connection, args)
    elif args.command == "status":
        print(connection.remote(f"if test -s {REMOTE}/manage.sh; then sh {REMOTE}/manage.sh status; else echo 'paired_deployment=not_installed'; fi"))
    else:
        print(connection.remote(f"sh {REMOTE}/manage.sh {args.command} && sleep 1", timeout=150))
        print(connection.remote(f"sh {REMOTE}/manage.sh status"))
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (Failure, OSError, subprocess.SubprocessError) as error:
        print(f"部署未完成：{error}", file=sys.stderr)
        sys.exit(1)
