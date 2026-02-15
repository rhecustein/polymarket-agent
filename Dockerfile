# ══════════════════════════════════════════════
# Polymarket AI Agent - Minimal Dockerfile
# Gemini-Only Mode (No Proxy)
# ══════════════════════════════════════════════

# ── Stage 1: Builder ──
FROM rustlang/rust:nightly-bookworm AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY agent/Cargo.toml ./agent/

# Create dummy source for dependency caching
RUN mkdir -p agent/src/bin && \
    echo 'fn main() {}' > agent/src/main.rs && \
    echo 'fn main() {}' > agent/src/bin/dashboard.rs && \
    cargo build --release --manifest-path agent/Cargo.toml 2>/dev/null || true && \
    rm -rf agent/src

# Copy actual source code
COPY agent/src ./agent/src

# Clean cache and build for real
RUN rm -rf target/release/deps/polymarket* target/release/deps/dashboard* && \
    cargo build --release --manifest-path agent/Cargo.toml

# ── Stage 2: Runtime ──
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy dashboard binary
COPY --from=builder /build/target/release/dashboard /app/dashboard

# Create data directories
RUN mkdir -p /app/data /app/configs

# Environment
ENV RUST_LOG=info
ENV GEMINI_ONLY=true

# Expose dashboard port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD test -e /app/data || exit 1

# Run dashboard
CMD ["/app/dashboard"]
