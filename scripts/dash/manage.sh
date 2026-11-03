#!/bin/sh
# Device-side transaction manager. All paths are private and project-owned.
set -eu
umask 077
ROOT=/data/u60-dash
RUN=/tmp/u60-dash-run
START=/data/local/tmp/start_zte_agent.sh
LEGACY=/data/zte-agent

fail() { echo "$*" >&2; exit 1; }
valid_id() { case "$1" in ''|*[!a-zA-Z0-9._-]*|.*) return 1;; esac; }
atomic() { printf '%s\n' "$2" > "$1.new"; chmod 600 "$1.new"; mv -f "$1.new" "$1"; sync; }
value() { if [ -s "$1" ]; then sed -n '1p' "$1"; else printf 'none\n'; fi; }
link_agent() {
    rm -f "$LEGACY.dash-next"
    ln -s "$ROOT/releases/$1/zte-agent" "$LEGACY.dash-next"
    mv -f "$LEGACY.dash-next" "$LEGACY"
}
agent_running() {
    for process in /proc/[0-9]*; do
        [ -r "$process/cmdline" ] || continue
        if [ "$(tr '\000' '\n' < "$process/cmdline" | head -n 1)" = "$1" ] &&
            [ "$(tr '\000' '\n' < "$process/cmdline" | wc -l)" -eq 1 ]; then return 0; fi
    done
    return 1
}
owned() {
    case "$1" in ''|*[!0-9]*) return 1;; esac
    [ -r "/proc/$1/cmdline" ] || return 1
    tr '\000' '\n' < "/proc/$1/cmdline" | grep -Fx -- "$2" >/dev/null
}
stop_binary() {
    for dir in /proc/[0-9]*; do
        pid=${dir##*/}
        if owned "$pid" "$1"; then
            # The independent proxy cleanup owner must see EOF and finish.
            if tr '\000' '\n' < "$dir/cmdline" | grep -Fx -- '--proxy-watchdog' >/dev/null; then continue; fi
            kill -TERM "$pid" 2>/dev/null || true
            n=0
            while owned "$pid" "$1" && [ "$n" -lt 8 ]; do sleep 1; n=$((n+1)); done
            if owned "$pid" "$1"; then kill -KILL "$pid" 2>/dev/null || true; fi
        fi
    done
}
stop_pair() {
    if [ -d "$RUN" ]; then
        touch "$RUN/intentional"
        touch "$RUN/stop"
        n=0
        while [ -d "$RUN" ] && [ "$n" -lt 60 ]; do
            # If both owners disappeared, invoke the same recovery code.
            release=$(value "$RUN/release");guard=$(value "$RUN/guard")
            if [ -f "$release/service.sh" ] && ! owned "$guard" "$release/service.sh"; then
                sh "$release/service.sh" __recover
            fi
            sleep 1;n=$((n+1))
        done
        [ ! -d "$RUN" ] || fail 'Screen recovery has not completed; inspect /data/u60-dash/last-error.'
    fi
    stop_current=$(value "$ROOT/current")
    if valid_id "$stop_current" && [ "$stop_current" != none ]; then stop_binary "$ROOT/releases/$stop_current/zte-agent"; fi
}
launch() {
    launch_current=$(value "$ROOT/current")
    valid_id "$launch_current" && [ "$launch_current" != none ] && [ -x "$ROOT/releases/$launch_current/zte-agent" ] || fail 'No paired release selected.'
    [ ! -d "$RUN" ] || return 0
    rm -f "$ROOT/last-error"
    link_agent "$launch_current"
    nohup sh "$ROOT/releases/$launch_current/service.sh" run </dev/null >"$ROOT/service.log" 2>&1 9>&- &
}
restore_original() {
    [ -s "$ROOT/original/startup.sh" ] && [ -s "$ROOT/original/agent-content" ] || fail 'Original backup is incomplete.'
    restore_probe=$(value "$ROOT/current")
    if [ "$restore_probe" = none ]; then restore_probe=$(value "$ROOT/pending"); fi
    valid_id "$restore_probe" && [ -x "$ROOT/releases/$restore_probe/zte-agent" ] || fail 'Recovery verifier missing.'
    if ! (. "$ROOT/agent.env"; "$ROOT/releases/$restore_probe/zte-agent" dash-prepare-network) >"$ROOT/original-health.log" 2>&1 9>&-; then
        echo 'Recovery requires attention: management port preparation failed; inspect original-health.log.' > "$ROOT/last-error"
        return 1
    fi
    cp -p "$ROOT/original/startup.sh" "$START.dash-next"
    mv -f "$START.dash-next" "$START"
    rm -f "$LEGACY.dash-next"
    # Restore the referenced OTA binary as well as its launcher symlink.
    if [ -L "$ROOT/original/agent" ]; then
        original_target=$(cat "$ROOT/original/target")
        cp -p "$ROOT/original/agent-content" "$original_target.dash-next"
        mv -f "$original_target.dash-next" "$original_target"
    fi
    cp -Pp "$ROOT/original/agent" "$LEGACY.dash-next"
    mv -f "$LEGACY.dash-next" "$LEGACY"
    atomic "$ROOT/current" none
    rm -f "$ROOT/enabled" "$ROOT/pending"
    sync
    sh "$START" </dev/null >"$ROOT/original-start.log" 2>&1 9>&-
    if ! (. "$ROOT/agent.env"; "$ROOT/releases/$restore_probe/zte-agent" dash-wait-ready) >>"$ROOT/original-health.log" 2>&1 9>&-; then
        echo 'Recovery requires attention: original agent did not become ready; inspect original-health.log.' > "$ROOT/last-error"
        return 1
    fi
}
recover_pending() {
    [ -s "$ROOT/pending" ] || return 0
    candidate=$(sed -n '1p' "$ROOT/pending")
    previous=$(sed -n '2p' "$ROOT/pending")
    if [ "$previous" = none ]; then
        restore_original
    else
        valid_id "$previous" && [ -x "$ROOT/releases/$previous/service.sh" ] || fail 'Previous release missing; keeping recovery evidence.'
        atomic "$ROOT/current" "$previous"
        rm -f "$ROOT/pending"
        launch
    fi
    echo "Candidate $candidate was rolled back." > "$ROOT/last-result"
}
prepare_original() {
    if [ -d "$ROOT/original" ]; then
        [ -s "$ROOT/original/startup.sh" ] && [ -s "$ROOT/original/agent-content" ] || fail 'Original backup is incomplete.'
        return
    fi
    [ -s "$START" ] && [ -x "$LEGACY" ] || fail 'A working existing agent installation is required.'
    [ ! -s /data/open-u60/update/pending ] || fail 'An existing OTA trial is active.'
    grep -qF "$START" /etc/rc.local || fail 'Existing rc.local startup hook missing; run initial agent setup first.'
    rm -rf "$ROOT/original.next"
    mkdir "$ROOT/original.next"
    cp -p "$START" "$ROOT/original.next/startup.sh"
    cp -Pp "$LEGACY" "$ROOT/original.next/agent"
    # Save the referenced binary too, so the provenance is reviewable even if
    # another tool later modifies a former OTA slot.
    cp -p "$LEGACY" "$ROOT/original.next/agent-content"
    if [ -L "$LEGACY" ]; then readlink -f "$LEGACY" > "$ROOT/original.next/target"; fi
    chmod 700 "$ROOT/original.next"
    sync
    mv "$ROOT/original.next" "$ROOT/original"
}
install_candidate() {
    candidate=$1
    valid_id "$candidate" || fail 'Invalid release ID.'
    release="$ROOT/releases/$candidate"
    [ -s "$release/manifest.sha256" ] || fail 'Release manifest missing.'
    (cd "$release" && sha256sum -c manifest.sha256 >/dev/null) || fail 'Candidate checksum mismatch.'
    "$release/zte-agent" --version
    "$release/u60-screen" --version
    [ ! -e /tmp/u60-native-screen-trial ] || fail 'Finish the screen trial before installing.'
    previous=$(value "$ROOT/current")
    if [ "$previous" = "$candidate" ] && [ ! -s "$ROOT/pending" ]; then
        if [ ! -d "$RUN" ]; then stop_pair; touch "$ROOT/enabled"; launch; fi
        echo 'This pair is already installed.'; return
    fi
    [ ! -s "$ROOT/pending" ] || fail 'A paired trial is active.'
    prepare_original
    "$release/zte-agent" dash-export-env
    [ -s "$ROOT/agent.env" ] || fail 'Private agent environment missing.'
    # The pending marker and boot wrapper are durable before any process is stopped.
    atomic "$ROOT/pending" "$candidate
$previous"
    cat >"$START.dash-next" <<'BOOT'
#!/bin/sh
exec sh /data/u60-dash/manage.sh boot
BOOT
    chmod 700 "$START.dash-next"
    mv -f "$START.dash-next" "$START"
    touch "$ROOT/enabled"
    sync
    rm -f "$ROOT/last-result"
    stop_pair
    if [ "$previous" = none ]; then
        stop_binary "$LEGACY"
        stop_binary /data/local/tmp/zte-agent
        if [ -L "$ROOT/original/agent" ]; then stop_binary "$(cat "$ROOT/original/target")"; fi
    fi
    atomic "$ROOT/current" "$candidate"
    launch
}
status() {
    printf 'selected=%s\n' "$(value "$ROOT/current")"
    printf 'previous=%s\n' "$(value "$ROOT/previous")"
    if [ -f "$ROOT/enabled" ]; then echo 'autostart=enabled'; else echo 'autostart=disabled'; fi
    if [ -f "$ROOT/pending" ]; then echo 'trial=pending'; else echo 'trial=none'; fi
    for key in owner guard screen; do
        pid=$(value "$RUN/$key")
        binary="$(value "$RUN/release")/service.sh"
        if [ "$key" = screen ]; then binary="$(value "$RUN/release")/u60-screen"; fi
        if owned "$pid" "$binary"; then printf '%s=running\n' "$key"; else printf '%s=stopped\n' "$key"; fi
    done
    selected=$(value "$ROOT/current")
    if [ "$selected" != none ] && agent_running "$ROOT/releases/$selected/zte-agent"; then echo 'agent=running'; else echo 'agent=stopped'; fi
    if [ "$selected" = none ]; then
        if agent_running "$LEGACY" || { [ -s "$ROOT/original/target" ] && agent_running "$(cat "$ROOT/original/target")"; }; then
            echo 'original_agent=running'
        else echo 'original_agent=stopped'; fi
    fi
    if [ -f "$RUN/ready" ]; then echo 'screen_frame=ready'; else echo 'screen_frame=not_ready'; fi
    if pidof zte_topsw_devui >/dev/null; then echo 'factory_ui=running'; else echo 'factory_ui=stopped'; fi
    if [ -s "$ROOT/last-error" ]; then cat "$ROOT/last-error"; fi
    if [ -s "$ROOT/last-result" ]; then cat "$ROOT/last-result"; fi
}

action=${1:-status}
[ "$action" != status ] || { status; exit; }
[ "$(id -u)" = 0 ] || fail 'Run on the router as root.'
exec 9>"$ROOT/operation.lock"
# Firmware BusyBox supports -n but not util-linux's -w timeout option.
lock_wait=0
while :; do
    if flock -n 9; then break; else lock_result=$?; fi
    [ "$lock_result" -eq 1 ] || fail "flock failed (exit $lock_result); deployment was not started."
    [ "$lock_wait" -lt 70 ] || fail 'Could not acquire the deployment lock within 70 seconds; inspect active operations and any flock errors above.'
    sleep 1
    lock_wait=$((lock_wait+1))
done
case "$action" in
    install)
        trap 'result=$?; if [ "$result" -ne 0 ] && [ -s "$ROOT/pending" ]; then stop_pair; recover_pending; fi' EXIT
        install_candidate "${2:?release ID}"
        ;;
    confirm)
        [ -s "$ROOT/pending" ] || exit 0
        [ "$(sed -n '1p' "$ROOT/pending")" = "${2:-}" ] && [ -s "$RUN/heartbeat" ] && [ -f "$RUN/ready" ] || fail 'Candidate not ready.'
        atomic "$ROOT/previous" "$(sed -n '2p' "$ROOT/pending")"
        rm -f "$ROOT/pending";sync
        echo 'Both programs passed the health trial.' > "$ROOT/last-result"
        ;;
    recover) recover_pending ;;
    boot)
        # A power interruption during a trial always boots the previous pair.
        if [ -s "$ROOT/pending" ]; then recover_pending
        elif [ -f "$ROOT/enabled" ]; then launch
        else
            . "$ROOT/agent.env"
            export ZTE_AGENT_DEPLOYMENT=paired
            export ZTE_OTA_ROOT="$ROOT/agent-update-disabled"
            export ZTE_OTA_SUPERVISOR="$ROOT/no-agent-only-supervisor"
            current=$(value "$ROOT/current")
            [ "$current" != none ] || exit 0
            agent_running "$ROOT/releases/$current/zte-agent" && exit 0
            link_agent "$current"
            nohup "$ROOT/releases/$current/zte-agent" </dev/null >"$ROOT/agent.log" 2>&1 9>&- &
        fi
        ;;
    start)
        [ ! -s "$ROOT/pending" ] || fail 'A paired trial is active.'
        stop_pair
        touch "$ROOT/enabled";launch
        ;;
    stop)
        [ ! -s "$ROOT/pending" ] || fail 'Wait for the paired trial to finish.'
        [ "$(value "$ROOT/current")" != none ] || fail 'No paired release installed.'
        rm -f "$ROOT/enabled"
        stop_pair
        # Keep remote administration usable while the original screen is active.
        . "$ROOT/agent.env"
        export ZTE_AGENT_DEPLOYMENT=paired
        export ZTE_OTA_ROOT="$ROOT/agent-update-disabled"
        export ZTE_OTA_SUPERVISOR="$ROOT/no-agent-only-supervisor"
        current=$(value "$ROOT/current")
        nohup "$ROOT/releases/$current/zte-agent" </dev/null >"$ROOT/agent.log" 2>&1 9>&- &
        ;;
    rollback)
        previous=$(value "$ROOT/previous")
        [ "$previous" != none ] || fail 'No previous paired release. Use restore to return to the original installation.'
        install_candidate "$previous"
        ;;
    fallback)
        [ ! -d "$RUN" ] || exit 0
        current=$(value "$ROOT/current")
        [ "$current" != none ] || exit 0
        agent_running "$ROOT/releases/$current/zte-agent" && exit 0
        . "$ROOT/agent.env"
        export ZTE_AGENT_DEPLOYMENT=paired
        export ZTE_OTA_ROOT="$ROOT/agent-update-disabled"
        export ZTE_OTA_SUPERVISOR="$ROOT/no-agent-only-supervisor"
        nohup "$ROOT/releases/$current/zte-agent" </dev/null >"$ROOT/agent.log" 2>&1 9>&- &
        ;;
    restore)
        [ ! -s "$ROOT/pending" ] || fail 'Wait for the paired trial to finish.'
        [ "$(value "$ROOT/current")" != none ] || { echo 'Original installation already selected.'; exit 0; }
        stop_pair;restore_original
        ;;
    *) fail 'Unknown device operation.' ;;
esac
