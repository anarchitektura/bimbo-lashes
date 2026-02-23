FROM rust:1.85-slim AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY apps/server ./apps/server
COPY apps/bot/Cargo.toml ./apps/bot/Cargo.toml
RUN mkdir -p apps/bot/src && echo "fn main(){}" > apps/bot/src/main.rs
RUN cargo build --release --package bimbo-lashes-server
RUN cargo test --release --package bimbo-lashes-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/bimbo-lashes-server .
ENV DATABASE_URL=sqlite:/app/data/bimbo.db?mode=rwc
EXPOSE 3000
CMD ["./bimbo-lashes-server"]
