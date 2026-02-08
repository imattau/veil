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
PROXY_HEALTH_USER=${PROXY_HEALTH_USER:-veil-health}
PROXY_HEALTH_PASS=${PROXY_HEALTH_PASS:-}
PROXY_HEALTH_HTPASSWD=${PROXY_HEALTH_HTPASSWD:-/etc/nginx/veil-vps-node.htpasswd}

NGINX_SITE_PATH=${NGINX_SITE_PATH:-/etc/nginx/sites-available/veil-vps-node.conf}
NGINX_SITE_LINK=${NGINX_SITE_LINK:-/etc/nginx/sites-enabled/veil-vps-node.conf}
CADDY_CONF_DIR=${CADDY_CONF_DIR:-/etc/caddy/conf.d}
CADDY_SITE_PATH=${CADDY_SITE_PATH:-${CADDY_CONF_DIR}/veil-vps-node.caddy}
CADDYFILE=${CADDYFILE:-/etc/caddy/Caddyfile}

random_string() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -hex 12
    return
  fi
  if [[ -r /dev/urandom ]]; then
    head -c 18 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 16
    return
  fi
  echo "veilhealth$(date +%s)"
}

if [[ "${EUID}" -ne 0 ]]; then
  if command -v sudo >/dev/null 2>&1; then
    exec sudo -E "$0" "$@"
  fi
  echo "This installer must run as root (sudo required)."
  exit 1
fi

if [[ -z "$PROXY_DOMAIN" && "$REVERSE_PROXY" != "none" ]]; then
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
    PROXY_HEALTH_PORT=$(pick_free_port "$PROXY_HEALTH_PORT" 9091 19090)
    echo "Health port in use, switching to ${PROXY_HEALTH_PORT}."
  fi
  local quic_port="5000"
  if [[ -n "${VEIL_VPS_QUIC_BIND:-}" ]]; then
    quic_port="${VEIL_VPS_QUIC_BIND##*:}"
  fi
  if port_in_use "$quic_port"; then
    local new_quic
    new_quic=$(pick_free_port "$quic_port" 5001 15000)
    if [[ "$new_quic" != "$quic_port" ]]; then
      export VEIL_VPS_QUIC_BIND="0.0.0.0:${new_quic}"
      echo "QUIC port in use, switching to ${new_quic}."
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

if [[ ! -f target/release/veil-vps-node ]]; then
  echo "Building veil-vps-node (release)..."
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
if grep -q "^VEIL_VPS_CORE_TAGS=" "$ENV_FILE"; then
  current=$(grep "^VEIL_VPS_CORE_TAGS=" "$ENV_FILE" | tail -n 1 | cut -d= -f2-)
  if [[ -z "$current" ]]; then
    sed -i "s|^VEIL_VPS_CORE_TAGS=.*|VEIL_VPS_CORE_TAGS=${DEFAULT_CORE_TAGS}|" "$ENV_FILE"
  fi
else
  echo "" >> "$ENV_FILE"
  echo "# Default core tags: Public Square, Local Builders, Civic Updates, Open Media" >> "$ENV_FILE"
  echo "VEIL_VPS_CORE_TAGS=${DEFAULT_CORE_TAGS}" >> "$ENV_FILE"
fi

set_env_var() {
  local key="$1"
  local value="$2"
  if grep -q "^${key}=" "$ENV_FILE"; then
    sed -i "s|^${key}=.*|${key}=${value}|" "$ENV_FILE"
  else
    echo "${key}=${value}" >> "$ENV_FILE"
  fi
}

HOSTNAME_FQDN=$(hostname -f 2>/dev/null || hostname)
if [[ -z "$PROXY_HEALTH_PASS" ]]; then
  PROXY_HEALTH_PASS=$(random_string)
