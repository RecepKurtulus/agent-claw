use std::collections::HashSet;

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use db::DBService;
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    dependency_graph::DependencyGraph,
    types::{
        OcOrchestrationRun, OcOrchestrationRunStatus, OcTaskDependency, OcTaskRunState,
        OcTaskRunStatus, RunPlanRequest,
    },
};

#[derive(Debug, Error)]
pub enum OrchestratorError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("Döngüsel bağımlılık tespit edildi")]
    CyclicDependency,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait]
pub trait OrchestratorService: Send + Sync {
    /// İki task arasına bağımlılık ekler.
    async fn add_dependency(
        &self,
        plan_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
    ) -> Result<OcTaskDependency, OrchestratorError>;

    /// Plan için tüm bağımlılıkları getirir.
    async fn get_dependencies(
        &self,
        plan_id: Uuid,
    ) -> Result<Vec<OcTaskDependency>, OrchestratorError>;

    /// Planı çalıştırır: bağımlılık grafiğini yükler, hazır task'ları sırasıyla çalıştırır.
    async fn run_plan(&self, req: RunPlanRequest) -> Result<OcOrchestrationRun, OrchestratorError>;

    /// Bir run'ın tüm task durum bilgilerini getirir.
    async fn get_run_states(&self, run_id: Uuid) -> Result<Vec<OcTaskRunState>, OrchestratorError>;

    /// Bir task tamamlandığında çağrılır; bağımlı task'ları unblock eder.
    async fn on_task_completed(
        &self,
        run_id: Uuid,
        task_id: Uuid,
        context_summary: Option<String>,
    ) -> Result<Vec<Uuid>, OrchestratorError>;

    /// Bir task başarısız olduğunda çağrılır.
    async fn on_task_failed(&self, run_id: Uuid, task_id: Uuid) -> Result<(), OrchestratorError>;

    /// Tamamlanmış bağımlılıkların özetlerini başına ekleyerek task için tam prompt döner.
    /// `base_prompt` None ise oc_plan_tasks.prompt kullanılır.
    async fn get_prompt_for_task(
        &self,
        run_id: Uuid,
        task_id: Uuid,
        base_prompt: Option<&str>,
    ) -> Result<String, OrchestratorError>;
}

pub struct OcOrchestrator {
    db: DBService,
}

impl OcOrchestrator {
    pub fn new(db: DBService) -> Self {
        Self { db }
    }

    /// DB'den bir planın dependency graph'ını yükler.
    async fn load_graph(&self, plan_id: Uuid) -> Result<DependencyGraph, OrchestratorError> {
        // Önce tüm task'ları yükle
        let task_rows = sqlx::query("SELECT id FROM oc_plan_tasks WHERE plan_id = ?")
            .bind(plan_id.to_string())
            .fetch_all(&self.db.pool)
            .await?;

        let mut graph = DependencyGraph::new();

        for row in task_rows {
            let id: String = row.try_get("id")?;
            graph.add_task(
                id.parse()
                    .map_err(|e| OrchestratorError::Other(anyhow!("{}", e)))?,
            );
        }

        // Bağımlılıkları ekle
        let dep_rows = sqlx::query(
            "SELECT task_id, depends_on_task_id FROM oc_task_dependencies WHERE plan_id = ?",
        )
        .bind(plan_id.to_string())
        .fetch_all(&self.db.pool)
        .await?;

        for row in dep_rows {
            let task_id: String = row.try_get("task_id")?;
            let depends_on: String = row.try_get("depends_on_task_id")?;
            graph.add_dependency(
                task_id
                    .parse()
                    .map_err(|e| OrchestratorError::Other(anyhow!("{}", e)))?,
                depends_on
                    .parse()
                    .map_err(|e| OrchestratorError::Other(anyhow!("{}", e)))?,
            );
        }

        if graph.has_cycle() {
            return Err(OrchestratorError::CyclicDependency);
        }

        Ok(graph)
    }
}

