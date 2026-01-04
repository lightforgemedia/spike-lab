use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use orchestrator_core::api::{
    ClaimRequest, ClaimResponse, CompleteRequest, CompleteResponse, DemoEnqueueRequest,
    DemoEnqueueResponse,
};

use crate::service::OrchestratorService;

#[derive(Clone)]
pub struct AppState {
    svc: Arc<OrchestratorService>,
}

pub fn router(svc: Arc<OrchestratorService>) -> Router {
    let state = AppState { svc };
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/demo/enqueue", post(demo_enqueue))
        .route("/v1/agent/claim", post(agent_claim))
        .route("/v1/agent/complete", post(agent_complete))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn demo_enqueue(
    State(st): State<AppState>,
    Json(req): Json<DemoEnqueueRequest>,
) -> Result<Json<DemoEnqueueResponse>, AppError> {
    let (intent_id, run_id) = st.svc.demo_enqueue(req).await?;
    Ok(Json(DemoEnqueueResponse { intent_id, run_id }))
}

async fn agent_claim(
    State(st): State<AppState>,
    Json(req): Json<ClaimRequest>,
) -> Result<Json<ClaimResponse>, AppError> {
    let lease = st.svc.claim_job(req.agent_id).await?;
    Ok(Json(ClaimResponse { lease }))
}

async fn agent_complete(
    State(st): State<AppState>,
    Json(req): Json<CompleteRequest>,
) -> Result<Json<CompleteResponse>, AppError> {
    let res = st
        .svc
        .complete_job(req.agent_id, req.job_id, req.lease_token, req.result)
        .await?;
    Ok(Json(res))
}

#[derive(Debug)]
pub struct AppError(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(value: E) -> Self {
        Self(value.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!(error = %self.0, "request failed");
        let body = Json(serde_json::json!({
            "error": self.0.to_string()
        }));
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}
