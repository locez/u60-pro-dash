#!/usr/bin/env python3
"""Deploy the opt-in Tailscale gateway to a legacy (pre-OTA) U60 installation."""
import argparse
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parent
PREFIX = "/data/local/tmp"
NET_SCRIPT = PREFIX + "/tailscale-gateway.sh"
LEGACY_NET_SCRIPT = PREFIX + "/u60-home-network.sh"
CONFIG = PREFIX + "/tailscale-gateway.conf"
BACKUP = "/data/tailscale-gateway-backup"
LEGACY_BACKUP = "/data/u60-home-gateway-backup"
TS = "/data/tailscale/bin/tailscale --socket=/tmp/tailscale/tailscaled.sock"
START = PREFIX + "/start_zte_agent.sh"
REMOTE_NETS = []
LAN_NET = ""

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--check", action="store_true", help="read-only preflight")
parser.add_argument("--rollback", action="store_true", help="restore the saved agent/startup and remove gateway rules")
parser.add_argument("--remote-cidr", action="append", default=[], help="approved IPv4 subnet to forward; repeat for several subnets")
parser.add_argument("--lan-interface", default="br-lan", help="hotspot LAN interface (default: br-lan)")
args = parser.parse_args()
adb = ["adb"]
if os.environ.get("ANDROID_SERIAL"):
    adb += ["-s", os.environ["ANDROID_SERIAL"]]


def say(message):
    print(message, flush=True)


def remote(command, timeout=60):
    # This firmware's legacy ADB shell may return exit 0 for a failed command.
    wrapped = "sh -c " + shlex.quote(command) + '; rc=$?; printf "\\n__U60_GATEWAY_RC__%s\\n" "$rc"'
    proc = subprocess.run(adb + ["shell", wrapped], text=True, capture_output=True, timeout=timeout)
    match = re.search(r"\n__U60_GATEWAY_RC__(\d+)\s*$", proc.stdout)
    if proc.returncode or not match:
        raise RuntimeError("ADB 未返回执行结果：" + proc.stderr.strip())
    result = proc.stdout[:match.start()].replace("\r\n", "\n")
    if int(match.group(1)):
        raise RuntimeError("设备执行失败：" + result[-1500:] + proc.stderr[-500:])
    return result.strip()


def push_checked(local, destination):
    content = Path(local).read_bytes()
    digest = hashlib.sha256(content).hexdigest()
    subprocess.run(adb + ["push", str(local), destination], check=True, capture_output=True)
    got = remote("sha256sum " + shlex.quote(destination)).split()[0]
    if got != digest:
        raise RuntimeError("文件传输 SHA-256 不匹配：" + destination)


def status():
    return json.loads(remote("pidof tailscaled >/dev/null && " + TS + " status --json", timeout=15))


def stop_processes():
    remote("killall zte-agent 2>/dev/null || true; sleep 1; "
           "killall tailscaled 2>/dev/null || true; "
           "n=0; while pidof tailscaled >/dev/null; do "
           'n=$((n+1)); [ "$n" -lt 20 ] || exit 1; sleep 1; done')


def wait_running(tun, seconds=100):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        try:
            value = status()
            if (value.get("BackendState") == "Running"
                    and value.get("TUN") is tun
                    and value.get("Self", {}).get("Online")):
                return value
        except (RuntimeError, ValueError):
            pass
        time.sleep(3)
    raise RuntimeError("Tailscale 未按预期联网。")


def rollback():
    remote("test -s " + BACKUP + "/zte-agent && test -s " + BACKUP + "/startup.sh")
    say("恢复原版 agent 和启动文件...")
    # Remove only the proxy owned by this deployment. Preflight required no
    # pre-existing Serve configuration.
    remote("if pidof tailscaled >/dev/null; then " + TS + " serve --tcp=9090 off >/dev/null 2>&1 || true; fi; "
           "if [ -f " + NET_SCRIPT + " ]; then sh " + NET_SCRIPT + " remove; "
           "elif [ -f " + LEGACY_NET_SCRIPT + " ]; then sh " + LEGACY_NET_SCRIPT + " remove; fi")
    stop_processes()
    remote("cp " + BACKUP + "/zte-agent /data/zte-agent.restore && "
           "chmod 755 /data/zte-agent.restore && mv /data/zte-agent.restore /data/zte-agent && "
           "cp " + BACKUP + "/startup.sh " + START + " && "
           "chmod 700 " + START + " && sh " + START + " </dev/null; sleep 2; pidof zte-agent >/dev/null")
    wait_running(False)
    pref = json.loads(remote("cat " + BACKUP + "/prefs.json"))
    mode = {0: "off", 1: "nodivert", 2: "on"}[pref["NetfilterMode"]]
    accept = str(pref["RouteAll"]).lower()
    remote(TS + " set --accept-routes=" + accept + " --netfilter-mode=" + mode)
    say("已恢复原版 userspace 模式；Tailscale 登录身份已保留。")


