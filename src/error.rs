// src/error.rs
use axum::{http::StatusCode, response::IntoResponse, response::Html}; // Adicionar Html
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Erro na base de dados: {0}")]
    SqlxError(#[from] sqlx::Error),

    #[error("Erro de migração da base de dados: {0}")]
    SqlxMigrateError(#[from] sqlx::migrate::MigrateError),

    #[error("Erro de variável de ambiente: {0}")]
    EnvVarError(#[from] std::env::VarError),

    // *** ADICIONADO: Erro para falhas no hash/verificação ***
    #[error("Erro ao processar password")]
    PasswordHashingError,

    // *** ADICIONADO: Erro para credenciais inválidas ***
    #[error("Credenciais inválidas")]
    InvalidCredentials,

    // *** ADICIONADO: Erro para falhas de sessão ***
    #[error("Erro na sessão: {0}")]
    SessionError(String),

    #[error("Erro interno inesperado")]
    InternalServerError,

    #[error("Não autorizado")]
    Unauthorized,
}

// Como converter AppError numa resposta HTTP
impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        // Loga o erro detalhado no servidor
        tracing::error!("Erro processado: {:?}", self);

        let (status, user_message) = match self {
            AppError::SqlxError(_) | AppError::SqlxMigrateError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Erro ao aceder aos dados.")
            }
            AppError::EnvVarError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Erro de configuração.")
            }
            // *** ADICIONADO: Mensagem para erro de hash ***
            AppError::PasswordHashingError => {
                 (StatusCode::INTERNAL_SERVER_ERROR, "Erro ao processar credenciais.")
            }
             // *** ADICIONADO: Mensagem para credenciais inválidas (seguro) ***
            AppError::InvalidCredentials => {
                (StatusCode::UNAUTHORIZED, "ID ou senha inválidos.") // Mensagem genérica
            }
             // *** ADICIONADO: Mensagem para erro de sessão ***
            AppError::SessionError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Erro na gestão da sua sessão.")
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Ocorreu um erro inesperado."),
        };

        // Retorna uma página HTML simples (ou poderia usar um template Askama de erro)
         (status, Html(format!(r#"
            <!DOCTYPE html><html><head><title>Erro</title><style>body{{font-family:sans-serif;}}</style></head>
            <body><h1>Erro {status_code}</h1><p>{message}</p><a href="javascript:history.back()">Voltar</a></body></html>
         "#, status_code=status.as_u16(), message=user_message))).into_response()
    }
}

// Tipo Result padrão para a aplicação
pub type AppResult<T = ()> = Result<T, AppError>;