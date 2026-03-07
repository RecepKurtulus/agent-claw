use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use db::DBService;
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    context::CodebaseScanner,
    dedup::{DuplicationChecker, ExistingTask},
    dependency::PlanDependencyResolver,
    llm::AnthropicLlmPlanner,
    types::{
        CreateOcPlanRequest, CreateOcPlanResponse, OcCodebaseContext, OcDuplicationWarning, OcPlan,
        OcPlanStatus, OcPlanTask, OcTaskComplexity,
    },
};

#[derive(Debug, Error)]
pub enum PlannerError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait]
pub trait PlannerService: Send + Sync {
    /// Prompt'u analiz edip plan + task listesi oluşturur (DB'ye yazar).
    async fn create_plan(
        &self,
        req: CreateOcPlanRequest,
    ) -> Result<CreateOcPlanResponse, PlannerError>;

    /// Plan ID'ye göre planı getirir.
    async fn get_plan(&self, plan_id: Uuid) -> Result<Option<OcPlan>, PlannerError>;

    /// Bir plana ait tüm task'ları getirir.
    async fn get_plan_tasks(&self, plan_id: Uuid) -> Result<Vec<OcPlanTask>, PlannerError>;

    /// Tüm planları getirir (proje bazlı).
    async fn list_plans(&self, project_id: Uuid) -> Result<Vec<OcPlan>, PlannerError>;

    /// Plan durumunu günceller.
    async fn update_plan_status(
        &self,
        plan_id: Uuid,
        status: OcPlanStatus,
    ) -> Result<(), PlannerError>;
}

pub struct OcPlanner {
    db: DBService,
}

/// Dahili görev adayı — hem LLM hem fallback çıktısı bu tipe dönüştürülür.
struct TaskCandidate {
    title: String,
    description: String,
    complexity: OcTaskComplexity,
    prompt: Option<String>,
    depends_on: Vec<String>,
}

impl OcPlanner {
    pub fn new(db: DBService) -> Self {
        Self { db }
    }

    /// Fallback: her satır → bir görev.
    fn parse_tasks_fallback(prompt: &str) -> Vec<TaskCandidate> {
        prompt
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let title = line.trim().to_string();
                let description = format!("Implement: {}", title);
                let complexity = if title.len() > 80 {
                    OcTaskComplexity::High
                } else if title.len() > 40 {
                    OcTaskComplexity::Medium
                } else {
                    OcTaskComplexity::Low
                };
                TaskCandidate {
                    title,
                    description,
                    complexity,
                    prompt: None,
                    depends_on: vec![],
                }
            })
            .collect()
    }

    /// LLM yanıtından TaskCandidate listesi üretir.
    async fn tasks_from_llm(
        user_prompt: &str,
        codebase_context: Option<&str>,
    ) -> Option<Vec<TaskCandidate>> {
        let llm = AnthropicLlmPlanner::from_env()?;
        match llm.generate_tasks(user_prompt, codebase_context).await {
            Ok(llm_tasks) => {
                let candidates = llm_tasks
                    .into_iter()
                    .map(|t| TaskCandidate {
                        complexity: t.to_complexity(),
                        title: t.title,
                        description: t.description,
                        prompt: t.prompt,
                        depends_on: t.depends_on,
                    })
                    .collect();
                Some(candidates)
            }
            Err(e) => {
                tracing::warn!(error = %e, "LLM planning failed, falling back to line parsing");
                None
            }
        }
    }

    /// Eski test uyumluluğu için korunan yardımcı.
    #[cfg(test)]
    fn parse_tasks_from_prompt(prompt: &str) -> Vec<(String, String, OcTaskComplexity)> {
        Self::parse_tasks_fallback(prompt)
            .into_iter()
            .map(|c| (c.title, c.description, c.complexity))
            .collect()
    }
}

