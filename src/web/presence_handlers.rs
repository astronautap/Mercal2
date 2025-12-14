// src/web/presence_handlers.rs
use crate::{
    error::{AppError, AppResult},
    models::presence::{
        PresencePerson, PresenceSocketAction, PresenceSocketUpdate, PresenceStats,
    }, // Modelos
    models::user::User,          // Para buscar ano do user
    services::{presence_service, user_service}, // Serviços
    state::AppState,            // Estado da aplicação (com PresenceWsState)
    templates::PresencePage,    // Template Askama
    web::mw_auth::UserId,       // Para ID do operador
};
use askama::Template;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade}, // Tipos WebSocket
        Query, State, Extension, // Extratores Axum
    },
    response::{Html, IntoResponse}, // Tipos de Resposta
};
use chrono::{DateTime, Local}; // Para formatar datas
use futures_util::{stream::{SplitSink, SplitStream, StreamExt}, SinkExt}; // Para manipular WS stream
use serde::Deserialize;
use std::sync::Arc; // Para clonar AppState
use tokio::sync::{mpsc, Mutex}; // Para canal WS
use uuid::Uuid; // Para IDs de conexão

// --- Handler HTTP (GET /presence) ---

// Struct para query parameter ?turma=X
#[derive(Deserialize, Debug)]
pub struct PresenceQuery {
    // Usa Option<i64> para default se não fornecido
    turma: Option<i64>,
}

/// Handler para servir a página HTML de controlo de presença.
/// Protegido por `require_auth` (e opcionalmente por roles como "policia").
pub async fn presence_page_handler(
    State(state): State<AppState>, // Obtém AppState
    // Extension(user_id_ext): Extension<UserId>, // Poderia obter UserId do operador
    Query(params): Query<PresenceQuery>, // Obtém "?turma="
) -> AppResult<impl IntoResponse> {
    // Define a turma a ser exibida (default para 1 se não especificado)
    let turma_selecionada = params.turma.unwrap_or(1);
    tracing::debug!("GET /presence: Carregando turma {}", turma_selecionada);

    // Busca a lista de pessoas e o estado de presença para a turma
    let pessoas = presence_service::get_presence_list_for_turma(&state.db_pool, turma_selecionada).await?;

    // Calcula as estatísticas
    let stats = presence_service::calcular_stats(&pessoas);

    // Cria a struct do template Askama
    let template = PresencePage {
        turma_selecionada,
        pessoas: &pessoas, // Passa como slice
        stats: &stats,     // Passa como referência
    };

    // Renderiza o template
    match template.render() {
        Ok(html) => Ok(Html(html).into_response()),
        Err(e) => {
            tracing::error!("Falha ao renderizar template PresencePage: {}", e);
            Err(AppError::InternalServerError)
        }
    }
}


// --- Handlers WebSocket (GET /presence/ws) ---

/// Handler para o upgrade da conexão HTTP para WebSocket.
/// Protegido por `require_auth`.
pub async fn presence_websocket_handler(
    ws: WebSocketUpgrade,          // Extrator para upgrade WS
    State(state): State<AppState>, // AppState (com db_pool e presence_state)
    Extension(user_id_ext): Extension<UserId>, // ID do operador (posto por require_auth)
) -> impl IntoResponse {
    let operator_id = user_id_ext.0; // Obtém o ID
    tracing::info!("Tentativa de upgrade WebSocket para Presença por {}", operator_id);
    // Inicia o processo de upgrade, passando o estado e ID do operador para a função `handle_socket`
    ws.on_upgrade(move |socket| handle_socket(socket, state, operator_id))
}

