#!/usr/bin/env bash
set -euo pipefail

PREFIX=${PREFIX:-/opt/veil-vps-node}
BIN=${BIN:-/usr/local/bin/veil-vps-node}
ENV_FILE=${ENV_FILE:-/opt/veil-vps-node/veil-vps-node.env}
SERVICE_FILE=${SERVICE_FILE:-/etc/systemd/system/veil-vps-node.service}
WEB_ROOT=${WEB_ROOT:-/var/www/veil-vps-node}
RUN_USER=${RUN_USER:-veil-vps}
RUN_GROUP=${RUN_GROUP:-veil-vps}

REVERSE_PROXY=${REVERSE_PROXY:-auto}  # auto|nginx|caddy|none
PROXY_DOMAIN=${PROXY_DOMAIN:-}
PROXY_HTTP_PORT=${PROXY_HTTP_PORT:-80}
PROXY_HTTPS_PORT=${PROXY_HTTPS_PORT:-443}
PROXY_WS_PORT=${PROXY_WS_PORT:-8080}
PROXY_HEALTH_PORT=${PROXY_HEALTH_PORT:-9090}

NGINX_SITE_PATH=${NGINX_SITE_PATH:-/etc/nginx/sites-available/veil-vps-node.conf}
NGINX_SITE_LINK=${NGINX_SITE_LINK:-/etc/nginx/sites-enabled/veil-vps-node.conf}
CADDY_CONF_DIR=${CADDY_CONF_DIR:-/etc/caddy/conf.d}
CADDY_SITE_PATH=${CADDY_SITE_PATH:-${CADDY_CONF_DIR}/veil-vps-node.caddy}
CADDYFILE=${CADDYFILE:-/etc/caddy/Caddyfile}
REUSE_EXISTING_SETTINGS=${REUSE_EXISTING_SETTINGS:-auto} # auto|1|0

env_file_value() {
  local key="$1"
  if [[ ! -f "$ENV_FILE" ]]; then
    return 1
  fi
  grep "^${key}=" "$ENV_FILE" | tail -n 1 | cut -d= -f2-
}

if [[ "${EUID}" -ne 0 ]]; then
  if command -v sudo >/dev/null 2>&1; then
    exec sudo -E "$0" "$@"
  fi
  echo "This installer must run as root (sudo required)."
  exit 1
fi

INSTALL_MODE="fresh"
if [[ -f "$ENV_FILE" || -x "$PREFIX/veil-vps-node" || -f "$SERVICE_FILE" ]]; then
  INSTALL_MODE="update"
fi

if [[ -z "$PROXY_DOMAIN" ]]; then
  existing_proxy_domain="$(env_file_value PROXY_DOMAIN || true)"
  if [[ -n "$existing_proxy_domain" ]]; then
    PROXY_DOMAIN="$existing_proxy_domain"
  fi
fi
if [[ "$REUSE_EXISTING_SETTINGS" == "auto" && "$INSTALL_MODE" == "update" ]]; then
  echo "Detected existing VEIL VPS node installation."
  echo "Installer can reuse existing settings from: $ENV_FILE"
  read -r -p "Reuse existing settings and perform update-only install? [Y/n] " reuse_confirm
  if [[ -z "$reuse_confirm" || "$reuse_confirm" =~ ^[Yy]$ ]]; then
    REUSE_EXISTING_SETTINGS="1"
  else
    REUSE_EXISTING_SETTINGS="0"
  fi
fi

if [[ -z "$PROXY_DOMAIN" && "$REVERSE_PROXY" != "none" && "$REUSE_EXISTING_SETTINGS" != "1" ]]; then
  read -r -p "Enter domain for TLS/HTTP (leave blank to skip proxy setup): " input_domain
  if [[ -n "$input_domain" ]]; then
    PROXY_DOMAIN="$input_domain"
  fi
fi

ensure_writable_dir() {
  local dir="$1"
  mkdir -p "$dir"
  if [[ ! -w "$dir" ]]; then
    echo "Permission check failed: $dir is not writable."
    exit 1
  fi
}

port_in_use() {
  local port="$1"
  if command -v ss >/dev/null 2>&1; then
    ss -ltnu | awk '{print $5}' | grep -qE "[:.]${port}\$"
    return $?
  fi
  if command -v lsof >/dev/null 2>&1; then
    lsof -i :"${port}" >/dev/null 2>&1
    return $?
  fi
  if command -v netstat >/dev/null 2>&1; then
    netstat -tuln | awk '{print $4}' | grep -qE "[:.]${port}\$"
    return $?
  fi
  return 1
}

