-- migrations/YYYYMMDDHHMMSS_create_temporary_roles_table.sql

-- Cria a tabela para guardar as roles temporárias atribuídas pela escala
CREATE TABLE IF NOT EXISTS user_temporary_roles (
    id INTEGER PRIMARY KEY AUTOINCREMENT, -- Chave primária simples para cada atribuição
    user_id TEXT NOT NULL,                -- ID do utilizador (chave estrangeira)
    role TEXT NOT NULL COLLATE NOCASE,    -- Nome da role/posto (ex: "policia")
    -- Guardar data/hora como TEXT ISO 8601 (RFC3339) para clareza com timezone
    start_datetime TEXT NOT NULL,         -- Data/hora de início da validade da role
    end_datetime TEXT NOT NULL,           -- Data/hora de fim da validade da role

    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE -- Liga à tabela users
);

-- Cria índices para otimizar as consultas de verificação de permissão
CREATE INDEX IF NOT EXISTS idx_user_temporary_roles_user_role ON user_temporary_roles (user_id, role);
CREATE INDEX IF NOT EXISTS idx_user_temporary_roles_datetime ON user_temporary_roles (start_datetime, end_datetime);