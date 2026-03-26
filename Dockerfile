# Stage 1: Build
FROM rust:1.85-bookworm as builder

WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
COPY config.toml ./config.toml

RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copia o binário compilado
COPY --from=builder /app/target/release/chickie-scheduler /app/chickie-scheduler

# ✅ CRUCIAL: Copiar o config.toml do builder para o runtime
COPY --from=builder /app/config.toml /app/config.toml

# Permissões para o usuário não-root
RUN useradd -m -u 1000 cronuser && chown -R cronuser:cronuser /app
USER cronuser

ENV CONFIG_PATH=/app/config.toml

CMD ["./chickie-scheduler"]