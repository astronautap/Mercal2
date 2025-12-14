// src/main.rs

// --- Declaração dos Módulos ---
mod db;
mod error;
mod models;
mod services;
mod state;
mod templates;
mod web;
// mod ws;

// --- Imports ---
use crate::state::AppState;
use axum::serve;
use std::{env, net::SocketAddr};
use time::Duration;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_cookies::{CookieManagerLayer, Key};
use tower_http::trace::TraceLayer;
use tower_sessions::{Expiry, SessionManagerLayer, ExpiredDeletion};
use tower_sessions_sqlx_store::SqliteStore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, fmt};


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // --- Configuração do Logging (Tracing) ---
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                env::var("RUST_LOG")
                    .unwrap_or_else(|_| "merca_simples=debug,tower_http=info,sqlx=warn,tower_sessions=info".into())
                    .into()
            }),
        )
        .with(fmt::layer())
        .init();

    tracing::info!("🚀 Iniciando servidor Merca Simples...");

    // --- Configuração da Base de Dados ---
    let db_pool = match db::create_db_pool().await {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("❌ Falha crítica ao inicializar a base de dados: {}", e);
            return Err(anyhow::anyhow!("Falha ao conectar/migrar DB: {}", e));
        }
    };

    // --- Configuração das Sessões ---
    // SqliteStore::new() já retorna Result, então precisamos extrair o valor
    let session_store = SqliteStore::new(db_pool.clone())
        .with_table_name("sessions")
        .map_err(|e| anyhow::anyhow!("Falha ao criar session store: {}", e))?;

    // Clone o store para a task de limpeza
    let session_store_clone = session_store.clone();
    tokio::spawn(async move {
        // Usa ExpiredDeletion trait através do método continuously_delete_expired
        if let Err(e) = session_store_clone
            .continuously_delete_expired(tokio::time::Duration::from_secs(60 * 60))
            .await
        {
            tracing::error!("Erro na task de limpeza de sessões: {:?}", e);
        }
    });
    tracing::info!("🧹 Tarefa de limpeza de sessões iniciada.");

    let secret_key_string = env::var("SESSION_SECRET")
        .map_err(|e| anyhow::anyhow!("!!! Variável de ambiente SESSION_SECRET não definida: {}", e))?;
    if secret_key_string.len() < 64 {
        tracing::warn!("⚠️ SESSION_SECRET é curta, considere usar uma chave mais longa e aleatória!");
    }
    let key = Key::from(secret_key_string.as_bytes());

    // Cria a camada de sessão
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_http_only(true)
        .with_expiry(Expiry::OnInactivity(Duration::days(1)));

    tracing::info!("🔑 Camada de sessão configurada.");

    // --- Criação do Estado da Aplicação ---
    let app_state = AppState { 
    db_pool,
    presence_state: state::PresenceWsState::default(),
};

    // --- Configuração do Endereço e Listener ---
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("📡 Servidor escutando em http://{}", addr);
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("❌ Falha ao iniciar listener na porta 3000: {}", e);
            return Err(e.into());
        }
    };

    // --- Criação do Router e Aplicação das Camadas (Middlewares) ---
    tracing::info!("🛠️ Construindo router e aplicando middlewares...");
    let app = web::routes::create_router(app_state.clone())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                // CookieManagerLayer::new() não aceita argumentos
                // A Key é configurada separadamente se necessário
                .layer(CookieManagerLayer::new())
                .layer(session_layer)
        );
    tracing::info!("✅ Router e middlewares configurados.");

    // --- Início do Servidor ---
    tracing::info!("👂 Servidor pronto para aceitar conexões...");
    if let Err(e) = serve(listener, app.into_make_service()).await {
        tracing::error!("❌ Erro fatal no servidor: {}", e);
        return Err(e.into());
    }

    Ok(())
}