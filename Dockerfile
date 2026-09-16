# GameCloud OS — web platform and Discord bot in one image.
#
# The build stage compiles the SSR server with its WASM bundle and styles
# (cargo-leptos fetches the matching wasm-bindgen and dart-sass itself) and
# the bot. The runtime stage keeps only the two binaries and the static
# site. The same image runs as two services, `web` and `bot`.

FROM rust:1.93-bookworm AS build
RUN rustup target add wasm32-unknown-unknown \
 && cargo install --locked cargo-leptos --version 0.3.6
WORKDIR /app
COPY . .
RUN cargo leptos build --release \
 && cargo build --release -p gamecloud-bot

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/gamecloud-web /app/bin/gamecloud-web
COPY --from=build /app/target/release/gamecloud-bot /app/bin/gamecloud-bot
COPY --from=build /app/target/site /app/site

# Leptos reads its settings from the environment when Cargo.toml is absent.
ENV LEPTOS_OUTPUT_NAME=gamecloud \
    LEPTOS_SITE_ROOT=/app/site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=0.0.0.0:8080 \
    LEPTOS_RELOAD_PORT=3001 \
    LEPTOS_ENV=PROD \
    BIND_ADDR=0.0.0.0:8080 \
    APP_ENV=production \
    SHARES_DIR=/data/shares \
    TESTS_DIR=/data/tests

EXPOSE 8080
# One image, two services: GAMECLOUD_PROCESS picks `web` (default) or `bot`.
# Hosts that assign the port through $PORT (Railway) are honoured.
CMD ["sh", "-c", "export BIND_ADDR=0.0.0.0:${PORT:-8080} LEPTOS_SITE_ADDR=0.0.0.0:${PORT:-8080}; exec /app/bin/gamecloud-${GAMECLOUD_PROCESS:-web}"]