/// Função que gere uma conexão WebSocket individual.
async fn handle_socket(socket: WebSocket, state: AppState, operator_id: String) {
    let conn_id = Uuid::new_v4(); // Gera ID único para esta conexão
    tracing::info!("🔌 Nova conexão WS Presença: {} (Operador: {})", conn_id, operator_id);

    // Divide o socket em 'sender' (para enviar) e 'receiver' (para receber)
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Cria um canal MPSC (Multiple Producer, Single Consumer)
    // O servidor (múltiplas tasks) pode enviar para 'tx', mas apenas uma task (abaixo) lê de 'rx'
    // e envia para o cliente via 'ws_sender'.
    let (tx, mut rx) = mpsc::channel::<Message>(32); // Buffer de 32 mensagens

    // Guarda o 'sender' (tx) no estado global para que outras tasks possam enviar msgs a este cliente
    state.presence_state.connections.lock().await.insert(conn_id, tx.clone());

    // --- Task 1: Enviar mensagens do canal MPSC para o cliente ---
    let state_clone_send = state.clone(); // Clona state para a task
    let conn_id_send = conn_id;
    let mut send_task = tokio::spawn(async move {
        // Loop enquanto houver mensagens no canal 'rx'
        while let Some(msg) = rx.recv().await {
            // Tenta enviar a mensagem para o cliente via WebSocket
            if ws_sender.send(msg).await.is_err() {
                // Se falhar (ex: cliente desconectou), termina a task
                tracing::warn!("Falha ao enviar msg WS para {}, terminando send_task.", conn_id_send);
                break;
            }
        }
        // Quando o loop termina (canal fechado), remove a conexão do estado
        state_clone_send.presence_state.connections.lock().await.remove(&conn_id_send);
    });


    // --- Task 2: Receber mensagens do cliente e processá-las ---
    let state_clone_recv = state.clone(); // Clona state para a task
    let conn_id_recv = conn_id;
    let operator_id_recv = operator_id.clone(); // Clona ID do operador
    let mut recv_task = tokio::spawn(async move {
        // Busca o nome do operador (para logs e mensagens de broadcast) uma vez
        let operator_name = user_service::find_user_by_id(&state_clone_recv.db_pool, &operator_id_recv)
            .await
            .ok() // Ignora erro de busca, usa ID como fallback
            .flatten() // Option<Option<User>> -> Option<User>
            .map_or(operator_id_recv.clone(), |u| u.name); // Pega nome ou ID

        // Loop enquanto houver mensagens do cliente
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    tracing::debug!("<- WS Presença Recebido de {}: {}", conn_id_recv, text);
                    // Tenta deserializar a ação enviada pelo cliente
                    match serde_json::from_str::<PresenceSocketAction>(&text) {
                        Ok(action) => {
                            // Processa a ação (chama o serviço e prepara broadcast)
                            let update_result = process_presence_action(
                                &state_clone_recv, // Passa AppState
                                &action,           // Ação recebida
                                &operator_name,    // Nome do operador
                            ).await;

                            // Serializa a mensagem de update (sucesso ou erro) para JSON
                            match serde_json::to_string(&update_result) {
                                Ok(broadcast_msg_text) => {
                                    // Envia a atualização para TODOS os clientes conectados
                                    tracing::debug!("-> WS Presença Enviando Broadcast: {}", broadcast_msg_text);
                                    state_clone_recv.presence_state.broadcast(broadcast_msg_text).await;
                                }
                                Err(e) => {
                                    tracing::error!("Erro ao serializar update WS Presença: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Mensagem WS Presença inválida (JSON parse falhou): {}, Erro: {}", text, e);
                            // Opcional: Enviar mensagem de erro de volta apenas para este cliente?
                        }
                    }
                }
                Message::Close(_) => {
                    tracing::info!("Cliente {} enviou Close frame.", conn_id_recv);
                    break; // Sai do loop para fechar a conexão
                }
                // Ignora outras mensagens (Ping, Pong, Binary) por agora
                _ => { tracing::trace!("Ignorando msg WS não-texto de {}", conn_id_recv); }
            }
        }
        // Fim do loop (cliente desconectou ou enviou Close)
    });


    // Espera que uma das tasks termine (ou dê erro)
    // Se uma terminar, aborta a outra para limpar recursos
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    // Garante que a conexão é removida do estado (caso send_task não tenha terminado ainda)
    state.presence_state.connections.lock().await.remove(&conn_id);
    tracing::info!("🔌 Conexão WS Presença {} fechada.", conn_id);
}


