-- OpenClaw Planner tables
-- oc_plans: user'ın bir prompt verdiği planlama oturumları
CREATE TABLE oc_plans (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    prompt TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending | analyzing | ready | running | completed | failed
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- oc_plan_tasks: planner'ın ürettiği görevler (issue'larla 1:1 eşleşebilir)
CREATE TABLE oc_plan_tasks (
    id TEXT PRIMARY KEY NOT NULL,
    plan_id TEXT NOT NULL REFERENCES oc_plans(id) ON DELETE CASCADE,
    issue_id TEXT,  -- mevcut bir issue'ya bağlanabilir (nullable)
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    estimated_complexity TEXT NOT NULL DEFAULT 'medium',  -- low | medium | high
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_oc_plan_tasks_plan_id ON oc_plan_tasks(plan_id);
