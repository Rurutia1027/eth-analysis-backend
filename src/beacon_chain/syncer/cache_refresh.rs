use sqlx::PgPool;

pub async fn update_deferrable_analysis(db_pool: &PgPool) -> anyhow::Result<()> {
    // todo : refresh update cache, but now we haven't implement this yet
    Ok(())
}