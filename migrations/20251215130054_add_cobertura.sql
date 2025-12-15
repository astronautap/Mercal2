-- Add migration script here
ALTER TABLE trocas ADD COLUMN tipo TEXT DEFAULT 'Cobertura'; -- 'Cobertura' ou 'Permuta'
ALTER TABLE trocas ADD COLUMN alocacao_substituto_id TEXT; -- ID do serviço que o substituto dá em troca (se houver)