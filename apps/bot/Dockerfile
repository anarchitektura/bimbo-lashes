FROM rust:1.85-slim AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY apps/bot ./apps/bot
COPY apps/server/Cargo.toml ./apps/server/Cargo.toml
RUN mkdir -p apps/server/src && echo "fn main(){}" > apps/server/src/main.rs
RUN cargo build --release --package bimbo-lashes-bot
RUN cargo test --release --package bimbo-lashes-bot

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/bimbo-lashes-bot .
ENV DATABASE_URL=sqlite:/app/data/bimbo.db?mode=rwc
CMD ["./bimbo-lashes-bot"]
