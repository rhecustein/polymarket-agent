FROM rust:1.82 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/polyagent .

ENV RUST_LOG=info
VOLUME ["/app/data"]

ENTRYPOINT ["./polyagent"]
