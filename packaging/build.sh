#!/bin/bash
#
# Unified build script for Wifisync packages using Docker
#
# Usage:
#   ./packaging/build.sh rpm      # Build RPM for Fedora (Docker)
#   ./packaging/build.sh deb      # Build DEB for Ubuntu (Docker)
#   ./packaging/build.sh all      # Build both packages
#   ./packaging/build.sh binary   # Build release binary locally
#   ./packaging/build.sh clean    # Clean build artifacts
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
Wifisync Package Builder (Docker-based)

Usage: $0 [COMMAND]

Commands:
    rpm         Build RPM package (Fedora) using Docker
    deb         Build DEB package (Ubuntu) using Docker
    all         Build both packages
    binary      Build release binary locally (no Docker)
    clean       Clean build artifacts and Docker images
    help        Show this help message

Examples:
    $0 rpm          # Build RPM only
    $0 deb          # Build DEB only
    $0 all          # Build both packages
    $0 binary       # Just compile release binary locally

Requirements:
    - Docker (for rpm/deb/all commands)
    - Rust toolchain (for binary command)

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

    info "Clean complete"
}

# Build RPM
build_rpm() {
    check_docker
    "$SCRIPT_DIR/build-rpm.sh"
}

# Build DEB
build_deb() {
    check_docker
    "$SCRIPT_DIR/build-deb.sh"
}

# Build all
build_all() {
    check_docker

    info "Building all packages..."
    echo ""

    build_rpm
    echo ""

    build_deb
    echo ""

    info "All builds completed!"
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
