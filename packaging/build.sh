#!/bin/bash
#
# Unified build script for Wifisync packages using Docker
#
# Usage:
#   ./packaging/build.sh rpm         # Build CLI RPM for Fedora (Docker)
#   ./packaging/build.sh deb         # Build CLI DEB for Ubuntu (Docker)
#   ./packaging/build.sh server-rpm  # Build server RPM for Fedora (Docker)
#   ./packaging/build.sh server-deb  # Build server DEB for Ubuntu (Docker)
#   ./packaging/build.sh server      # Build both server packages
#   ./packaging/build.sh android     # Build Android APKs
#   ./packaging/build.sh all         # Build all packages
#   ./packaging/build.sh binary      # Build release binary locally
#   ./packaging/build.sh clean       # Clean build artifacts
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Package info
PKG_NAME="wifisync"
PKG_VERSION="0.1.0"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Show usage
usage() {
    cat << EOF
Wifisync Package Builder

Usage: $0 [COMMAND]

Commands:
    rpm         Build CLI RPM package (Fedora) using Docker
    deb         Build CLI DEB package (Ubuntu) using Docker
    server-rpm  Build server RPM package (Fedora) using Docker
    server-deb  Build server DEB package (Ubuntu) using Docker
    server      Build both server packages (RPM and DEB)
    android     Build Android APKs (debug and release)
    all         Build all packages (CLI, server, Android)
    binary      Build release binary locally (no Docker)
    clean       Clean build artifacts and Docker images
    help        Show this help message

Examples:
    $0 rpm          # Build CLI RPM only
    $0 deb          # Build CLI DEB only
    $0 server       # Build server RPM and DEB
    $0 server-rpm   # Build server RPM only
    $0 android      # Build Android APKs
    $0 all          # Build all packages
    $0 binary       # Just compile release binary locally

Requirements:
    - Docker (for rpm/deb/server commands)
    - Rust toolchain (for binary command)
    - Android SDK, NDK, and cargo-ndk (for android command)

Output:
    Packages are placed in ./dist/

EOF
}

# Check Docker is available
check_docker() {
    if ! command -v docker &> /dev/null; then
        error "Docker is required but not installed. Please install Docker first."
    fi

    if ! docker info &> /dev/null; then
        error "Docker daemon is not running or you don't have permission to use it."
    fi
}

# Build release binary locally
build_binary() {
    info "Building release binary locally..."
    cd "$PROJECT_ROOT"
    cargo build --release

    mkdir -p "$PROJECT_ROOT/dist"
    cp target/release/wifisync "$PROJECT_ROOT/dist/"

    info "Binary built: dist/wifisync"
}

# Clean build artifacts
clean() {
    info "Cleaning build artifacts..."
    cd "$PROJECT_ROOT"

    # Clean Rust target
    cargo clean 2>/dev/null || true

    # Clean dist directory
    rm -rf "$PROJECT_ROOT/dist"

    # Clean Docker images
    docker rmi wifisync-rpm-builder 2>/dev/null || true
    docker rmi wifisync-deb-builder 2>/dev/null || true
    docker rmi wifisync-server-rpm-builder 2>/dev/null || true
    docker rmi wifisync-server-deb-builder 2>/dev/null || true

    # Clean Android build artifacts
    if [[ -d "$PROJECT_ROOT/android" ]]; then
        info "Cleaning Android build artifacts..."
        rm -rf "$PROJECT_ROOT/android/build"
        rm -rf "$PROJECT_ROOT/android/app/build"
        rm -rf "$PROJECT_ROOT/android/app/src/main/jniLibs"
        rm -rf "$PROJECT_ROOT/android/.gradle"
        # Clean Gradle cache (optional, may want to keep for faster rebuilds)
        # rm -rf ~/.gradle/caches/build-cache-*
    fi

    info "Clean complete"
}

# Build RPM (CLI)
build_rpm() {
    check_docker
    "$SCRIPT_DIR/build-rpm.sh"
}

# Build DEB (CLI)
build_deb() {
    check_docker
    "$SCRIPT_DIR/build-deb.sh"
}

# Build server RPM
build_server_rpm() {
    check_docker
    "$SCRIPT_DIR/build-rpm-server.sh"
}

# Build server DEB
build_server_deb() {
    check_docker
    "$SCRIPT_DIR/build-deb-server.sh"
}

# Build both server packages
build_server() {
    info "Building server packages..."
    echo ""
    build_server_rpm
    echo ""
    build_server_deb
    echo ""
    info "Server packages complete!"
}

