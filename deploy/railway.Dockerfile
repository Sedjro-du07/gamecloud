# Runtime image for binaries compiled *locally* and uploaded with
# `railway up` (see deploy/railway-up.sh). Nothing is compiled here.
#
# Ubuntu 24.04 rather than the Debian image the repository Dockerfile
# builds on: the binaries come from an Ubuntu 24.04 machine and must find
# the same glibc and OpenSSL they were linked against.
FROM ubuntu:24.04
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3t64 \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY bin/gamecloud-web /app/bin/gamecloud-web
COPY bin/gamecloud-bot /app/bin/gamecloud-bot
COPY site /app/site

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
CMD ["sh", "-c", "export BIND_ADDR=0.0.0.0:${PORT:-8080} LEPTOS_SITE_ADDR=0.0.0.0:${PORT:-8080}; exec /app/bin/gamecloud-${GAMECLOUD_PROCESS:-web}"]
