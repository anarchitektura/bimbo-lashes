#!/usr/bin/env bash
# Send a message to the admin via Telegram Bot API
# Usage: ./scripts/notify.sh "message text"
# Requires: BOT_TOKEN, ADMIN_TG_ID environment variables (or .env file)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Load .env if variables not already set
if [[ -z "${BOT_TOKEN:-}" ]] && [[ -f "$PROJECT_DIR/.env" ]]; then
    set -a
    source "$PROJECT_DIR/.env"
    set +a
fi

if [[ -z "${BOT_TOKEN:-}" ]] || [[ -z "${ADMIN_TG_ID:-}" ]]; then
    echo "ERROR: BOT_TOKEN and ADMIN_TG_ID must be set"
    exit 1
fi

MESSAGE="${1:?Usage: notify.sh \"message\"}"

curl -sf -X POST "https://api.telegram.org/bot${BOT_TOKEN}/sendMessage" \
    -H "Content-Type: application/json" \
    -d "{\"chat_id\": ${ADMIN_TG_ID}, \"text\": \"${MESSAGE}\", \"parse_mode\": \"HTML\"}" \
    > /dev/null 2>&1 || echo "WARNING: Failed to send Telegram notification"
