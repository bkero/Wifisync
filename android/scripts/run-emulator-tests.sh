#!/usr/bin/env bash
#
# run-emulator-tests.sh - Run Android instrumented tests on an emulator
#
# Usage: ./android/scripts/run-emulator-tests.sh [OPTIONS]
#
# Options:
#   --keep-emulator    Don't shut down the emulator after tests
#   --reuse-emulator   Skip AVD creation and emulator boot if already running
#   --help             Show this help message
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ANDROID_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

AVD_NAME="wifisync-test-api34"
SYSTEM_IMAGE="system-images;android-34;google_apis;x86_64"
DEVICE_PROFILE="pixel_6"
BOOT_TIMEOUT=120

KEEP_EMULATOR=false
REUSE_EMULATOR=false
EMULATOR_PID=""

usage() {
    sed -n '2,/^$/s/^# //p' "$0"
    exit 0
}

log() {
    echo "[$(date '+%H:%M:%S')] $*"
}

die() {
    echo "ERROR: $*" >&2
    exit 1
}

cleanup() {
    if [ "$KEEP_EMULATOR" = true ]; then
        log "Keeping emulator running (--keep-emulator)"
        return
    fi

    if [ -n "$EMULATOR_PID" ] && kill -0 "$EMULATOR_PID" 2>/dev/null; then
        log "Shutting down emulator (PID $EMULATOR_PID)..."
        adb -s emulator-5554 emu kill 2>/dev/null || true
        wait "$EMULATOR_PID" 2>/dev/null || true
        log "Emulator stopped"
    fi
}

trap cleanup EXIT

# Parse arguments
while [ $# -gt 0 ]; do
    case "$1" in
        --keep-emulator)  KEEP_EMULATOR=true ;;
        --reuse-emulator) REUSE_EMULATOR=true; KEEP_EMULATOR=true ;;
        --help|-h)        usage ;;
        *)                die "Unknown option: $1" ;;
    esac
    shift
done

# Check prerequisites
[ -n "${ANDROID_HOME:-}" ] || die "ANDROID_HOME is not set"
[ -d "$ANDROID_HOME" ] || die "ANDROID_HOME ($ANDROID_HOME) does not exist"

for tool in sdkmanager avdmanager emulator adb; do
    if ! command -v "$tool" &>/dev/null && [ ! -x "$ANDROID_HOME/cmdline-tools/latest/bin/$tool" ] && [ ! -x "$ANDROID_HOME/emulator/$tool" ] && [ ! -x "$ANDROID_HOME/platform-tools/$tool" ]; then
        die "$tool not found. Ensure Android SDK tools are installed and on PATH."
    fi
done

# Add SDK tools to PATH if needed
export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/emulator:$ANDROID_HOME/platform-tools:$PATH"

# Check if emulator is already running
emulator_running() {
    adb devices 2>/dev/null | grep -q "emulator-5554"
}

if [ "$REUSE_EMULATOR" = true ] && emulator_running; then
    log "Reusing existing emulator"
else
    # Install system image if missing
    if ! sdkmanager --list_installed 2>/dev/null | grep -q "$SYSTEM_IMAGE"; then
        log "Installing system image: $SYSTEM_IMAGE"
        yes | sdkmanager "$SYSTEM_IMAGE" || die "Failed to install system image"
    fi

    # Create AVD if missing
    if ! avdmanager list avd -c 2>/dev/null | grep -q "^${AVD_NAME}$"; then
        log "Creating AVD: $AVD_NAME (device: $DEVICE_PROFILE)"
        echo "no" | avdmanager create avd \
            -n "$AVD_NAME" \
            -k "$SYSTEM_IMAGE" \
            -d "$DEVICE_PROFILE" \
            --force || die "Failed to create AVD"
    fi

    # Kill any existing emulator
    if emulator_running; then
        log "Stopping existing emulator..."
        adb -s emulator-5554 emu kill 2>/dev/null || true
        sleep 3
    fi

    # Start emulator
    log "Starting emulator: $AVD_NAME"
    emulator -avd "$AVD_NAME" \
        -no-window \
        -no-audio \
        -no-boot-anim \
        -gpu swiftshader_indirect \
        -no-snapshot-save \
        &
    EMULATOR_PID=$!

    # Wait for boot
    log "Waiting for emulator to boot (timeout: ${BOOT_TIMEOUT}s)..."
    elapsed=0
    while [ $elapsed -lt $BOOT_TIMEOUT ]; do
        if [ "$(adb -s emulator-5554 shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; then
            break
        fi
        sleep 2
        elapsed=$((elapsed + 2))
    done

    if [ $elapsed -ge $BOOT_TIMEOUT ]; then
        die "Emulator failed to boot within ${BOOT_TIMEOUT}s"
    fi

    log "Emulator booted in ${elapsed}s"

    # Disable animations for test reliability
    log "Disabling animations..."
    adb -s emulator-5554 shell settings put global window_animation_scale 0
    adb -s emulator-5554 shell settings put global transition_animation_scale 0
    adb -s emulator-5554 shell settings put global animator_duration_scale 0
fi

# Run tests
log "Running instrumented tests..."
cd "$ANDROID_DIR"
./gradlew connectedAndroidTest
test_exit=$?

# Collect results
REPORT_DIR="$ANDROID_DIR/app/build/reports/androidTests/connected"
if [ -d "$REPORT_DIR" ]; then
    log "Test reports available at: $REPORT_DIR/index.html"
else
    log "Warning: Test report directory not found at $REPORT_DIR"
fi

if [ $test_exit -eq 0 ]; then
    log "All instrumented tests passed"
else
    log "Some tests failed (exit code: $test_exit)"
fi

exit $test_exit
