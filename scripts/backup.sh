#!/usr/bin/env bash
# Backup SQLite database with 7-day rotation
# Uses sqlite3 .backup for safe WAL-mode backup
# Usage: ./scripts/backup.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

BACKUP_DIR="$PROJECT_DIR/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="$BACKUP_DIR/bimbo_${TIMESTAMP}.db"
KEEP_DAYS=7

# Load .env for notifications
if [[ -f "$PROJECT_DIR/.env" ]]; then
    set -a
    source "$PROJECT_DIR/.env"
    set +a
fi

mkdir -p "$BACKUP_DIR"

echo "Starting SQLite backup..."

# Use sqlite3 .backup inside the running container for a safe, consistent backup
if docker compose -f "$PROJECT_DIR/docker-compose.yml" exec -T server test -f /app/data/bimbo.db 2>/dev/null; then
    docker compose -f "$PROJECT_DIR/docker-compose.yml" exec -T server sh -c "
        apt-get update -qq > /dev/null 2>&1 && apt-get install -y -qq sqlite3 > /dev/null 2>&1 || true
        sqlite3 /app/data/bimbo.db '.backup /app/data/bimbo_backup.db'
    "
    docker cp bimbo-server:/app/data/bimbo_backup.db "$BACKUP_FILE"
    docker compose -f "$PROJECT_DIR/docker-compose.yml" exec -T server rm -f /app/data/bimbo_backup.db
else
    echo "ERROR: Cannot find database in server container"
    "$SCRIPT_DIR/notify.sh" "Backup FAILED: cannot find bimbo.db in container"
    exit 1
fi

# Verify backup file exists and is not empty
if [[ -s "$BACKUP_FILE" ]]; then
    SIZE=$(du -h "$BACKUP_FILE" | cut -f1)
    echo "Backup created: $BACKUP_FILE ($SIZE)"
else
    echo "ERROR: Backup file is empty or missing"
    "$SCRIPT_DIR/notify.sh" "Backup FAILED: backup file is empty"
    exit 1
fi

# Rotate: delete backups older than KEEP_DAYS
DELETED=$(find "$BACKUP_DIR" -name "bimbo_*.db" -mtime +$KEEP_DAYS -delete -print | wc -l)
REMAINING=$(find "$BACKUP_DIR" -name "bimbo_*.db" | wc -l)

echo "Rotation: deleted $DELETED old backups, $REMAINING remaining"

"$SCRIPT_DIR/notify.sh" "Backup OK: $SIZE ($REMAINING backups kept)"

echo "Backup complete."
