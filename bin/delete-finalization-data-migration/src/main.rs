use eyre::Result;
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<()> {
    let pgpool = PgPool::connect(&dotenvy::var("DATABASE_URL")?).await?;

    storage::delete_finalization_data_content(&pgpool, 1000).await?;

    Ok(())
}
