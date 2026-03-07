// OpenClaw frontend types.
// These mirror the Rust types in crates/openclaw-planner/src/types.rs.
// Will be auto-replaced when `pnpm run generate-types` is run.

export type OcPlanStatus =
  | 'pending'
  | 'analyzing'
  | 'ready'
  | 'running'
  | 'completed'
  | 'failed';

export type OcTaskComplexity = 'low' | 'medium' | 'high';

export type OcPlan = {
  id: string;
  project_id: string;
  prompt: string;
  status: OcPlanStatus;
  codebase_context?: string;
  created_at: string;
  updated_at: string;
};

export type OcPlanTask = {
  id: string;
  plan_id: string;
  issue_id?: string;
  title: string;
  description: string;
  prompt?: string;
  estimated_complexity: OcTaskComplexity;
  depends_on: string[];
  order_index: number;
  created_at: string;
};

export type OcDuplicationWarning = {
  new_task_title: string;
  similar_task_title: string;
  similarity_score: number;
  existing_status: string;
};

export type OcCodebaseContext = {
  project_type: string;
  key_file_count: number;
  existing_task_count: number;
  summary: string;
};

export type CreateOcPlanRequest = {
  project_id: string;
  prompt: string;
  repo_paths?: string[];
};

export type CreateOcPlanResponse = {
  plan: OcPlan;
  tasks: OcPlanTask[];
  codebase_context?: OcCodebaseContext;
  duplication_warnings: OcDuplicationWarning[];
};
