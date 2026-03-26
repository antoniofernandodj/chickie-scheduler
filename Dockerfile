# Stage 1: Build
FROM rust:1.85-bookworm as builder

WORKDIR /app
COPY Cargo.toml ./
COPY src ./src

RUN cargo build --release

# Stage 2: Run
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/rust-cron /app/rust-cron

RUN useradd -m -u 1000 cronuser
USER cronuser

CMD ["./chickie-scheduler"]