pick_free_port() {
  local preferred="$1"
  shift
  if ! port_in_use "$preferred"; then
    echo "$preferred"
    return
  fi
  for candidate in "$@"; do
    if ! port_in_use "$candidate"; then
      echo "$candidate"
      return
    fi
  done
  echo "$preferred"
}

resolve_ports() {
  local existing_quic_bind=""
  if [[ -f "$ENV_FILE" ]]; then
    existing_quic_bind=$(grep "^VEIL_VPS_QUIC_BIND=" "$ENV_FILE" | tail -n 1 | cut -d= -f2-)
  fi
  if [[ -n "$existing_quic_bind" ]]; then
    VEIL_VPS_QUIC_BIND="$existing_quic_bind"
  fi
  if port_in_use "$PROXY_HTTP_PORT"; then
    if [[ "$REVERSE_PROXY" == "caddy" || "$REVERSE_PROXY" == "auto" ]] && systemctl is-active --quiet caddy; then
      echo "HTTP port ${PROXY_HTTP_PORT} already in use by Caddy; continuing."
    else
      echo "HTTP port ${PROXY_HTTP_PORT} is in use. Please free it to use standard ports."
      exit 1
    fi
  fi
  if port_in_use "$PROXY_HTTPS_PORT"; then
    if [[ "$REVERSE_PROXY" == "caddy" || "$REVERSE_PROXY" == "auto" ]] && systemctl is-active --quiet caddy; then
      echo "HTTPS port ${PROXY_HTTPS_PORT} already in use by Caddy; continuing."
    else
      echo "HTTPS port ${PROXY_HTTPS_PORT} is in use. Please free it to use standard ports."
      exit 1
    fi
  fi
  if port_in_use "$PROXY_HEALTH_PORT"; then
    local existing_health=""
    if [[ -f "$ENV_FILE" ]]; then
      existing_health=$(grep "^VEIL_VPS_HEALTH_PORT=" "$ENV_FILE" | tail -n 1 | cut -d= -f2-)
    fi
    if [[ -n "$existing_health" && "$existing_health" == "$PROXY_HEALTH_PORT" ]]; then
      echo "Health port ${PROXY_HEALTH_PORT} already used by veil-vps-node; continuing."
    else
      PROXY_HEALTH_PORT=$(pick_free_port "$PROXY_HEALTH_PORT" 9091 19090)
      echo "Health port in use, switching to ${PROXY_HEALTH_PORT}."
    fi
  fi
  local quic_port="5000"
  if [[ -n "${VEIL_VPS_QUIC_BIND:-}" ]]; then
    quic_port="${VEIL_VPS_QUIC_BIND##*:}"
  fi
  if port_in_use "$quic_port"; then
    local existing_quic_port=""
    if [[ -n "$existing_quic_bind" ]]; then
      existing_quic_port="${existing_quic_bind##*:}"
    fi
    if [[ -n "$existing_quic_port" && "$existing_quic_port" == "$quic_port" ]]; then
      echo "QUIC port ${quic_port} already used by veil-vps-node; continuing."
    else
      local new_quic
      new_quic=$(pick_free_port "$quic_port" 5001 15000)
      if [[ "$new_quic" != "$quic_port" ]]; then
        export VEIL_VPS_QUIC_BIND="0.0.0.0:${new_quic}"
        echo "QUIC port in use, switching to ${new_quic}."
      fi
    fi
  fi
}

has_systemd() {
  command -v systemctl >/dev/null 2>&1
}

detect_pkg_mgr() {
  if command -v apt-get >/dev/null 2>&1; then
    echo "apt"
  elif command -v dnf >/dev/null 2>&1; then
    echo "dnf"
  elif command -v yum >/dev/null 2>&1; then
    echo "yum"
  elif command -v pacman >/dev/null 2>&1; then
    echo "pacman"
  else
    echo "none"
  fi
}

install_pkgs() {
  local mgr="$1"
  shift
  case "$mgr" in
    apt)
      apt-get update -y
      apt-get install -y "$@"
      ;;
    dnf)
      dnf install -y "$@"
      ;;
    yum)
      yum install -y "$@"
      ;;
    pacman)
      pacman -Sy --noconfirm "$@"
      ;;
    *)
      return 1
      ;;
  esac
}

