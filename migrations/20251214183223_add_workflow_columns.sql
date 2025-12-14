-- Adiciona o status à tabela escalas. Default é 'Rascunho' (Prévia).
-- Valores possíveis: 'Rascunho', 'Publicada'
ALTER TABLE escalas ADD COLUMN status TEXT DEFAULT 'Rascunho';

-- Adiciona o motivo à tabela trocas (para o utilizador justificar)
ALTER TABLE trocas ADD COLUMN motivo TEXT;