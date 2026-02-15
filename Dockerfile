# ══════════════════════════════════════════════
# Polymarket AI Agent - Multi-stage Docker Build
# Builds: dashboard, polyproxy binaries
# ══════════════════════════════════════════════

# ── Stage 1: Builder ──
FROM rust:latest AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy workspace manifests for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY agent/Cargo.toml ./agent/
COPY proxy/Cargo.toml ./proxy/

# Create dummy source files to cache dependencies
RUN mkdir -p agent/src/bin proxy/src && \
    echo 'fn main() {}' > agent/src/main.rs && \
    echo 'fn main() {}' > agent/src/bin/dashboard.rs && \
    echo 'fn main() {}' > proxy/src/main.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf agent/src proxy/src

# Copy actual source code
COPY agent/src ./agent/src
COPY proxy/src ./proxy/src

# Build all binaries with release optimizations
RUN cargo build --release --workspace

# ── Stage 2: Dashboard Runtime ──
FROM debian:bookworm-slim AS dashboard

RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy dashboard binary
COPY --from=builder /build/target/release/dashboard /app/dashboard

# Create directories for data and configs
RUN mkdir -p /app/data /app/configs

ENV RUST_LOG=info
EXPOSE 3000

CMD ["/app/dashboard"]

# ── Stage 3: Proxy Runtime ──
FROM debian:bookworm-slim AS proxy

RUN apt-get update && \
    apt-get install -y ca-certificates bash && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy proxy binary
COPY --from=builder /build/target/release/polyproxy /app/polyproxy

ENV RUST_LOG=info
EXPOSE 3001

CMD ["/app/polyproxy"]