/// Função auxiliar para processar uma ação recebida via WebSocket.
async fn process_presence_action(
    state: &AppState,
    action: &PresenceSocketAction,
    operator_name: &str, // Usar nome para mensagens
) -> PresenceSocketUpdate { // Retorna sempre um PresenceSocketUpdate (sucesso ou erro)

    // 1. Tenta executar a ação na base de dados
    let db_result = match action.action.as_str() {
        "saida" => presence_service::marcar_saida(&state.db_pool, &action.user_id, operator_name).await,
        "retorno" => presence_service::marcar_retorno(&state.db_pool, &action.user_id, operator_name).await,
        _ => {
            tracing::warn!("Ação WS Presença desconhecida: {}", action.action);
            // Retorna um erro interno simulado
            Err(AppError::InternalServerError) // Ou um erro mais específico
        }
    };

    // 2. Prepara a mensagem de update base (com ID e stats default)
    let mut update = PresenceSocketUpdate {
        user_id: action.user_id.clone(),
        stats: PresenceStats::default(), // Será preenchido depois
        ..Default::default()
    };

    // 3. Verifica o resultado da DB e busca dados atualizados
    match db_result {
        Ok(_) => { // Ação na DB foi bem-sucedida
            update.success = true;
            // Busca o user afetado para saber a turma (ano)
            match user_service::find_user_by_id(&state.db_pool, &action.user_id).await {
                Ok(Some(user)) => {
                    // Busca a lista atualizada da turma para calcular stats e obter dados formatados
                    match presence_service::get_presence_list_for_turma(&state.db_pool, user.ano).await {
                        Ok(pessoas_turma) => {
                            // Calcula stats atualizadas
                            update.stats = presence_service::calcular_stats(&pessoas_turma);
                            // Encontra os dados atualizados da pessoa específica
                            if let Some(pessoa_atualizada) = pessoas_turma.iter().find(|p| p.id == action.user_id) {
                                update.esta_fora = pessoa_atualizada.esta_fora;
                                // Formata as infos de data/hora/operador para HTML
                                let (saida_html, retorno_html) = format_presence_info_html(pessoa_atualizada);
                                update.saida_info_html = saida_html;
                                update.retorno_info_html = retorno_html;
                                update.message = format!(
                                    "{} {} por {}",
                                    user.name,
                                    if update.esta_fora { "marcado(a) como FORA" } else { "marcado(a) como A BORDO" },
                                    operator_name
                                );
                            } else {
                                 tracing::error!("Inconsistência: User {} atualizado mas não encontrado na releitura da turma!", action.user_id);
                                 update.success = false;
                                 update.message = "Erro: Inconsistência de dados.".to_string();
                            }
                        }
                        Err(e) => {
                            tracing::error!("Erro ao reler presença da turma {} após update: {:?}", user.ano, e);
                            update.success = false;
                            update.message = "Erro ao buscar dados atualizados da turma.".to_string();
                        }
                    }
                }
                Ok(None) => { // User não encontrado (estranho se a marcação deu certo)
                    tracing::error!("Inconsistência: User {} não encontrado após marcação bem-sucedida!", action.user_id);
                    update.success = false;
                    update.message = "Erro: Utilizador não encontrado.".to_string();
                }
                Err(e) => { // Erro ao buscar user
                     tracing::error!("Erro ao buscar user {} após marcação: {:?}", action.user_id, e);
                     update.success = false;
                     update.message = "Erro ao buscar dados do utilizador.".to_string();
                }
            }
        }
        Err(e) => { // Ação na DB falhou
            tracing::error!("Erro ao marcar presença para {} na DB: {:?}", action.user_id, e);
            update.success = false;
            update.message = match e {
                AppError::SqlxError(_) => "Erro na base de dados.".to_string(),
                _ => "Erro desconhecido ao marcar presença.".to_string(),
            };
            // Tenta buscar stats mesmo assim? Ou deixa default? Vamos deixar default.
        }
    }
    update // Retorna a mensagem de update (sucesso ou erro)
}

/// Função auxiliar para formatar a info de presença para HTML (usado no broadcast).
fn format_presence_info_html(pessoa: &PresencePerson) -> (String, String) {
    let format_single = |dt_opt: &Option<DateTime<Local>>, op_opt: &Option<String>| -> String {
        match (dt_opt, op_opt) {
            (Some(dt), Some(op)) => format!(
                r#"<span class="datetime">{}</span><span class="operator">{}</span>"#,
                dt.format("%d/%m %H:%M"),
                op // Assume que op é o ID ou nome
            ),
            (Some(dt), None) => format!(
                r#"<span class="datetime">{}</span><span class="operator">?</span>"#, // Operador desconhecido
                dt.format("%d/%m %H:%M")
            ),
            _ => "---".to_string(), // Sem data
        }
    };
    (
        format_single(&pessoa.ultima_saida, &pessoa.usuario_saida),
        format_single(&pessoa.ultimo_retorno, &pessoa.usuario_retorno),
    )
}