if ! id "$RUN_USER" >/dev/null 2>&1; then
  useradd --system --no-create-home --shell /usr/sbin/nologin "$RUN_USER" || true
fi
if ! getent group "$RUN_GROUP" >/dev/null 2>&1; then
  groupadd --system "$RUN_GROUP" || true
fi
usermod -a -G "$RUN_GROUP" "$RUN_USER" || true

ensure_writable_dir "$PREFIX"
ensure_writable_dir "$PREFIX/data"
ensure_writable_dir "$WEB_ROOT"
chmod 0700 "$PREFIX" "$PREFIX/data" || true
chown -R "$RUN_USER:$RUN_GROUP" "$PREFIX" "$WEB_ROOT" || true

ensure_cargo() {
  if command -v cargo >/dev/null 2>&1; then
    return 0
  fi
  echo "Rust toolchain not found."
  if [[ -n "${VEIL_VPS_USE_DOCKER:-}" ]]; then
    return 1
  fi
  read -r -p "Install Rust toolchain via rustup? [y/N] " confirm
  if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
    return 1
  fi
  if ! command -v curl >/dev/null 2>&1; then
    echo "curl is required to install rustup."
    local mgr
    mgr=$(detect_pkg_mgr)
    if [[ "$mgr" != "none" ]]; then
      install_pkgs "$mgr" curl
    else
      echo "No package manager found to install curl."
      return 1
    fi
  fi
  curl https://sh.rustup.rs -sSf | sh -s -- -y
  export PATH="$HOME/.cargo/bin:$PATH"
  if ! command -v cargo >/dev/null 2>&1; then
    return 1
  fi
  return 0
}

if command -v cargo >/dev/null 2>&1; then
  echo "Building veil-vps-node (release)..."
  cargo build -p veil-vps-node --release
elif [[ ! -f target/release/veil-vps-node ]]; then
  echo "Cargo not found and no binary present."
  if ! ensure_cargo; then
    echo "Cargo not available. You can set VEIL_VPS_USE_DOCKER=1 and run via Docker."
    exit 1
  fi
  cargo build -p veil-vps-node --release
fi

resolve_ports

install -m 0755 target/release/veil-vps-node "$PREFIX/veil-vps-node"
ln -sf "$PREFIX/veil-vps-node" "$BIN"

if [[ ! -f "$ENV_FILE" ]]; then
  install -m 0600 docs/runbooks/veil-vps-node.env.example "$ENV_FILE"
  echo "Wrote env template to $ENV_FILE"
fi
chmod 0600 "$ENV_FILE" || true
chown "$RUN_USER:$RUN_GROUP" "$ENV_FILE" || true

DEFAULT_CORE_TAGS="6914e6d3b151b9ac372db7c201ae4e043af645245ecce6175648d42b6177a9ca,7f3612b9145b9ae924e119dbce48ea5bba8ef366d50f10fdf490fc88378c7180,040257d0dadd0ec43e267cc60c2a3c4306e1665273e0ba88065254bbd082a590,7f3fccfbad7a618eecccf31277a79691c5d6a657e50f45dd671319f84ee1d010"
if ! grep -q "^VEIL_VPS_CORE_TAGS=" "$ENV_FILE"; then
  echo "" >> "$ENV_FILE"
  echo "# Default core tags: Public Square, Local Builders, Civic Updates, Open Media" >> "$ENV_FILE"
  echo "VEIL_VPS_CORE_TAGS=${DEFAULT_CORE_TAGS}" >> "$ENV_FILE"
fi

set_env_var() {
  local key="$1"
  local value="$2"
  if ! grep -q "^${key}=" "$ENV_FILE"; then
    echo "${key}=${value}" >> "$ENV_FILE"
  fi
}

