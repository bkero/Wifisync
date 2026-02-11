#!/usr/bin/env bash
#
# E2E Test Orchestrator for Wifisync
#
# Builds the CLI, starts a Docker server, and runs end-to-end tests.
#
# Usage:
#   ./tests/e2e/run-e2e.sh              # Full suite (CLI + Android)
#   ./tests/e2e/run-e2e.sh --cli-only   # CLI tests only (no emulator)
#   ./tests/e2e/run-e2e.sh --android-only # Android only (assumes server running)
#   ./tests/e2e/run-e2e.sh --keep-server # Don't tear down server after tests
#
# Environment variables:
#   E2E_SERVER_PORT       Server port (default: 18080)
#   E2E_SHORT_JWT_PORT    Short-JWT server port (default: 18081)
#   E2E_SKIP_BUILD        Skip cargo build if set to 1
#   E2E_RUST_LOG          Server log level (default: info)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Defaults
CLI_ONLY=0
ANDROID_ONLY=0
KEEP_SERVER=0
RUN_JWT_TESTS=0
E2E_SERVER_PORT="${E2E_SERVER_PORT:-18080}"
E2E_SHORT_JWT_PORT="${E2E_SHORT_JWT_PORT:-18081}"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --cli-only)
            CLI_ONLY=1
            shift
            ;;
        --android-only)
            ANDROID_ONLY=1
            shift
            ;;
        --keep-server)
            KEEP_SERVER=1
            shift
            ;;
        --jwt-tests)
            RUN_JWT_TESTS=1
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--cli-only] [--android-only] [--keep-server] [--jwt-tests]"
            echo ""
            echo "Options:"
            echo "  --cli-only      Run only CLI E2E tests (no Android emulator)"
            echo "  --android-only  Run only Android E2E tests (server must be running)"
            echo "  --keep-server   Don't tear down the Docker server after tests"
            echo "  --jwt-tests     Also start short-JWT server and run JWT expiry tests"
            exit 0
            ;;
        *)
            echo "Unknown argument: $1"
            exit 1
            ;;
    esac
done

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No color

info()  { echo -e "${BLUE}[E2E]${NC} $*"; }
ok()    { echo -e "${GREEN}[E2E]${NC} $*"; }
warn()  { echo -e "${YELLOW}[E2E]${NC} $*"; }
err()   { echo -e "${RED}[E2E]${NC} $*"; }

# Track whether we started the server (for cleanup)
SERVER_STARTED=0

cleanup() {
    local exit_code=$?
    if [[ "$KEEP_SERVER" -eq 0 && "$SERVER_STARTED" -eq 1 ]]; then
        info "Tearing down E2E server..."
        cd "$PROJECT_ROOT"
        docker compose \
            -f docker-compose.yml \
            -f tests/e2e/docker-compose.e2e.yml \
            --profile jwt-tests \
            down -v --remove-orphans 2>/dev/null || true
    fi
    if [[ $exit_code -ne 0 ]]; then
        err "E2E tests FAILED (exit code $exit_code)"
    fi
    exit $exit_code
}
trap cleanup EXIT

# ─── Step 1: Build ────────────────────────────────────────────────────────────

if [[ "${E2E_SKIP_BUILD:-0}" -ne 1 && "$ANDROID_ONLY" -eq 0 ]]; then
    info "Building wifisync CLI (release)..."
    cd "$PROJECT_ROOT"
    cargo build --release -p wifisync 2>&1
    ok "CLI build complete: target/release/wifisync"
fi

# ─── Step 2: Start server ────────────────────────────────────────────────────

