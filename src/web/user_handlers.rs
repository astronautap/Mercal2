// src/web/user_handlers.rs
use crate::{
    error::{AppError, AppResult}, // Usar AppResult e AppError
    //models::user::User,          // Usar o modelo User (se buscar na DB)
    services::user_service,     // Para buscar dados do user
    state::AppState,
    templates::UserPage,        // A struct do template Askama
    web::mw_auth::UserId,       // Importar UserId das extensões
    web::mw_presence::ROLES_QUE_ACEDEM_PRESENCA,
};
use askama::Template;
use axum::{
    extract::{Extension, State}, // Adicionar Extension
    response::{Html, IntoResponse},
};
// Remover Session daqui, pois o middleware já validou e passou o ID via Extension
// use tower_sessions::Session;

// Handler para GET /user (protegido pelo middleware)
pub async fn user_page_handler(
    State(state): State<AppState>, // Acesso ao pool da DB
    Extension(user_id_ext): Extension<UserId>, // <<< Obtém o UserId da extensão (posto pelo middleware)
) -> AppResult<impl IntoResponse> { // Retorna AppResult com UserPage ou erro

    let user_id = user_id_ext.0; // Extrai o ID da struct UserId
    tracing::debug!("GET /user: Acesso para {}", user_id);

    // Busca os detalhes do utilizador na base de dados
    // (Necessário para obter o nome e outros detalhes)
    let user = user_service::find_user_by_id(&state.db_pool, &user_id)
        .await? // Propaga erro da DB
        .ok_or_else(|| { // Se o user_id (validado pelo middleware) não existir mais na DB (!)
            tracing::error!("CRÍTICO: user_id '{}' autenticado não encontrado na DB!", user_id);
            // Neste caso, talvez forçar logout seria o ideal, mas por agora erro interno.
            AppError::InternalServerError
        })?;


    let roles = user_service::get_user_roles(&state.db_pool, &user_id).await?;
    let is_admin = roles.iter().any(|r| r.eq_ignore_ascii_case("admin"));
    tracing::debug!("User '{}' é admin? {}", user_id, is_admin);

    let presence_roles = ROLES_QUE_ACEDEM_PRESENCA;
    let can_access_presence = user_service::check_user_role_any(&state.db_pool, &user_id, &presence_roles).await?;

    // Cria a struct do template com os dados do utilizador
    let template = UserPage {
        user_id: user.id, // Passa o ID para o template
        user_name: user.name, // Passa o nome para o template
        is_admin,
        can_access_presence,
    };

    // Renderiza o template
    match template.render() {
        Ok(html) => Ok(Html(html).into_response()), // Ok com UserPage renderizada
        Err(e) => {
            tracing::error!("Falha ao renderizar template UserPage: {}", e);
            Err(AppError::InternalServerError) // Erro interno se a renderização falhar
        }
    }
}