#!/usr/bin/env bash
# Verify all services are healthy after deploy
# Exit code 0 = healthy, 1 = unhealthy
# Usage: ./scripts/health-check.sh [max_attempts] [delay_seconds]

set -euo pipefail

MAX_ATTEMPTS="${1:-12}"
DELAY="${2:-5}"
HEALTH_URL="http://localhost:3000/api/health"
WEB_URL="http://localhost:8080/"

echo "--- Health Check: waiting for services (max ${MAX_ATTEMPTS} x ${DELAY}s) ---"

# Phase 1: Wait for server health endpoint
for i in $(seq 1 "$MAX_ATTEMPTS"); do
    if RESPONSE=$(curl -sf "$HEALTH_URL" 2>/dev/null); then
        STATUS=$(echo "$RESPONSE" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)
        VERSION=$(echo "$RESPONSE" | grep -o '"version":"[^"]*"' | cut -d'"' -f4)
        DB_OK=$(echo "$RESPONSE" | grep -o '"db_ok":[a-z]*' | cut -d: -f2)

        if [[ "$STATUS" == "ok" ]] && [[ "$DB_OK" == "true" ]]; then
            echo "Server healthy: version=$VERSION, db_ok=$DB_OK"
            break
        else
            echo "Server degraded: status=$STATUS, db_ok=$DB_OK"
            if [[ "$i" -eq "$MAX_ATTEMPTS" ]]; then
                echo "FAIL: Server never reached healthy state"
                exit 1
            fi
        fi
    else
        echo "Attempt $i/$MAX_ATTEMPTS: server not responding..."
        if [[ "$i" -eq "$MAX_ATTEMPTS" ]]; then
            echo "FAIL: Server health endpoint unreachable"
            exit 1
        fi
    fi
    sleep "$DELAY"
done

# Phase 2: Verify web frontend
if curl -sf "$WEB_URL" > /dev/null 2>&1; then
    echo "Web frontend: OK"
else
    echo "FAIL: Web frontend not responding"
    exit 1
fi

# Phase 3: Verify Docker service states
for SVC in server bot web; do
    STATE=$(docker compose ps "$SVC" --format '{{.State}}' 2>/dev/null || echo "unknown")
    if [[ "$STATE" != "running" ]]; then
        echo "FAIL: $SVC state is '$STATE', expected 'running'"
        exit 1
    fi
    echo "Docker $SVC: running"
done

echo "--- All health checks passed ---"
