#!/bin/bash
#
# Entrypoint script for RPM Docker build
#

set -euo pipefail

PKG_NAME="wifisync"
PKG_VERSION="${PKG_VERSION:-0.1.0}"

echo "==> Building Wifisync RPM v${PKG_VERSION}"

# Create source tarball
echo "==> Creating source tarball..."
cd /build
tarball_name="${PKG_NAME}-${PKG_VERSION}"
mkdir -p "/root/rpmbuild/SOURCES"

tar --transform "s,^,${tarball_name}/," \
    --exclude='target' \
    --exclude='.git' \
    --exclude='dist' \
    --exclude='build-deb' \
    -czf "/root/rpmbuild/SOURCES/${tarball_name}.tar.gz" \
    .

# Copy spec file
echo "==> Copying spec file..."
cp /build/packaging/rpm/wifisync.spec /root/rpmbuild/SPECS/

# Build RPM
echo "==> Building RPM..."
cd /root/rpmbuild/SPECS
rpmbuild -ba wifisync.spec

# Copy output to mounted volume
echo "==> Copying packages to output directory..."
mkdir -p /build/dist
cp /root/rpmbuild/RPMS/*/*.rpm /build/dist/ 2>/dev/null || true
cp /root/rpmbuild/SRPMS/*.rpm /build/dist/ 2>/dev/null || true

echo "==> Build complete!"
echo "==> Packages:"
ls -la /build/dist/*.rpm 2>/dev/null || echo "No RPM files found"
