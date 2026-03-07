-- Phase 2.2: LLM planner tarafından üretilen görev alanları
-- prompt: agent'a verilecek detaylı görev açıklaması (LLM üretir)
-- depends_on_titles: JSON array, hangi task title'larına bağımlı (örn: '["DB şeması oluştur"]')

ALTER TABLE oc_plan_tasks ADD COLUMN prompt TEXT;
ALTER TABLE oc_plan_tasks ADD COLUMN depends_on_titles TEXT;
