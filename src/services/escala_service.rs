// src/services/escala_service.rs
use crate::models::escala::{Posto, Candidato};
use sqlx::SqlitePool;
use uuid::Uuid;

// Enum para facilitar a escolha do tipo de dia
pub enum TipoRotina {
    RN, // Rotina Normal (Seg-Sex, contador 'Preta')
    RD, // Rotina Domingo/Feriado (Contador 'Vermelha')
}

impl TipoRotina {
    pub fn as_str(&self) -> &'static str {
        match self {
            TipoRotina::RN => "RN",
            TipoRotina::RD => "RD",
        }
    }
}

/// Gera a escala para um dia específico seguindo todas as regras militares
pub async fn gerar_escala_diaria(
    pool: &SqlitePool,
    data_alvo: &str, // Formato YYYY-MM-DD
    tipo: TipoRotina
) -> Result<String, String> {

    // Iniciamos uma transação: ou grava a escala toda correta, ou não grava nada.
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 1. Registar o dia na tabela de escalas
    // 'INSERT OR IGNORE' garante que não duplicamos se clicarem no botão 2 vezes
    sqlx::query("INSERT OR IGNORE INTO escalas (data, tipo_rotina) VALUES (?, ?)")
        .bind(data_alvo)
        .bind(tipo.as_str())
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    // 2. Buscar todos os postos
    let postos = sqlx::query_as::<_, Posto>("SELECT * FROM postos")
        .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;

    for posto in postos {
        // 3. Seleção de Candidatos (Query Inteligente)
        // Selecionamos utilizadores que:
        // - Respeitam o género do posto
        // - NÃO têm indisponibilidade (baixa médica, dispensa) na data alvo
        // Ordenamos por:
        // - Saldo de Punições DESC (Quem deve, paga primeiro)
        // - Contador específico (RN ou RD) ASC (Quem tem menos, é escalado para equilibrar)
        
        let coluna_servico = match tipo {
            TipoRotina::RN => "servicos_rn",
            TipoRotina::RD => "servicos_rd",
        };

        // Nota: Injetamos o nome da coluna com format! porque SQL não permite bind em nomes de coluna
        let query = format!(
            r#"
            SELECT u.id, u.name, u.genero, u.turma, u.servicos_rn, u.servicos_rd, u.saldo_punicoes 
            FROM users u
            WHERE (u.genero = ? OR ? = 'Misto')
            AND NOT EXISTS (
                SELECT 1 FROM indisponibilidades i 
                WHERE i.user_id = u.id 
                AND ? BETWEEN i.data_inicio AND i.data_fim
            )
            ORDER BY 
                u.saldo_punicoes DESC, 
                u.{} ASC
            "#,
            coluna_servico
        );

        let candidatos = sqlx::query_as::<_, Candidato>(&query)
            .bind(&posto.genero_restricao)
            .bind(&posto.genero_restricao)
            .bind(data_alvo)
            .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;

        let mut escolhido: Option<Candidato> = None;

        for user in candidatos {
            // REGRA 1: Hierarquia Rígida (Turma)
            // Se o posto exige '3º Ano' e o user é '1º Ano', pula.
            if !posto.aceita_turma(&user.turma) { continue; }

            // REGRA 2: Fadiga (24h de intervalo)
            // Verifica se existe alocação ONTEM (-1 day) ou AMANHÃ (+1 day)
            // Também impede dobra no mesmo dia.
            let tem_conflito: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM alocacoes 
                    WHERE user_id = ? 
                    AND date(data) BETWEEN date(?, '-1 day') AND date(?, '+1 day')
                )
                "#
            )
            .bind(&user.id)
            .bind(data_alvo)
            .bind(data_alvo)
            .fetch_one(&mut *tx).await.unwrap_or(false);

            if !tem_conflito {
                escolhido = Some(user);
                break; // Encontramos o candidato ideal (primeiro da lista ordenada)
            }
        }

        match escolhido {
            Some(user) => {
                let is_punicao = user.saldo_punicoes > 0;
                let uuid = Uuid::new_v4().to_string();

                // 4. Inserir a Alocação
                sqlx::query(
                    "INSERT INTO alocacoes (id, user_id, posto_id, data, is_punicao) VALUES (?, ?, ?, ?, ?)"
                )
                .bind(uuid)
                .bind(&user.id)
                .bind(posto.id)
                .bind(data_alvo)
                .bind(is_punicao)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;

                // 5. Atualizar Contadores
                if is_punicao {
                    // Se é punição: Paga 1 de saldo, mas NÃO conta serviço (é "invisível" para equidade)
                    sqlx::query("UPDATE users SET saldo_punicoes = saldo_punicoes - 1 WHERE id = ?")
                        .bind(&user.id)
                        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
                } else {
                    // Se é serviço normal: Incrementa o contador da rotina específica (RN ou RD)
                    let sql_update = format!("UPDATE users SET {} = {} + 1 WHERE id = ?", coluna_servico, coluna_servico);
                    sqlx::query(&sql_update)
                        .bind(&user.id)
                        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
                }
            },
            None => {
                // REGRA RÍGIDA: Se ninguém serve, aborta tudo.
                return Err(format!(
                    "ERRO CRÍTICO: Ninguém disponível para o posto '{}'. Verifique restrições de turma ({}) ou falta de pessoal.", 
                    posto.nome, posto.turmas_permitidas
                ));
            }
        }
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(format!("Escala para {} ({}) gerada com sucesso.", data_alvo, tipo.as_str()))
}

