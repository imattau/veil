#!/usr/bin/env bash
set -euo pipefail

PREFIX=${PREFIX:-/opt/veil-vps-node}
BIN=${BIN:-/usr/local/bin/veil-vps-node}
ENV_FILE=${ENV_FILE:-/opt/veil-vps-node/veil-vps-node.env}
SERVICE_FILE=${SERVICE_FILE:-/etc/systemd/system/veil-vps-node.service}

REVERSE_PROXY=${REVERSE_PROXY:-auto}  # auto|nginx|caddy|none
PROXY_DOMAIN=${PROXY_DOMAIN:-}
PROXY_HTTP_PORT=${PROXY_HTTP_PORT:-80}
PROXY_WS_PORT=${PROXY_WS_PORT:-8080}
PROXY_HEALTH_PORT=${PROXY_HEALTH_PORT:-9090}

NGINX_SITE_PATH=${NGINX_SITE_PATH:-/etc/nginx/sites-available/veil-vps-node.conf}
NGINX_SITE_LINK=${NGINX_SITE_LINK:-/etc/nginx/sites-enabled/veil-vps-node.conf}
CADDY_SITE_PATH=${CADDY_SITE_PATH:-/etc/caddy/veil-vps-node.caddy}
CADDYFILE=${CADDYFILE:-/etc/caddy/Caddyfile}

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

if [[ -f docs/runbooks/veil-vps-node.service ]]; then
  install -m 0644 docs/runbooks/veil-vps-node.service "$SERVICE_FILE" || true
  systemctl daemon-reload || true
  systemctl enable veil-vps-node.service || true
fi

configure_nginx() {
  if [[ -f "$NGINX_SITE_PATH" ]]; then
    echo "nginx config exists at $NGINX_SITE_PATH (skipping)"
    return 0
  fi
  if [[ -z "$PROXY_DOMAIN" ]]; then
    echo "PROXY_DOMAIN not set; skipping nginx config"
    return 0
  fi
  cat <<NGINXCONF > "$NGINX_SITE_PATH"
server {
    listen ${PROXY_HTTP_PORT};
    server_name ${PROXY_DOMAIN};

    location /ws {
        proxy_pass http://127.0.0.1:${PROXY_WS_PORT};
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
    }

    location /health {
        proxy_pass http://127.0.0.1:${PROXY_HEALTH_PORT};
        proxy_set_header Host $host;
    }
    location /metrics {
        proxy_pass http://127.0.0.1:${PROXY_HEALTH_PORT};
        proxy_set_header Host $host;
    }
}
NGINXCONF
  if [[ ! -e "$NGINX_SITE_LINK" ]]; then
    ln -s "$NGINX_SITE_PATH" "$NGINX_SITE_LINK" || true
  fi
  nginx -t && systemctl reload nginx || true
}

configure_caddy() {
  if [[ -f "$CADDY_SITE_PATH" ]]; then
    echo "caddy config exists at $CADDY_SITE_PATH (skipping)"
    return 0
  fi
  if [[ -z "$PROXY_DOMAIN" ]]; then
    echo "PROXY_DOMAIN not set; skipping caddy config"
    return 0
  fi
  cat <<CADDYCONF > "$CADDY_SITE_PATH"
${PROXY_DOMAIN} {
  reverse_proxy /ws/* 127.0.0.1:${PROXY_WS_PORT}
  reverse_proxy /health 127.0.0.1:${PROXY_HEALTH_PORT}
  reverse_proxy /metrics 127.0.0.1:${PROXY_HEALTH_PORT}
}
CADDYCONF
  if [[ -f "$CADDYFILE" ]] && ! grep -q "veil-vps-node.caddy" "$CADDYFILE"; then
    echo "import $CADDY_SITE_PATH" >> "$CADDYFILE"
  fi
  caddy validate --config "$CADDYFILE" && systemctl reload caddy || true
}

if [[ "$REVERSE_PROXY" != "none" ]]; then
  if [[ "$REVERSE_PROXY" == "nginx" ]]; then
    configure_nginx || true
  elif [[ "$REVERSE_PROXY" == "caddy" ]]; then
    configure_caddy || true
  else
    if command -v nginx >/dev/null 2>&1; then
      configure_nginx || true
    elif command -v caddy >/dev/null 2>&1; then
      configure_caddy || true
    fi
  fi
fi

echo "Installed veil-vps-node. Edit $ENV_FILE then:"
echo "  systemctl start veil-vps-node.service"
