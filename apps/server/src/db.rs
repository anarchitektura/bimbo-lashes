use sqlx::SqlitePool;

pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    // Enable WAL mode for better concurrent access
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(pool)
        .await?;

    // Create migrations tracking table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _migrations (
            name TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await?;

    // Run 001_init only if not already applied
    let applied: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM _migrations WHERE name = '001_init'"
    )
    .fetch_one(pool)
    .await?;

    if !applied {
        let migration_sql = include_str!("../migrations/001_init.sql");
        for statement in migration_sql.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(pool).await.ok();
            }
        }
        sqlx::query("INSERT INTO _migrations (name) VALUES ('001_init')")
            .execute(pool)
            .await?;
        tracing::info!("Applied migration: 001_init");
    }

    // One-time fix: remove duplicate services (keep lowest ID per name)
    let dedup_applied: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM _migrations WHERE name = '002_dedup_services'"
    )
    .fetch_one(pool)
    .await?;

    if !dedup_applied {
        sqlx::query(
            "DELETE FROM services WHERE id NOT IN (
                SELECT MIN(id) FROM services GROUP BY name
            )"
        )
        .execute(pool)
        .await
        .ok();
        sqlx::query("INSERT INTO _migrations (name) VALUES ('002_dedup_services')")
            .execute(pool)
            .await?;
        tracing::info!("Applied migration: 002_dedup_services (removed duplicate services)");
    }

    // 003: Replace services with new catalog
    let catalog_applied: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM _migrations WHERE name = '003_new_catalog'"
    )
    .fetch_one(pool)
    .await?;

    if !catalog_applied {
        // Deactivate all old services
        sqlx::query("UPDATE services SET is_active = 0").execute(pool).await.ok();

        // Insert new catalog
        sqlx::query(
            "INSERT INTO services (name, description, price, duration_min, sort_order, is_active) VALUES
                ('Наращивание ресниц', 'Любой объём', 2500, 120, 1, 1),
                ('Наращивание нижних', 'Наращивание только нижних ресниц', 500, 20, 2, 1),
                ('Коррекция', 'Коррекция наращивания', 1500, 60, 3, 1)"
        )
        .execute(pool)
        .await
        .ok();

        sqlx::query("INSERT INTO _migrations (name) VALUES ('003_new_catalog')")
            .execute(pool)
            .await?;
        tracing::info!("Applied migration: 003_new_catalog");
    }

    tracing::info!("Database migrations up to date");
    Ok(())
}
