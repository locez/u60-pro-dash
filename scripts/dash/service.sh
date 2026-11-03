#!/bin/sh
# One release owns the pair. A detached guard restores the factory UI even
# after SIGKILL of the supervisor or a stalled screen event loop.
set -eu
umask 077
ROOT=/data/u60-dash
RUN=/tmp/u60-dash-run
PREFS=/data/local/tmp/u60-native-screen/appearance.json
VENDOR=/etc/init.d/zte_topsw_devui
BACKLIGHT=/sys/class/leds/led:lcd/brightness
RELEASE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

is_process() {
    case "${1:-}" in ''|*[!0-9]*) return 1;; esac
    [ -r "/proc/$1/cmdline" ] && tr '\000' '\n' < "/proc/$1/cmdline" | grep -Fx -- "$2" >/dev/null
}
kill_binary() {
    for dir in /proc/[0-9]*; do
        pid=${dir##*/}
        if is_process "$pid" "$1"; then
            if tr '\000' '\n' < "$dir/cmdline" | grep -Fx -- '--proxy-watchdog' >/dev/null; then continue; fi
            kill -TERM "$pid" 2>/dev/null || true
            wait_count=0
            while is_process "$pid" "$1" && [ "$wait_count" -lt 5 ]; do sleep 1; wait_count=$((wait_count+1)); done
            if is_process "$pid" "$1"; then kill -KILL "$pid" 2>/dev/null || true; fi
        fi
    done
}
wait_vendor_stop() {
    n=0
    while pidof zte_topsw_devui >/dev/null; do
        [ "$n" -lt 20 ] || return 1
        sleep 1; n=$((n+1))
    done
}
recover() {
    exec 9>"$ROOT/recovery.lock"
    flock -n 9 || exit 0
    [ -d "$RUN" ] || exit 0
    intentional=no
    if [ -f "$RUN/intentional" ]; then intentional=yes; fi
    touch "$RUN/recovering"
    if [ -s "$RUN/owner" ]; then
        owner=$(cat "$RUN/owner")
        if is_process "$owner" "$RELEASE/service.sh"; then
            kill -TERM "$owner" 2>/dev/null || true
            owner_wait=0
            while is_process "$owner" "$RELEASE/service.sh" && [ "$owner_wait" -lt 5 ]; do sleep 1; owner_wait=$((owner_wait+1)); done
            if is_process "$owner" "$RELEASE/service.sh"; then kill -KILL "$owner" 2>/dev/null || true; fi
        fi
    fi
    kill_binary "$RELEASE/u60-screen"
    # An isolated screen failure must not interrupt remote networking. Stop
    # the agent only for an explicit lifecycle operation or candidate rollback.
    if [ "$intentional" = yes ] || [ -s "$ROOT/pending" ]; then kill_binary "$RELEASE/zte-agent"; fi
    if [ -f "$RUN/handoff" ]; then
        if [ ! -f "$RUN/vendor-quiesced" ]; then wait_vendor_stop || true; fi
        if ! pidof zte_topsw_devui >/dev/null; then "$VENDOR" start; fi
        sleep 2
        if ! pidof zte_topsw_devui >/dev/null; then "$VENDOR" restart; sleep 2; fi
    fi
    if [ -s "$RUN/brightness" ]; then cat "$RUN/brightness" > "$BACKLIGHT"; fi
    if ! pidof zte_topsw_devui >/dev/null; then
        echo 'Recovery requires attention: factory UI did not restart.' > "$ROOT/last-error"
        exit 1
    fi
    rm -rf "$RUN"
    flock -u 9
    if [ "$intentional" = no ]; then
        if [ -s "$ROOT/pending" ]; then sh "$ROOT/manage.sh" recover 9>&-;
        else sh "$ROOT/manage.sh" fallback 9>&-; fi
    fi
}

if [ "${1:-}" = __recover ]; then recover; exit; fi

if [ "${1:-}" = __guard ]; then
    owner=$2
    touch "$RUN/guard-ready"
    stale=0; last=''
    while [ ! -f "$RUN/stop" ] && is_process "$owner" "$RELEASE/service.sh"; do
        pulse=$(cat "$RUN/owner-heartbeat" 2>/dev/null || true)
        if [ "$pulse" = "$last" ]; then stale=$((stale+1)); else stale=0; last=$pulse; fi
        [ "$stale" -lt 45 ] || break
        sleep 1
    done
    recover
    exit
fi

[ "${1:-}" = run ] || exit 2
mkdir "$RUN" || { echo 'A paired service or recovery is already active.' >&2; exit 1; }
echo "$$" > "$RUN/owner"
printf '%s\n' "$RELEASE" > "$RUN/release"
cat "$BACKLIGHT" > "$RUN/brightness"
guard=''
finish() {
    [ -d "$RUN" ] || return 0
    touch "$RUN/stop"
    if ! is_process "$guard" "$RELEASE/service.sh"; then
        nohup sh "$RELEASE/service.sh" __recover </dev/null >>"$ROOT/service.log" 2>&1 9>&- &
    fi
}
trap finish EXIT
trap 'exit 143' TERM INT HUP
nohup sh "$RELEASE/service.sh" __guard "$$" </dev/null >>"$ROOT/service.log" 2>&1 &
guard=$!
echo "$guard" > "$RUN/guard"
n=0
while [ ! -f "$RUN/guard-ready" ]; do
    kill -0 "$guard" 2>/dev/null && [ "$n" -lt 5 ] || exit 1
    sleep 1; n=$((n+1))
done
sequence=0
pulse() { sequence=$((sequence+1)); printf '%s\n' "$sequence" > "$RUN/owner-heartbeat"; }
# Keep the early-boot registration intact; stop only after the master barrier.
n=0
while ! ubus call zwrt_topsw_daemon.sync get_sync_info '{}' 2>/dev/null | grep -q '"noSyncModuleName"[[:space:]]*:[[:space:]]*"sync success"'; do
    pulse; [ "$n" -lt 120 ] || { echo 'Firmware startup sync timed out.' > "$ROOT/last-error"; exit 1; }
    sleep 1; n=$((n+1))
done
pidof zte_topsw_devui >/dev/null || { echo 'Factory UI is not running.' > "$ROOT/last-error"; exit 1; }
. "$ROOT/agent.env"
export ZTE_AGENT_DEPLOYMENT=paired
export ZTE_OTA_ROOT="$ROOT/agent-update-disabled"
export ZTE_OTA_SUPERVISOR="$ROOT/no-agent-only-supervisor"
"$RELEASE/zte-agent" >"$ROOT/agent.log" 2>&1 &
agent=$!
echo "$agent" > "$RUN/agent"
pulse
if ! "$RELEASE/zte-agent" dash-wait-ready >"$ROOT/probe.log" 2>&1; then
    echo 'The candidate agent HTTP listener or authentication did not become ready.' > "$ROOT/last-error"; exit 1
fi
pulse
if ! "$RELEASE/u60-screen" --probe-agent --local-agent --prefs "$PREFS" >>"$ROOT/probe.log" 2>&1; then
    echo 'The candidate agent failed authenticated live-data verification.' > "$ROOT/last-error"; exit 1
fi
pulse
touch "$RUN/handoff"
"$VENDOR" stop
wait_vendor_stop || { echo 'Factory UI did not finish stopping.' > "$ROOT/last-error"; exit 1; }
touch "$RUN/vendor-quiesced"
U60_SCREEN_HANDOFF=service "$RELEASE/u60-screen" --device --service --local-agent --prefs "$PREFS" >"$ROOT/screen.log" 2>&1 &
screen=$!
echo "$screen" > "$RUN/screen"
n=0
while ! grep -q 'display: first frame submitted' "$ROOT/screen.log" || [ ! -s "$RUN/heartbeat" ]; do
    pulse
    is_process "$screen" "$RELEASE/u60-screen" && [ "$n" -lt 30 ] || { echo 'Screen did not submit a first frame.' > "$ROOT/last-error"; exit 1; }
    sleep 1; n=$((n+1))
done
touch "$RUN/ready"
last='';stale=0
trial_started=$(cut -d. -f1 /proc/uptime)
while [ ! -f "$RUN/stop" ]; do
    pulse
    if ! kill -0 "$guard" 2>/dev/null; then
        nohup sh "$RELEASE/service.sh" __guard "$$" </dev/null >>"$ROOT/service.log" 2>&1 &
        guard=$!;echo "$guard" > "$RUN/guard"
        echo 'Recovery guard stopped; ending paired session.' > "$ROOT/last-error"; exit 1
    fi
    is_process "$agent" "$RELEASE/zte-agent" && is_process "$screen" "$RELEASE/u60-screen" || { echo 'A paired process stopped unexpectedly.' > "$ROOT/last-error"; exit 1; }
    heartbeat=$(cat "$RUN/heartbeat" 2>/dev/null || true)
    if [ "$heartbeat" = "$last" ]; then stale=$((stale+1)); else stale=0;last=$heartbeat;fi
    [ "$stale" -lt 20 ] || { echo 'Screen event loop stopped responding.' > "$ROOT/last-error"; exit 1; }
    stable=$(($(cut -d. -f1 /proc/uptime)-trial_started))
    if [ "$stable" -ge 30 ] && [ -s "$ROOT/pending" ]; then
        if "$RELEASE/zte-agent" dash-health >"$ROOT/health.log" 2>&1; then
            sh "$ROOT/manage.sh" confirm "${RELEASE##*/}"
        elif [ "$stable" -gt 80 ]; then
            echo 'Required network services did not recover; rolling back.' > "$ROOT/last-error"; exit 1
        fi
    fi
    sleep 1
done
exit 0
