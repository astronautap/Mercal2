// src/services/user_service.rs
use crate::{
    error::{AppError, AppResult},
    models::user::User, // Modelo User completo
};
use chrono::Utc;
use sqlx::SqlitePool;

pub const DEFINED_ROLES: &[&str] = &[
    "admin",
    "rancheiro",
    "escalante",
    "monal",
    "adal",
    "comal",
    "loja",
    // Adicionar outras roles permanentes aqui se necessário no futuro
];


/// Busca um utilizador na base de dados pelo seu ID (Movido de auth_service).
pub async fn find_user_by_id(db_pool: &SqlitePool, user_id: &str) -> AppResult<Option<User>> {
    tracing::debug!("Buscando utilizador (completo) por ID: {}", user_id);
    // Query com conversão explícita de timestamps para NaiveDateTime
    let user = sqlx::query_as!(
        User,
        r#"
        SELECT 
            id, 
            password_hash, 
            name, 
            turma, 
            ano, 
            curso, 
            genero, 
            created_at as "created_at: chrono::NaiveDateTime", 
            updated_at as "updated_at: chrono::NaiveDateTime"
        FROM users
        WHERE id = ?1
        "#,
        user_id
    )
    .fetch_optional(db_pool)
    .await?;

    if user.is_some() {
        tracing::debug!("Utilizador '{}' encontrado.", user_id);
    } else {
        tracing::debug!("Utilizador '{}' não encontrado.", user_id);
    }
    Ok(user)
}

/// Busca as roles (funções) de um utilizador específico.
pub async fn get_user_roles(db_pool: &SqlitePool, user_id: &str) -> AppResult<Vec<String>> {
    tracing::debug!("Buscando roles para user ID: {}", user_id);
    // Query simples para buscar as strings das roles
    let roles = sqlx::query!(
        r#"
        SELECT role FROM user_roles WHERE user_id = ?1 ORDER BY role ASC
        "#,
        user_id
    )
    .fetch_all(db_pool) // Busca todas as linhas
    .await? // Propaga erro
    .into_iter() // Converte em iterador
    .map(|record| record.role) // Extrai a string 'role' de cada registo
    .collect(); // Coleta num Vec<String>

    tracing::debug!("Roles encontradas para {}: {:?}", user_id, roles);
    Ok(roles)
}

// --- Funções para Admin (serão usadas depois) ---

/// Busca todos os utilizadores (sem password_hash por segurança/eficiência).
/// Retorna uma Vec<UserSummary> ou similar. Vamos retornar User por agora.
pub async fn find_all_users(db_pool: &SqlitePool) -> AppResult<Vec<User>> {
    tracing::debug!("Buscando todos os utilizadores...");
    // Seleciona todas as colunas necessárias com conversão de timestamps
    let users = sqlx::query_as!(
        User,
        r#"
        SELECT 
            id, 
            password_hash, 
            name, 
            turma, 
            ano, 
            curso, 
            genero, 
            created_at as "created_at: chrono::NaiveDateTime", 
            updated_at as "updated_at: chrono::NaiveDateTime"
        FROM users
        ORDER BY id ASC
        "#
    )
    .fetch_all(db_pool)
    .await?;
    tracing::debug!("Encontrados {} utilizadores.", users.len());
    Ok(users)
}

