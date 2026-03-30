//! 请求执行器骨架
//!
//! 当前只实现单阶段执行器，保持现有兼容行为。
//! 后续 two-phase/native-like 流程可在此基础上扩展。

use std::sync::Arc;

use axum::{
    body::Body,
    http::{StatusCode, header},
    response::Response,
};

use crate::kiro::provider::KiroProvider;

use super::handlers::{
    build_non_stream_response_from_upstream, create_buffered_sse_stream, create_sse_stream,
    map_provider_error,
};
use super::planner::{ExecutionMode, PhaseKind, RequestPlan};
use super::stream::{BufferedStreamContext, StreamContext};

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

        let call_ctx = match self
            .provider
            .token_manager()
            .acquire_context(Some(input.model))
            .await
        {
            Ok(ctx) => ctx,
            Err(e) => return map_provider_error(e),
        };
        let request_body = match attach_profile_arn(
            input.request_body,
            call_ctx.credentials.profile_arn.as_deref(),
        ) {
            Ok(body) => body,
            Err(e) => return map_provider_error(e.into()),
        };
        let response = match self
            .provider
            .call_api_stream_with_context(&call_ctx, &request_body)
            .await
        {
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
        let call_ctx = match self
            .provider
            .token_manager()
            .acquire_context(Some(model))
            .await
        {
            Ok(ctx) => ctx,
            Err(e) => return map_provider_error(e),
        };
        let request_body =
            match attach_profile_arn(request_body, call_ctx.credentials.profile_arn.as_deref()) {
                Ok(body) => body,
                Err(e) => return map_provider_error(e.into()),
            };
        let response = match self
            .provider
            .call_api_with_context(&call_ctx, &request_body)
            .await
        {
            Ok(resp) => resp,
            Err(e) => return map_provider_error(e),
        };
        build_non_stream_response_from_upstream(response, model, input_tokens).await
    }
}

pub struct TwoPhaseExecutor {
    provider: Arc<KiroProvider>,
}

impl TwoPhaseExecutor {
    pub fn new(provider: Arc<KiroProvider>) -> Self {
        Self { provider }
    }

    pub async fn execute_stream(
        &self,
        plan: &RequestPlan,
        input: StreamExecutionInput<'_>,
    ) -> Response {
        debug_assert!(matches!(plan.mode, ExecutionMode::TwoPhaseNativeLike));

        let main_phase = plan
            .phases
            .iter()
            .find(|p| matches!(p.phase, PhaseKind::MainModel))
            .unwrap_or_else(|| &plan.phases[0]);

        let call_ctx = match self
            .provider
            .token_manager()
            .acquire_context(Some(&main_phase.model_id))
            .await
        {
            Ok(ctx) => ctx,
            Err(e) => return map_provider_error(e),
        };

        let main_request_body = match attach_profile_arn(
            input.request_body,
            call_ctx.credentials.profile_arn.as_deref(),
        ) {
            Ok(body) => body,
            Err(e) => return map_provider_error(e.into()),
        };

        if let Ok(preflight_body_raw) = build_preflight_request_body(input.request_body, plan) {
            let preflight_body = attach_profile_arn(
                &preflight_body_raw,
                call_ctx.credentials.profile_arn.as_deref(),
            )
            .unwrap_or(preflight_body_raw);
            tracing::debug!(
                conversation_id = %plan.identity.conversation_id,
                credential_id = call_ctx.id,
                preflight_model = "simple-task",
                main_model = %main_phase.model_id,
                "Executing preflight phase"
            );
            match self
                .provider
                .call_api_stream_with_context(&call_ctx, &preflight_body)
                .await
            {
                Ok(resp) => {
                    if let Err(err) = resp.bytes().await {
                        tracing::warn!("preflight response consume failed: {}", err);
                    }
                }
                Err(err) => {
                    if should_abort_on_preflight_error(&err) {
                        tracing::warn!(
                            "preflight phase failed with credential-scoped error, aborting main phase: {}",
                            err
                        );
                        return map_provider_error(err);
                    }
                    tracing::warn!(
                        "preflight phase failed, continuing with main phase: {}",
                        err
                    );
                }
            }
        }

        let response = match self
            .provider
            .call_api_stream_with_context(&call_ctx, &main_request_body)
            .await
        {
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
        debug_assert!(matches!(plan.mode, ExecutionMode::TwoPhaseNativeLike));

        let main_phase = plan
            .phases
            .iter()
            .find(|p| matches!(p.phase, PhaseKind::MainModel))
            .unwrap_or_else(|| &plan.phases[0]);

        let call_ctx = match self
            .provider
            .token_manager()
            .acquire_context(Some(&main_phase.model_id))
            .await
        {
            Ok(ctx) => ctx,
            Err(e) => return map_provider_error(e),
        };

        let main_request_body =
            match attach_profile_arn(request_body, call_ctx.credentials.profile_arn.as_deref()) {
                Ok(body) => body,
                Err(e) => return map_provider_error(e.into()),
            };

        if let Ok(preflight_body_raw) = build_preflight_request_body(request_body, plan) {
            let preflight_body = attach_profile_arn(
                &preflight_body_raw,
                call_ctx.credentials.profile_arn.as_deref(),
            )
            .unwrap_or(preflight_body_raw);
            tracing::debug!(
                conversation_id = %plan.identity.conversation_id,
                credential_id = call_ctx.id,
                preflight_model = "simple-task",
                main_model = %main_phase.model_id,
                "Executing non-stream preflight phase"
            );
            match self
                .provider
                .call_api_with_context(&call_ctx, &preflight_body)
                .await
            {
                Ok(resp) => {
                    if let Err(err) = resp.bytes().await {
                        tracing::warn!("preflight response consume failed: {}", err);
                    }
                }
                Err(err) => {
                    if should_abort_on_preflight_error(&err) {
                        tracing::warn!(
                            "preflight phase failed with credential-scoped error, aborting main phase: {}",
                            err
                        );
                        return map_provider_error(err);
                    }
                    tracing::warn!(
                        "preflight phase failed, continuing with main phase: {}",
                        err
                    );
                }
            }
        }

        let response = match self
            .provider
            .call_api_with_context(&call_ctx, &main_request_body)
            .await
        {
            Ok(resp) => resp,
            Err(e) => return map_provider_error(e),
        };

        build_non_stream_response_from_upstream(response, model, input_tokens).await
    }
}

fn build_preflight_request_body(request_body: &str, plan: &RequestPlan) -> anyhow::Result<String> {
    let preflight_phase = plan
        .phases
        .iter()
        .find(|p| matches!(p.phase, PhaseKind::PreflightSimpleTask))
        .ok_or_else(|| anyhow::anyhow!("missing preflight phase"))?;

    let mut json: serde_json::Value = serde_json::from_str(request_body)?;
    let conversation = json
        .get_mut("conversationState")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| anyhow::anyhow!("missing conversationState"))?;

