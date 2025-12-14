// src/services/escala_service.rs
use crate::models::escala::{Posto, Candidato};
use sqlx::SqlitePool;
use uuid::Uuid;
use chrono::{NaiveDate, Datelike, Duration}; // Importante para calcular dias da semana

pub enum TipoRotina { RN, RD }

impl TipoRotina {
    pub fn as_str(&self) -> &'static str {
        match self { TipoRotina::RN => "RN", TipoRotina::RD => "RD" }
    }
}

// --- FUNÇÃO PRINCIPAL: GERAR PERÍODO ---
pub async fn gerar_escala_periodo(
    pool: &SqlitePool,
    inicio_str: &str,
    fim_str: &str
) -> Result<String, String> {
    
    // Converter strings para Datas
    let inicio = NaiveDate::parse_from_str(inicio_str, "%Y-%m-%d").map_err(|_| "Data início inválida")?;
    let fim = NaiveDate::parse_from_str(fim_str, "%Y-%m-%d").map_err(|_| "Data fim inválida")?;

    if fim < inicio { return Err("Data fim deve ser depois do início".into()); }

    let mut data_atual = inicio;
    let mut dias_gerados = 0;

    // Loop dia a dia
    while data_atual <= fim {
        let data_str = data_atual.format("%Y-%m-%d").to_string();

        // 1. REGRA AUTOMÁTICA (Opção A Modificada)
        // Sexta(Fri), Sábado(Sat), Domingo(Sun) -> RD
        let tipo = match data_atual.weekday() {
            chrono::Weekday::Fri | chrono::Weekday::Sat | chrono::Weekday::Sun => TipoRotina::RD,
            _ => TipoRotina::RN,
        };

        // 2. Tentar gerar o dia
        // Nota: Precisamos passar a pool diretamente. A transação será por dia para não bloquear tudo se um falhar.
        // (Ou podíamos fazer uma transação gigante, mas por dia é mais seguro para debug)
        match gerar_escala_diaria(pool, &data_str, tipo).await {
            Ok(_) => dias_gerados += 1,
            Err(e) => {
                // Se der erro num dia (ex: ninguém disponível), paramos e avisamos? 
                // Ou continuamos? Vamos parar para o Admin corrigir.
                return Err(format!("Falha ao gerar dia {}: {}", data_str, e));
            }
        }

        data_atual += Duration::days(1);
    }

    Ok(format!("Período gerado com sucesso! {} dias processados.", dias_gerados))
}

