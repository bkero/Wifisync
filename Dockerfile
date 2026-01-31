# Dockerfile for wifisync-server
#
# Build: docker build -t wifisync-server .
# Run:   docker run -p 8080:8080 -v wifisync-data:/data wifisync-server
#

# =============================================================================
# Builder stage
# =============================================================================
FROM rust:1.85-bookworm AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/wifisync-sync-protocol/Cargo.toml crates/wifisync-sync-protocol/
COPY crates/wifisync-server/Cargo.toml crates/wifisync-server/
COPY crates/wifisync-core/Cargo.toml crates/wifisync-core/
COPY crates/wifisync-cli/Cargo.toml crates/wifisync-cli/
COPY crates/wifisync-jni/Cargo.toml crates/wifisync-jni/

# Create dummy source files for dependency caching
RUN mkdir -p crates/wifisync-sync-protocol/src \
    && echo "pub fn dummy() {}" > crates/wifisync-sync-protocol/src/lib.rs \
    && mkdir -p crates/wifisync-server/src \
    && echo "fn main() {}" > crates/wifisync-server/src/main.rs \
    && mkdir -p crates/wifisync-core/src \
    && echo "pub fn dummy() {}" > crates/wifisync-core/src/lib.rs \
    && mkdir -p crates/wifisync-cli/src \
    && echo "fn main() {}" > crates/wifisync-cli/src/main.rs \
    && mkdir -p crates/wifisync-jni/src \
    && echo "pub fn dummy() {}" > crates/wifisync-jni/src/lib.rs

# Build dependencies only (this layer is cached)
RUN cargo build --release -p wifisync-server 2>/dev/null || true

# Copy actual source code
COPY crates/wifisync-sync-protocol/src crates/wifisync-sync-protocol/src
COPY crates/wifisync-server/src crates/wifisync-server/src
COPY crates/wifisync-core/src crates/wifisync-core/src

# Touch source files to ensure rebuild
RUN touch crates/wifisync-sync-protocol/src/lib.rs \
    && touch crates/wifisync-server/src/main.rs \
    && touch crates/wifisync-core/src/lib.rs

# Build the actual binary
RUN cargo build --release -p wifisync-server

# =============================================================================
# Runtime stage
# =============================================================================
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd --gid 1000 wifisync \
    && useradd --uid 1000 --gid 1000 --home-dir /data --no-create-home wifisync

# Create data directory
RUN mkdir -p /data && chown wifisync:wifisync /data

# Copy binary from builder
COPY --from=builder /build/target/release/wifisync-server /usr/local/bin/

# Set ownership
RUN chmod +x /usr/local/bin/wifisync-server

# Switch to non-root user
USER wifisync

# Set working directory
WORKDIR /data

# Environment variables
ENV DATABASE_URL=sqlite:/data/wifisync.db?mode=rwc
ENV BIND_ADDRESS=0.0.0.0:8080
ENV RUST_LOG=info

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD sqlite3 /data/wifisync.db "SELECT 1" 2>/dev/null || exit 1

# Run the server
CMD ["wifisync-server"]
