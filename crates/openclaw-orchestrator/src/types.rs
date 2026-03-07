use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OcTaskDependency {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub task_id: Uuid,
    pub depends_on_task_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OcOrchestrationRun {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub status: OcOrchestrationRunStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
pub enum OcOrchestrationRunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl OcOrchestrationRunStatus {
    pub fn as_str(&self) -> &str {
        match self {
            OcOrchestrationRunStatus::Pending => "pending",
            OcOrchestrationRunStatus::Running => "running",
            OcOrchestrationRunStatus::Completed => "completed",
            OcOrchestrationRunStatus::Failed => "failed",
            OcOrchestrationRunStatus::Cancelled => "cancelled",
        }
    }
}

impl TryFrom<&str> for OcOrchestrationRunStatus {
    type Error = anyhow::Error;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "pending" => Ok(OcOrchestrationRunStatus::Pending),
            "running" => Ok(OcOrchestrationRunStatus::Running),
            "completed" => Ok(OcOrchestrationRunStatus::Completed),
            "failed" => Ok(OcOrchestrationRunStatus::Failed),
            "cancelled" => Ok(OcOrchestrationRunStatus::Cancelled),
            other => Err(anyhow::anyhow!(
                "Unknown OcOrchestrationRunStatus: {}",
                other
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OcTaskRunState {
    pub id: Uuid,
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub status: OcTaskRunStatus,
    /// Bu task'ı bloklayan task ID'leri
    pub blocked_by: Vec<Uuid>,
    /// Tamamlandığında üretilen özet — bir sonraki agent'ın prompt'una eklenir
    pub context_summary: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
pub enum OcTaskRunStatus {
    Pending,
    Blocked,
    Running,
    Completed,
    Failed,
}

impl OcTaskRunStatus {
    pub fn as_str(&self) -> &str {
        match self {
            OcTaskRunStatus::Pending => "pending",
            OcTaskRunStatus::Blocked => "blocked",
            OcTaskRunStatus::Running => "running",
            OcTaskRunStatus::Completed => "completed",
            OcTaskRunStatus::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for OcTaskRunStatus {
    type Error = anyhow::Error;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "pending" => Ok(OcTaskRunStatus::Pending),
            "blocked" => Ok(OcTaskRunStatus::Blocked),
            "running" => Ok(OcTaskRunStatus::Running),
            "completed" => Ok(OcTaskRunStatus::Completed),
            "failed" => Ok(OcTaskRunStatus::Failed),
            other => Err(anyhow::anyhow!("Unknown OcTaskRunStatus: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RunPlanRequest {
    pub plan_id: Uuid,
}
