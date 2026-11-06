#!/bin/sh
# On-device manual trial. No init/rc.local/OTA-slot edits. An independent process
# owns recovery even if this shell, adb, or the Rust process dies.
set -eu
umask 077
TRIAL_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
LOCK=/tmp/u60-native-screen-trial
VENDOR=/etc/init.d/zte_topsw_devui
BACKLIGHT=/sys/class/leds/led:lcd/brightness

screen_is_ours() {
    [ -r "/proc/$1/cmdline" ] && tr '\000' '\n' < "/proc/$1/cmdline" | head -n 1 | grep -Fx "$TRIAL_DIR/u60-screen" >/dev/null
}

wait_vendor_stop() {
    stop_wait=0
    while pidof zte_topsw_devui >/dev/null; do
        [ "$stop_wait" -lt 20 ] || return 1
        sleep 1
        stop_wait=$((stop_wait + 1))
    done
}

recover() {
    # Scan by exact executable path as well as recording the PID below: the
    # parent could die between fork/exec and writing screen.pid.
    for proc_dir in /proc/[0-9]*; do
        screen_pid=${proc_dir##*/}
        if screen_is_ours "$screen_pid"; then
            kill -TERM "$screen_pid" 2>/dev/null || true
            sleep 2
            if screen_is_ours "$screen_pid"; then kill -KILL "$screen_pid" 2>/dev/null || true; fi
        fi
    done
    if [ -f "$LOCK/handoff" ]; then
        # procd's stop request returns before SIGTERM/kill timeout has finished.
        # Starting a replacement before that exit races the pending stop.
        if [ ! -f "$LOCK/vendor-quiesced" ]; then wait_vendor_stop || true; fi
        if ! pidof zte_topsw_devui >/dev/null; then "$VENDOR" start; fi
        sleep 2
        if ! pidof zte_topsw_devui >/dev/null; then
            echo 'recovery: vendor UI did not start; retrying service restart' >&2
            "$VENDOR" restart
            sleep 2
        fi
    fi
    if [ -f "$LOCK/brightness" ]; then cat "$LOCK/brightness" > "$BACKLIGHT"; fi
    if pidof zte_topsw_devui >/dev/null; then
        echo 'recovery: vendor UI running; backlight restored'
        rm -f "$LOCK/owner" "$LOCK/ready" "$LOCK/handoff" "$LOCK/vendor-quiesced" "$LOCK/finished" "$LOCK/screen.pid" "$LOCK/brightness"
        rmdir "$LOCK"
    else
        echo 'recovery failed: run /etc/init.d/zte_topsw_devui restart; inspect /tmp/u60-native-screen-trial' >&2
        return 1
    fi
}

if [ "${1:-}" = __guard ]; then
    owner=$2
    limit=$3
    touch "$LOCK/ready"
    elapsed=0
    while [ ! -f "$LOCK/finished" ] && kill -0 "$owner" 2>/dev/null && [ "$elapsed" -lt "$limit" ]; do
        sleep 1
        elapsed=$((elapsed + 1))
    done
    recover
    exit
fi

seconds=90
case "${1:-}" in
    --seconds) seconds=${2:?missing duration}; shift 2 ;;
esac
case "$seconds" in ''|*[!0-9]*) echo 'duration must be 5..600 seconds' >&2; exit 2;; esac
[ "$seconds" -ge 5 ] && [ "$seconds" -le 600 ] || { echo 'duration must be 5..600 seconds' >&2; exit 2; }
[ "$(id -u)" = 0 ] || { echo 'run as root on the U60' >&2; exit 1; }
[ -x "$TRIAL_DIR/u60-screen" ] || { echo 'screen binary missing' >&2; exit 1; }
[ -x "$VENDOR" ] && pidof zte_topsw_devui >/dev/null || { echo 'vendor UI must be running before the trial' >&2; exit 1; }
ubus call zwrt_topsw_daemon.sync get_sync_info '{}' | grep -q '"noSyncModuleName"[[:space:]]*:[[:space:]]*"sync success"' || { echo 'firmware boot sync is not complete' >&2; exit 1; }
mkdir "$LOCK" || { echo 'another trial or recovery is active; inspect /tmp/u60-native-screen-trial' >&2; exit 1; }
echo "$$" > "$LOCK/owner"
cat "$BACKLIGHT" > "$LOCK/brightness"
trap 'touch "$LOCK/finished"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
nohup sh "$TRIAL_DIR/device-trial.sh" __guard "$$" "$((seconds + 30))" </dev/null &
guard=$!
ready_wait=0
while [ ! -f "$LOCK/ready" ]; do
    kill -0 "$guard" 2>/dev/null || { echo 'recovery guard failed to start' >&2; exit 1; }
    [ "$ready_wait" -lt 5 ] || { echo 'recovery guard timeout' >&2; exit 1; }
    sleep 1
    ready_wait=$((ready_wait + 1))
done
echo "trial: recovery guard ready; taking over display for $seconds seconds"
touch "$LOCK/handoff"
"$VENDOR" stop
# Stop only the running service, never disable its early-boot registration.
if ! wait_vendor_stop; then echo 'vendor UI did not stop within 20 seconds' >&2; exit 1; fi
touch "$LOCK/vendor-quiesced"
U60_SCREEN_HANDOFF=guarded "$TRIAL_DIR/u60-screen" --device --seconds "$seconds" --prefs "$TRIAL_DIR/appearance.json" "$@" &
screen_pid=$!
echo "$screen_pid" > "$LOCK/screen.pid"
result=0
wait "$screen_pid" || result=$?
touch "$LOCK/finished"
wait "$guard" || result=1
trap - EXIT
exit "$result"
