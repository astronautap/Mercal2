-- Adiciona contadores e saldo de punição à tabela de usuários existente
-- Usamos 'IF NOT EXISTS' para evitar erros se rodar múltiplas vezes
ALTER TABLE users ADD COLUMN servicos_rn INTEGER DEFAULT 0; -- Contador Rotina Normal (Preta)
ALTER TABLE users ADD COLUMN servicos_rd INTEGER DEFAULT 0; -- Contador Rotina Domingo (Vermelha)
ALTER TABLE users ADD COLUMN saldo_punicoes INTEGER DEFAULT 0; -- Quantos serviços de punição deve

-- 1. Tabela de Postos
-- 'turmas_permitidas' será guardado como texto "1,2,3" para simplificar no SQLite
CREATE TABLE postos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    nome TEXT NOT NULL,
    genero_restricao TEXT DEFAULT 'Misto', -- 'M', 'F', 'Misto'
    turmas_permitidas TEXT NOT NULL,       -- Ex: "1,2"
    peso INTEGER DEFAULT 1                 -- 1=Normal, 2=Domingo/Feriado (informativo)
);

-- 2. Tabela de Indisponibilidades (Baixas médicas, dispensas)
CREATE TABLE indisponibilidades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    data_inicio TEXT NOT NULL, -- YYYY-MM-DD
    data_fim TEXT NOT NULL,    -- YYYY-MM-DD
    motivo TEXT,
    FOREIGN KEY(user_id) REFERENCES users(id)
);

-- 3. Tabela de Escalas (Define se o dia é RN ou RD)
CREATE TABLE escalas (
    data TEXT PRIMARY KEY,    -- YYYY-MM-DD
    tipo_rotina TEXT NOT NULL -- 'RN' ou 'RD'
);

-- 4. Tabela de Alocações (Quem faz o quê e quando)
CREATE TABLE alocacoes (
    id TEXT PRIMARY KEY NOT NULL, -- UUID
    user_id TEXT NOT NULL,
    posto_id INTEGER NOT NULL,
    data TEXT NOT NULL,           -- YYYY-MM-DD
    is_punicao BOOLEAN DEFAULT 0, -- 0=False, 1=True
    
    FOREIGN KEY(user_id) REFERENCES users(id),
    FOREIGN KEY(posto_id) REFERENCES postos(id),
    FOREIGN KEY(data) REFERENCES escalas(data),

    -- REGRA DE OURO: Um militar não pode ter dois registos no mesmo dia
    UNIQUE(user_id, data)
);

-- 5. Tabela de Trocas
CREATE TABLE trocas (
    id TEXT PRIMARY KEY NOT NULL, -- UUID
    solicitante_id TEXT NOT NULL,
    substituto_id TEXT NOT NULL,
    alocacao_id TEXT NOT NULL,
    status TEXT DEFAULT 'Pendente', -- 'Pendente', 'Aprovada', 'Recusada'
    criado_em TEXT DEFAULT (datetime('now')),
    data_resposta TEXT,
    
    FOREIGN KEY(solicitante_id) REFERENCES users(id),
    FOREIGN KEY(substituto_id) REFERENCES users(id),
    FOREIGN KEY(alocacao_id) REFERENCES alocacoes(id)
);