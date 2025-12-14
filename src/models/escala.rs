// src/models/escala.rs
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// --- Estruturas que espelham as Tabelas da DB ---

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct Posto {
    pub id: i64,          // SQLite usa i64 para inteiros
    pub nome: String,
    pub genero_restricao: String,
    pub turmas_permitidas: String, // Vem do banco como string "1,2"
    pub peso: i64,
}

impl Posto {
    // Função auxiliar para verificar se uma turma (ex: "3") está na lista permitida (ex: "1,2,3")
    pub fn aceita_turma(&self, turma_user: &str) -> bool {
        self.turmas_permitidas
            .split(',')
            .any(|t| t.trim() == turma_user)
    }
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Alocacao {
    pub id: String,       // UUID
    pub user_id: String,
    pub posto_id: i64,
    pub data: String,     // YYYY-MM-DD
    pub is_punicao: bool,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Troca {
    pub id: String,
    pub solicitante_id: String,
    pub substituto_id: String,
    pub alocacao_id: String,
    pub status: String,      // 'Pendente', 'Aprovada', 'Recusada'
    pub criado_em: Option<String>,
    pub data_resposta: Option<String>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Indisponibilidade {
    pub id: i64,
    pub user_id: String,
    pub data_inicio: String,
    pub data_fim: String,
    pub motivo: Option<String>,
}

// --- Estruturas Auxiliares para o Algoritmo ---

/// Representa um utilizador candidato à escala.
/// Não usamos o model `User` completo para ser mais leve e focar nos contadores.
#[derive(Debug, FromRow)]
pub struct Candidato {
    pub id: String,
    pub name: String,
    pub genero: String,
    pub turma: String,
    // Campos novos adicionados na migração:
    pub servicos_rn: i64, 
    pub servicos_rd: i64,
    pub saldo_punicoes: i64,
}

/// Usado para receber pedidos de troca via JSON na API
#[derive(Debug, Deserialize)]
pub struct PedidoTrocaPayload {
    pub alocacao_id: String,
    pub substituto_id: String,
}