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

    // 004: Delete old inactive services (no booking history)
    let cleanup_applied: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM _migrations WHERE name = '004_delete_old_services'"
    )
    .fetch_one(pool)
    .await?;

    if !cleanup_applied {
        sqlx::query("DELETE FROM services WHERE is_active = 0")
            .execute(pool)
            .await
            .ok();
        sqlx::query("INSERT INTO _migrations (name) VALUES ('004_delete_old_services')")
            .execute(pool)
            .await?;
        tracing::info!("Applied migration: 004_delete_old_services");
    }

    // 005: Smart slots — 1-hour base slots, multi-slot bookings, addon support
    let smart_applied: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM _migrations WHERE name = '005_smart_slots'"
    )
    .fetch_one(pool)
    .await?;

    if !smart_applied {
        // Add booking_id to available_slots (multi-slot booking tracking)
        sqlx::query("ALTER TABLE available_slots ADD COLUMN booking_id INTEGER")
            .execute(pool).await.ok();

        // Store date/time directly on bookings (no more JOIN dependency)
        sqlx::query("ALTER TABLE bookings ADD COLUMN date TEXT")
            .execute(pool).await.ok();
        sqlx::query("ALTER TABLE bookings ADD COLUMN start_time TEXT")
            .execute(pool).await.ok();
        sqlx::query("ALTER TABLE bookings ADD COLUMN end_time TEXT")
            .execute(pool).await.ok();
        sqlx::query("ALTER TABLE bookings ADD COLUMN with_lower_lashes INTEGER NOT NULL DEFAULT 0")
            .execute(pool).await.ok();

        // Service type: 'main' (bookable) vs 'addon' (checkbox add-on)
        sqlx::query("ALTER TABLE services ADD COLUMN service_type TEXT NOT NULL DEFAULT 'main'")
            .execute(pool).await.ok();
        sqlx::query("UPDATE services SET service_type = 'addon' WHERE name LIKE '%нижних%'")
            .execute(pool).await.ok();

        // Clear old slots (no booking history exists)
        sqlx::query("DELETE FROM available_slots")
            .execute(pool).await.ok();

        sqlx::query("INSERT INTO _migrations (name) VALUES ('005_smart_slots')")
            .execute(pool)
            .await?;
        tracing::info!("Applied migration: 005_smart_slots");
    }

    // 006: Payment support (YooKassa prepayment)
    let payment_applied: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM _migrations WHERE name = '006_payments'"
    )
    .fetch_one(pool)
    .await?;

    if !payment_applied {
        sqlx::query("ALTER TABLE bookings ADD COLUMN payment_status TEXT NOT NULL DEFAULT 'none'")
            .execute(pool).await.ok();
        sqlx::query("ALTER TABLE bookings ADD COLUMN yookassa_payment_id TEXT")
            .execute(pool).await.ok();
        sqlx::query("ALTER TABLE bookings ADD COLUMN prepaid_amount INTEGER NOT NULL DEFAULT 0")
            .execute(pool).await.ok();

        sqlx::query("INSERT INTO _migrations (name) VALUES ('006_payments')")
            .execute(pool)
            .await?;
        tracing::info!("Applied migration: 006_payments");
    }

    // 007: Add performance indexes
    let indexes_applied: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM _migrations WHERE name = '007_indexes'"
    )
    .fetch_one(pool)
    .await?;

    if !indexes_applied {
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_bookings_client_tg_id ON bookings(client_tg_id)")
            .execute(pool).await.ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_bookings_date ON bookings(date)")
            .execute(pool).await.ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_bookings_status ON bookings(status)")
            .execute(pool).await.ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_bookings_payment_status ON bookings(payment_status)")
            .execute(pool).await.ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_slots_date ON available_slots(date)")
            .execute(pool).await.ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_slots_booking_id ON available_slots(booking_id)")
            .execute(pool).await.ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_slots_date_booked ON available_slots(date, is_booked)")
            .execute(pool).await.ok();

        sqlx::query("INSERT INTO _migrations (name) VALUES ('007_indexes')")
            .execute(pool)
            .await?;
        tracing::info!("Applied migration: 007_indexes");
    }

    tracing::info!("Database migrations up to date");
    Ok(())
}
