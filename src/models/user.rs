use chrono::NaiveDateTime;
// src/models/user.rs
// use chrono::NaiveDateTime; // Remover esta linha se não usar mais NaiveDateTime aqui
use serde::Deserialize;
use sqlx::FromRow;

// Representa um utilizador lido da tabela 'users'
#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: String,
    pub password_hash: String,
    pub name: String,
    pub turma: String,
    pub ano: i64, // SQLite INTEGER -> i64
    pub curso: String,
    pub genero: String, // "M" ou "F"
    pub updated_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
}

// Struct para dados do formulário de login
#[derive(Debug, Deserialize)]
pub struct LoginForm {
    #[serde(rename = "username")] // Mapeia do HTML 'username'
    pub id: String,               // Para o campo 'id' do User/DB
    pub password: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserRole {
    pub user_id: String,
    pub role: String,
}