// --- GERAÇÃO DIÁRIA (Com limpeza de Rascunho) ---
pub async fn gerar_escala_diaria(
    pool: &SqlitePool, 
    data_alvo: &str, 
    tipo: TipoRotina
) -> Result<String, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 1. VERIFICAR STATUS E LIMPAR DADOS ANTERIORES (Regeneração)
    // Se já houver escala para este dia, verificamos se podemos mexer nela.
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM escalas WHERE data = ?")
        .bind(data_alvo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    if let Some(s) = status {
        if s == "Publicada" {
            return Err(format!("O dia {} já está PUBLICADO. Use a Errata para reabrir antes de regenerar.", data_alvo));
        }
        
        // Se for Rascunho, limpamos tudo para gerar de novo (Reset Limpo)
        // a) Devolver pontos aos usuários (desfazer contabilidade)
        let alocados = sqlx::query!(
            r#"SELECT user_id, is_punicao, e.tipo_rotina 
               FROM alocacoes a 
               JOIN escalas e ON a.data = e.data 
               WHERE a.data = ?"#, 
            data_alvo
        ).fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;

        for row in alocados {
            if row.is_punicao.unwrap_or(false) { // Era punição? Devolve a dívida (+1 no saldo)
                 sqlx::query("UPDATE users SET saldo_punicoes = saldo_punicoes + 1 WHERE id = ?")
                    .bind(row.user_id).execute(&mut *tx).await.ok();
            } else { // Era serviço normal? Remove o ponto da contagem (-1 no serviço)
                 let col = if row.tipo_rotina == "RN" { "servicos_rn" } else { "servicos_rd" };
                 let sql = format!("UPDATE users SET {} = {} - 1 WHERE id = ?", col, col);
                 sqlx::query(&sql).bind(row.user_id).execute(&mut *tx).await.ok();
            }
        }
        
        // b) Apagar as alocações antigas deste dia
        sqlx::query("DELETE FROM alocacoes WHERE data = ?")
            .bind(data_alvo)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    // 2. CRIAR/ATUALIZAR CABEÇALHO (Sempre Rascunho ao gerar)
    sqlx::query("INSERT OR REPLACE INTO escalas (data, tipo_rotina, status) VALUES (?, ?, 'Rascunho')")
        .bind(data_alvo)
        .bind(tipo.as_str())
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    // 3. ALGORITMO DE ALOCAÇÃO
    let postos = sqlx::query_as::<_, Posto>("SELECT * FROM postos")
        .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
    
    for posto in postos {
        let coluna_servico = match tipo { TipoRotina::RN => "servicos_rn", TipoRotina::RD => "servicos_rd" };
        
        // QUERY: Trazemos 'u.ano' para validar a hierarquia numérica
        let query = format!(
            r#"
            SELECT u.id, u.name, u.genero, u.turma, u.ano, u.servicos_rn, u.servicos_rd, u.saldo_punicoes 
            FROM users u
            WHERE (u.genero = ? OR ? = 'Misto')
            AND NOT EXISTS (
                SELECT 1 FROM indisponibilidades i 
                WHERE i.user_id = u.id AND ? BETWEEN i.data_inicio AND i.data_fim
            )
            ORDER BY u.saldo_punicoes DESC, u.{} ASC
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
            // REGRA 1: HIERARQUIA POR ANO (1, 2, 3)
            // O posto tem "1,2" -> O user tem ano 1 -> OK
            if !posto.aceita_ano(user.ano) { continue; }

            // REGRA 2: FADIGA (24h)
            let conflito: bool = sqlx::query_scalar(
                r#"SELECT EXISTS(
                    SELECT 1 FROM alocacoes 
                    WHERE user_id = ? 
                    AND date(data) BETWEEN date(?, '-1 day') AND date(?, '+1 day')
                )"#
            )
            .bind(&user.id)
            .bind(data_alvo)
            .bind(data_alvo)
            .fetch_one(&mut *tx).await.unwrap_or(false);

            if !conflito { 
                escolhido = Some(user); 
                break; 
            }
        }

        if let Some(user) = escolhido {
            let is_punicao = user.saldo_punicoes > 0;
            let uuid = Uuid::new_v4().to_string();
            
            // Gravar Alocação
            sqlx::query("INSERT INTO alocacoes (id, user_id, posto_id, data, is_punicao) VALUES (?, ?, ?, ?, ?)")
                .bind(uuid)
                .bind(&user.id)
                .bind(posto.id)
                .bind(data_alvo)
                .bind(is_punicao)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;
            
            // Atualizar Contadores
            if is_punicao {
                sqlx::query("UPDATE users SET saldo_punicoes = saldo_punicoes - 1 WHERE id = ?")
                    .bind(&user.id).execute(&mut *tx).await.ok();
            } else {
                let sql_up = format!("UPDATE users SET {} = {} + 1 WHERE id = ?", coluna_servico, coluna_servico);
                sqlx::query(&sql_up).bind(&user.id).execute(&mut *tx).await.ok();
            }
        } else {
             // Se ninguém servir, abortamos para o admin saber que falta gente
             return Err(format!("ERRO CRÍTICO: Ninguém disponível para o posto '{}' (Ano exigido: {}). Verifique efetivo ou restrições.", posto.nome, posto.turmas_permitidas));
        }
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(format!("Escala para {} gerada com sucesso.", data_alvo))
}

// --- PUBLICAR PERÍODO ---
pub async fn publicar_escala(pool: &SqlitePool, inicio: &str, fim: &str) -> Result<String, String> {
    // Muda tudo o que é Rascunho para Publicada nesse intervalo
    let res = sqlx::query(
        "UPDATE escalas SET status = 'Publicada' WHERE data BETWEEN ? AND ? AND status = 'Rascunho'"
    )
    .bind(inicio)
    .bind(fim)
    .execute(pool).await.map_err(|e| e.to_string())?;

    if res.rows_affected() == 0 {
        return Err("Nenhuma escala 'Rascunho' encontrada neste período para publicar.".into());
    }
    Ok(format!("{} dias de escala foram tornados OFICIAIS (Publicados).", res.rows_affected()))
}

// --- SOLICITAR TROCA (Com Motivo e Validação de Status) ---
pub async fn solicitar_troca(
    pool: &SqlitePool, 
    solicitante_id: &str, 
    alocacao_id: &str, 
    substituto_id: &str,
    motivo: &str
) -> Result<String, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 1. Validar: A escala ainda é Rascunho?
    let info = sqlx::query!(
        r#"SELECT e.status, a.data FROM alocacoes a JOIN escalas e ON a.data = e.data WHERE a.id = ?"#,
        alocacao_id
    ).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;

    let (status, data_servico) = match info {
        Some(i) => (i.status.unwrap_or("Rascunho".to_string()), i.data),
        None => return Err("Alocação não encontrada".into())
    };

    if status == "Publicada" {
        return Err("Esta escala já está PUBLICADA. Alterações só via Admin/Escalante.".into());
    }

    // 2. Validar Fadiga do Substituto
    let conflito: bool = sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM alocacoes WHERE user_id = ? AND date(data) BETWEEN date(?, '-1 day') AND date(?, '+1 day'))"#)
        .bind(substituto_id).bind(&data_servico).bind(&data_servico)
        .fetch_one(&mut *tx).await.unwrap_or(false);
    
    if conflito { return Err("O substituto viola a regra de fadiga (24h).".into()); }

    // 3. Inserir Pedido
    let uuid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO trocas (id, solicitante_id, substituto_id, alocacao_id, status, motivo) VALUES (?, ?, ?, ?, 'Pendente', ?)")
        .bind(uuid).bind(solicitante_id).bind(substituto_id).bind(alocacao_id).bind(motivo)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok("Troca solicitada! Aguarde aprovação do Escalante.".into())
}

