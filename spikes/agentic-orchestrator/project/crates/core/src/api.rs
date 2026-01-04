use serde::{Deserialize, Serialize};

use crate::model::{ExecBlockResult, JobLease};

/// Demo: enqueue a run for a project path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoEnqueueRequest {
    pub project_path: String,
    pub description: String,
}

/// Demo enqueue response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoEnqueueResponse {
    pub intent_id: String,
    pub run_id: String,
}

/// Agent claim request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimRequest {
    pub agent_id: String,
}

/// Agent claim response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimResponse {
    pub lease: Option<JobLease>,
}

/// Agent complete request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteRequest {
    pub agent_id: String,
    pub job_id: String,
    pub lease_token: String,
    pub result: ExecBlockResult,
}

/// Agent complete response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteResponse {
    pub ok: bool,
    pub message: Option<String>,
}
