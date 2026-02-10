#!/bin/bash
#
# Build RPM package for Wifisync using Docker
#
# Usage: ./packaging/build-rpm.sh
#
# Requirements:
#   - Docker
#

set -euo pipefail

export DOCKER_BUILDKIT=1

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Package info
PKG_NAME="wifisync"
PKG_VERSION="0.1.0"
IMAGE_NAME="wifisync-rpm-builder"

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
        -f "$SCRIPT_DIR/docker/Dockerfile.fedora" \
        "$SCRIPT_DIR"

    info "Docker image built successfully"
}

# Run the build in Docker
run_build() {
    info "Running RPM build in Docker container..."

    # Create dist directory
    mkdir -p "$PROJECT_ROOT/dist"

    # Build docker run arguments
    local docker_args=(
        --rm
        -v cargo-registry:/root/.cargo/registry
        -v cargo-git:/root/.cargo/git
        -v "$PROJECT_ROOT:/build:rw"
        -e "PKG_VERSION=${PKG_VERSION}"
    )

    # Pass through prebuilt binary path if set
    if [[ -n "${PREBUILT_BINARY:-}" ]]; then
        docker_args+=(-e "PREBUILT_BINARY=${PREBUILT_BINARY}")
    fi

    docker_args+=("$IMAGE_NAME")

    # Run the container
    docker run "${docker_args[@]}"

    info "Docker build completed"
}

# Show results
show_results() {
    echo ""
    info "RPM build completed successfully!"
    echo ""
    echo "Packages available in: $PROJECT_ROOT/dist/"
    ls -la "$PROJECT_ROOT/dist/"*.rpm 2>/dev/null || warn "No RPM files found"
    echo ""
    echo "To install on Fedora/RHEL:"
    echo "  sudo dnf install ./dist/${PKG_NAME}-${PKG_VERSION}-*.rpm"
    echo ""
    echo "To enable the Secret Agent daemon:"
    echo "  systemctl --user enable --now wifisync-agent.service"
}

# Main
main() {
    info "Building Wifisync RPM package v${PKG_VERSION} (Docker)"

    check_docker
    build_image
    run_build
    show_results
}

main "$@"
