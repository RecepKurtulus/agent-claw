use axum::{
    Json, Router,
    extract::{Path, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use deployment::Deployment;
use openclaw_orchestrator::{
    OcOrchestrator, OcTaskDependency, RunPlanRequest,
    orchestrator::OrchestratorService,
};
use openclaw_planner::{
    CreateOcPlanRequest, OcPlan, OcPlanTask,
    planner::PlannerService,
    OcPlanner,
};
use serde::{Deserialize, Serialize};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

// ── Request / Response tipleri ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AddDependencyRequest {
    pub task_id: Uuid,
    pub depends_on_task_id: Uuid,
}

#[derive(Serialize)]
pub struct PlanDetailResponse {
    pub plan: OcPlan,
    pub tasks: Vec<OcPlanTask>,
}

// ── Handlers ───────────────────────────────────────────────────────────────

pub async fn create_plan(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateOcPlanRequest>,
) -> Result<ResponseJson<ApiResponse<openclaw_planner::CreateOcPlanResponse>>, ApiError> {
    let planner = OcPlanner::new(deployment.db().clone());
    let resp = planner
        .create_plan(req)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(ResponseJson(ApiResponse::success(resp)))
}

pub async fn list_plans(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Vec<OcPlan>>>, ApiError> {
    let planner = OcPlanner::new(deployment.db().clone());
    let plans = planner
        .list_plans(project_id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(ResponseJson(ApiResponse::success(plans)))
}

pub async fn get_plan(
    State(deployment): State<DeploymentImpl>,
    Path(plan_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<PlanDetailResponse>>, ApiError> {
    let planner = OcPlanner::new(deployment.db().clone());
    let plan = planner
        .get_plan(plan_id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?
        .ok_or_else(|| ApiError::BadRequest("Plan bulunamadı".to_string()))?;
    let tasks = planner
        .get_plan_tasks(plan_id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(ResponseJson(ApiResponse::success(PlanDetailResponse {
        plan,
        tasks,
    })))
}

pub async fn run_plan(
    State(deployment): State<DeploymentImpl>,
    Path(plan_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<openclaw_orchestrator::OcOrchestrationRun>>, ApiError> {
    let orchestrator = OcOrchestrator::new(deployment.db().clone());
    let run = orchestrator
        .run_plan(RunPlanRequest { plan_id })
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(ResponseJson(ApiResponse::success(run)))
}

pub async fn get_run(
    State(deployment): State<DeploymentImpl>,
    Path(run_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Vec<openclaw_orchestrator::OcTaskRunState>>>, ApiError> {
    let orchestrator = OcOrchestrator::new(deployment.db().clone());
    let states = orchestrator
        .get_run_states(run_id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(ResponseJson(ApiResponse::success(states)))
}

pub async fn add_dependency(
    State(deployment): State<DeploymentImpl>,
    Path(plan_id): Path<Uuid>,
    Json(req): Json<AddDependencyRequest>,
) -> Result<ResponseJson<ApiResponse<OcTaskDependency>>, ApiError> {
    let orchestrator = OcOrchestrator::new(deployment.db().clone());
    let dep = orchestrator
        .add_dependency(plan_id, req.task_id, req.depends_on_task_id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(ResponseJson(ApiResponse::success(dep)))
}

pub async fn list_dependencies(
    State(deployment): State<DeploymentImpl>,
    Path(plan_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Vec<OcTaskDependency>>>, ApiError> {
    let orchestrator = OcOrchestrator::new(deployment.db().clone());
    let deps = orchestrator
        .get_dependencies(plan_id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(ResponseJson(ApiResponse::success(deps)))
}

// ── Router ─────────────────────────────────────────────────────────────────

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        // Plan CRUD
        .route("/openclaw/plans", post(create_plan))
        .route("/openclaw/plans/project/:project_id", get(list_plans))
        .route("/openclaw/plans/:plan_id", get(get_plan))
        // Orchestration
        .route("/openclaw/plans/:plan_id/run", post(run_plan))
        .route("/openclaw/runs/:run_id", get(get_run))
        // Dependency graph
        .route("/openclaw/plans/:plan_id/dependencies", post(add_dependency))
        .route(
            "/openclaw/plans/:plan_id/dependencies",
            get(list_dependencies),
        )
        .with_state(deployment.clone())
}
