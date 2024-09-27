FROM rust:1.53 AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
RUN cargo build --release --features 'cli server'

FROM debian:buster-slim
RUN apt-get update -q && apt-get install -qqy git gpg libssl-dev openssh-client xz-utils jq
COPY --from=builder /src/target/release/server /usr/local/bin/registry-server
COPY contrib/docker-entrypoint.sh contrib/hook.sh /app/
EXPOSE 8000
WORKDIR /app

ENV DB_GIT_REMOTE=git@github.com:Blockstream/asset_registry_db.git
ENV ESPLORA_URL=https://blockstream.info/liquid/api
ENV ADDR=0.0.0.0:8000
ENV DB_PATH=/app/db
ENV WWW_PATH=/app/www
ENV HOOK_CMD=/app/hook.sh

# The private key to sign commits with, also used to verify incoming git changes.
# Needs to be mounted-in.
ENV GPG_KEY_PATH=/app/signing-privkey.asc

ENTRYPOINT [ "/app/docker-entrypoint.sh" ]