#[async_trait]
impl OrchestratorService for OcOrchestrator {
    async fn add_dependency(
        &self,
        plan_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
    ) -> Result<OcTaskDependency, OrchestratorError> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO oc_task_dependencies (id, plan_id, task_id, depends_on_task_id, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(plan_id.to_string())
        .bind(task_id.to_string())
        .bind(depends_on_task_id.to_string())
        .bind(now.to_rfc3339())
        .execute(&self.db.pool)
        .await?;

        Ok(OcTaskDependency {
            id,
            plan_id,
            task_id,
            depends_on_task_id,
            created_at: now,
        })
    }

    async fn get_dependencies(
        &self,
        plan_id: Uuid,
    ) -> Result<Vec<OcTaskDependency>, OrchestratorError> {
        let rows = sqlx::query(
            "SELECT id, plan_id, task_id, depends_on_task_id, created_at
             FROM oc_task_dependencies WHERE plan_id = ?",
        )
        .bind(plan_id.to_string())
        .fetch_all(&self.db.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                let plan_id: String = row.try_get("plan_id")?;
                let task_id: String = row.try_get("task_id")?;
                let depends_on: String = row.try_get("depends_on_task_id")?;
                let created_at_str: String = row.try_get("created_at")?;

                Ok(OcTaskDependency {
                    id: id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
                    plan_id: plan_id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
                    task_id: task_id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
                    depends_on_task_id: depends_on.parse().map_err(|_| sqlx::Error::RowNotFound)?,
                    created_at: created_at_str.parse().unwrap_or_else(|_| Utc::now()),
                })
            })
            .collect()
    }

    async fn run_plan(&self, req: RunPlanRequest) -> Result<OcOrchestrationRun, OrchestratorError> {
        let graph = self.load_graph(req.plan_id).await?;
        let run_id = Uuid::new_v4();
        let now = Utc::now();

        // Run kaydını oluştur
        sqlx::query(
            "INSERT INTO oc_orchestration_runs (id, plan_id, status, started_at, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(run_id.to_string())
        .bind(req.plan_id.to_string())
        .bind(OcOrchestrationRunStatus::Running.as_str())
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.db.pool)
        .await?;

        // Tüm task'lar için başlangıç durumlarını oluştur
        let task_rows =
            sqlx::query("SELECT id FROM oc_plan_tasks WHERE plan_id = ? ORDER BY order_index ASC")
                .bind(req.plan_id.to_string())
                .fetch_all(&self.db.pool)
                .await?;

        let completed: HashSet<Uuid> = HashSet::new();

        for row in task_rows {
            let task_id_str: String = row.try_get("id")?;
            let task_id: Uuid = task_id_str
                .parse()
                .map_err(|e| OrchestratorError::Other(anyhow!("{}", e)))?;

            let blocking = graph.blocking_tasks(task_id, &completed);
            let status = if blocking.is_empty() {
                OcTaskRunStatus::Pending
            } else {
                OcTaskRunStatus::Blocked
            };

            let blocked_by_json = serde_json::to_string(
                &blocking.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
            )
            .unwrap_or_else(|_| "[]".to_string());

            let state_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO oc_task_run_state (id, run_id, task_id, status, blocked_by, created_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(state_id.to_string())
            .bind(run_id.to_string())
            .bind(task_id.to_string())
            .bind(status.as_str())
            .bind(blocked_by_json)
            .bind(now.to_rfc3339())
            .execute(&self.db.pool)
            .await?;
        }

        // Plan durumunu running'e geçir
        sqlx::query("UPDATE oc_plans SET status = 'running', updated_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(req.plan_id.to_string())
            .execute(&self.db.pool)
            .await?;

        Ok(OcOrchestrationRun {
            id: run_id,
            plan_id: req.plan_id,
            status: OcOrchestrationRunStatus::Running,
            started_at: Some(now),
            completed_at: None,
            created_at: now,
        })
    }

    async fn get_run_states(&self, run_id: Uuid) -> Result<Vec<OcTaskRunState>, OrchestratorError> {
        let rows = sqlx::query(
            "SELECT id, run_id, task_id, workspace_id, status, blocked_by, context_summary,
                    started_at, completed_at, created_at
             FROM oc_task_run_state WHERE run_id = ?",
        )
        .bind(run_id.to_string())
        .fetch_all(&self.db.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                let run_id: String = row.try_get("run_id")?;
                let task_id: String = row.try_get("task_id")?;
                let workspace_id: Option<String> = row.try_get("workspace_id")?;
                let status_str: String = row.try_get("status")?;
                let blocked_by_json: Option<String> = row.try_get("blocked_by")?;
                let context_summary: Option<String> = row.try_get("context_summary")?;
                let started_at: Option<String> = row.try_get("started_at")?;
                let completed_at: Option<String> = row.try_get("completed_at")?;
                let created_at_str: String = row.try_get("created_at")?;

                let blocked_by: Vec<Uuid> = blocked_by_json
                    .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                Ok(OcTaskRunState {
                    id: id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
                    run_id: run_id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
                    task_id: task_id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
                    workspace_id: workspace_id.and_then(|s| s.parse().ok()),
                    status: OcTaskRunStatus::try_from(status_str.as_str())
                        .unwrap_or(OcTaskRunStatus::Pending),
                    blocked_by,
                    context_summary,
                    started_at: started_at.and_then(|s| s.parse().ok()),
                    completed_at: completed_at.and_then(|s| s.parse().ok()),
                    created_at: created_at_str.parse().unwrap_or_else(|_| Utc::now()),
                })
            })
            .collect()
    }

    async fn on_task_completed(
        &self,
        run_id: Uuid,
        task_id: Uuid,
        context_summary: Option<String>,
    ) -> Result<Vec<Uuid>, OrchestratorError> {
        let now = Utc::now();

        // Bu task'ı completed olarak işaretle
        sqlx::query(
            "UPDATE oc_task_run_state
             SET status = 'completed', completed_at = ?, context_summary = ?
             WHERE run_id = ? AND task_id = ?",
        )
        .bind(now.to_rfc3339())
        .bind(&context_summary)
        .bind(run_id.to_string())
        .bind(task_id.to_string())
        .execute(&self.db.pool)
        .await?;

        // Plan ID'yi al
        let plan_id_row = sqlx::query("SELECT plan_id FROM oc_orchestration_runs WHERE id = ?")
            .bind(run_id.to_string())
            .fetch_one(&self.db.pool)
            .await?;
        let plan_id_str: String = plan_id_row.try_get("plan_id")?;
        let plan_id: Uuid = plan_id_str
            .parse()
            .map_err(|e| OrchestratorError::Other(anyhow!("{}", e)))?;

        // Güncel grafiği yükle
        let graph = self.load_graph(plan_id).await?;

        // Tüm tamamlananları topla
        let completed_rows = sqlx::query(
            "SELECT task_id FROM oc_task_run_state WHERE run_id = ? AND status = 'completed'",
        )
        .bind(run_id.to_string())
        .fetch_all(&self.db.pool)
        .await?;

        let completed: HashSet<Uuid> = completed_rows
            .into_iter()
            .filter_map(|row| {
                let s: String = row.try_get("task_id").ok()?;
                s.parse().ok()
            })
            .collect();

        // Artık hazır olan (blocked → pending) task'ları bul
        let newly_ready = graph.ready_tasks(&completed);
        let mut unblocked = Vec::new();

        for ready_task_id in newly_ready {
            let blocking = graph.blocking_tasks(ready_task_id, &completed);
            if blocking.is_empty() {
                // Daha önce blocked olan task'ı pending'e al
                let updated = sqlx::query(
                    "UPDATE oc_task_run_state SET status = 'pending', blocked_by = '[]'
                     WHERE run_id = ? AND task_id = ? AND status = 'blocked'",
                )
                .bind(run_id.to_string())
                .bind(ready_task_id.to_string())
                .execute(&self.db.pool)
                .await?;

                if updated.rows_affected() > 0 {
                    unblocked.push(ready_task_id);
                    tracing::info!(
                        task_id = %ready_task_id,
                        run_id = %run_id,
                        "Task unblocked, ready to run"
                    );
                }
            }
        }

        // Tüm task'lar tamamlandı mı?
        let pending_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM oc_task_run_state
             WHERE run_id = ? AND status NOT IN ('completed', 'failed')",
        )
        .bind(run_id.to_string())
        .fetch_one(&self.db.pool)
        .await?;

        if pending_count == 0 {
            sqlx::query(
                "UPDATE oc_orchestration_runs SET status = 'completed', completed_at = ? WHERE id = ?",
            )
            .bind(now.to_rfc3339())
            .bind(run_id.to_string())
            .execute(&self.db.pool)
            .await?;

            sqlx::query("UPDATE oc_plans SET status = 'completed', updated_at = ? WHERE id = ?")
                .bind(now.to_rfc3339())
                .bind(plan_id.to_string())
                .execute(&self.db.pool)
                .await?;
        }

        Ok(unblocked)
    }

    async fn on_task_failed(&self, run_id: Uuid, task_id: Uuid) -> Result<(), OrchestratorError> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE oc_task_run_state SET status = 'failed', completed_at = ?
             WHERE run_id = ? AND task_id = ?",
        )
        .bind(now.to_rfc3339())
        .bind(run_id.to_string())
        .bind(task_id.to_string())
        .execute(&self.db.pool)
        .await?;

        sqlx::query(
            "UPDATE oc_orchestration_runs SET status = 'failed', completed_at = ? WHERE id = ?",
        )
        .bind(now.to_rfc3339())
        .bind(run_id.to_string())
        .execute(&self.db.pool)
        .await?;

        Ok(())
    }

    async fn get_prompt_for_task(
        &self,
        run_id: Uuid,
        task_id: Uuid,
        base_prompt: Option<&str>,
    ) -> Result<String, OrchestratorError> {
        // Görevin kendi prompt'unu al (base_prompt verilmemişse DB'den çek)
        let task_prompt = if let Some(p) = base_prompt {
            p.to_string()
        } else {
            let row = sqlx::query("SELECT prompt FROM oc_plan_tasks WHERE id = ?")
                .bind(task_id.to_string())
                .fetch_optional(&self.db.pool)
                .await?;
            match row {
                Some(r) => r
                    .try_get::<Option<String>, _>("prompt")?
                    .unwrap_or_default(),
                None => String::new(),
            }
        };

        // Bağımlılıklardan tamamlanmış özetleri topla
        let dep_rows = sqlx::query(
            "SELECT pt.title, trs.context_summary
             FROM oc_task_run_state trs
             JOIN oc_plan_tasks pt ON pt.id = trs.task_id
             WHERE trs.run_id = ?
               AND trs.task_id IN (
                   SELECT depends_on_task_id
                   FROM oc_task_dependencies
                   WHERE task_id = ?
               )
               AND trs.status = 'completed'
               AND trs.context_summary IS NOT NULL
             ORDER BY trs.completed_at ASC",
        )
        .bind(run_id.to_string())
        .bind(task_id.to_string())
        .fetch_all(&self.db.pool)
        .await?;

        if dep_rows.is_empty() {
            return Ok(task_prompt);
        }

        let mut context_lines = Vec::new();
        for row in dep_rows {
            let title: String = row
                .try_get("title")
                .unwrap_or_else(|_| "Önceki adım".into());
            let summary: String = row
                .try_get::<Option<String>, _>("context_summary")
                .unwrap_or_default()
                .unwrap_or_default();
            context_lines.push(format!("- {title}: {summary}"));
        }

        let context_block = context_lines.join("\n");
        Ok(format!(
            "Önceki adımlarda yapılanlar:\n{context_block}\n\nŞimdi senin görevin:\n{task_prompt}"
        ))
    }
}
