#!/usr/bin/env bash
set -euo pipefail
SCREEN_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$SCREEN_ROOT"
command=${1:-preview}
if (($#)); then shift; fi
BUILD_ROOT=${U60_BUILD_ROOT:-$HOME/.local/share/u60-build}
if [[ -z ${CARGO_HOME:-} && -d $BUILD_ROOT/cargo ]]; then export CARGO_HOME="$BUILD_ROOT/cargo"; fi
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$SCREEN_ROOT/target/u60-screen}
screen_cargo=${CARGO:-cargo}
case "$command" in
    preview)
        exec "$screen_cargo" run --locked --manifest-path u60-screen/Cargo.toml -- --prefs "${U60_SCREEN_PREFS:-$SCREEN_ROOT/u60-screen/appearance.json}" "$@"
        ;;
    snapshot)
        exec "$screen_cargo" run --locked --manifest-path u60-screen/Cargo.toml --no-default-features -- --screenshot "${1:?supply an output PNG}" "${@:2}"
        ;;
    test)
        python3 scripts/i18n/build.py --check
        node u60-screen/scripts/export-catalog.cjs --check
        python3 -m unittest discover -s u60-screen/scripts/tests -v
        exec "$screen_cargo" test --locked --manifest-path u60-screen/Cargo.toml --features device "$@"
        ;;
    build-device)
        cross_toolchain="$BUILD_ROOT/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin"
        if [[ -z ${RUSTC:-} && -x $cross_toolchain/rustc ]]; then
            export RUSTC="$cross_toolchain/rustc"
            screen_cargo="$cross_toolchain/cargo"
        fi
        if [[ -z ${CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER:-} ]]; then
            if command -v aarch64-linux-musl-gcc >/dev/null; then
                export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc
            elif command -v aarch64-unknown-linux-gnu-gcc >/dev/null; then
                # Rust's musl target supplies its own CRT/libc; the linker driver
                # is also usable when the distro only exposes the GNU triplet.
                export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-unknown-linux-gnu-gcc
            else
                echo 'Set CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER to your cross linker.' >&2; exit 1
            fi
        fi
        export CC_aarch64_unknown_linux_musl=${CC_aarch64_unknown_linux_musl:-$CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER}
        exec "$screen_cargo" build --locked --release --target aarch64-unknown-linux-musl --manifest-path u60-screen/Cargo.toml --no-default-features --features device "$@"
        ;;
    trial)
        binary=${U60_SCREEN_BINARY:-${CARGO_TARGET_DIR:-$SCREEN_ROOT/target}/aarch64-unknown-linux-musl/release/u60-screen}
        [[ -f $binary ]] || { echo 'Run bash u60-screen/native.sh build-device first.' >&2; exit 1; }
        # Forward only a duration; other device options can be used by invoking
        # the on-device script directly, without building shell text from input.
        seconds=${1:-90}
        [[ $seconds =~ ^[0-9]+$ ]] && ((seconds >= 5 && seconds <= 600)) || { echo 'Duration: 5..600 seconds' >&2; exit 2; }
        adb get-state >/dev/null
        adb shell 'test ! -d /tmp/u60-native-screen-trial && mkdir -p /data/local/tmp/u60-native-screen && chmod 700 /data/local/tmp/u60-native-screen'
        adb push "$binary" /data/local/tmp/u60-native-screen/u60-screen
        adb push u60-screen/scripts/device-trial.sh /data/local/tmp/u60-native-screen/device-trial.sh
        adb shell 'chmod 755 /data/local/tmp/u60-native-screen/u60-screen'
        # Let nohup install its signal handling and the recovery guard report
        # readiness before adbd closes its shell/PTY on this firmware.
        adb shell "nohup sh /data/local/tmp/u60-native-screen/device-trial.sh --seconds $seconds > /data/local/tmp/u60-native-screen/trial.log 2>&1 </dev/null & sleep 2; cat /data/local/tmp/u60-native-screen/trial.log; test -d /tmp/u60-native-screen-trial"
        adb shell 'tries=0; while test -d /tmp/u60-native-screen-trial && test "$tries" -lt 25; do if grep -q "display: first frame submitted" /data/local/tmp/u60-native-screen/trial.log; then cat /data/local/tmp/u60-native-screen/trial.log; exit 0; fi; sleep 1; tries=$((tries + 1)); done; cat /data/local/tmp/u60-native-screen/trial.log; exit 1'
        echo "Trial started for $seconds seconds. Status: bash u60-screen/native.sh status"
        ;;
    status)
        exec adb shell 'cat /data/local/tmp/u60-native-screen/trial.log; if test -d /tmp/u60-native-screen-trial; then echo "trial/recovery active"; else echo "no active trial"; fi'
        ;;
    stop)
        exec adb shell 'if test -d /tmp/u60-native-screen-trial; then touch /tmp/u60-native-screen-trial/finished; fi'
        ;;
    *) echo 'Usage: bash u60-screen/native.sh {preview|snapshot FILE|test|build-device|trial [seconds]|status|stop}' >&2; exit 2;;
esac
