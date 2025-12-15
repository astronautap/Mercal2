// src/models/escala.rs
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// --- Estruturas que espelham as Tabelas da DB ---

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct Posto {
    pub id: i64,
    pub nome: String,
    pub genero_restricao: String,
    pub turmas_permitidas: String, // Ex: "1,2" (Guardado como texto)
    pub peso: i64,
}

impl Posto {
    // --- ALTERADO: Agora valida pelo Ano (i64) em vez da string Turma ---
    pub fn aceita_ano(&self, ano_user: i64) -> bool {
        let ano_str = ano_user.to_string();
        self.turmas_permitidas
            .split(',')
            .any(|t| t.trim() == ano_str)
    }
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
    pub ano: i64,
    pub servicos_rn: i64, 
    pub servicos_rd: i64,
    pub saldo_punicoes: i64,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Alocacao {
    pub id: String,
    pub user_id: String,
    pub posto_id: i64,
    pub data: String,
    pub is_punicao: bool,
    // (Opcional) Poderíamos trazer o status da escala aqui, mas faremos via JOIN
}

// Payload para Gerar em Lote (Admin)
#[derive(Debug, Deserialize)]
pub struct GerarPeriodoRequest {
    pub data_inicio: String, // YYYY-MM-DD
    pub data_fim: String,    // YYYY-MM-DD
}

// Payload para Publicar (Admin)
#[derive(Debug, Deserialize)]
pub struct PublicarRequest {
    pub data_inicio: String,
    pub data_fim: String,
}

// Payload para Pedir Troca (User)
#[derive(Debug, Deserialize)]
pub struct PedidoTrocaPayload {
    pub alocacao_id: String,
    pub substituto_id: String,
    pub motivo: String, // Obrigatório agora
    pub alocacao_substituto_id: Option<String>,
}
