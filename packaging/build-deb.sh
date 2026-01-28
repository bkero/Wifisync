#!/bin/bash
#
# Build DEB package for Wifisync using Docker
#
# Usage: ./packaging/build-deb.sh
#
# Requirements:
#   - Docker
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Package info
PKG_NAME="wifisync"
PKG_VERSION="0.1.0"
IMAGE_NAME="wifisync-deb-builder"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Check Docker is available
check_docker() {
    if ! command -v docker &> /dev/null; then
        error "Docker is required but not installed. Please install Docker first."
    fi

    if ! docker info &> /dev/null; then
        error "Docker daemon is not running or you don't have permission to use it."
    fi

    info "Docker is available"
}

# Build the Docker image
build_image() {
    info "Building Docker image: ${IMAGE_NAME}..."

    docker build \
        -t "$IMAGE_NAME" \
        -f "$SCRIPT_DIR/docker/Dockerfile.ubuntu" \
        "$SCRIPT_DIR"

    info "Docker image built successfully"
}

# Run the build in Docker
run_build() {
    info "Running DEB build in Docker container..."

    # Create dist directory
    mkdir -p "$PROJECT_ROOT/dist"

    # Run the container
    docker run --rm \
        -v "$PROJECT_ROOT:/build:rw" \
        -e "PKG_VERSION=${PKG_VERSION}" \
        "$IMAGE_NAME"

    info "Docker build completed"
}

# Show results
show_results() {
    echo ""
    info "DEB build completed successfully!"
    echo ""
    echo "Packages available in: $PROJECT_ROOT/dist/"
    ls -la "$PROJECT_ROOT/dist/"*.deb 2>/dev/null || warn "No DEB files found"
    echo ""
    echo "To install on Ubuntu/Debian:"
    echo "  sudo apt install ./dist/${PKG_NAME}_${PKG_VERSION}-1_*.deb"
    echo ""
    echo "To enable the Secret Agent daemon:"
    echo "  systemctl --user enable --now wifisync-agent.service"
}

# Main
main() {
    info "Building Wifisync DEB package v${PKG_VERSION} (Docker)"

    check_docker
    build_image
    run_build
    show_results
}

main "$@"
