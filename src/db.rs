// src/db.rs
use crate::error::AppResult;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use std::time::Duration; // Usar std::time::Duration aqui

pub async fn create_db_pool() -> AppResult<SqlitePool> {
    dotenvy::dotenv().ok(); // Carrega .env
    let database_url = std::env::var("DATABASE_URL")?; // Lê URL da DB

    tracing::info!("Ligando à base de dados: {}", database_url);

    // Opções de conexão (criar se não existir, timeout)
    let options = SqliteConnectOptions::from_str(&database_url)?
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(5));

    // Cria o pool (conjunto de conexões reutilizáveis)
    let pool = SqlitePoolOptions::new()
        .max_connections(5) // Número máximo de conexões simultâneas
        .connect_with(options)
        .await?; // Conecta e retorna erro se falhar

    tracing::info!("Executando migrações da base de dados...");
    // Executa automaticamente os ficheiros SQL em ./migrations
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Migrações concluídas.");

    Ok(pool)
}