//! 请求执行器骨架
//!
//! 当前只实现单阶段执行器，保持现有兼容行为。
//! 后续 two-phase/native-like 流程可在此基础上扩展。

use std::sync::Arc;

use axum::{body::Body, http::{StatusCode, header}, response::{IntoResponse, Response}};

use crate::kiro::provider::KiroProvider;

use super::planner::{ExecutionMode, RequestPlan};
use super::stream::{BufferedStreamContext, StreamContext};
use super::handlers::{create_buffered_sse_stream, create_sse_stream, map_provider_error};

pub enum StreamMode {
    Direct,
    Buffered,
}

pub struct StreamExecutionInput<'a> {
    pub request_body: &'a str,
    pub model: &'a str,
    pub input_tokens: i32,
    pub thinking_enabled: bool,
    pub stream_mode: StreamMode,
}

pub struct SinglePhaseExecutor {
    provider: Arc<KiroProvider>,
}

impl SinglePhaseExecutor {
    pub fn new(provider: Arc<KiroProvider>) -> Self {
        Self { provider }
    }

    pub async fn execute_stream(
        &self,
        plan: &RequestPlan,
        input: StreamExecutionInput<'_>,
    ) -> Response {
        debug_assert!(matches!(plan.mode, ExecutionMode::SinglePhase));

        let response = match self.provider.call_api_stream(input.request_body).await {
            Ok(resp) => resp,
            Err(e) => return map_provider_error(e),
        };

        let body = match input.stream_mode {
            StreamMode::Direct => {
                let mut ctx = StreamContext::new_with_thinking(
                    input.model,
                    input.input_tokens,
                    input.thinking_enabled,
                );
                let initial_events = ctx.generate_initial_events();
                Body::from_stream(create_sse_stream(response, ctx, initial_events))
            }
            StreamMode::Buffered => {
                let ctx = BufferedStreamContext::new(
                    input.model,
                    input.input_tokens,
                    input.thinking_enabled,
                );
                Body::from_stream(create_buffered_sse_stream(response, ctx))
            }
        };

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::CONNECTION, "keep-alive")
            .body(body)
            .unwrap()
    }

    pub async fn execute_non_stream(
        &self,
        plan: &RequestPlan,
        request_body: &str,
        model: &str,
        input_tokens: i32,
    ) -> Response {
        debug_assert!(matches!(plan.mode, ExecutionMode::SinglePhase));
        super::handlers::handle_non_stream_request(self.provider.clone(), request_body, model, input_tokens).await
    }
}
