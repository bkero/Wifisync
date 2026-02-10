#!/bin/bash
#
# Entrypoint script for RPM Docker build
#

set -euo pipefail

PKG_NAME="wifisync"
PKG_VERSION="${PKG_VERSION:-0.1.0}"
SPEC_FILE="${SPEC_FILE:-wifisync.spec}"
PREBUILT_BINARY="${PREBUILT_BINARY:-}"

echo "==> Building Wifisync RPM v${PKG_VERSION} (spec: ${SPEC_FILE})"

# If a prebuilt binary is provided, place it where rpmbuild expects it
if [[ -n "$PREBUILT_BINARY" ]]; then
    echo "==> Using prebuilt binary: ${PREBUILT_BINARY}"
    mkdir -p /build/target/release
    cp "$PREBUILT_BINARY" /build/target/release/
fi

# Create source tarball
echo "==> Creating source tarball..."
cd /build
tarball_name="${PKG_NAME}-${PKG_VERSION}"
mkdir -p "/root/rpmbuild/SOURCES"

if [[ -n "$PREBUILT_BINARY" ]]; then
    # Include target/release/ so the prebuilt binary is available in the build tree
    tar --transform "s,^,${tarball_name}/," \
        --exclude='target/debug' \
        --exclude='.git' \
        --exclude='dist' \
        --exclude='build-deb' \
        -czf "/root/rpmbuild/SOURCES/${tarball_name}.tar.gz" \
        .
else
    tar --transform "s,^,${tarball_name}/," \
        --exclude='target' \
        --exclude='.git' \
        --exclude='dist' \
        --exclude='build-deb' \
        -czf "/root/rpmbuild/SOURCES/${tarball_name}.tar.gz" \
        .
fi

# Copy spec file
echo "==> Copying spec file..."
cp "/build/packaging/rpm/${SPEC_FILE}" /root/rpmbuild/SPECS/

# Build RPM
echo "==> Building RPM..."
cd /root/rpmbuild/SPECS
if [[ -n "$PREBUILT_BINARY" ]]; then
    rpmbuild -ba "${SPEC_FILE}" --define 'skip_build 1'
else
    rpmbuild -ba "${SPEC_FILE}"
fi

# Copy output to mounted volume
echo "==> Copying packages to output directory..."
mkdir -p /build/dist
cp /root/rpmbuild/RPMS/*/*.rpm /build/dist/ 2>/dev/null || true
cp /root/rpmbuild/SRPMS/*.rpm /build/dist/ 2>/dev/null || true

echo "==> Build complete!"
echo "==> Packages:"
ls -la /build/dist/*.rpm 2>/dev/null || echo "No RPM files found"
