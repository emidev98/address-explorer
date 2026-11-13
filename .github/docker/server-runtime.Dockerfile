# Runtime-only image used by the release workflow.
#
# The build context contains one directory per target architecture:
#   amd64/address-explorer + amd64/btc_usd.bin
#   arm64/address-explorer + arm64/btc_usd.bin

FROM debian:bookworm-slim

ARG TARGETARCH

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libssl3 passwd \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --uid 10001 --shell /usr/sbin/nologin app \
    && mkdir -p /data /usr/local/share/address-explorer /usr/local/bin/public

COPY ${TARGETARCH}/address-explorer /usr/local/bin/address-explorer
COPY ${TARGETARCH}/btc_usd.bin /usr/local/share/address-explorer/btc_usd.bin

RUN chown -R app:app /data /usr/local/share/address-explorer

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