// Função para criar user (será usada pelo admin handler)
// Nota: Recebe roles como Vec<String> e insere na tabela user_roles
pub async fn create_user(
    db_pool: &SqlitePool,
    id: &str,
    name: &str,
    raw_password: &str,
    turma: &str,
    ano: i64,
    curso: &str,
    genero: &str,
    roles: &[String], // Recebe slice de roles
) -> AppResult<()> {
    tracing::info!("Tentando criar utilizador: {}", id);
    // 1. Gera o hash da senha (usando a função de auth_service)
    let password_hash = crate::services::auth_service::hash_password(raw_password).await?;

    // 2. Usa uma transação para garantir atomicidade
    let mut tx = db_pool.begin().await?; // Inicia transação

    // 3. Insere na tabela 'users'
    let insert_user_result = sqlx::query!(
        r#"
        INSERT INTO users (id, password_hash, name, turma, ano, curso, genero)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        id, password_hash, name, turma, ano, curso, genero
    )
    .execute(&mut *tx) // Executa dentro da transação
    .await;

    // Verifica erro de constraint (ID duplicado)
    if let Err(sqlx::Error::Database(db_err)) = &insert_user_result {
        // Verifica se é erro de UNIQUE constraint (código 19 no SQLite)
        if db_err.code().map_or(false, |c| c == "19" || c == "2067" || c == "1555") { // Códigos comuns para UNIQUE
            tracing::warn!("Falha ao criar user: ID '{}' já existe.", id);
            tx.rollback().await?; // Desfaz a transação
            // Retorna um erro específico seria melhor, mas vamos usar Internal por agora
            return Err(AppError::InternalServerError); // Ou um AppError::UserAlreadyExists
        }
    }
    insert_user_result?; // Propaga outros erros da inserção

    // 4. Insere as roles na tabela 'user_roles'
    if !roles.is_empty() {
        // Prepara a query para inserção múltipla (mais eficiente)
        // Ex: INSERT INTO user_roles (user_id, role) VALUES ('id', 'role1'), ('id', 'role2'), ...
        // No SQLite, a forma mais simples pode ser um loop
        for role in roles {
            // Usar INSERT OR IGNORE para o caso de a role já existir (não deve acontecer se a validação for boa)
            sqlx::query!(
                r#"
                INSERT OR IGNORE INTO user_roles (user_id, role) VALUES (?1, ?2)
                "#,
                id, role
            )
            .execute(&mut *tx) // Executa dentro da transação
            .await?;
        }
    }

    // 5. Confirma a transação
    tx.commit().await?;
    tracing::info!("✅ Utilizador '{}' criado com sucesso.", id);
    Ok(())
}

// Função para alterar senha (será usada pelo admin handler)
pub async fn update_user_password(
    db_pool: &SqlitePool,
    user_id: &str,
    new_raw_password: &str,
) -> AppResult<()> {
    tracing::info!("Tentando alterar senha para user: {}", user_id);
    // 1. Gera o novo hash
    let new_password_hash = crate::services::auth_service::hash_password(new_raw_password).await?;

    // 2. Atualiza na DB
    let rows_affected = sqlx::query!(
        r#"
        UPDATE users SET password_hash = ?1 WHERE id = ?2
        "#,
        new_password_hash, user_id
    )
    .execute(db_pool)
    .await?
    .rows_affected();

    // 3. Verifica se o utilizador existia
    if rows_affected == 0 {
        tracing::warn!("Falha ao alterar senha: Utilizador '{}' não encontrado.", user_id);
        // Retorna um erro específico seria melhor
        Err(AppError::InternalServerError) // Ou um AppError::UserNotFound
    } else {
        tracing::info!("✅ Senha alterada com sucesso para user: {}", user_id);
        Ok(())
    }
}

// (Adicionar funções para add/remove role depois, se necessário)

pub async fn check_user_role_any(
    db_pool: &SqlitePool,
    user_id: &str,
    required_roles: &[&str],
) -> AppResult<bool> {
    if required_roles.is_empty() {
        return Ok(true);
    }
    tracing::debug!("Verificando roles {:?} para user '{}'", required_roles, user_id);

    // 1. Verifica Roles Permanentes
    let permanent_roles = get_user_roles(db_pool, user_id).await?;
    if permanent_roles.iter().any(|role| required_roles.iter().any(|&req| req.eq_ignore_ascii_case(role))) {
        tracing::debug!("Role permanente encontrada para '{}'. Acesso concedido.", user_id);
        return Ok(true);
    }
    tracing::debug!("Nenhuma role permanente encontrada para '{}'.", user_id);

    // 2. Verifica Roles Temporárias ATIVAS
    let now_utc_str = Utc::now().to_rfc3339();
    // Prepara a lista de roles como string JSON
    let required_roles_json = serde_json::to_string(required_roles)
        .map_err(|e| {
            tracing::error!("Erro ao serializar roles para JSON: {:?}", e);
            AppError::InternalServerError // Ou outro erro apropriado
        })?;

    // *** ALTERADO: Usar query! e map para evitar Option<Option> ***
    let has_active_temp_role: Option<i64> = sqlx::query!(
        r#"
        SELECT 1 as "found: i64"
        FROM user_temporary_roles
        WHERE user_id = ?1
          AND role IN (SELECT value FROM json_each(?2))
          AND ?3 >= start_datetime
          AND ?3 < end_datetime
        LIMIT 1
        "#,
        user_id,
        required_roles_json, // Passa a string JSON
        now_utc_str
    )
    .fetch_optional(db_pool) // Retorna Option<record>
    .await?
    .and_then(|rec| rec.found); // Extrai o campo 'found' do record, produzindo Option<i64>

    if has_active_temp_role.is_some() {
        tracing::debug!("Role temporária ativa encontrada para user '{}'. Acesso concedido.", user_id);
        Ok(true)
    } else {
        tracing::debug!("Nenhuma role temporária ativa encontrada para user '{}'. Acesso negado.", user_id);
        Ok(false)
    }
}