// --- APROVAR TROCA (Mantém-se igual, mas agora lê da tabela trocas) ---
pub async fn aprovar_troca(pool: &SqlitePool, troca_id: &str) -> Result<String, String> {
    // ... (Use a implementação anterior, ela já está correta para processar) ...
    // Apenas certifique-se de que ela funciona
    // ... (Código omitido por brevidade, é igual ao anterior)
    // Se quiser, posso repetir aqui.
    crate::services::escala_service::aprovar_troca_impl_completa(pool, troca_id).await
}

// Helper interno para não duplicar código na resposta
async fn aprovar_troca_impl_completa(pool: &SqlitePool, troca_id: &str) -> Result<String, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let dados = sqlx::query!(
        r#"SELECT t.solicitante_id, t.substituto_id, t.alocacao_id, a.data as "data!", e.tipo_rotina, a.is_punicao
           FROM trocas t JOIN alocacoes a ON t.alocacao_id = a.id JOIN escalas e ON a.data = e.data
           WHERE t.id = ? AND t.status = 'Pendente'"#,
        troca_id
    ).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;
    
    let d = match dados { Some(v) => v, None => return Err("Troca inválida".into()) };
    
    // Fadiga check double-check (is_punicao é Option<bool>)
    let conflito: bool = sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM alocacoes WHERE user_id = ? AND date(data) BETWEEN date(?, '-1 day') AND date(?, '+1 day'))"#)
        .bind(&d.substituto_id).bind(&d.data).bind(&d.data)
        .fetch_one(&mut *tx).await.unwrap_or(false);
    if conflito { return Err("Substituto com fadiga".into()); }

    sqlx::query("UPDATE alocacoes SET user_id = ? WHERE id = ?").bind(&d.substituto_id).bind(&d.alocacao_id).execute(&mut *tx).await.ok();
    
    if !d.is_punicao.unwrap_or(false) { // is_punicao é Option<bool>
        let col = if d.tipo_rotina == "RN" { "servicos_rn" } else { "servicos_rd" };
        let s_dec = format!("UPDATE users SET {} = {} - 1 WHERE id = ?", col, col);
        let s_inc = format!("UPDATE users SET {} = {} + 1 WHERE id = ?", col, col);
        sqlx::query(&s_dec).bind(&d.solicitante_id).execute(&mut *tx).await.ok();
        sqlx::query(&s_inc).bind(&d.substituto_id).execute(&mut *tx).await.ok();
    }
    sqlx::query("UPDATE trocas SET status = 'Aprovada', data_resposta = datetime('now') WHERE id = ?").bind(troca_id).execute(&mut *tx).await.ok();
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok("Troca Aprovada".into())
}

