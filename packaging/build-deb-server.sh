#!/bin/bash
#
# Build DEB package for Wifisync Server using Docker
#
# Usage: ./packaging/build-deb-server.sh
#
# Requirements:
#   - Docker
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Package info
PKG_NAME="wifisync-server"
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
    info "Running DEB build for ${PKG_NAME} in Docker container..."

    # Create dist directory
    mkdir -p "$PROJECT_ROOT/dist"

    # Run the container with server debian directory
    docker run --rm \
        -v "$PROJECT_ROOT:/build:rw" \
        -e "PKG_VERSION=${PKG_VERSION}" \
        -e "DEBIAN_DIR=deb-server" \
        "$IMAGE_NAME"

    info "Docker build completed"
}

# Show results
show_results() {
    echo ""
    info "DEB build completed successfully!"
    echo ""
    echo "Packages available in: $PROJECT_ROOT/dist/"
    ls -la "$PROJECT_ROOT/dist/"*server*.deb 2>/dev/null || warn "No server DEB files found"
    echo ""
    echo "To install on Ubuntu/Debian:"
    echo "  sudo apt install ./dist/${PKG_NAME}_${PKG_VERSION}-1_*.deb"
    echo ""
    echo "To configure and start the server:"
    echo "  # Set JWT secret (required for production)"
    echo "  sudo systemctl edit wifisync-server.service"
    echo "  # Add: Environment=JWT_SECRET=your-secure-secret"
    echo ""
    echo "  sudo systemctl enable --now wifisync-server.service"
    echo ""
    echo "The server will listen on port 8080 by default."
}

# Main
main() {
    info "Building Wifisync Server DEB package v${PKG_VERSION} (Docker)"

    check_docker
    build_image
    run_build
    show_results
}

main "$@"