fi
set_env_var "VEIL_VPS_STATE_PATH" "${PREFIX}/data/node_state.cbor"
set_env_var "VEIL_VPS_NODE_KEY_PATH" "${PREFIX}/data/node_identity.key"
set_env_var "VEIL_VPS_QUIC_CERT_PATH" "${PREFIX}/data/quic_cert.der"
set_env_var "VEIL_VPS_QUIC_KEY_PATH" "${PREFIX}/data/quic_key.der"
set_env_var "VEIL_VPS_QUIC_BIND" "${VEIL_VPS_QUIC_BIND:-0.0.0.0:5000}"
set_env_var "VEIL_VPS_FAST_PEERS" ""
set_env_var "VEIL_VPS_CORE_TAGS" "${DEFAULT_CORE_TAGS}"
set_env_var "VEIL_VPS_PEER_DB_PATH" "${PREFIX}/data/peers.db"
set_env_var "VEIL_VPS_MAX_DYNAMIC_PEERS" "512"
set_env_var "VEIL_VPS_WS_URL" ""
set_env_var "VEIL_VPS_WS_PEER" "${HOSTNAME_FQDN:-veil-vps}"
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
set_env_var "VEIL_VPS_HEALTH_BIND" "127.0.0.1"
set_env_var "VEIL_VPS_HEALTH_PORT" "${PROXY_HEALTH_PORT}"
set_env_var "PROXY_HEALTH_USER" "${PROXY_HEALTH_USER}"
set_env_var "PROXY_HEALTH_PASS" "${PROXY_HEALTH_PASS}"

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
  local health_auth_snippet=""
  if ! command -v openssl >/dev/null 2>&1; then
    local mgr
    mgr=$(detect_pkg_mgr)
    if [[ "$mgr" != "none" ]]; then
      install_pkgs "$mgr" openssl || true
    fi
  fi
  if command -v openssl >/dev/null 2>&1; then
    install -d -m 0755 "$(dirname "$PROXY_HEALTH_HTPASSWD")"
    local health_hash
    health_hash=$(openssl passwd -apr1 "$PROXY_HEALTH_PASS")
    echo "${PROXY_HEALTH_USER}:${health_hash}" > "$PROXY_HEALTH_HTPASSWD"
    chmod 0640 "$PROXY_HEALTH_HTPASSWD" || true
    health_auth_snippet=$(cat <<EOF
        auth_basic "Veil Health";
        auth_basic_user_file ${PROXY_HEALTH_HTPASSWD};
EOF
)
  else
    echo "openssl not available; skipping health basic auth for nginx."
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
        ${health_auth_snippet}
    }
    location /metrics {
        proxy_pass http://127.0.0.1:${PROXY_HEALTH_PORT};
        proxy_set_header Host $host;
        limit_req zone=veil_health burst=10 nodelay;
        ${health_auth_snippet}
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
  local health_auth_block=""
  if command -v caddy >/dev/null 2>&1; then
    local health_hash
    health_hash=$(caddy hash-password --plaintext "$PROXY_HEALTH_PASS" 2>/dev/null || true)
    if [[ -n "$health_hash" ]]; then
      health_auth_block=$(cat <<EOF
  @veil_health path /health /metrics
  basic_auth @veil_health {
    ${PROXY_HEALTH_USER} ${health_hash}
  }
EOF
)
    fi
  fi
  if [[ -z "$health_auth_block" ]]; then
    echo "Warning: unable to generate Caddy basic auth hash for health endpoints."
  fi
  install -d -m 0755 "$CADDY_CONF_DIR"
  cat <<CADDYCONF > "$CADDY_SITE_PATH"
${PROXY_DOMAIN} {
  root * ${WEB_ROOT}
  file_server
  reverse_proxy /ws/* 127.0.0.1:${PROXY_WS_PORT}
${health_auth_block}
  reverse_proxy /health 127.0.0.1:${PROXY_HEALTH_PORT}
  reverse_proxy /metrics 127.0.0.1:${PROXY_HEALTH_PORT}
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