def preflight():
    global LAN_NET, REMOTE_NETS
    if not re.fullmatch(r"[A-Za-z0-9_.:-]{1,15}", args.lan_interface):
        raise RuntimeError("LAN 接口名称无效。")
    if not args.remote_cidr:
        raise RuntimeError("请使用 --remote-cidr 显式指定目标子网，可重复此选项。")
    REMOTE_NETS = list(dict.fromkeys(str(ipaddress.IPv4Network(net, strict=True)) for net in args.remote_cidr))
    if len(REMOTE_NETS) > 8:
        raise RuntimeError("一次最多配置 8 条目标子网。")
    remote("test -c /dev/net/tun && test -s " + START
           + " && test -x /data/zte-agent && test -x /data/tailscale/bin/tailscaled"
           + " && test \"$(cat /proc/sys/net/ipv4/ip_forward)\" = 1")
    value = status()
    if not value.get("Version", "").startswith("1.104.4-"):
        raise RuntimeError("此部署包按 Tailscale 1.104.4 验证；当前版本不匹配。")
    if value.get("BackendState") != "Running" or not value.get("Self", {}).get("Online"):
        raise RuntimeError("先通过 Tailscale 完成登录并联网。")
    if value.get("TUN"):
        raise RuntimeError("已经是 TUN 模式；请先检查现有配置，或使用 --rollback。")
    lan = remote("ip -o -4 addr show dev " + shlex.quote(args.lan_interface))
    addresses = re.findall(r"\binet ([0-9.]+/[0-9]+)", lan)
    if len(addresses) != 1:
        raise RuntimeError("需要唯一的热点 LAN IPv4 地址，当前接口无法明确识别。")
    LAN_NET = str(ipaddress.IPv4Interface(addresses[0]).network)
    for net in REMOTE_NETS:
        if ipaddress.ip_network(LAN_NET).overlaps(ipaddress.ip_network(net)):
            raise RuntimeError("目标子网与热点 LAN 重叠：" + net)
        providers = [p for p in value.get("Peer", {}).values()
                     if net in (p.get("PrimaryRoutes") or []) and p.get("Online")]
        if not providers:
            raise RuntimeError("未发现在线且已批准的子网路由 " + net)
    serve = json.loads(remote(TS + " serve status --json") or "{}")
    if serve:
        raise RuntimeError("已有 Tailscale Serve 配置；本部署包不会覆盖它。")
    say("检测通过：热点 LAN " + LAN_NET + " → Tailscale → " + ", ".join(REMOTE_NETS))
    return value


