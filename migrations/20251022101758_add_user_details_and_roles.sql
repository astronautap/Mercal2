-- migrations/YYYYMMDDHHMMSS_add_user_details_and_roles.sql

-- Adiciona novas colunas à tabela 'users'
-- Usamos ALTER TABLE ADD COLUMN; o IF NOT EXISTS não é padrão para ADD COLUMN no SQLite,
-- mas o sqlx migrate garante que isto só corre uma vez.
ALTER TABLE users ADD COLUMN turma TEXT NOT NULL DEFAULT ''; -- Adiciona 'turma'
ALTER TABLE users ADD COLUMN ano INTEGER NOT NULL DEFAULT 0;   -- Adiciona 'ano'
ALTER TABLE users ADD COLUMN curso TEXT NOT NULL DEFAULT ''; -- Adiciona 'curso' (como TEXT/String)
ALTER TABLE users ADD COLUMN genero TEXT NOT NULL DEFAULT 'M'; -- Adiciona 'genero' (M/F)

-- Adiciona 'updated_at' sem um default não-constante para compatibilidade com versões mais antigas do SQLite.
ALTER TABLE users ADD COLUMN updated_at DATETIME;
UPDATE users SET updated_at = datetime('now', 'localtime'); -- Define o valor inicial para as linhas existentes.

-- Cria a tabela 'user_roles' para associar funções aos utilizadores
CREATE TABLE IF NOT EXISTS user_roles (
    user_id TEXT NOT NULL,
    role TEXT NOT NULL COLLATE NOCASE, -- COLLATE NOCASE para tornar a role case-insensitive
    PRIMARY KEY (user_id, role),       -- Chave primária composta
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE -- Liga a 'users' e apaga roles se user for apagado
);

-- (Opcional) Atualiza o trigger 'updated_at' se ainda não existir (não deve ser necessário se já estava na migração anterior)
-- CREATE TRIGGER IF NOT EXISTS trigger_users_updated_at ... ;