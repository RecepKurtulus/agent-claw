-- OpenClaw QA tables
-- oc_qa_runs: bir workspace'in QA çalıştırması
CREATE TABLE oc_qa_runs (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL,
    execution_process_id TEXT NOT NULL,  -- tetikleyen coding agent EP
    test_command TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending | running | passed | failed | exhausted
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_oc_qa_runs_workspace_id ON oc_qa_runs(workspace_id);
CREATE INDEX idx_oc_qa_runs_execution_process_id ON oc_qa_runs(execution_process_id);

-- oc_qa_results: her denemenin sonucu
CREATE TABLE oc_qa_results (
    id TEXT PRIMARY KEY NOT NULL,
    qa_run_id TEXT NOT NULL REFERENCES oc_qa_runs(id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL,
    exit_code INTEGER,
    output TEXT NOT NULL DEFAULT '',
    passed INTEGER NOT NULL DEFAULT 0,  -- boolean (0/1)
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_oc_qa_results_qa_run_id ON oc_qa_results(qa_run_id);
