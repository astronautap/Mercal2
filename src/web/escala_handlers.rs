// src/web/escala_handlers.rs
use axum::{
    extract::{Json, Path, State}, http::StatusCode, response::{Html, IntoResponse, Redirect}
};
use crate::{
    state::AppState,
    services::escala_service,
    models::escala::{PedidoTrocaPayload, GerarPeriodoRequest, PublicarRequest},
    templates::{EscalaTemplate, EscalaDiaView, AlocacaoExibicao, AdminEscalaPage},
};
use tower_sessions::Session;
use chrono::Datelike;
use std::collections::BTreeMap;
use askama::Template;

// --- HANDLER DA PÁGINA PRINCIPAL (GET /escala/) ---
pub async fn handle_pagina_escala(
    State(state): State<AppState>,
    session: Session,
) -> impl IntoResponse {
    let user_atual_id = session.get::<String>("user_id")
        .await.ok().flatten().unwrap_or_default();
    
    // 1. Verificar se é Admin
    let is_admin = if !user_atual_id.is_empty() {
        sqlx::query_scalar!(
            "SELECT COUNT(*) FROM user_roles WHERE user_id = ? AND role = 'admin'", 
            user_atual_id
        )
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0) > 0
    } else { 
        false 
    };

    // 2. Buscar dados da BD
    let hoje = chrono::Local::now().date_naive();
    
    // NOTA: A sintaxe 'as "nome?"' força o SQLx a tratar o campo como Option<String>
    // Isso é crucial para LEFT JOINs onde os dados podem não existir.
    let rows = sqlx::query!(
        r#"
        SELECT 
            e.data, 
            e.tipo_rotina, 
            e.status,
            a.id as "aloc_id?", 
            a.user_id as "user_id?", 
            u.name as "militar?", 
            p.nome as "posto?", 
            u.turma as "turma?", 
            a.is_punicao as "is_punicao?"
        FROM escalas e
        LEFT JOIN alocacoes a ON e.data = a.data
        LEFT JOIN users u ON a.user_id = u.id
        LEFT JOIN postos p ON a.posto_id = p.id
        WHERE e.data >= ? 
        ORDER BY e.data ASC, p.peso DESC, p.nome ASC
        "#,
        hoje
    ).fetch_all(&state.db_pool).await.unwrap_or_default();

    // 3. Processar e Agrupar
    let mut dias_map: BTreeMap<String, EscalaDiaView> = BTreeMap::new();

    for row in rows {
        // e.data, e.status, e.tipo_rotina são da tabela principal (não Option)
        let data_key = row.data.clone().unwrap_or_else(|| hoje.to_string());
        let entry = dias_map.entry(data_key.clone()).or_insert_with(|| {
            let d = chrono::NaiveDate::parse_from_str(&data_key, "%Y-%m-%d").unwrap_or(hoje);
            
            let dia_semana = match d.weekday() {
                chrono::Weekday::Mon => "Segunda", 
                chrono::Weekday::Tue => "Terça",
                chrono::Weekday::Wed => "Quarta", 
                chrono::Weekday::Thu => "Quinta",
                chrono::Weekday::Fri => "Sexta", 
                chrono::Weekday::Sat => "Sábado",
                chrono::Weekday::Sun => "Domingo",
            };
            
            // garantir que temos Strings (fornecer valores padrão se forem Option)
            let status = row.status.clone().unwrap_or_else(|| "Rascunho".to_string());
            let tipo = row.tipo_rotina.clone();

            EscalaDiaView {
                data: data_key.clone(),
                data_formatada: format!("{}, {}", dia_semana, d.format("%d/%m")),
                tipo,
                status,
                alocacoes: Vec::new(),
            }
        });

        // Adicionar alocação se existir (LEFT JOIN não nulo)
        if let Some(aloc_id) = row.aloc_id {
            let u_id = row.user_id.unwrap_or_default();
            entry.alocacoes.push(AlocacaoExibicao {
                alocacao_id: aloc_id,
                user_id: u_id.clone(),
                posto: row.posto.unwrap_or("Indefinido".to_string()),
                militar: row.militar.unwrap_or("Sem Nome".to_string()),
                turma: row.turma.unwrap_or_default(),
                is_punicao: row.is_punicao.unwrap_or(false),
                is_meu: u_id == user_atual_id,
            });
        }
    }

    // 4. Separar em Abas
    let mut dias_publicados = Vec::new();
    let mut dias_rascunho = Vec::new();

    for (_, dia) in dias_map {
        if dia.status == "Publicada" {
            dias_publicados.push(dia);
        } else {
            dias_rascunho.push(dia);
        }
    }

    let template = EscalaTemplate {
        dias_publicados,
        dias_rascunho,
        is_admin,
        user_atual_id,
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR, 
            format!("Erro ao renderizar template: {}", e)
        ).into_response()
    }
}

// --- HANDLERS DA API ---

pub async fn handle_gerar_periodo(
    State(state): State<AppState>,
    Json(payload): Json<GerarPeriodoRequest>,
) -> impl IntoResponse {
    match escala_service::gerar_escala_periodo(&state.db_pool, &payload.data_inicio, &payload.data_fim).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

pub async fn handle_publicar_periodo(
    State(state): State<AppState>,
    Json(payload): Json<PublicarRequest>,
) -> impl IntoResponse {
    match escala_service::publicar_escala(&state.db_pool, &payload.data_inicio, &payload.data_fim).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

pub async fn handle_solicitar_troca(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<PedidoTrocaPayload>,
) -> impl IntoResponse {
    let user_id = match session.get::<String>("user_id").await {
        Ok(Some(id)) => id,
        _ => return (StatusCode::UNAUTHORIZED, "Login necessário").into_response(),
    };
    match escala_service::solicitar_troca(&state.db_pool, &user_id, &payload.alocacao_id, &payload.substituto_id, &payload.motivo).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

pub async fn handle_aprovar_troca(
    State(state): State<AppState>,
    Path(troca_id): Path<String>,
) -> impl IntoResponse {
    match escala_service::aprovar_troca(&state.db_pool, &troca_id).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

pub async fn handle_errata(
    State(state): State<AppState>,
    Path(data): Path<String>,
) -> impl IntoResponse {
    match escala_service::errata_dia(&state.db_pool, &data).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

pub async fn handle_admin_escala_page(
    State(state): State<AppState>,
    session: Session,
) -> impl IntoResponse {
    let user_id = match session.get::<String>("user_id").await {
        Ok(Some(id)) => id,
        _ => return Redirect::to("/").into_response(), // Redireciona se não logado
    };

    // Verificar permissões (Admin ou Escalante)
    let tem_permissao = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM user_roles 
           WHERE user_id = ? AND role IN ('admin', 'escalante')"#, 
        user_id
    )
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0) > 0;

    if !tem_permissao {
        return (StatusCode::FORBIDDEN, "Acesso restrito ao Escalante.").into_response();
    }

    // Buscar nome para exibir
    let user_name = sqlx::query_scalar!("SELECT name FROM users WHERE id = ?", user_id)
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or("Admin".to_string());

    let template = AdminEscalaPage {
        user_name,
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}