#!/usr/bin/env bash
# Rollback to previous Docker images
# Looks for images tagged :previous and swaps them with :latest
# Usage: ./scripts/rollback.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

echo "--- Starting rollback ---"

# Check that :previous images exist
SERVICES=("bimbo-lashes-server" "bimbo-lashes-bot" "bimbo-lashes-web")
for IMG in "${SERVICES[@]}"; do
    if ! docker image inspect "${IMG}:previous" > /dev/null 2>&1; then
        echo "ERROR: No previous image found for $IMG. Cannot rollback."
        "$SCRIPT_DIR/notify.sh" "Rollback FAILED: no previous images found"
        exit 1
    fi
done

# Stop current containers
docker compose down

# Swap tags: current -> failed, previous -> latest
for IMG in "${SERVICES[@]}"; do
    docker tag "${IMG}:latest" "${IMG}:failed" 2>/dev/null || true
    docker tag "${IMG}:previous" "${IMG}:latest"
done

# Bring up with the restored images (no --build, use existing :latest)
docker compose up -d

# Health check
if "$SCRIPT_DIR/health-check.sh" 12 5; then
    echo "Rollback successful"
    "$SCRIPT_DIR/notify.sh" "Rollback completed successfully"
    # Clean up failed images
    for IMG in "${SERVICES[@]}"; do
        docker rmi "${IMG}:failed" 2>/dev/null || true
    done
else
    echo "CRITICAL: Rollback also failed!"
    "$SCRIPT_DIR/notify.sh" "CRITICAL: Rollback ALSO failed! Manual intervention required."
    exit 1
fi
