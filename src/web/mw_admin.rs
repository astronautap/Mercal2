// src/web/mw_admin.rs
use crate::{
    error::AppError,        // Nosso tipo de erro
    services::user_service, // Para buscar roles
    state::AppState,        // Para aceder ao db_pool
    web::mw_auth::UserId,   // Para obter o user_id das extensões
};
use axum::{
    extract::{Extension, Request, State}, // Usar Request e State
    middleware::Next,                    // Próximo handler
    response::{IntoResponse, Response}, // REMOVER Redirect daqui
};
use tower_sessions::Session; // Para aceder à sessão

/// Middleware que verifica se o utilizador logado tem a role "admin".
/// Deve ser executado *depois* do middleware `require_auth`.
// *** CORRIGIDO: Remover o genérico <B> da assinatura ***
pub async fn require_admin(
    State(state): State<AppState>,           // Obtém o AppState (com db_pool)
    Extension(user_id_ext): Extension<UserId>, // Obtém UserId posto por require_auth
    request: Request,                      // A requisição (sem genérico)
    next: Next,                            // O próximo passo
) -> Result<Response, AppError> { // Retorna Response ou AppError

    let user_id = user_id_ext.0; // Extrai o ID
    tracing::debug!("Admin MW: Verificando role 'admin' para {}", user_id);

    // Busca as roles do utilizador na base de dados
    match user_service::get_user_roles(&state.db_pool, &user_id).await {
        Ok(roles) => {
            // Verifica se a lista de roles contém "admin" (case-insensitive)
            if roles.iter().any(|r| r.eq_ignore_ascii_case("admin")) {
                tracing::debug!("Admin MW: Acesso admin concedido para {}", user_id);
                // Tem a role "admin", continua para o handler final
                Ok(next.run(request).await) // Passa a request (sem genérico)
            } else {
                // Não tem a role "admin"
                tracing::warn!("Admin MW: Acesso negado para {} (sem role admin).", user_id);
                // *** CORRIGIDO: Retorna AppError::Unauthorized (precisa existir em error.rs) ***
                Err(AppError::Unauthorized)
            }
        }
        Err(e) => {
            // Erro ao buscar roles na DB
            tracing::error!("Admin MW: Erro ao buscar roles para {}: {:?}", user_id, e);
            Err(e) // Retorna o erro da DB
        }
    }
}