/// Aprova uma troca de serviço (permuta)
/// Transfere a responsabilidade e os pontos do Solicitante para o Substituto
pub async fn aprovar_troca(
    pool: &SqlitePool,
    troca_id: &str
) -> Result<String, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 1. Buscar dados da troca pendente
    let dados = sqlx::query!(
        r#"
        SELECT t.solicitante_id, t.substituto_id, t.alocacao_id, 
               a.data as "data_servico!", e.tipo_rotina as "tipo_rotina!", a.is_punicao
        FROM trocas t
        JOIN alocacoes a ON t.alocacao_id = a.id
        JOIN escalas e ON a.data = e.data
        WHERE t.id = ? AND t.status = 'Pendente'
        "#,
        troca_id
    )
    .fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;

    let dados = match dados {
        Some(d) => d,
        None => return Err("Troca não encontrada ou já processada.".into()),
    };

    // 2. Validar Fadiga do Substituto (Regra de Ouro)
    let tem_conflito: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM alocacoes 
            WHERE user_id = ? 
            AND date(data) BETWEEN date(?, '-1 day') AND date(?, '+1 day')
        )
        "#
    )
    .bind(&dados.substituto_id)
    .bind(&dados.data_servico)
    .bind(&dados.data_servico)
    .fetch_one(&mut *tx).await.unwrap_or(false);

    if tem_conflito {
 return Err("Troca recusada: O substituto violaria a regra de fadiga (24h) se assumisse este serviço.".into());
    }

    // 3. Efetivar a troca na alocação
    sqlx::query("UPDATE alocacoes SET user_id = ? WHERE id = ?")
        .bind(&dados.substituto_id)
        .bind(&dados.alocacao_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    // 4. Ajustar Pontuação (Se não for serviço de punição)
    if !dados.is_punicao.unwrap_or(false) {
        let coluna = if dados.tipo_rotina == "RN" { "servicos_rn" } else { "servicos_rd" };
        
        // Remove ponto do solicitante (ele livrou-se do serviço)
        let sql_dec = format!("UPDATE users SET {} = {} - 1 WHERE id = ?", coluna, coluna);
        sqlx::query(&sql_dec).bind(&dados.solicitante_id).execute(&mut *tx).await.ok();

        // Adiciona ponto ao substituto (ele vai executar)
        let sql_inc = format!("UPDATE users SET {} = {} + 1 WHERE id = ?", coluna, coluna);
        sqlx::query(&sql_inc).bind(&dados.substituto_id).execute(&mut *tx).await.ok();
    }

    // 5. Marcar troca como Aprovada
    sqlx::query("UPDATE trocas SET status = 'Aprovada', data_resposta = datetime('now') WHERE id = ?")
        .bind(troca_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok("Troca aprovada com sucesso.".into())
}