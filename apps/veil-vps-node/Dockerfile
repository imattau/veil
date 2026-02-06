FROM rust:1.75-slim as builder
WORKDIR /app

# Cache deps
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/veil-vps-node ./apps/veil-vps-node

RUN cargo build -p veil-vps-node --release

FROM debian:bookworm-slim
WORKDIR /opt/veil-vps-node
COPY --from=builder /app/target/release/veil-vps-node /opt/veil-vps-node/veil-vps-node
COPY apps/veil-vps-node/entrypoint.sh /opt/veil-vps-node/entrypoint.sh

# Runtime defaults
ENV VEIL_VPS_STATE_PATH=/opt/veil-vps-node/data/node_state.cbor
ENV VEIL_VPS_NODE_KEY_PATH=/opt/veil-vps-node/data/node_identity.key
ENV VEIL_VPS_QUIC_CERT_PATH=/opt/veil-vps-node/data/quic_cert.der
ENV VEIL_VPS_QUIC_KEY_PATH=/opt/veil-vps-node/data/quic_key.der
ENV VEIL_VPS_QUIC_BIND=0.0.0.0:5000

EXPOSE 5000/udp

VOLUME ["/opt/veil-vps-node/data"]

ENTRYPOINT ["/opt/veil-vps-node/entrypoint.sh"]
