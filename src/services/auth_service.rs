// src/services/auth_service.rs
use crate::{
    error::{AppError, AppResult},
    models::user::User, // User agora espera created_at: Option<String>
};
use sqlx::SqlitePool;


// ... (verify_password e hash_password permanecem iguais) ...
/// Verifica se a senha fornecida corresponde ao hash guardado.
pub async fn verify_password(password: &str, stored_hash: &str) -> AppResult<bool> {
    let password = password.to_string();
    let stored_hash = stored_hash.to_string();
    tokio::task::spawn_blocking(move || {
        tracing::debug!("Verificando hash bcrypt...");
        bcrypt::verify(&password, &stored_hash)
    })
    .await
    .map_err(|e| {
        tracing::error!("Erro na task spawn_blocking (verify_password): {:?}", e);
        AppError::InternalServerError
    })?
    .map_err(|e| {
        tracing::error!("Erro bcrypt ao verificar senha: {:?}", e);
        AppError::PasswordHashingError
    })
}

/// Gera um hash bcrypt para uma senha.
pub async fn hash_password(password: &str) -> AppResult<String> {
    let password = password.to_string();
    tokio::task::spawn_blocking(move || {
        tracing::debug!("Gerando hash bcrypt...");
        bcrypt::hash(&password, bcrypt::DEFAULT_COST)
    })
    .await
    .map_err(|e| {
        tracing::error!("Erro na task spawn_blocking (hash_password): {:?}", e);
        AppError::InternalServerError
    })?
    .map_err(|e| {
        tracing::error!("Erro bcrypt ao gerar hash: {:?}", e);
        AppError::PasswordHashingError
    })
}