-- Adiciona a tabela de Dívidas
CREATE TABLE IF NOT EXISTS dividas (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    devedor_id TEXT NOT NULL,
    credor_id TEXT NOT NULL,
    origem_troca_id TEXT,
    status TEXT DEFAULT 'PENDENTE', -- PENDENTE ou PAGA
    criado_em TEXT DEFAULT CURRENT_TIMESTAMP,
    data_pagamento TEXT,
    FOREIGN KEY(devedor_id) REFERENCES users(id),
    FOREIGN KEY(credor_id) REFERENCES users(id)
);

-- Adiciona a coluna de Tags na tabela Alocacoes
-- (SQLite suporta ADD COLUMN de forma nativa e segura)
ALTER TABLE alocacoes ADD COLUMN tag TEXT DEFAULT NULL;