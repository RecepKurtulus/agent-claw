use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OcPlan {
    pub id: Uuid,
    pub project_id: Uuid,
    pub prompt: String,
    pub status: OcPlanStatus,
    /// Codebase taramasından üretilen bağlam özeti (LLM'e verilir).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub codebase_context: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
pub enum OcPlanStatus {
    Pending,
    Analyzing,
    Ready,
    Running,
    Completed,
    Failed,
}

impl OcPlanStatus {
    pub fn as_str(&self) -> &str {
        match self {
            OcPlanStatus::Pending => "pending",
            OcPlanStatus::Analyzing => "analyzing",
            OcPlanStatus::Ready => "ready",
            OcPlanStatus::Running => "running",
            OcPlanStatus::Completed => "completed",
            OcPlanStatus::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for OcPlanStatus {
    type Error = anyhow::Error;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "pending" => Ok(OcPlanStatus::Pending),
            "analyzing" => Ok(OcPlanStatus::Analyzing),
            "ready" => Ok(OcPlanStatus::Ready),
            "running" => Ok(OcPlanStatus::Running),
            "completed" => Ok(OcPlanStatus::Completed),
            "failed" => Ok(OcPlanStatus::Failed),
            other => Err(anyhow::anyhow!("Unknown OcPlanStatus: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OcPlanTask {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub issue_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    /// LLM'in ürettiği detaylı agent prompt'u.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prompt: Option<String>,
    pub estimated_complexity: OcTaskComplexity,
    /// Bu task'tan önce tamamlanması gereken task başlıkları.
    pub depends_on: Vec<String>,
    pub order_index: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
pub enum OcTaskComplexity {
    Low,
    Medium,
    High,
}

impl OcTaskComplexity {
    pub fn as_str(&self) -> &str {
        match self {
            OcTaskComplexity::Low => "low",
            OcTaskComplexity::Medium => "medium",
            OcTaskComplexity::High => "high",
        }
    }
}

impl TryFrom<&str> for OcTaskComplexity {
    type Error = anyhow::Error;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "low" => Ok(OcTaskComplexity::Low),
            "medium" => Ok(OcTaskComplexity::Medium),
            "high" => Ok(OcTaskComplexity::High),
            other => Err(anyhow::anyhow!("Unknown OcTaskComplexity: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateOcPlanRequest {
    pub project_id: Uuid,
    pub prompt: String,
    /// Codebase taraması için repo dizin yolları (opsiyonel).
    /// Verilirse planner otomatik proje tipini tespit eder ve bağlam toplar.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub repo_paths: Option<Vec<String>>,
}

/// Phase 2.1 taramasından dönen özet (response'a eklenir).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OcCodebaseContext {
    pub project_type: String,
    pub key_file_count: usize,
    pub existing_task_count: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateOcPlanResponse {
    pub plan: OcPlan,
    pub tasks: Vec<OcPlanTask>,
    /// Codebase taraması yapıldıysa özet bilgisi.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub codebase_context: Option<OcCodebaseContext>,
}
