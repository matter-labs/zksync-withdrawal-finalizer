use eyre::Result;
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<()> {
    let pgpool = PgPool::connect(&dotenvy::var("DATABASE_URL")?).await?;

    storage::delete_db_content(&pgpool).await?;

    Ok(())
}