if [[ "$ANDROID_ONLY" -eq 0 ]]; then
    info "Starting E2E Docker server on port $E2E_SERVER_PORT..."
    cd "$PROJECT_ROOT"

    # Remove old volumes for a clean slate
    docker compose \
        -f docker-compose.yml \
        -f tests/e2e/docker-compose.e2e.yml \
        --profile jwt-tests \
        down -v --remove-orphans 2>/dev/null || true

    # Determine which profiles to start
    COMPOSE_PROFILES=""
    if [[ "$RUN_JWT_TESTS" -eq 1 ]]; then
        COMPOSE_PROFILES="--profile jwt-tests"
    fi

    docker compose \
        -f docker-compose.yml \
        -f tests/e2e/docker-compose.e2e.yml \
        $COMPOSE_PROFILES \
        up -d --build --wait 2>&1

    SERVER_STARTED=1

    # Wait for health check
    info "Waiting for server to be healthy..."
    MAX_WAIT=60
    WAITED=0
    while [[ $WAITED -lt $MAX_WAIT ]]; do
        if curl -sf "http://localhost:${E2E_SERVER_PORT}/health" >/dev/null 2>&1; then
            ok "Server healthy on port $E2E_SERVER_PORT"
            break
        fi
        sleep 1
        WAITED=$((WAITED + 1))
    done

    if [[ $WAITED -ge $MAX_WAIT ]]; then
        err "Server failed to become healthy within ${MAX_WAIT}s"
        docker compose \
            -f docker-compose.yml \
            -f tests/e2e/docker-compose.e2e.yml \
            logs wifisync-server 2>&1 | tail -30
        exit 1
    fi

    if [[ "$RUN_JWT_TESTS" -eq 1 ]]; then
        info "Waiting for short-JWT server to be healthy..."
        WAITED=0
        while [[ $WAITED -lt $MAX_WAIT ]]; do
            if curl -sf "http://localhost:${E2E_SHORT_JWT_PORT}/health" >/dev/null 2>&1; then
                ok "Short-JWT server healthy on port $E2E_SHORT_JWT_PORT"
                break
            fi
            sleep 1
            WAITED=$((WAITED + 1))
        done

        if [[ $WAITED -ge $MAX_WAIT ]]; then
            warn "Short-JWT server failed to start; JWT expiry tests will be skipped"
            RUN_JWT_TESTS=0
        fi
    fi
fi

# ─── Step 3: Run CLI E2E tests ───────────────────────────────────────────────

if [[ "$ANDROID_ONLY" -eq 0 ]]; then
    info "Running CLI E2E tests..."
    cd "$PROJECT_ROOT"

    export E2E_SERVER_URL="http://localhost:${E2E_SERVER_PORT}"
    export E2E_CLI_BINARY="$PROJECT_ROOT/target/release/wifisync"

    if [[ "$RUN_JWT_TESTS" -eq 1 ]]; then
        export E2E_SHORT_JWT_SERVER_URL="http://localhost:${E2E_SHORT_JWT_PORT}"
    fi

    cargo test --test e2e_cli -- --test-threads=1 2>&1
    ok "CLI E2E tests passed"
fi

# ─── Step 4: Run Android E2E tests ───────────────────────────────────────────

if [[ "$CLI_ONLY" -eq 0 ]]; then
    # Check if emulator is available
    if ! command -v adb &>/dev/null; then
        warn "adb not found; skipping Android E2E tests"
    elif ! adb devices 2>/dev/null | grep -q "device$"; then
        warn "No Android device/emulator connected; skipping Android E2E tests"
    else
        info "Running Android E2E tests..."

        # For Android emulator, the host loopback is 10.0.2.2
        export WIFISYNC_SERVER_URL="http://10.0.2.2:${E2E_SERVER_PORT}"
        export WIFISYNC_USERNAME="android_e2e_$(date +%s)"
        export WIFISYNC_PASSWORD="e2e_test_password_$(date +%s)"

        cd "$PROJECT_ROOT/android"
        ./gradlew connectedAndroidTest \
            -Pandroid.testInstrumentationRunnerArguments.class=com.wifisync.android.LiveSyncE2eTest \
            2>&1
        ok "Android E2E tests passed"
    fi
fi

# ─── Done ─────────────────────────────────────────────────────────────────────

echo ""
ok "All E2E tests passed!"
