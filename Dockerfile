# Source-build image for the optional Address Explorer remote server.
#
# The release workflow uses .github/docker/server-runtime.Dockerfile with the
# prebuilt native server binary instead of rebuilding here.

FROM rust:1.99-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config libssl-dev curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# `build.rs` compiles the Tailwind stylesheet before rustc runs.
ARG TAILWIND_VERSION=v4.1.5
RUN case "$(dpkg --print-architecture)" in \
        amd64) arch=x64 ;; \
        arm64) arch=arm64 ;; \
        *) echo "unsupported architecture: $(dpkg --print-architecture)" >&2; exit 1 ;; \
    esac \
    && curl --fail --location --retry 3 \
        --output /usr/local/bin/tailwindcss \
        "https://github.com/tailwindlabs/tailwindcss/releases/download/${TAILWIND_VERSION}/tailwindcss-linux-${arch}" \
    && chmod 0755 /usr/local/bin/tailwindcss

WORKDIR /app
COPY . .

RUN cargo build --release --locked --no-default-features --features server

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libssl3 passwd \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --uid 10001 --shell /usr/sbin/nologin app \
    && mkdir -p /data /usr/local/share/address-explorer /usr/local/bin/public \
    && chown -R app:app /data /usr/local/share/address-explorer

COPY --from=builder /app/target/release/address-explorer /usr/local/bin/address-explorer
COPY --from=builder /app/assets/pricing/btc_usd.bin /usr/local/share/address-explorer/btc_usd.bin

ENV IP=0.0.0.0 \
    PORT=8080 \
    DATABASE_PATH=/data/app.db \
    ADDRESS_EXPLORER_TLS_DIR=/data/tls \
    ADDRESS_EXPLORER_PRICING_FILE=/usr/local/share/address-explorer/btc_usd.bin

USER app
VOLUME ["/data"]
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fk https://127.0.0.1:8080/api/health || exit 1

ENTRYPOINT ["/usr/local/bin/address-explorer"]
