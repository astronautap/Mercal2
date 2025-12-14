// src/web/mw_presence.rs
use crate::{
    error::AppError,
    // *** CORRIGIDO: Usar user_service diretamente ***
    services::user_service, // Para chamar check_user_role_any
    state::AppState,
    web::mw_auth::UserId,   // Para obter user_id das extensões
};
use axum::{
    extract::{Extension, Request, State}, // Usar Request e State
    middleware::Next,
    response::Response, // Retornar Response ou AppError
};

pub const ROLES_QUE_ACEDEM_PRESENCA: &[&str] = &["admin", "policia", "chefe_de_dia"];

/// Middleware que verifica se o utilizador logado tem permissão para aceder à Presença.
/// Deve ser executado *depois* do middleware `require_auth`.
pub async fn require_presence_access(
    State(state): State<AppState>,           // Obtém o AppState (com db_pool)
    Extension(user_id_ext): Extension<UserId>, // Obtém UserId posto por require_auth
    request: Request,                      // A requisição (sem genérico <B>)
    next: Next,                            // O próximo passo
) -> Result<Response, AppError> { // Retorna Response ou AppError

    let user_id = user_id_ext.0; // Extrai o ID
    tracing::debug!("Presence MW: Verificando acesso para {}", user_id);

    // Define as roles que permitem acesso ao módulo de presença
    let required_roles = ROLES_QUE_ACEDEM_PRESENCA; // Ajuste conforme necessário

    // Chama a função centralizada para verificar se o user tem alguma destas roles (permanente ou temporária ativa)
    match user_service::check_user_role_any(&state.db_pool, &user_id, &required_roles).await {
        Ok(true) => {
            // Permissão concedida
            tracing::debug!("Presence MW: Acesso concedido para {}", user_id);
            // Continua para o próximo middleware ou handler
            Ok(next.run(request).await)
        }
        Ok(false) => {
            // Permissão negada
            tracing::warn!("Presence MW: Acesso negado para {} (sem roles requeridas: {:?}).", user_id, required_roles);
            // Retorna erro Unauthorized (que o error.rs trata como redirect/forbidden)
            Err(AppError::Unauthorized)
        }
        Err(e) => {
            // Erro ao consultar a base de dados
            tracing::error!("Presence MW: Erro ao verificar roles para {}: {:?}", user_id, e);
            // Propaga o erro (geralmente resulta em 500 Internal Server Error)
            Err(e)
        }
    }
}