# Check Android build requirements
check_android() {
    local missing=()

    # Check for ANDROID_HOME or ANDROID_SDK_ROOT
    if [[ -z "${ANDROID_HOME:-}" ]] && [[ -z "${ANDROID_SDK_ROOT:-}" ]]; then
        missing+=("ANDROID_HOME or ANDROID_SDK_ROOT environment variable")
    fi

    # Check for cargo-ndk
    if ! command -v cargo-ndk &> /dev/null; then
        missing+=("cargo-ndk (install with: cargo install cargo-ndk)")
    fi

    # Check for Rust Android targets
    if ! rustup target list --installed | grep -q "aarch64-linux-android"; then
        missing+=("Rust Android targets (install with: rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android)")
    fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        error "Missing Android build requirements:\n  - $(printf '%s\n  - ' "${missing[@]}")"
    fi
}

# Build Android APKs
build_android() {
    info "Building Android APKs..."

    check_android

    cd "$PROJECT_ROOT"

    # Create dist directory
    mkdir -p "$PROJECT_ROOT/dist"

    # Step 1: Build the Rust JNI library for all Android architectures
    info "Building Rust JNI library for Android..."

    local jni_libs_dir="$PROJECT_ROOT/android/app/src/main/jniLibs"
    mkdir -p "$jni_libs_dir"

    cargo ndk \
        -t arm64-v8a \
        -t armeabi-v7a \
        -t x86_64 \
        -o "$jni_libs_dir" \
        build --release -p wifisync-jni

    # Verify the libraries were built
    if [[ ! -f "$jni_libs_dir/arm64-v8a/libwifisync_jni.so" ]]; then
        error "Failed to build Rust JNI library"
    fi

    info "Rust JNI library built successfully"

    # Step 2: Build the Android APKs using Gradle
    info "Building Android APKs with Gradle..."

    cd "$PROJECT_ROOT/android"

    # Build debug APK
    # --no-configuration-cache avoids issues with Java 21
    ./gradlew assembleDebug --no-daemon --no-configuration-cache

    # Build release APK (unsigned)
    ./gradlew assembleRelease --no-daemon --no-configuration-cache

    # Copy APKs to dist directory
    info "Copying APKs to dist..."

    if [[ -f "app/build/outputs/apk/debug/app-debug.apk" ]]; then
        cp "app/build/outputs/apk/debug/app-debug.apk" "$PROJECT_ROOT/dist/wifisync-${PKG_VERSION}-debug.apk"
        info "Debug APK: dist/wifisync-${PKG_VERSION}-debug.apk"
    else
        warn "Debug APK not found"
    fi

    if [[ -f "app/build/outputs/apk/release/app-release-unsigned.apk" ]]; then
        cp "app/build/outputs/apk/release/app-release-unsigned.apk" "$PROJECT_ROOT/dist/wifisync-${PKG_VERSION}-release-unsigned.apk"
        info "Release APK (unsigned): dist/wifisync-${PKG_VERSION}-release-unsigned.apk"
    elif [[ -f "app/build/outputs/apk/release/app-release.apk" ]]; then
        cp "app/build/outputs/apk/release/app-release.apk" "$PROJECT_ROOT/dist/wifisync-${PKG_VERSION}-release.apk"
        info "Release APK: dist/wifisync-${PKG_VERSION}-release.apk"
    else
        warn "Release APK not found"
    fi

    cd "$PROJECT_ROOT"
    info "Android build complete!"
}

# Build all
build_all() {
    info "Building all packages..."
    echo ""

    # Build Linux packages (require Docker)
    if command -v docker &> /dev/null && docker info &> /dev/null 2>&1; then
        info "Building CLI packages..."
        build_rpm
        echo ""

        build_deb
        echo ""

        info "Building server packages..."
        build_server_rpm
        echo ""

        build_server_deb
        echo ""
    else
        warn "Docker not available, skipping RPM and DEB builds"
        echo ""
    fi

    # Build Android (requires Android SDK)
    if [[ -n "${ANDROID_HOME:-}" ]] || [[ -n "${ANDROID_SDK_ROOT:-}" ]]; then
        build_android
        echo ""
    else
        warn "Android SDK not configured, skipping Android build"
        echo ""
    fi

    info "All available builds completed!"
    echo ""
    echo "Packages available in: $PROJECT_ROOT/dist/"
    ls -la "$PROJECT_ROOT/dist/" 2>/dev/null || warn "No packages found"
}

# Main
main() {
    local command="${1:-help}"

    case "$command" in
        rpm)
            build_rpm
            ;;
        deb)
            build_deb
            ;;
        server-rpm)
            build_server_rpm
            ;;
        server-deb)
            build_server_deb
            ;;
        server)
            build_server
            ;;
        android|apk)
            build_android
            ;;
        all)
            build_all
            ;;
        binary)
            build_binary
            ;;
        clean)
            clean
            ;;
        help|-h|--help)
            usage
            ;;
        *)
            error "Unknown command: $command\nRun '$0 help' for usage."
            ;;
    esac
}

main "$@"