pub async fn errata_dia(pool: &SqlitePool, data: &str) -> Result<String, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 1. Verificar o status atual
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM escalas WHERE data = ?")
        .bind(data)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    match status {
        Some(s) if s == "Publicada" => {
            // 2. Reverter status para 'Rascunho'
            // Isto permite que o admin volte a ver os botões de "Trocar" e "Gerar"
            sqlx::query("UPDATE escalas SET status = 'Rascunho' WHERE data = ?")
                .bind(data)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            
            tx.commit().await.map_err(|e| e.to_string())?;
            
            Ok(format!("O dia {} foi reaberto em modo RASCUNHO. Pode agora fazer alterações manuais ou regenerar.", data))
        },
        Some(_) => Err(format!("O dia {} ainda não está publicado. Não é necessário criar errata.", data)),
        None => Err(format!("Não existe escala gerada para o dia {}.", data)),
    }
}

pub async fn responder_troca_usuario(
    pool: &SqlitePool,
    troca_id: &str,
    user_id: &str, // ID de quem está a responder (segurança)
    acao: &str,    // "aceitar" ou "recusar"
) -> Result<String, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 1. Validar se o pedido existe e é para este utilizador
    let troca = sqlx::query!(
        "SELECT substituto_id, status FROM trocas WHERE id = ?",
        troca_id
    )
    .fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;

    let troca = match troca {
        Some(t) => t,
        None => return Err("Pedido de troca não encontrado.".into()),
    };

    if troca.substituto_id != user_id {
        return Err("Não tem permissão para responder a este pedido.".into());
    }

    if troca.status.as_deref() != Some("Pendente") {
        return Err("Este pedido já foi respondido ou processado.".into());
    }

    // 2. Processar Ação
    if acao == "aceitar" {
        // Muda para um estado que o Escalante veja (ex: 'AguardandoEscalante')
        sqlx::query("UPDATE trocas SET status = 'AguardandoEscalante' WHERE id = ?")
            .bind(troca_id)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
        
        tx.commit().await.map_err(|e| e.to_string())?;
        Ok("Confirmou a troca! Agora aguarde a aprovação final do Escalante.".into())
    } else {
        // Recusa e fecha o processo
        sqlx::query("UPDATE trocas SET status = 'Recusada', data_resposta = datetime('now') WHERE id = ?")
            .bind(troca_id)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
            
        tx.commit().await.map_err(|e| e.to_string())?;
        Ok("Pedido de troca recusado.".into())
    }
}