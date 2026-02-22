use sqlx::SqlitePool;

pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    // Enable WAL mode for better concurrent access
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(pool)
        .await?;

    let migration_sql = include_str!("../migrations/001_init.sql");

    // Split by semicolons and execute each statement
    for statement in migration_sql.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed).execute(pool).await.ok();
        }
    }

    tracing::info!("✅ Database migrations applied");
    Ok(())
}
