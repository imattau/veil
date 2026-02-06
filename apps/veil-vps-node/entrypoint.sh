#!/usr/bin/env bash
set -euo pipefail

warn() {
  echo "[veil-vps-node] $*" >&2
}

# Best-effort reverse proxy detection (container context is limited).
# We only emit guidance; we do not modify host configs.
if [[ -n "${PROXY_DOMAIN:-}" ]]; then
  warn "PROXY_DOMAIN set to '${PROXY_DOMAIN}' (assuming reverse proxy configured)"
else
  if [[ -n "${VIRTUAL_HOST:-}" || -n "${CADDY_HOST:-}" || -n "${TRAEFIK_ROUTER:-}" ]]; then
    warn "Detected proxy-related env vars (VIRTUAL_HOST/CADDY_HOST/TRAEFIK_*)."
  else
    warn "No reverse proxy hint detected. If you expect WebSocket access, configure a proxy and set PROXY_DOMAIN."
  fi
fi

exec /opt/veil-vps-node/veil-vps-node