    if let Some(user_input) = conversation
        .get_mut("currentMessage")
        .and_then(|v| v.get_mut("userInputMessage"))
        .and_then(|v| v.as_object_mut())
    {
        user_input.insert(
            "modelId".to_string(),
            serde_json::Value::String(preflight_phase.model_id.clone()),
        );
        if let Some(ctx) = user_input
            .get_mut("userInputMessageContext")
            .and_then(|v| v.as_object_mut())
        {
            ctx.remove("tools");
            ctx.remove("toolResults");
        }
    }

    if let Some(history) = conversation
        .get_mut("history")
        .and_then(|v| v.as_array_mut())
    {
        for entry in history.iter_mut() {
            if let Some(user) = entry
                .get_mut("userInputMessage")
                .and_then(|v| v.as_object_mut())
            {
                user.insert(
                    "modelId".to_string(),
                    serde_json::Value::String(preflight_phase.model_id.clone()),
                );
                if let Some(ctx) = user
                    .get_mut("userInputMessageContext")
                    .and_then(|v| v.as_object_mut())
                {
                    ctx.remove("tools");
                    ctx.remove("toolResults");
                }
            }
            if let Some(assistant) = entry
                .get_mut("assistantResponseMessage")
                .and_then(|v| v.as_object_mut())
            {
                assistant.remove("toolUses");
            }
        }
    }

    Ok(serde_json::to_string(&json)?)
}

fn attach_profile_arn(request_body: &str, profile_arn: Option<&str>) -> anyhow::Result<String> {
    let Some(profile_arn) = profile_arn else {
        return Ok(request_body.to_string());
    };

    let mut json: serde_json::Value = serde_json::from_str(request_body)?;
    let obj = json
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("request body is not a JSON object"))?;
    obj.insert(
        "profileArn".to_string(),
        serde_json::Value::String(profile_arn.to_string()),
    );
    Ok(serde_json::to_string(&json)?)
}

fn should_abort_on_preflight_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    msg.contains(" 401 ")
        || msg.contains(" 403 ")
        || msg.contains(" 402 ")
        || msg.contains("MONTHLY_REQUEST_COUNT")
        || msg.contains("所有凭据已用尽")
}
