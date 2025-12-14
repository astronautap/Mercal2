-- migrations/YYYYMMDDHHMMSS_create_users_table.sql

-- Cria a tabela 'users' se ela ainda não existir.
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,    -- ID do utilizador (ex: "1001"), chave primária
    password_hash TEXT NOT NULL,     -- Hash da senha (IMPORTANTE: nunca guardar a senha original)
    name TEXT NOT NULL,              -- Nome do utilizador
    created_at DATETIME DEFAULT (datetime('now','localtime')) -- Data/hora de criação (automático)
);

-- Tabela para sessões (usada por tower-sessions com backend sqlx)
CREATE TABLE IF NOT EXISTS sessions(
    id TEXT PRIMARY KEY NOT NULL,
    data BLOB NOT NULL, -- Guarda os dados serializados da sessão
    expiry_date INTEGER NOT NULL -- Guarda o timestamp Unix de expiração
);
-- Índice na coluna de expiração
CREATE INDEX IF NOT EXISTS sessions_expiry_idx ON sessions (expiry_date);