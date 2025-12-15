// src/web/escala_handlers.rs
use axum::{
    extract::{Json, Path, State}, http::StatusCode, response::{Html, IntoResponse, Redirect}
};
use crate::{
    state::AppState,
    services::escala_service,
    models::escala::{PedidoTrocaPayload, GerarPeriodoRequest, PublicarRequest},
    templates::{EscalaTemplate, EscalaDiaView, AlocacaoExibicao, AdminEscalaPage, UserPunido, TrocaPendenteAdmin},
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

    // Passamos payload.alocacao_substituto_id (que deve ser Option<String> na struct)
    match escala_service::solicitar_troca(
        &state.db_pool, 
        &user_id, 
        &payload.alocacao_id, 
        &payload.substituto_id, 
        payload.alocacao_substituto_id, // <--- Passando o novo campo
        &payload.motivo
    ).await {
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
    // 1. Verificar se há sessão (Login)
    let user_id = match session.get::<String>("user_id").await {
        Ok(Some(id)) => id,
        _ => return Redirect::to("/").into_response(),
    };

    // 2. Verificar Permissão e Buscar Nome (SIMPLIFICAÇÃO: 1 Query Única)
    // Busca o nome APENAS se o usuário tiver a role 'admin' ou 'escalante'.
    // Se não retornar nada, significa que não tem permissão.
    let acesso = sqlx::query!(
        r#"
        SELECT u.name 
        FROM users u
        JOIN user_roles ur ON u.id = ur.user_id
        WHERE u.id = ? AND ur.role IN ('admin', 'escalante')
        LIMIT 1
        "#,
        user_id
    )
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    let user_name = match acesso {
        Some(registro) => registro.name,
        None => return (StatusCode::FORBIDDEN, "Acesso negado. Apenas Escalantes.").into_response(),
    };

    // 3. Buscar Lista de Punidos (Quem deve serviço)
    // Ordenado por quem deve mais.
    let punidos = sqlx::query_as!(
        UserPunido,
        r#"
        SELECT id, name, saldo_punicoes as "saldo!"
        FROM users 
        WHERE saldo_punicoes > 0 
        ORDER BY saldo_punicoes DESC, name ASC
        "#
    )
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    // 4. Buscar Trocas Pendentes de Aprovação
    // JOINs necessários para transformar IDs em Nomes legíveis
    let trocas_rows = sqlx::query!(
        r#"
        SELECT 
            t.id, 
            t.motivo, 
            u1.name as solicitante, 
            u2.name as substituto, 
            e.data, 
            p.nome as posto
        FROM trocas t
        JOIN users u1 ON t.solicitante_id = u1.id
        JOIN users u2 ON t.substituto_id = u2.id
        JOIN alocacoes a ON t.alocacao_id = a.id
        JOIN escalas e ON a.data = e.data
        JOIN postos p ON a.posto_id = p.id
        WHERE t.status = 'AguardandoEscalante'
        ORDER BY e.data ASC
        "#
    )
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    // Converter resultados do banco para a struct do Template
    let trocas_pendentes = trocas_rows.into_iter().map(|row| TrocaPendenteAdmin {
        id: row.id,
        solicitante: row.solicitante,
        substituto: row.substituto,
        data: row.data.unwrap_or_else(|| "".to_string()),
        posto: row.posto,
        motivo: row.motivo.unwrap_or_else(|| "".to_string()),
    }).collect();

    // 5. Renderizar Template
    let template = AdminEscalaPage {
        user_name,
        punidos,
        trocas_pendentes,
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Erro ao renderizar painel: {}", e)).into_response(),
    }
}