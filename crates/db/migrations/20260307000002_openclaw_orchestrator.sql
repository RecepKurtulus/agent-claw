-- OpenClaw Orchestrator tables
-- oc_task_dependencies: görevler arası bağımlılık grafı (DAG)
CREATE TABLE oc_task_dependencies (
    id TEXT PRIMARY KEY NOT NULL,
    plan_id TEXT NOT NULL REFERENCES oc_plans(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES oc_plan_tasks(id) ON DELETE CASCADE,
    depends_on_task_id TEXT NOT NULL REFERENCES oc_plan_tasks(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(task_id, depends_on_task_id)
);

CREATE INDEX idx_oc_task_deps_task_id ON oc_task_dependencies(task_id);
CREATE INDEX idx_oc_task_deps_depends_on ON oc_task_dependencies(depends_on_task_id);

-- oc_orchestration_runs: bir planın çalıştırılması
CREATE TABLE oc_orchestration_runs (
    id TEXT PRIMARY KEY NOT NULL,
    plan_id TEXT NOT NULL REFERENCES oc_plans(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending | running | completed | failed | cancelled
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_oc_runs_plan_id ON oc_orchestration_runs(plan_id);

-- oc_task_run_state: her görevin bu run içindeki durumu
CREATE TABLE oc_task_run_state (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES oc_orchestration_runs(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES oc_plan_tasks(id) ON DELETE CASCADE,
    workspace_id TEXT,  -- oluşturulduğunda workspace_id set edilir
    status TEXT NOT NULL DEFAULT 'pending',  -- pending | blocked | running | completed | failed
    blocked_by TEXT,   -- JSON array of blocking task IDs
    context_summary TEXT,  -- tamamlandığında üretilen özet (sonraki ajanın prompt'una eklenir)
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(run_id, task_id)
);

CREATE INDEX idx_oc_task_run_state_run_id ON oc_task_run_state(run_id);
CREATE INDEX idx_oc_task_run_state_workspace_id ON oc_task_run_state(workspace_id);
