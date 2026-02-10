#!/bin/bash
#
# Entrypoint script for DEB Docker build
#

set -euo pipefail

PKG_NAME="wifisync"
PKG_VERSION="${PKG_VERSION:-0.1.0}"
DEBIAN_DIR="${DEBIAN_DIR:-deb}"
PREBUILT_BINARY="${PREBUILT_BINARY:-}"
export SKIP_CARGO_BUILD="${SKIP_CARGO_BUILD:-}"

echo "==> Building Wifisync DEB v${PKG_VERSION} (debian: ${DEBIAN_DIR})"

# If a prebuilt binary is provided, ensure it's in target/release/
if [[ -n "$PREBUILT_BINARY" ]]; then
    echo "==> Using prebuilt binary: ${PREBUILT_BINARY}"
    mkdir -p /build/target/release
    # Skip copy if already in the right place (mounted volume)
    local_dest="/build/target/release/$(basename "$PREBUILT_BINARY")"
    if [[ "$(realpath "$PREBUILT_BINARY")" != "$(realpath "$local_dest")" ]]; then
        cp "$PREBUILT_BINARY" /build/target/release/
    fi
    export SKIP_CARGO_BUILD=1
fi

# Setup build directory
echo "==> Setting up build directory..."
BUILD_DIR="/tmp/build-deb"
PKG_DIR="${BUILD_DIR}/${PKG_NAME}-${PKG_VERSION}"

rm -rf "$BUILD_DIR"
mkdir -p "$PKG_DIR"

# Copy source files
if [[ -n "$PREBUILT_BINARY" ]]; then
    # Include target/release/ so the prebuilt binary is available in the build tree
    rsync -a \
        --exclude='target/debug' \
        --exclude='.git' \
        --exclude='dist' \
        --exclude='build-deb' \
        /build/ "$PKG_DIR/"
else
    rsync -a \
        --exclude='target' \
        --exclude='.git' \
        --exclude='dist' \
        --exclude='build-deb' \
        /build/ "$PKG_DIR/"
fi

# Copy debian directory
cp -r "/build/packaging/${DEBIAN_DIR}/debian" "$PKG_DIR/"
chmod +x "$PKG_DIR/debian/rules"
# Make maintainer scripts executable if they exist
for script in postinst postrm preinst prerm; do
    if [ -f "$PKG_DIR/debian/$script" ]; then
        chmod +x "$PKG_DIR/debian/$script"
    fi
done

# Create orig tarball
echo "==> Creating orig tarball..."
cd "$BUILD_DIR"
tar --exclude='debian' -czf "${PKG_NAME}_${PKG_VERSION}.orig.tar.gz" "${PKG_NAME}-${PKG_VERSION}"

# Build package
echo "==> Building DEB package..."
cd "$PKG_DIR"
# -us -uc: skip signing, -b: binary only, -d: skip build-dep checks (rust via rustup)
dpkg-buildpackage -us -uc -b -d

# Copy output to mounted volume
echo "==> Copying packages to output directory..."
mkdir -p /build/dist
cp "$BUILD_DIR"/*.deb /build/dist/ 2>/dev/null || true
cp "$BUILD_DIR"/*.changes /build/dist/ 2>/dev/null || true
cp "$BUILD_DIR"/*.buildinfo /build/dist/ 2>/dev/null || true

echo "==> Build complete!"
echo "==> Packages:"
ls -la /build/dist/*.deb 2>/dev/null || echo "No DEB files found"