#[async_trait]
impl PlannerService for OcPlanner {
    async fn create_plan(
        &self,
        req: CreateOcPlanRequest,
    ) -> Result<CreateOcPlanResponse, PlannerError> {
        let plan_id = Uuid::new_v4();
        let now = Utc::now();

        // Codebase taraması yap (repo_paths verilmişse)
        let (ctx_opt, ctx_summary_opt): (Option<OcCodebaseContext>, Option<String>) =
            if let Some(ref paths) = req.repo_paths {
                if !paths.is_empty() {
                    let scanner = CodebaseScanner::new(self.db.clone());
                    let ctx = scanner.scan(req.project_id, paths).await;
                    let summary = ctx.summary.clone();
                    let brief = OcCodebaseContext {
                        project_type: ctx.project_type.label(),
                        key_file_count: ctx.key_files.len(),
                        existing_task_count: ctx.existing_tasks.len(),
                        summary: summary.clone(),
                    };
                    tracing::info!(
                        project_type = %ctx.project_type.label(),
                        key_files = ctx.key_files.len(),
                        existing_tasks = ctx.existing_tasks.len(),
                        "Codebase context collected"
                    );
                    (Some(brief), Some(summary))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        // Plan kaydını oluştur (context dahil)
        sqlx::query(
            "INSERT INTO oc_plans (id, project_id, prompt, status, codebase_context, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(plan_id.to_string())
        .bind(req.project_id.to_string())
        .bind(&req.prompt)
        .bind(OcPlanStatus::Analyzing.as_str())
        .bind(&ctx_summary_opt)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.db.pool)
        .await?;

        // Task'ları üret: LLM varsa LLM, yoksa fallback
        let candidates: Vec<TaskCandidate> = {
            let llm_result = Self::tasks_from_llm(&req.prompt, ctx_summary_opt.as_deref()).await;
            llm_result.unwrap_or_else(|| {
                tracing::info!("Using fallback line-based task parser");
                Self::parse_tasks_fallback(&req.prompt)
            })
        };

        let mut tasks = Vec::new();

        for (index, candidate) in candidates.into_iter().enumerate() {
            let task_id = Uuid::new_v4();
            let depends_on_json =
                serde_json::to_string(&candidate.depends_on).unwrap_or_else(|_| "[]".to_string());

            sqlx::query(
                "INSERT INTO oc_plan_tasks
                 (id, plan_id, title, description, estimated_complexity, prompt, depends_on_titles, order_index, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(task_id.to_string())
            .bind(plan_id.to_string())
            .bind(&candidate.title)
            .bind(&candidate.description)
            .bind(candidate.complexity.as_str())
            .bind(&candidate.prompt)
            .bind(&depends_on_json)
            .bind(index as i64)
            .bind(now.to_rfc3339())
            .execute(&self.db.pool)
            .await?;

            tasks.push(OcPlanTask {
                id: task_id,
                plan_id,
                issue_id: None,
                title: candidate.title,
                description: candidate.description,
                prompt: candidate.prompt,
                estimated_complexity: candidate.complexity,
                depends_on: candidate.depends_on,
                order_index: index as i64,
                created_at: now,
            });
        }

        // ── Bağımlılık çözümleme ve topolojik sıralama ────────────────────────
        match PlanDependencyResolver::resolve(&tasks) {
            Ok(resolved) => {
                // order_index'i topological sıralamaya göre güncelle
                for (new_index, &task_id) in resolved.ordered_task_ids.iter().enumerate() {
                    sqlx::query("UPDATE oc_plan_tasks SET order_index = ? WHERE id = ?")
                        .bind(new_index as i64)
                        .bind(task_id.to_string())
                        .execute(&self.db.pool)
                        .await?;

                    // tasks vektöründeki order_index'i de güncelle
                    if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id) {
                        t.order_index = new_index as i64;
                    }
                }

                // Bağımlılık kenarlarını oc_task_dependencies'e yaz
                for (task_id, dep_id) in &resolved.edges {
                    let edge_id = Uuid::new_v4();
                    sqlx::query(
                        "INSERT OR IGNORE INTO oc_task_dependencies
                         (id, plan_id, task_id, depends_on_task_id, created_at)
                         VALUES (?, ?, ?, ?, ?)",
                    )
                    .bind(edge_id.to_string())
                    .bind(plan_id.to_string())
                    .bind(task_id.to_string())
                    .bind(dep_id.to_string())
                    .bind(now.to_rfc3339())
                    .execute(&self.db.pool)
                    .await?;
                }

                tracing::info!(
                    tasks = tasks.len(),
                    edges = resolved.edges.len(),
                    "Dependency graph resolved and persisted"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "Bağımlılık grafiği çözümlenemedi, sıralama değişmedi");
            }
        }

        // tasks vektörünü order_index'e göre sırala
        tasks.sort_by_key(|t| t.order_index);

        // Status'u ready'e geçir
        sqlx::query("UPDATE oc_plans SET status = ?, updated_at = ? WHERE id = ?")
            .bind(OcPlanStatus::Ready.as_str())
            .bind(Utc::now().to_rfc3339())
            .bind(plan_id.to_string())
            .execute(&self.db.pool)
            .await?;

        // ── Duplikasyon kontrolü ───────────────────────────────────────────────
        let duplication_warnings: Vec<OcDuplicationWarning> = {
            // Açık Kanban task'larını çek (cancelled + done hariç)
            let existing_rows = sqlx::query(
                "SELECT title, status FROM tasks
                 WHERE project_id = ? AND status NOT IN ('cancelled', 'done')
                 ORDER BY created_at DESC LIMIT 50",
            )
            .bind(req.project_id.to_string())
            .fetch_all(&self.db.pool)
            .await
            .unwrap_or_default();

            let existing: Vec<ExistingTask> = existing_rows
                .into_iter()
                .filter_map(|row| {
                    let title: String = row.try_get("title").ok()?;
                    let status: String = row.try_get("status").ok()?;
                    Some(ExistingTask { title, status })
                })
                .collect();

            if existing.is_empty() {
                vec![]
            } else {
                let warnings = DuplicationChecker::check(&tasks, &existing, 0.30);
                if !warnings.is_empty() {
                    tracing::warn!(
                        count = warnings.len(),
                        "Duplikasyon uyarısı: yeni task'lar mevcut issue'larla örtüşüyor"
                    );
                }
                warnings
            }
        };

        let plan = OcPlan {
            id: plan_id,
            project_id: req.project_id,
            prompt: req.prompt,
            status: OcPlanStatus::Ready,
            codebase_context: ctx_summary_opt,
            created_at: now,
            updated_at: Utc::now(),
        };

        Ok(CreateOcPlanResponse {
            plan,
            tasks,
            codebase_context: ctx_opt,
            duplication_warnings,
        })
    }

    async fn get_plan(&self, plan_id: Uuid) -> Result<Option<OcPlan>, PlannerError> {
        let row = sqlx::query(
            "SELECT id, project_id, prompt, status, codebase_context, created_at, updated_at FROM oc_plans WHERE id = ?",
        )
        .bind(plan_id.to_string())
        .fetch_optional(&self.db.pool)
        .await?;

        let Some(row) = row else { return Ok(None) };

        let id: String = row.try_get("id")?;
        let project_id: String = row.try_get("project_id")?;
        let status_str: String = row.try_get("status")?;
        let created_at_str: String = row.try_get("created_at")?;
        let updated_at_str: String = row.try_get("updated_at")?;
        let codebase_context: Option<String> = row.try_get("codebase_context").ok().flatten();

        Ok(Some(OcPlan {
            id: id
                .parse()
                .map_err(|e| PlannerError::Other(anyhow!("{}", e)))?,
            project_id: project_id
                .parse()
                .map_err(|e| PlannerError::Other(anyhow!("{}", e)))?,
            prompt: row.try_get("prompt")?,
            status: OcPlanStatus::try_from(status_str.as_str()).map_err(PlannerError::Other)?,
            codebase_context,
            created_at: created_at_str
                .parse()
                .map_err(|e| PlannerError::Other(anyhow!("{}", e)))?,
            updated_at: updated_at_str
                .parse()
                .map_err(|e| PlannerError::Other(anyhow!("{}", e)))?,
        }))
    }

    async fn get_plan_tasks(&self, plan_id: Uuid) -> Result<Vec<OcPlanTask>, PlannerError> {
        let rows = sqlx::query(
            "SELECT id, plan_id, issue_id, title, description, estimated_complexity,
                    prompt, depends_on_titles, order_index, created_at
             FROM oc_plan_tasks WHERE plan_id = ? ORDER BY order_index ASC",
        )
        .bind(plan_id.to_string())
        .fetch_all(&self.db.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                let plan_id: String = row.try_get("plan_id")?;
                let issue_id: Option<String> = row.try_get("issue_id")?;
                let complexity_str: String = row.try_get("estimated_complexity")?;
                let created_at_str: String = row.try_get("created_at")?;
                let depends_on_json: Option<String> =
                    row.try_get("depends_on_titles").ok().flatten();
                let depends_on: Vec<String> = depends_on_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();

                Ok(OcPlanTask {
                    id: id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
                    plan_id: plan_id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
                    issue_id: issue_id.and_then(|s| s.parse().ok()),
                    title: row.try_get("title")?,
                    description: row.try_get("description")?,
                    prompt: row.try_get("prompt").ok().flatten(),
                    estimated_complexity: OcTaskComplexity::try_from(complexity_str.as_str())
                        .unwrap_or(OcTaskComplexity::Medium),
                    depends_on,
                    order_index: row.try_get("order_index")?,
                    created_at: created_at_str.parse().unwrap_or_else(|_| Utc::now()),
                })
            })
            .collect()
    }

    async fn list_plans(&self, project_id: Uuid) -> Result<Vec<OcPlan>, PlannerError> {
        let rows = sqlx::query(
            "SELECT id, project_id, prompt, status, codebase_context, created_at, updated_at
             FROM oc_plans WHERE project_id = ? ORDER BY created_at DESC",
        )
        .bind(project_id.to_string())
        .fetch_all(&self.db.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                let project_id: String = row.try_get("project_id")?;
                let status_str: String = row.try_get("status")?;
                let created_at_str: String = row.try_get("created_at")?;
                let updated_at_str: String = row.try_get("updated_at")?;
                let codebase_context: Option<String> =
                    row.try_get("codebase_context").ok().flatten();

                Ok(OcPlan {
                    id: id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
                    project_id: project_id.parse().map_err(|_| sqlx::Error::RowNotFound)?,
                    prompt: row.try_get("prompt")?,
                    status: OcPlanStatus::try_from(status_str.as_str())
                        .unwrap_or(OcPlanStatus::Pending),
                    codebase_context,
                    created_at: created_at_str.parse().unwrap_or_else(|_| Utc::now()),
                    updated_at: updated_at_str.parse().unwrap_or_else(|_| Utc::now()),
                })
            })
            .collect()
    }

    async fn update_plan_status(
        &self,
        plan_id: Uuid,
        status: OcPlanStatus,
    ) -> Result<(), PlannerError> {
        sqlx::query("UPDATE oc_plans SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(Utc::now().to_rfc3339())
            .bind(plan_id.to_string())
            .execute(&self.db.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tasks_from_prompt() {
        let prompt = "Kullanıcı giriş sistemi yaz\nAPI endpoint'lerini ekle\nTestleri yaz";
        let tasks = OcPlanner::parse_tasks_from_prompt(prompt);
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].0, "Kullanıcı giriş sistemi yaz");
    }

    #[test]
    fn test_parse_tasks_empty_lines_filtered() {
        let prompt = "Task 1\n\n\nTask 2\n";
        let tasks = OcPlanner::parse_tasks_from_prompt(prompt);
        assert_eq!(tasks.len(), 2);
    }
}
