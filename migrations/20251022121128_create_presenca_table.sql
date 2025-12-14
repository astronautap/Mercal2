-- migrations/YYYYMMDDHHMMSS_create_presenca_table.sql

-- Cria a tabela 'presenca' para guardar o último estado de entrada/saída
CREATE TABLE IF NOT EXISTS presenca (
    user_id TEXT PRIMARY KEY NOT NULL, -- Chave estrangeira para a tabela 'users'
    -- Guardar data/hora como TEXT no formato ISO 8601 (RFC3339) é recomendado para SQLite
    ultima_saida TEXT,                 -- Ex: '2025-10-22T10:30:00.123-03:00' ou NULL
    ultimo_retorno TEXT,               -- Ex: '2025-10-22T18:05:15.456-03:00' ou NULL
    usuario_saida TEXT,                -- ID do operador que marcou a última saída
    usuario_retorno TEXT,              -- ID do operador que marcou o último retorno
    -- Garante que user_id existe na tabela users e apaga esta entrada se o user for apagado
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);