HOSTNAME_FQDN=$(hostname -f 2>/dev/null || hostname)
set_env_var "VEIL_VPS_STATE_PATH" "${PREFIX}/data/node_state.cbor"
set_env_var "VEIL_VPS_NODE_KEY_PATH" "${PREFIX}/data/node_identity.key"
set_env_var "VEIL_VPS_QUIC_CERT_PATH" "${PREFIX}/data/quic_cert.der"
set_env_var "VEIL_VPS_QUIC_KEY_PATH" "${PREFIX}/data/quic_key.der"
set_env_var "VEIL_VPS_QUIC_BIND" "${VEIL_VPS_QUIC_BIND:-0.0.0.0:5000}"
set_env_var "VEIL_VPS_QUIC_ALPN" "veil-quic/1,veil/1,veil-node,veil,h3,hq-29"
set_env_var "VEIL_VPS_FAST_PEERS" ""
set_env_var "VEIL_VPS_CORE_TAGS" "${DEFAULT_CORE_TAGS}"
set_env_var "VEIL_VPS_PEER_DB_PATH" "${PREFIX}/data/peers.db"
set_env_var "VEIL_VPS_MAX_DYNAMIC_PEERS" "512"
set_env_var "VEIL_VPS_WS_URL" ""
set_env_var "VEIL_VPS_WS_PEER" "${HOSTNAME_FQDN:-veil-vps}"
set_env_var "VEIL_VPS_WS_LISTEN" "127.0.0.1:${PROXY_WS_PORT}"
set_env_var "VEIL_VPS_TOR_SOCKS_ADDR" ""
set_env_var "VEIL_VPS_TOR_PEERS" ""
set_env_var "VEIL_VPS_BLE_ENABLE" "0"
set_env_var "VEIL_VPS_BLE_PEERS" ""
set_env_var "VEIL_VPS_BLE_ALLOWLIST" ""
set_env_var "VEIL_VPS_BLE_MTU" "180"
set_env_var "VEIL_VPS_MAX_CACHE_SHARDS" "200000"
set_env_var "VEIL_VPS_BUCKET_JITTER" "0"
set_env_var "VEIL_VPS_REQUIRED_SIGNED_NAMESPACES" ""
set_env_var "VEIL_VPS_ADAPTIVE_LANE_SCORING" "1"
set_env_var "VEIL_VPS_SNAPSHOT_SECS" "60"
set_env_var "VEIL_VPS_TICK_MS" "50"
set_env_var "VEIL_VPS_HEALTH_BIND" "0.0.0.0"
set_env_var "VEIL_VPS_HEALTH_PORT" "${PROXY_HEALTH_PORT}"
set_env_var "PROXY_DOMAIN" "${PROXY_DOMAIN}"
set_env_var "VEIL_VPS_OPEN_RELAY" "0"
set_env_var "VEIL_VPS_BLOCKED_PEERS" ""
set_env_var "VEIL_VPS_NOSTR_BRIDGE_ENABLE" "0"
set_env_var "VEIL_VPS_NOSTR_RELAYS" "wss://relay.damus.io,wss://nos.lol,wss://relay.snort.social"
set_env_var "VEIL_VPS_NOSTR_CHANNEL_ID" "nostr-bridge"
set_env_var "VEIL_VPS_NOSTR_NAMESPACE" "32"
set_env_var "VEIL_VPS_NOSTR_SINCE_SECS" "3600"
set_env_var "VEIL_VPS_NOSTR_BRIDGE_STATE_PATH" "${PREFIX}/data/nostr-bridge-state.json"
set_env_var "VEIL_VPS_NOSTR_MAX_SEEN_IDS" "20000"
set_env_var "VEIL_VPS_NOSTR_PERSIST_EVERY_UPDATES" "32"

if [[ -f docs/runbooks/veil-vps-node.service ]]; then
  install -m 0644 docs/runbooks/veil-vps-node.service "$SERVICE_FILE" || true
  if has_systemd; then
    systemctl daemon-reload || true
    systemctl enable veil-vps-node.service || true
    systemctl restart veil-vps-node.service || true
  else
    echo "systemd not detected; skipping service enable."
  fi
fi

if [[ -d apps/veil-vps-node/web ]]; then
  install -d -m 0755 "$WEB_ROOT"
  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete apps/veil-vps-node/web/ "$WEB_ROOT"/
  else
    rm -rf "${WEB_ROOT:?}/"*
    cp -a apps/veil-vps-node/web/. "$WEB_ROOT"/
  fi
  quic_bind_value="${VEIL_VPS_QUIC_BIND:-0.0.0.0:5000}"
  quic_port="${quic_bind_value##*:}"
  quic_cert_path="${VEIL_VPS_QUIC_CERT_PATH:-${PREFIX}/data/quic_cert.der}"
  quic_cert_b64=""
  if [[ -f "${quic_cert_path}" ]]; then
    if command -v base64 >/dev/null 2>&1; then
      quic_cert_b64=$(base64 -w0 "${quic_cert_path}" 2>/dev/null || base64 "${quic_cert_path}" | tr -d '\n')
    fi
  fi
  {
    echo "window.VEIL_VPS_QUIC_PORT = ${quic_port:-5000};"
    echo "window.VEIL_VPS_QUIC_CERT_B64 = \"${quic_cert_b64}\";"
  } > "${WEB_ROOT}/config.js"
  chown -R "$RUN_USER:$RUN_GROUP" "$WEB_ROOT" || true
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
limit_req_zone \$binary_remote_addr zone=veil_ws:10m rate=30r/s;
limit_req_zone \$binary_remote_addr zone=veil_health:1m rate=5r/s;

