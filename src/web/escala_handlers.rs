// src/web/escala_handlers.rs
use axum::{
    extract::{State, Json, Path, Query}, // <--- ADICIONADO: Query
    response::{IntoResponse, Html},      // <--- ADICIONADO: Html
    http::StatusCode,
};
use crate::{
    state::AppState,
    services::escala_service::{self, TipoRotina},
    models::escala::PedidoTrocaPayload,
};
use serde::Deserialize;
use askama::Template; // Importar o Trait Template

// --- Estruturas de Entrada (Payloads) ---

#[derive(Deserialize)]
pub struct GerarEscalaRequest {
    pub data: String, // YYYY-MM-DD
    pub tipo: String, // "RN" ou "RD"
}

#[derive(Deserialize)]
pub struct PageQuery {
    pub data: Option<String>,
    pub tipo: Option<String>,
}

// --- Estruturas para o Template ---

// 1. Struct para representar uma linha da tabela na interface
#[derive(Debug, serde::Serialize)]
pub struct AlocacaoExibicao {
    pub posto: String,
    pub militar: String,
    pub turma: String,
    pub is_punicao: bool,
}

// 2. Struct do Template (O que o HTML vai receber)
#[derive(Template)]
#[template(path = "escala.html")] 
pub struct EscalaTemplate {
    pub data_selecionada: String,
    pub tipo_selecionado: String,
    pub alocacoes: Vec<AlocacaoExibicao>,
    pub mensagem_erro: Option<String>,
    pub mensagem_sucesso: Option<String>,
}

// --- HANDLERS ---

/// GET /escala/ (Página Principal)
pub async fn handle_pagina_escala(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>, // <--- Agora funciona
) -> impl IntoResponse {
    
    let data = q.data.unwrap_or_default();
    let tipo = q.tipo.unwrap_or("RN".to_string());
    let mut alocacoes: Vec<AlocacaoExibicao> = vec![];

    // Se houver data, buscamos a escala
    if !data.is_empty() {
        let resultado = sqlx::query!(
            r#"
            SELECT p.nome as posto, u.name as militar, u.turma, a.is_punicao
            FROM alocacoes a
            JOIN users u ON a.user_id = u.id
            JOIN postos p ON a.posto_id = p.id
            WHERE a.data = ?
            ORDER BY p.peso DESC, p.nome ASC
            "#,
            data
        )
        .fetch_all(&state.db_pool)
        .await;

        if let Ok(linhas) = resultado {
            alocacoes = linhas.into_iter().map(|r| AlocacaoExibicao {
                posto: r.posto,
                militar: r.militar,
                turma: r.turma,
                // CORREÇÃO: unwrap_or(false) porque o banco pode retornar NULL/Option
                is_punicao: r.is_punicao.unwrap_or(false), 
            }).collect();
        }
    }

    let template = EscalaTemplate {
        data_selecionada: data,
        tipo_selecionado: tipo,
        alocacoes,
        mensagem_erro: None,
        mensagem_sucesso: None,
    };

    // CORREÇÃO: Renderização manual em vez de usar HtmlTemplate inexistente
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Erro ao renderizar template: {}", e),
        ).into_response()
    }
}

/// POST /escala/gerar
pub async fn handle_gerar_escala(
    State(state): State<AppState>,
    Json(payload): Json<GerarEscalaRequest>,
) -> impl IntoResponse {
    
    let tipo_rotina = match payload.tipo.as_str() {
        "RN" => TipoRotina::RN,
        "RD" => TipoRotina::RD,
        _ => return (StatusCode::BAD_REQUEST, "Tipo inválido. Use 'RN' ou 'RD'.").into_response(),
    };

    match escala_service::gerar_escala_diaria(&state.db_pool, &payload.data, tipo_rotina).await {
        Ok(mensagem) => (StatusCode::OK, mensagem).into_response(),
        Err(erro) => (StatusCode::INTERNAL_SERVER_ERROR, erro).into_response(),
    }
}

/// POST /escala/trocas/{id}/aprovar
pub async fn handle_aprovar_troca(
    State(state): State<AppState>,
    Path(troca_id): Path<String>,
) -> impl IntoResponse {
    
    match escala_service::aprovar_troca(&state.db_pool, &troca_id).await {
        Ok(mensagem) => (StatusCode::OK, mensagem).into_response(),
        Err(erro) => (StatusCode::BAD_REQUEST, erro).into_response(),
    }
}

/// GET /escala/ver (API simples para Debug)
pub async fn handle_ver_escala(
    State(state): State<AppState>,
    Query(query): Query<crate::web::escala_handlers::PageQuery>, // Reutiliza struct
) -> impl IntoResponse {
    let data = query.data.unwrap_or_default();
    
    let resultado = sqlx::query!(
        r#"
        SELECT u.name, p.nome as posto, a.is_punicao
        FROM alocacoes a
        JOIN users u ON a.user_id = u.id
        JOIN postos p ON a.posto_id = p.id
        WHERE a.data = ?
        "#,
        data
    )
    .fetch_all(&state.db_pool)
    .await;

    match resultado {
        Ok(linhas) => {
            let mut resposta = format!("Escala do dia {}:\n", data);
            for r in linhas {
                resposta.push_str(&format!("- {} [{}] (Punição: {})\n", 
                    r.name, r.posto, r.is_punicao.unwrap_or(false)));
            }
            (StatusCode::OK, resposta).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}