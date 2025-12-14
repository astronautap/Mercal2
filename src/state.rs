// src/state.rs
use axum::extract::ws::{Message, WebSocket}; // Adicionar imports WebSocket
use futures_util::stream::SplitSink; // Adicionar SplitSink
use sqlx::SqlitePool;
use std::{collections::HashMap, sync::Arc}; // Adicionar Arc, HashMap
use tokio::sync::{mpsc, Mutex}; // Adicionar mpsc, Mutex
use uuid::Uuid; // Adicionar Uuid

// Tipo para o 'sender' de uma conexão WebSocket individual
type WsTx = mpsc::Sender<Message>;

// Estrutura para gerir as conexões WebSocket de presença
#[derive(Debug, Clone, Default)]
pub struct PresenceWsState {
    // Usamos Arc<Mutex<...>> para permitir acesso seguro de múltiplos threads/tasks
    // O HashMap guarda o ID da conexão (Uuid) e o canal (Sender) para enviar mensagens
    pub connections: Arc<Mutex<HashMap<Uuid, WsTx>>>,
}

impl PresenceWsState {
    /// Envia uma mensagem para TODAS as conexões ativas.
    pub async fn broadcast(&self, message_text: String) {
        let connections = self.connections.lock().await;
        let message = Message::Text(message_text.into()); // Cria a mensagem WebSocket

        // Itera sobre os senders no HashMap
        for tx in connections.values() {
            // Tenta enviar a mensagem. Se falhar (ex: cliente desconectado), ignora o erro.
            // Usar tx.send().await pode bloquear um pouco se o buffer estiver cheio.
            // Para alta performance, considerar tx.try_send() ou spawns.
            let _ = tx.send(message.clone()).await; // Clona a mensagem para cada envio
        }
    }
}


// Atualiza o AppState para incluir o estado do WebSocket
#[derive(Clone)]
pub struct AppState {
    pub db_pool: SqlitePool,
    // Adiciona o estado das conexões WebSocket de presença
    pub presence_state: PresenceWsState,
}

// Permite extrair o pool da DB diretamente
impl axum::extract::FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> SqlitePool {
        state.db_pool.clone()
    }
}
// (Opcional) Permite extrair PresenceWsState diretamente
impl axum::extract::FromRef<AppState> for PresenceWsState {
    fn from_ref(state: &AppState) -> PresenceWsState {
        state.presence_state.clone()
    }
}