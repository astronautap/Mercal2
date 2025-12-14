// src/web/mw_auth.rs
use crate::error::AppError; // Nosso tipo de erro
use axum::{
    extract::Request, // Usar Request em vez de Parts para ter extensões
    middleware::Next, // Para chamar o próximo handler/middleware
    response::{IntoResponse, Response, Redirect}, // Tipos de resposta
};
use tower_sessions::Session; // Para aceder à sessão

// Middleware que verifica se o utilizador está logado
pub async fn require_auth(
    session: Session,                // Extrai a sessão atual
    mut request: Request,            // A requisição original (mutável para adicionar extensões)
    next: Next,                    // O próximo passo
) -> Result<Response, AppError> { // Retorna ou a resposta do 'next' ou um erro

    // Tenta obter o 'user_id' da sessão
    match session.get::<String>("user_id").await {
        Ok(Some(user_id)) => {
            // Utilizador está logado!
            tracing::debug!("Autenticação MW: Utilizador '{}' autenticado. Prosseguindo...", user_id);

            // Opcional: Adiciona o user_id às extensões da requisição
            // para que os handlers protegidos possam aceder facilmente
            request.extensions_mut().insert(UserId(user_id));

            // Chama o próximo middleware ou o handler final e retorna a sua resposta
            Ok(next.run(request).await)
        }
        Ok(None) => {
            // Não há 'user_id' na sessão -> Não está logado
            tracing::debug!("Autenticação MW: Não autenticado (sem user_id). Redirecionando para /login");
            // Retorna um redirecionamento direto para /login
            // (Alternativa: retornar Err(AppError::Unauthorized) e tratar o redirecionamento em IntoResponse)
            Ok(Redirect::to("/login").into_response())
        }
        Err(e) => {
            // Erro ao tentar ler a sessão (ex: problema na DB)
            tracing::error!("Autenticação MW: Erro ao ler sessão: {:?}", e);
            // Retorna um erro interno, que será tratado pelo IntoResponse de AppError
            Err(AppError::SessionError(format!("Erro ao verificar sessão: {}", e)))
        }
    }
}

// Struct simples para guardar o user_id nas extensões da requisição (opcional)
#[derive(Clone, Debug)]
pub struct UserId(pub String);