pub async fn set_user_roles(
    db_pool: &SqlitePool,
    user_id: &str,
    new_roles: &[String], // Lista das novas roles a serem atribuídas
) -> AppResult<()> {
    tracing::info!("Atualizando roles para user '{}': {:?}", user_id, new_roles);

    // Validar se as roles fornecidas estão na lista DEFINED_ROLES? (Opcional, segurança extra)
    // for role in new_roles {
    //     if !DEFINED_ROLES.iter().any(|&defined_role| defined_role.eq_ignore_ascii_case(role)) {
    //         tracing::warn!("Tentativa de atribuir role inválida ('{}') para user {}", role, user_id);
    //         // Retornar um erro específico? Por agora, vamos permitir (confiando na UI)
    //     }
    // }

    // Inicia uma transação na base de dados
    let mut tx = db_pool.begin().await?;

    // 1. Apaga TODAS as roles permanentes existentes para este utilizador
    tracing::debug!("Removendo roles antigas para {}", user_id);
    sqlx::query!(
        r#"
        DELETE FROM user_roles WHERE user_id = ?1
        "#,
        user_id
    )
    .execute(&mut *tx) // Executa dentro da transação
    .await?;

    // 2. Insere as novas roles (se houver alguma)
    if !new_roles.is_empty() {
        tracing::debug!("Inserindo novas roles para {}: {:?}", user_id, new_roles);
        // Prepara a query para inserção (ignora erro se a role já existir - não deve acontecer após DELETE)
        // Usamos um loop simples, mas para muitas roles, batch insert seria mais eficiente
        for role in new_roles {
             // Validar role novamente aqui se não validado antes
             if role.trim().is_empty() { continue; } // Ignora roles vazias

             sqlx::query!(
                r#"
                INSERT INTO user_roles (user_id, role) VALUES (?1, ?2)
                "#,
                user_id,
                role // A tabela tem COLLATE NOCASE, então 'admin' e 'Admin' são tratados como iguais
            )
            .execute(&mut *tx) // Executa dentro da transação
            .await?;
        }
    } else {
        tracing::debug!("Nenhuma nova role para inserir para {}", user_id);
    }

    // 3. Confirma a transação
    tx.commit().await?;

    tracing::info!("✅ Roles atualizadas com sucesso para user {}", user_id);
    Ok(())
}

pub async fn update_user(
    db_pool: &SqlitePool,
    user_id_to_update: &str, // ID do utilizador a ser atualizado
    name: &str,              // Novos dados
    turma: &str,
    ano: i64,
    curso: &str,
    genero: &str,
) -> AppResult<()> {
    tracing::info!("Atualizando dados para user: {}", user_id_to_update);

    // Executa a query UPDATE na tabela 'users'
    // O trigger 'trigger_users_updated_at' atualizará automaticamente a coluna 'updated_at'
    let rows_affected = sqlx::query!(
        r#"
        UPDATE users
        SET
            name = ?1,
            turma = ?2,
            ano = ?3,
            curso = ?4,
            genero = ?5
            -- updated_at é atualizado pelo trigger
        WHERE id = ?6
        "#,
        name,
        turma,
        ano,
        curso,
        genero,
        user_id_to_update // Condição WHERE para atualizar apenas o user correto
    )
    .execute(db_pool) // Executa a query
    .await? // Propaga erro SqlxError
    .rows_affected(); // Obtém o número de linhas afetadas

    // Verifica se alguma linha foi realmente atualizada
    if rows_affected == 0 {
        // Se 0 linhas foram afetadas, significa que o user_id não foi encontrado
        tracing::warn!(
            "Falha ao atualizar dados: Utilizador '{}' não encontrado.",
            user_id_to_update
        );
        // Retorna um erro específico (poderíamos criar AppError::UserNotFound)
        // Por agora, usamos InternalServerError como placeholder
        Err(AppError::InternalServerError) // TODO: Mudar para AppError::NotFound ou similar
    } else {
        // Se 1 linha foi afetada, a atualização foi bem-sucedida
        tracing::info!("✅ Dados atualizados com sucesso para user: {}", user_id_to_update);
        Ok(())
    }
}