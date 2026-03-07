-- Codebase context column for OpenClaw planner
-- Tarama sonrası LLM'e verilecek proje bağlamı burada saklanır

ALTER TABLE oc_plans ADD COLUMN codebase_context TEXT;
