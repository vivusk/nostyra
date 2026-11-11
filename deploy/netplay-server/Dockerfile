# Netplay signaling server image (M20). Build context = the repository root:
#   docker compose -f deploy/netplay-server/docker-compose.yml build
# (Dockerfile.dockerignore beside this file trims the context to the Cargo workspace.)

FROM rust:1.99.1-slim-bookworm AS build
WORKDIR /src
COPY . .
# --locked: the repository's Cargo.lock decides every version.
RUN cargo build --release --locked -p netplay-server \
    && cp target/release/netplay-server /netplay-server

FROM debian:bookworm-slim
# Unprivileged user; the server keeps nothing on disk.
RUN useradd --system --no-create-home --shell /usr/sbin/nologin netplay
COPY --from=build /netplay-server /usr/local/bin/netplay-server
USER netplay
# Inside the container listen on all interfaces; compose publishes the port on the
# host loopback only (127.0.0.1:3536), so nginx is the only way in.
ENV NETPLAY_BIND=0.0.0.0:3536
EXPOSE 3536
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/netplay-server"]
