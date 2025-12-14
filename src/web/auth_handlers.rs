// src/web/auth_handlers.rs
use crate::{
    error::{AppError, AppResult}, // Usar AppError e AppResult
    models::user::LoginForm,      // Usar LoginForm do models
    services::{auth_service, user_service},     // Usar o serviço de autenticação
    state::AppState,
    templates::LoginPage,
};
use askama::Template; // Trait Template para render()
use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect}, // Usar Html para erros de render
};
use tower_sessions::Session; // Importar Session para gestão de login

// GET /login (como antes, mas verifica sessão e renderiza explicitamente)
pub async fn show_login_form(session: Session) -> impl IntoResponse {
    // Verifica se já existe um 'user_id' na sessão
    if session.get::<String>("user_id").await.ok().flatten().is_some() {
        tracing::debug!("GET /login: Utilizador já logado, redirecionando para /user");
        // Se sim, redireciona para a página do utilizador (será criada)
        return Redirect::to("/user").into_response();
    }

    // Se não está logado, renderiza a página de login
    let template = LoginPage { error: None };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("Falha ao renderizar template de login: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Erro ao carregar a página.",
            )
                .into_response()
        }
    }
}

// POST /login (Lógica de processamento do formulário)
pub async fn handle_login(
    State(state): State<AppState>, // Acesso ao AppState (db_pool)
    session: Session,              // Acesso à sessão
    Form(form): Form<LoginForm>,   // Dados do formulário (id, password)
) -> AppResult<impl IntoResponse> { // Retorna AppResult com Redirect ou LoginPage com erro

    tracing::info!("Tentativa de login para ID: {}", form.id);

    // 1. Tenta encontrar o utilizador na base de dados pelo ID (username)
    match user_service::find_user_by_id(&state.db_pool, &form.id).await {
        Ok(Some(user)) => { // Utilizador encontrado
            tracing::debug!("Utilizador {} encontrado, verificando senha...", form.id);
            // 2. Verifica se a senha fornecida corresponde ao hash guardado
            match auth_service::verify_password(&form.password, &user.password_hash).await {
                Ok(true) => { // Senha correta
                    // 3. Autentica a sessão
                    session.cycle_id().await // Gera novo ID de sessão (segurança)
                        .map_err(|e| AppError::SessionError(format!("Falha ao rodar ID: {}", e)))?;
                    session.insert("user_id", &user.id).await // Guarda o ID na sessão
                        .map_err(|e| AppError::SessionError(format!("Falha ao inserir na sessão: {}", e)))?;

                    tracing::info!("✅ Login bem-sucedido para: {}", user.id);
                    // 4. Redireciona para a página do utilizador
                    Ok(Redirect::to("/user").into_response()) // Ok com Redirect
                }
                Ok(false) => { // Senha incorreta
                    tracing::warn!("Senha incorreta para ID: {}", form.id);
                    // Renderiza novamente a página de login com mensagem de erro
                    let template = LoginPage { error: Some("ID ou senha inválidos.".to_string()) };
                    match template.render() {
                        Ok(html) => Ok(Html(html).into_response()), // Ok com LoginPage + erro
                        Err(e) => { // Erro ao renderizar a própria página de erro
                            tracing::error!("Falha ao renderizar template de login com erro: {}", e);
                            Err(AppError::InternalServerError) // Retorna erro interno
                        }
                    }
                }
                Err(e) => { // Erro ao verificar a senha (ex: hash inválido, erro bcrypt)
                    tracing::error!("Erro ao verificar senha para {}: {:?}", form.id, e);
                    Err(e) // Propaga o AppError (PasswordHashingError ou InternalServerError)
                }
            }
        }
        Ok(None) => { // Utilizador não encontrado
            tracing::warn!("Utilizador não encontrado: {}", form.id);
            // Renderiza novamente a página de login com mensagem de erro genérica
            let template = LoginPage { error: Some("ID ou senha inválidos.".to_string()) };
             match template.render() {
                Ok(html) => Ok(Html(html).into_response()), // Ok com LoginPage + erro
                Err(e) => {
                    tracing::error!("Falha ao renderizar template de login com erro: {}", e);
                    Err(AppError::InternalServerError)
                }
            }
        }
        Err(e) => { // Erro ao buscar utilizador na DB
            tracing::error!("Erro ao buscar utilizador {}: {:?}", form.id, e);
            Err(e) // Propaga o AppError (SqlxError ou outro)
        }
    }
}

// GET /logout
pub async fn handle_logout(session: Session) -> AppResult<Redirect> { // Retorna AppResult<Redirect>
    let user_id: Option<String> = session.get("user_id").await.ok().flatten();

    // Apaga todos os dados da sessão atual
    session.delete().await
        .map_err(|e| AppError::SessionError(format!("Falha ao apagar sessão: {}", e)))?;

    if let Some(id) = user_id {
        tracing::info!("🚪 Utilizador '{}' desligado.", id);
    } else {
        tracing::info!("🚪 Sessão anónima desligada.");
    }

    // Redireciona para a página de login
    Ok(Redirect::to("/login"))
}