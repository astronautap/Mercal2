// src/web/user_handlers.rs
use crate::state::AppState;
// Importar Template é obrigatório para usar .render()
use askama::Template; 
use crate::templates::{UserPage, MeuServico, NotificacaoTroca};
use crate::services::escala_service;
use axum::{
    extract::{State, Form},
    response::{Html, IntoResponse, Redirect},
};
use tower_sessions::Session;
use chrono::{Datelike, Local};
use serde::Deserialize;

// Helper para traduzir dias
fn weekday_to_pt(wd: chrono::Weekday) -> &'static str {
    match wd {
        chrono::Weekday::Mon => "Segunda", chrono::Weekday::Tue => "Terça",
        chrono::Weekday::Wed => "Quarta", chrono::Weekday::Thu => "Quinta",
        chrono::Weekday::Fri => "Sexta", chrono::Weekday::Sat => "Sábado",
        chrono::Weekday::Sun => "Domingo",
    }
}
fn month_to_pt(m: u32) -> &'static str {
    match m {
        1 => "Jan", 2 => "Fev", 3 => "Mar", 4 => "Abr", 5 => "Mai", 6 => "Jun",
        7 => "Jul", 8 => "Ago", 9 => "Set", 10 => "Out", 11 => "Nov", 12 => "Dez", _ => ""
    }
}

// Payload do formulário de resposta
#[derive(Deserialize)]
pub struct RespostaTrocaForm {
    pub troca_id: String,
    pub acao: String, // "aceitar" | "recusar"
}

// --- HANDLER DASHBOARD ---
pub async fn user_page_handler(
    State(state): State<AppState>,
    session: Session,
) -> impl IntoResponse {
    let user_id = session.get::<String>("user_id").await.unwrap().unwrap_or_default();
    
    // 1. Dados do Utilizador
    let user = sqlx::query!("SELECT name FROM users WHERE id = ?", user_id)
        .fetch_one(&state.db_pool).await.unwrap();

    // 2. Meus Serviços Futuros
    let hoje = Local::now().date_naive();
    let servicos_db = sqlx::query!(
        r#"
        SELECT a.data, p.nome as posto 
        FROM alocacoes a
        JOIN postos p ON a.posto_id = p.id
        WHERE a.user_id = ? AND a.data >= ?
        ORDER BY a.data ASC LIMIT 5
        "#,
        user_id, hoje
    ).fetch_all(&state.db_pool).await.unwrap_or_default();

    let meus_servicos = servicos_db.into_iter().map(|s| {
        let d = chrono::NaiveDate::parse_from_str(&s.data, "%Y-%m-%d").unwrap_or(hoje);
        MeuServico {
            data: s.data,
            dia_semana: weekday_to_pt(d.weekday()).to_string(),
            dia_mes: d.format("%d").to_string(),
            mes_extenso: month_to_pt(d.month()).to_string(),
            posto: s.posto,
        }
    }).collect();

    // 3. Trocas Pendentes (Onde EU sou o substituto)
    let trocas_db = sqlx::query!(
        r#"
        SELECT t.id, t.motivo, u.name as solicitante, p.nome as posto, a.data
        FROM trocas t
        JOIN users u ON t.solicitante_id = u.id
        JOIN alocacoes a ON t.alocacao_id = a.id
        JOIN postos p ON a.posto_id = p.id
        WHERE t.substituto_id = ? AND t.status = 'Pendente'
        ORDER BY t.criado_em DESC
        "#,
        user_id
    ).fetch_all(&state.db_pool).await.unwrap_or_default();

    let trocas_pendentes = trocas_db.into_iter().map(|t| {
        let d = chrono::NaiveDate::parse_from_str(&t.data, "%Y-%m-%d").unwrap_or(hoje);
        NotificacaoTroca {
            troca_id: t.id,
            solicitante: t.solicitante,
            data: d.format("%d/%m").to_string(),
            posto: t.posto,
            motivo: t.motivo.unwrap_or_default(),
        }
    }).collect();

    // Instancia a struct definida em templates.rs
    let template = UserPage {
        user_id,
        name: user.name, // Campo correto (não é user_name)
        meus_servicos,
        trocas_pendentes, // Campo correto
    };
    
    // Renderiza
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR, 
            format!("Erro ao renderizar template: {}", e)
        ).into_response()
    }
}

// --- HANDLER POST: RESPONDER TROCA ---
pub async fn handle_responder_troca(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<RespostaTrocaForm>,
) -> impl IntoResponse {
    let user_id = match session.get::<String>("user_id").await {
        Ok(Some(id)) => id,
        _ => return Redirect::to("/").into_response(),
    };

    let _ = escala_service::responder_troca_usuario(&state.db_pool, &form.troca_id, &user_id, &form.acao).await;
    
    Redirect::to("/user").into_response()
}