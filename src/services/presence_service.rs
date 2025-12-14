// src/services/presence_service.rs
use crate::{
    error::{AppError, AppResult}, // Erros e Result da aplicação
    models::{
        presence::{PresenceEntry, PresencePerson, PresenceStats}, // Modelos de presença
        user::User, // Modelo User para obter dados básicos
    },
    services::user_service, // Para buscar todos os users de uma turma
};
use chrono::{DateTime, Local}; // Para trabalhar com data/hora local
use sqlx::SqlitePool;
use std::collections::HashMap; // Para mapear entradas de presença por user_id

/// Marca a saída de um utilizador na base de dados.
/// Usa UPSERT para inserir ou atualizar o registo existente.
pub async fn marcar_saida(
    db_pool: &SqlitePool,
    user_id: &str,
    operator_id: &str, // ID do operador que fez a marcação
) -> AppResult<()> {
    // Obtém a data/hora atual e formata como string ISO 8601/RFC3339
    let now_str = Local::now().to_rfc3339();
    tracing::debug!(
        "Marcando SAÍDA para user {} por {} em {}",
        user_id,
        operator_id,
        now_str
    );

    // Executa a query UPSERT
    sqlx::query!(
        r#"
        INSERT INTO presenca (user_id, ultima_saida, usuario_saida)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(user_id) DO UPDATE SET
           ultima_saida = excluded.ultima_saida,
           usuario_saida = excluded.usuario_saida
        "#,
        user_id,
        now_str, // Passa a string formatada
        operator_id
    )
    .execute(db_pool)
    .await?; // Propaga o erro se a query falhar

    Ok(()) // Retorna Ok se a execução foi bem-sucedida
}

/// Marca o retorno de um utilizador na base de dados.
/// Usa UPSERT para inserir ou atualizar o registo existente.
pub async fn marcar_retorno(
    db_pool: &SqlitePool,
    user_id: &str,
    operator_id: &str, // ID do operador que fez a marcação
) -> AppResult<()> {
    let now_str = Local::now().to_rfc3339();
    tracing::debug!(
        "Marcando RETORNO para user {} por {} em {}",
        user_id,
        operator_id,
        now_str
    );

    sqlx::query!(
        r#"
        INSERT INTO presenca (user_id, ultimo_retorno, usuario_retorno)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(user_id) DO UPDATE SET
           ultimo_retorno = excluded.ultimo_retorno,
           usuario_retorno = excluded.usuario_retorno
        "#,
        user_id,
        now_str,
        operator_id
    )
    .execute(db_pool)
    .await?;

    Ok(())
}

/// Busca a lista combinada de utilizadores e estado de presença para uma turma.
pub async fn get_presence_list_for_turma(
    db_pool: &SqlitePool,
    turma_num: i64, // Usar i64 para corresponder ao 'ano' na DB
) -> AppResult<Vec<PresencePerson>> {
    tracing::debug!("Buscando lista de presença para turma {}", turma_num);

    // 1. Busca todos os utilizadores da turma especificada
    //    (Idealmente, user_service teria uma função find_users_by_turma)
    //    Por agora, buscamos todos e filtramos. Cuidado com a performance se houver muitos users.
    let all_users = user_service::find_all_users(db_pool).await?;
    let users_in_turma: Vec<User> = all_users
        .into_iter()
        .filter(|u| u.ano == turma_num)
        .collect();

    if users_in_turma.is_empty() {
        tracing::debug!("Nenhum utilizador encontrado para a turma {}", turma_num);
        return Ok(Vec::new()); // Retorna lista vazia se a turma não tiver alunos
    }

    // Extrai os IDs dos utilizadores da turma para a query de presença
    let user_ids: Vec<String> = users_in_turma.iter().map(|u| u.id.clone()).collect();

    // 2. Busca as entradas de presença APENAS para os utilizadores dessa turma
    //    Usamos `query_as` para mapear para a struct PresenceEntry
    //    A cláusula IN pode ser lenta em SQLite com muitos IDs, mas para uma turma deve ser ok.
    //    Precisamos construir a query IN dinamicamente ou usar outra abordagem se forem muitos IDs.
    //    Por simplicidade, vamos buscar todas as presenças e filtrar depois (menos eficiente).
    let all_presence_entries: Vec<PresenceEntry> = sqlx::query_as!(
        PresenceEntry,
        r#"
        SELECT user_id, ultima_saida, ultimo_retorno, usuario_saida, usuario_retorno
        FROM presenca
        "#
        // WHERE user_id IN (?) -- SQLx não suporta IN (?) diretamente assim fácil
    )
    .fetch_all(db_pool)
    .await?;

    // Mapeia as entradas de presença por user_id para acesso rápido
    let presence_map: HashMap<String, PresenceEntry> = all_presence_entries
        .into_iter()
        .map(|entry| (entry.user_id.clone(), entry))
        .collect();

    // 3. Combina os dados e calcula o estado
    let mut presence_list = Vec::new();
    for user in users_in_turma {
        // Obtém a entrada de presença para este user (ou default se não existir)
        let entry = presence_map.get(&user.id).cloned().unwrap_or_default();

        // Tenta fazer o parse das strings de data/hora para DateTime<Local>
        let ultima_saida_dt = entry.ultima_saida.as_ref().and_then(|s| {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Local)) // Converte para timezone local
                .map_err(|e| tracing::warn!("Erro ao parsear ultima_saida para {}: {}", user.id, e)) // Loga erro de parse
                .ok() // Descarta o erro, resultando em None se falhar
        });
        let ultimo_retorno_dt = entry.ultimo_retorno.as_ref().and_then(|s| {
             DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Local))
                .map_err(|e| tracing::warn!("Erro ao parsear ultimo_retorno para {}: {}", user.id, e))
                .ok()
        });


        // Calcula se está fora
        let esta_fora = match (&ultima_saida_dt, &ultimo_retorno_dt) {
            (Some(saida), Some(retorno)) => saida > retorno, // Compara DateTime<Local>
            (Some(_), None) => true, // Tem saída mas não tem retorno -> Fora
            _ => false, // Sem saída OU retorno mais recente -> Dentro
        };

        presence_list.push(PresencePerson {
            id: user.id,
            nome: user.name,
            turma: user.turma,
            ano: user.ano,
            ultima_saida: ultima_saida_dt,
            ultimo_retorno: ultimo_retorno_dt,
            usuario_saida: entry.usuario_saida,
            usuario_retorno: entry.usuario_retorno,
            esta_fora, // Guarda o estado calculado
        });
    }

    // Ordena a lista pelo ID do utilizador
    presence_list.sort_by(|a, b| a.id.cmp(&b.id));

    tracing::debug!("Lista de presença para turma {} carregada ({} pessoas).", turma_num, presence_list.len());
    Ok(presence_list)
}

/// Calcula as estatísticas (fora/dentro/total) a partir de uma lista de PresencePerson.
// Esta função pode ficar aqui ou ser movida para models/presence.rs ou para o handler.
pub fn calcular_stats(pessoas: &[PresencePerson]) -> PresenceStats {
    let mut fora = 0;
    for pessoa in pessoas {
        if pessoa.esta_fora {
            fora += 1;
        }
    }
    let total = pessoas.len();
    PresenceStats {
        fora,
        dentro: total - fora,
        total,
    }
}