def main():
    global BACKUP
    state = subprocess.run(adb + ["get-state"], capture_output=True, text=True, check=True)
    if state.stdout.strip() != "device":
        raise RuntimeError("ADB 设备未就绪。")
    # The upstream OTA bootstrap owns /data/zte-agent as a slot symlink.
    # This legacy deploy/rollback path must never replace that layout.
    layout = remote("if [ -L /data/zte-agent ] || [ -e /data/open-u60/update/active ]; "
                    "then echo ota; else echo legacy; fi")
    if layout != "legacy":
        raise RuntimeError("检测到 OTA 槽位安装。本脚本只支持旧版直启布局，未执行部署或回滚；请参阅本目录 README。")
    # Reuse an existing original backup; renaming must never replace its provenance.
    backup_kind = remote("if [ -d " + LEGACY_BACKUP + " ]; then echo legacy; else echo current; fi")
    if backup_kind == "legacy":
        BACKUP = LEGACY_BACKUP
    if args.rollback:
        rollback()
        return
    preflight()
    if args.check:
        return
    artifacts = Path(os.environ.get("U60_GATEWAY_ARTIFACT_DIR", ROOT))
    binary = artifacts / "zte-agent"
    network = ROOT / "tailscale-gateway.sh"
    if not binary.is_file() or not network.is_file():
        raise RuntimeError("部署包不完整：需要 zte-agent 和 tailscale-gateway.sh。")
    if (artifacts / "zte-agent.sha256").read_text().split()[0] != hashlib.sha256(binary.read_bytes()).hexdigest():
        raise RuntimeError("本地 agent 与发布包校验值不匹配。")
    say("保存原版 agent、启动脚本和 Tailscale 偏好...")
    # Retain the first known-good original across reruns/rollback.
    remote("umask 077; if [ ! -e " + BACKUP + " ]; then "
           "mkdir " + BACKUP + "; cp /data/zte-agent " + BACKUP + "/zte-agent; "
           "cp " + START + " " + BACKUP + "/startup.sh; fi; "
           "test -s " + BACKUP + "/zte-agent && test -s " + BACKUP + "/startup.sh")
    prefs = json.loads(remote(TS + " debug prefs"))
    saved = json.dumps({key: prefs[key] for key in ("RouteAll", "NetfilterMode")})
    remote("if [ ! -s " + BACKUP + "/prefs.json ]; then printf '%s\\n' "
           + shlex.quote(saved) + " >" + BACKUP + "/prefs.json; fi")
    push_checked(binary, "/data/zte-agent.gateway-new")
    push_checked(network, NET_SCRIPT + ".new")
    push_checked(ROOT.parent / "u60-home-gateway/u60-home-network.sh", LEGACY_NET_SCRIPT + ".new")
    with tempfile.TemporaryDirectory(prefix="u60-gateway-") as directory:
        config = Path(directory) / "tailscale-gateway.conf"
        config.write_text("LAN_IF=" + args.lan_interface + "\nLAN_NET=" + LAN_NET
                          + "\nREMOTE_NETS=" + " ".join(REMOTE_NETS) + "\n")
        push_checked(config, CONFIG + ".new")
    remote("sh -n " + NET_SCRIPT + ".new && chmod 700 " + NET_SCRIPT + ".new")
    # No credential is read or printed on the host. Keep every existing
    # startup line and insert the one opt-in flag immediately after the shebang.
    remote("sed '2i export ZTE_AGENT_TAILSCALE_GATEWAY=1' "
           + START + " >" + START + ".gateway-new && "
           "chmod 700 " + START + ".gateway-new && sh -n " + START + ".gateway-new")
    try:
        # Set netfilter=off BEFORE the first native daemon spawn. Otherwise
        # saved preferences could briefly install cellular-conflicting rules.
        remote(TS + " set --netfilter-mode=off --accept-routes=true")
        say("切换网关模式：短暂重启 agent/Tailscale...")
        stop_processes()
        remote("mv " + CONFIG + ".new " + CONFIG + " && chmod 600 " + CONFIG + " && "
               "mv " + NET_SCRIPT + ".new " + NET_SCRIPT + " && "
               "mv " + LEGACY_NET_SCRIPT + ".new " + LEGACY_NET_SCRIPT + " && "
               "chmod 755 /data/zte-agent.gateway-new && "
               "mv /data/zte-agent.gateway-new /data/zte-agent && "
               "mv " + START + ".gateway-new " + START + " && sh " + START + " </dev/null; sleep 2; pidof zte-agent >/dev/null")
        current = wait_running(True)
        # The supervisor may still be applying preferences while status first
        # reports Running; wait briefly for the imported subnet.
        for net in REMOTE_NETS:
            network = ipaddress.ip_network(net)
            destination = str(network.network_address + (network.num_addresses > 1))
            for attempt in range(15):
                route = remote("ip -4 route get " + destination)
                if "dev tailscale0" in route:
                    break
                time.sleep(2)
            else:
                raise RuntimeError("子网路由未进入 tailscale0：" + net)
        live = json.loads(remote(TS + " debug prefs"))
        if live.get("RouteAll") is not True or live.get("NetfilterMode") != 0:
            raise RuntimeError("Tailscale 接受路由或防火墙模式不符合网关配置。")
        remote("sh " + NET_SCRIPT + " apply")
        say("子网路由和热点转发规则已就绪；请从热点客户端检测指定目标服务。")
        say("设备原始备份：" + BACKUP)
        say("回滚：bash " + str(ROOT / "manage-gateway.sh") + " --rollback")
        say("U60 Tailscale IP：" + ", ".join(current.get("TailscaleIPs", [])))
    except BaseException:
        say("网关验证失败，正在回滚...")
        try:
            rollback()
        except Exception as recovery_error:
            say("自动回滚未完成：" + str(recovery_error))
            say("保留 USB 连接，运行本脚本 --rollback；备份位于 " + BACKUP)
        raise


try:
    main()
except (RuntimeError, ValueError, OSError, subprocess.SubprocessError) as error:
    say("错误：" + str(error))
    sys.exit(1)
except KeyboardInterrupt:
    say("已中断。")
    sys.exit(130)
