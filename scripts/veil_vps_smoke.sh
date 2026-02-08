#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/veil_vps_smoke.sh <base_url|host> [--ws <ws_url>] [--health <health_url>] [--quic-port <port>]

Examples:
  scripts/veil_vps_smoke.sh https://veilnode.example.com
  scripts/veil_vps_smoke.sh veilnode.example.com --ws wss://veilnode.example.com/ws --quic-port 5000

Checks:
  - /health (HTTP)
  - /peers (HTTP)
  - WebSocket handshake (if websocat or wscat is installed)
  - QUIC UDP port probe (if nc is installed)
USAGE
}

BASE_INPUT="${1:-}"
shift || true

if [[ -z "${BASE_INPUT}" || "${BASE_INPUT}" == "-h" || "${BASE_INPUT}" == "--help" ]]; then
  usage
  exit 0
fi

WS_URL=""
HEALTH_URL=""
QUIC_PORT="5000"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --ws)
      WS_URL="${2:-}"
      shift 2
      ;;
    --health)
      HEALTH_URL="${2:-}"
      shift 2
      ;;
    --quic-port)
      QUIC_PORT="${2:-}"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1"
      usage
      exit 1
      ;;
  esac
done

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required for HTTP checks"
  exit 1
fi

base="$BASE_INPUT"
if [[ "$base" != http*://* ]]; then
  base="https://${base}"
fi

if [[ -z "${HEALTH_URL}" ]]; then
  HEALTH_URL="${base%/}/health"
fi

if [[ -z "${WS_URL}" ]]; then
  if [[ "$base" == https://* ]]; then
    WS_URL="wss://${base#https://}"
  else
    WS_URL="ws://${base#http://}"
  fi
  WS_URL="${WS_URL%/}/ws"
fi

echo "== VEIL VPS smoke test =="
echo "Base:   ${base}"
echo "Health: ${HEALTH_URL}"
echo "WS:     ${WS_URL}"
echo "QUIC:   ${QUIC_PORT}"
echo

echo "-- HTTP health"
curl -fsS "${HEALTH_URL}" >/dev/null
echo "OK"

echo "-- HTTP peers"
curl -fsS "${base%/}/peers?limit=5" >/dev/null || echo "WARN: /peers not reachable"

echo "-- WebSocket handshake"
if command -v websocat >/dev/null 2>&1; then
  echo -n | timeout 3 websocat -n1 "${WS_URL}" >/dev/null || {
    echo "WARN: websocat failed to connect"
  }
  echo "OK (websocat)"
elif command -v wscat >/dev/null 2>&1; then
  timeout 3 wscat -c "${WS_URL}" -x "" >/dev/null || {
    echo "WARN: wscat failed to connect"
  }
  echo "OK (wscat)"
else
  echo "SKIP: websocat or wscat not installed"
fi

echo "-- QUIC UDP probe"
if command -v nc >/dev/null 2>&1; then
  host="${base#https://}"
  host="${host#http://}"
  host="${host%%/*}"
  if nc -z -u -w 2 "${host}" "${QUIC_PORT}" >/dev/null 2>&1; then
    echo "OK"
  else
    echo "WARN: UDP probe failed (port closed or filtered)"
  fi
else
  echo "SKIP: nc not installed"
fi

echo "Done."
