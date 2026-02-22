#!/usr/bin/env bash
# Idempotent VPS provisioning script
# Usage: ./scripts/setup-vps.sh <domain>
# Can be run from GitHub Actions or directly on the VPS

set -euo pipefail

DOMAIN="${1:?Usage: setup-vps.sh <domain>}"
PROJECT_DIR="/opt/bimbo-lashes"
REPO_URL="https://github.com/anarchitektura/bimbo-lashes.git"

echo "=== Bimbo Lashes VPS Setup ==="
echo "Domain: $DOMAIN"
echo ""

# 1. System packages
echo "--- Step 1: System packages ---"
apt-get update -qq
apt-get upgrade -y -qq
apt-get install -y -qq curl git ufw sqlite3

# 2. Docker
echo "--- Step 2: Docker ---"
if ! command -v docker &> /dev/null; then
    echo "Installing Docker..."
    curl -fsSL https://get.docker.com | sh
    systemctl enable docker
    systemctl start docker
else
    echo "Docker already installed: $(docker --version)"
fi

if ! docker compose version &> /dev/null; then
    echo "Installing docker compose plugin..."
    apt-get install -y -qq docker-compose-plugin
else
    echo "Docker Compose: $(docker compose version)"
fi

# 3. Firewall
echo "--- Step 3: Firewall ---"
ufw --force reset > /dev/null 2>&1
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp
ufw allow 80/tcp
ufw allow 443/tcp
ufw --force enable
echo "UFW configured"

# 4. Swap (2GB)
echo "--- Step 4: Swap ---"
if [[ ! -f /swapfile ]]; then
    echo "Creating 2GB swap..."
    fallocate -l 2G /swapfile
    chmod 600 /swapfile
    mkswap /swapfile
    swapon /swapfile
    echo '/swapfile none swap sw 0 0' >> /etc/fstab
    sysctl vm.swappiness=10
    echo 'vm.swappiness=10' >> /etc/sysctl.conf
else
    echo "Swap already configured: $(free -h | grep Swap | awk '{print $2}')"
fi

# 5. Caddy
echo "--- Step 5: Caddy ---"
if ! command -v caddy &> /dev/null; then
    echo "Installing Caddy..."
    apt-get install -y -qq debian-keyring debian-archive-keyring apt-transport-https
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | \
        gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | \
        tee /etc/apt/sources.list.d/caddy-stable.list
    apt-get update -qq
    apt-get install -y -qq caddy
else
    echo "Caddy already installed: $(caddy version)"
fi

# 6. Project directory
echo "--- Step 6: Project ---"
mkdir -p "$PROJECT_DIR"
mkdir -p "$PROJECT_DIR/backups"

if [[ -d "$PROJECT_DIR/.git" ]]; then
    echo "Repo exists, pulling latest..."
    cd "$PROJECT_DIR"
    git pull origin main
else
    echo "Cloning repo..."
    git clone "$REPO_URL" "$PROJECT_DIR"
    cd "$PROJECT_DIR"
fi

# 7. Caddyfile
echo "--- Step 7: Caddyfile ---"
mkdir -p /var/log/caddy
sed "s/{domain}/$DOMAIN/g" "$PROJECT_DIR/infra/Caddyfile" > /etc/caddy/Caddyfile
systemctl reload caddy || systemctl restart caddy
echo "Caddy configured for $DOMAIN"

# 8. Environment file
echo "--- Step 8: Environment ---"
if [[ ! -f "$PROJECT_DIR/.env" ]]; then
    echo "WARNING: .env not found! Copying template..."
    cp "$PROJECT_DIR/.env.example" "$PROJECT_DIR/.env"
    echo "EDIT $PROJECT_DIR/.env before starting services!"
else
    echo ".env exists"
fi

# 9. Scripts
echo "--- Step 9: Scripts ---"
chmod +x "$PROJECT_DIR/scripts/"*.sh

# 10. Start services
echo "--- Step 10: Starting services ---"
cd "$PROJECT_DIR"
docker compose up -d --build

if "$PROJECT_DIR/scripts/health-check.sh" 20 5; then
    echo ""
    echo "=== VPS Setup Complete ==="
    echo "URL: https://$DOMAIN"
    docker compose ps
else
    echo ""
    echo "=== WARNING: Health check failed ==="
    echo "Check logs: docker compose logs"
fi