server {
    listen ${PROXY_HTTP_PORT};
    server_name ${PROXY_DOMAIN};

    root ${WEB_ROOT};

    location / {
        try_files \$uri \$uri/ /index.html;
    }

    location /ws {
        proxy_pass http://127.0.0.1:${PROXY_WS_PORT};
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        limit_req zone=veil_ws burst=60 nodelay;
    }

    location /health {
        proxy_pass http://127.0.0.1:${PROXY_HEALTH_PORT};
        proxy_set_header Host $host;
        limit_req zone=veil_health burst=10 nodelay;
    }
    location /metrics {
        proxy_pass http://127.0.0.1:${PROXY_HEALTH_PORT};
        proxy_set_header Host $host;
        limit_req zone=veil_health burst=10 nodelay;
    }
    location /peers {
        proxy_pass http://127.0.0.1:${PROXY_HEALTH_PORT};
        proxy_set_header Host $host;
        limit_req zone=veil_health burst=10 nodelay;
    }
    location /admin-api {
        proxy_pass http://127.0.0.1:${PROXY_HEALTH_PORT};
        proxy_set_header Host $host;
        limit_req zone=veil_health burst=10 nodelay;
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
  install -d -m 0755 "$CADDY_CONF_DIR"
  cat <<CADDYCONF > "$CADDY_SITE_PATH"
${PROXY_DOMAIN} {
  root * ${WEB_ROOT}
  file_server
  reverse_proxy /ws* 127.0.0.1:${PROXY_WS_PORT}
  reverse_proxy /health 127.0.0.1:${PROXY_HEALTH_PORT}
  reverse_proxy /metrics 127.0.0.1:${PROXY_HEALTH_PORT}
  reverse_proxy /peers 127.0.0.1:${PROXY_HEALTH_PORT}
  reverse_proxy /admin-api* 127.0.0.1:${PROXY_HEALTH_PORT}
}
CADDYCONF
  if [[ -f "$CADDYFILE" ]] && ! grep -q "conf.d" "$CADDYFILE"; then
    echo "import ${CADDY_CONF_DIR}/*" >> "$CADDYFILE"
  fi
  if [[ -f "$CADDYFILE" ]] && ! grep -q "http_port" "$CADDYFILE"; then
    true
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
    else
      echo "No reverse proxy found. Installing Caddy..."
      PKG_MGR=$(detect_pkg_mgr)
      if [[ "$PKG_MGR" == "apt" ]]; then
        install_pkgs apt debian-keyring debian-archive-keyring apt-transport-https curl gpg
        curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
        curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' > /etc/apt/sources.list.d/caddy-stable.list
        apt-get update -y
        apt-get install -y caddy
        configure_caddy || true
      elif [[ "$PKG_MGR" == "dnf" || "$PKG_MGR" == "yum" ]]; then
        install_pkgs "$PKG_MGR" yum-utils curl
        yum-config-manager --add-repo https://dl.cloudsmith.io/public/caddy/stable/rpm/caddy-stable.repo || true
        install_pkgs "$PKG_MGR" caddy
        configure_caddy || true
      elif [[ "$PKG_MGR" == "pacman" ]]; then
        install_pkgs pacman caddy
        configure_caddy || true
      else
        echo "No supported package manager found to install Caddy."
        echo "Please install Caddy manually, then re-run this installer."
      fi
    fi
  fi
fi

echo "Installed veil-vps-node. Edit $ENV_FILE then:"
echo "  systemctl start veil-vps-node.service"

# Display node identity for admin login
if [[ -x "$BIN" ]]; then
  echo ""
  echo "--- NODE IDENTITY (Admin Login) ---"
  # Run as root but use the config from env file if available
  # Note: the binary might need to run as RUN_USER to access data dir if it already exists
  # but here we just want to export the identity.
  # We use the env file to find the key path.
  NODE_KEY_PATH=$(grep "^VEIL_VPS_NODE_KEY_PATH=" "$ENV_FILE" | cut -d= -f2- || echo "${PREFIX}/data/node_identity.key")
  
  # Ensure the data directory exists and is owned by the run user for when the service starts
  mkdir -p "$(dirname "$NODE_KEY_PATH")"
  chown -R "$RUN_USER:$RUN_GROUP" "$PREFIX"
  
  # Export identity
  if [[ -f "$NODE_KEY_PATH" ]]; then
    # Key already exists, just show it
    VEIL_LOG=info sudo -u "$RUN_USER" "$BIN" --config "$ENV_FILE" identity || true
  else
    # First time generation
    VEIL_LOG=info sudo -u "$RUN_USER" "$BIN" --config "$ENV_FILE" identity || true
  fi
  echo "-----------------------------------"
  echo "Use the 'nsec' or 'hex' secret above to log in to the admin dashboard."
  echo ""
fi

check_service() {
  local health_url="http://127.0.0.1:${PROXY_HEALTH_PORT}/health"
  if has_systemd && systemctl is-active --quiet veil-vps-node.service; then
    echo "veil-vps-node service is active."
    if command -v curl >/dev/null 2>&1; then
      if curl -fsS "$health_url" >/dev/null; then
        echo "Health check OK: $health_url"
      else
        echo "Health check failed: $health_url"
      fi
    else
      echo "curl not found."
      read -r -p "Install curl to run health checks? [y/N] " confirm
      if [[ "$confirm" =~ ^[Yy]$ ]]; then
        PKG_MGR=$(detect_pkg_mgr)
        if [[ "$PKG_MGR" != "none" ]]; then
          install_pkgs "$PKG_MGR" curl
          if curl -fsS "$health_url" >/dev/null; then
            echo "Health check OK: $health_url"
          else
            echo "Health check failed: $health_url"
          fi
        else
          echo "No supported package manager found to install curl."
        fi
      fi
    fi
  else
    echo "veil-vps-node service is not active yet (or systemd missing)."
  fi
}

configure_firewall() {
  local quic_bind="${VEIL_VPS_QUIC_BIND:-0.0.0.0:5000}"
  local quic_port="${quic_bind##*:}"
  if command -v ufw >/dev/null 2>&1; then
    ufw allow "${PROXY_HTTP_PORT}"/tcp || true
    ufw allow "${PROXY_HTTPS_PORT}"/tcp || true
    # Health port is bound to localhost by default; no public firewall rule.
    ufw allow "${quic_port}"/udp || true
    ufw --force enable || true
    echo "Configured UFW rules."
    return
  fi
  if command -v firewall-cmd >/dev/null 2>&1; then
    firewall-cmd --permanent --add-port="${PROXY_HTTP_PORT}"/tcp || true
    firewall-cmd --permanent --add-port="${PROXY_HTTPS_PORT}"/tcp || true
    # Health port is bound to localhost by default; no public firewall rule.
    firewall-cmd --permanent --add-port="${quic_port}"/udp || true
    firewall-cmd --reload || true
    echo "Configured firewalld rules."
    return
  fi
  echo "No supported firewall manager found (ufw/firewalld)."
}

if [[ -n "$PROXY_DOMAIN" ]]; then
  configure_firewall || true
else
  echo "PROXY_DOMAIN not set; skipping firewall config."
fi

configure_tor() {
  if [[ "$REUSE_EXISTING_SETTINGS" == "1" ]]; then
    echo "Tor prompt skipped (update mode reusing existing settings)."
    return
  fi
  read -r -p "Enable Tor SOCKS5 fallback? [y/N] " confirm
  if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
    echo "Tor setup skipped."
    return
  fi
  if ! command -v tor >/dev/null 2>&1; then
    echo "Tor not found. Installing Tor daemon..."
    PKG_MGR=$(detect_pkg_mgr)
    if [[ "$PKG_MGR" != "none" ]]; then
      install_pkgs "$PKG_MGR" tor
    else
      echo "No supported package manager found to install Tor."
      return
    fi
  fi
  if has_systemd; then
    systemctl enable tor || true
    systemctl restart tor || true
  fi
  set_env_var "VEIL_VPS_TOR_SOCKS_ADDR" "127.0.0.1:9050"
  set_env_var "VEIL_VPS_TOR_PEERS" ""
  echo "Tor configured: VEIL_VPS_TOR_SOCKS_ADDR=127.0.0.1:9050"
}

configure_tor

check_service
