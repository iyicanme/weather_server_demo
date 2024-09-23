FROM lukemathwalker/cargo-chef:latest-rust-1.81 AS chef

WORKDIR /app
RUN apt update && apt install lld clang -y

FROM chef AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin weather_server_demo

FROM debian:bookworm-slim AS runtime

WORKDIR /app
RUN apt-get update -y \
        && apt-get install -y --no-install-recommends openssl ca-certificates \
        && apt-get autoremove -y \
        && apt-get clean -y \
        && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/weather_server_demo weather_server_demo
COPY config.toml config.toml
ENTRYPOINT ["./weather_server_demo"]
