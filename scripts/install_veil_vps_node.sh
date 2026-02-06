#!/usr/bin/env bash
set -euo pipefail

PREFIX=${PREFIX:-/opt/veil-vps-node}
BIN=${BIN:-/usr/local/bin/veil-vps-node}
ENV_FILE=${ENV_FILE:-/opt/veil-vps-node/veil-vps-node.env}
SERVICE_FILE=${SERVICE_FILE:-/etc/systemd/system/veil-vps-node.service}

mkdir -p "$PREFIX"/data

if [[ ! -f target/release/veil-vps-node ]]; then
  echo "Building veil-vps-node (release)..."
  cargo build -p veil-vps-node --release
fi

install -m 0755 target/release/veil-vps-node "$PREFIX/veil-vps-node"
ln -sf "$PREFIX/veil-vps-node" "$BIN"

if [[ ! -f "$ENV_FILE" ]]; then
  install -m 0644 docs/runbooks/veil-vps-node.env.example "$ENV_FILE"
  echo "Wrote env template to $ENV_FILE"
fi

install -m 0644 docs/runbooks/veil-vps-node.service "$SERVICE_FILE"

systemctl daemon-reload
systemctl enable veil-vps-node.service

echo "Installed veil-vps-node. Edit $ENV_FILE then:"
echo "  systemctl start veil-vps-node.service"
