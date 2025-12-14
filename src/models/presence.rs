// src/models/presence.rs
use chrono::{DateTime, Local}; // Usaremos DateTime<Local> para lógica interna
use serde::{Deserialize, Serialize}; // Para possíveis usos em JSON (ex: WebSockets)
use sqlx::FromRow; // Para ler da base de dados

/// Representa uma linha lida diretamente da tabela `presenca`.
/// As datas são guardadas como TEXT (String) na DB (formato ISO 8601/RFC3339).
#[derive(Debug, Clone, Default, FromRow)]
pub struct PresenceEntry {
    pub user_id: String,
    pub ultima_saida: Option<String>,    // ISO 8601 string or NULL
    pub ultimo_retorno: Option<String>,  // ISO 8601 string or NULL
    pub usuario_saida: Option<String>,   // ID do operador
    pub usuario_retorno: Option<String>, // ID do operador
}

/// Representa os dados combinados de um utilizador e o seu estado de presença,
/// formatado para exibição ou uso na lógica da aplicação.
#[derive(Debug, Clone, Serialize, Deserialize)] // Serialize/Deserialize úteis para WebSockets
pub struct PresencePerson {
    // Dados básicos do utilizador (virão da struct User)
    pub id: String,
    pub nome: String,
    pub turma: String,
    pub ano: i64, // Mantemos i64 por consistência com a DB
    // ... (outros campos do User se necessário, ex: curso, genero)

    // Dados de presença processados
    // Usamos DateTime<Local> para comparações e exibição formatada
    pub ultima_saida: Option<DateTime<Local>>,
    pub ultimo_retorno: Option<DateTime<Local>>,
    pub usuario_saida: Option<String>, // Pode ser ID ou nome (depende da implementação)
    pub usuario_retorno: Option<String>, // Pode ser ID ou nome

    // Estado calculado (A Bordo / Fora)
    pub esta_fora: bool,
}

/// Estrutura para as estatísticas de presença (ex: para uma turma).
#[derive(Debug, Clone, Default, Serialize, Deserialize)] // Útil para WebSockets
pub struct PresenceStats {
    pub fora: usize,  // Quantidade de pessoas fora
    pub dentro: usize, // Quantidade de pessoas a bordo
    pub total: usize, // Total de pessoas na lista
}

// --- Structs para comunicação WebSocket (definimos aqui por conveniência) ---

/// Ação enviada pelo cliente (operador) via WebSocket.
#[derive(Debug, Deserialize)]
pub struct PresenceSocketAction {
    pub action: String, // "saida" ou "retorno"
    pub user_id: String, // ID do utilizador a marcar
}

/// Atualização enviada pelo servidor para todos os clientes via WebSocket.
#[derive(Debug, Serialize, Clone, Default)]
pub struct PresenceSocketUpdate {
    pub success: bool,          // A ação foi bem-sucedida?
    pub message: String,        // Mensagem de feedback (opcional)
    pub user_id: String,        // ID do utilizador que foi atualizado
    pub esta_fora: bool,        // Novo estado (true=fora, false=dentro)
    pub saida_info_html: String, // HTML formatado para coluna "Última Saída"
    pub retorno_info_html: String, // HTML formatado para coluna "Último Retorno"
    pub stats: PresenceStats, // Estatísticas atualizadas da turma afetada
}