//! Admin API HTTP 处理器

use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};

use super::{
    middleware::AdminState,
    types::{
        AddCredentialRequest, AddProxyRequest, BatchAddProxyRequest, SetAllowOverageRequest,
        SetDisabledRequest, SetDisplayNameRequest, SetLoadBalancingModeRequest, SetPriorityRequest,
        SetProxyDisabledRequest, SetRateLimitsRequest, SuccessResponse,
        UpdateProxyPoolSettingsRequest, UpdateProxyRequest, UpdateRuntimeSettingsRequest,
        VersionInfoResponse,
    },
};

/// GET /api/admin/version
/// 获取 KissAPI 二次开发版本信息
pub async fn get_version_info() -> impl IntoResponse {
    let info = crate::version::app_version_info();
    Json(VersionInfoResponse {
        version: info.version,
        channel: info.channel,
        codename: info.codename,
        date: info.date,
        summary: info.summary,
        package_version: info.package_version,
        git_sha: info.git_sha,
        build_tag: info.build_tag,
        changelog: info.changelog,
    })
}

/// GET /api/admin/credentials
/// 获取所有凭据状态
pub async fn get_all_credentials(State(state): State<AdminState>) -> impl IntoResponse {
    let response = state.service.get_all_credentials();
    Json(response)
}

/// POST /api/admin/credentials/:id/disabled
/// 设置凭据禁用状态
pub async fn set_credential_disabled(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetDisabledRequest>,
) -> impl IntoResponse {
    match state.service.set_disabled(id, payload.disabled) {
        Ok(_) => {
            let action = if payload.disabled { "禁用" } else { "启用" };
            Json(SuccessResponse::new(format!("凭据 #{} 已{}", id, action))).into_response()
        }
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/:id/priority
/// 设置凭据优先级
pub async fn set_credential_priority(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetPriorityRequest>,
) -> impl IntoResponse {
    match state.service.set_priority(id, payload.priority) {
        Ok(_) => Json(SuccessResponse::new(format!(
            "凭据 #{} 优先级已设置为 {}",
            id, payload.priority
        )))
        .into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// PUT /api/admin/credentials/:id/allow-overage
/// 设置凭据超额模式
pub async fn set_credential_allow_overage(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetAllowOverageRequest>,
) -> impl IntoResponse {
    match state.service.set_allow_overage(id, payload.allow_overage) {
        Ok(_) => {
            let action = if payload.allow_overage {
                "开启"
            } else {
                "关闭"
            };
            Json(SuccessResponse::new(format!(
                "凭据 #{} 超额模式已{}",
                id, action
            )))
            .into_response()
        }
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// PUT /api/admin/credentials/:id/rate-limits
/// 设置凭据级限流规则
pub async fn set_credential_rate_limits(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetRateLimitsRequest>,
) -> impl IntoResponse {
    match state
        .service
        .set_credential_rate_limits(id, payload.rate_limits)
    {
        Ok(_) => Json(SuccessResponse::new(format!("凭据 #{} 限流规则已更新", id))).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// PUT /api/admin/credentials/:id/display-name
/// 设置凭据自定义显示名称
pub async fn set_credential_display_name(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetDisplayNameRequest>,
) -> impl IntoResponse {
    match state.service.set_display_name(id, payload.display_name) {
        Ok(_) => Json(SuccessResponse::new(format!("凭据 #{} 显示名称已更新", id))).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/:id/reset
/// 重置失败计数并重新启用
pub async fn reset_failure_count(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.reset_and_enable(id) {
        Ok(_) => Json(SuccessResponse::new(format!(
            "凭据 #{} 失败计数已重置并重新启用",
            id
        )))
        .into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/:id/clear-cooldown
/// 手动清除凭据运行时风控冷却
pub async fn clear_credential_cooldown(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.clear_cooldown(id) {
        Ok(_) => Json(SuccessResponse::new(format!("凭据 #{} 已退出风控冷却", id))).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// GET /api/admin/credentials/:id/balance
/// 获取指定凭据的余额
pub async fn get_credential_balance(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.get_balance(id).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials
/// 添加新凭据
pub async fn add_credential(
    State(state): State<AdminState>,
    Json(payload): Json<AddCredentialRequest>,
) -> impl IntoResponse {
    match state.service.add_credential(payload).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// DELETE /api/admin/credentials/:id
/// 删除凭据
pub async fn delete_credential(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.delete_credential(id) {
        Ok(_) => Json(SuccessResponse::new(format!("凭据 #{} 已删除", id))).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/:id/refresh
/// 强制刷新凭据 Token
pub async fn force_refresh_token(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.force_refresh_token(id).await {
        Ok(_) => Json(SuccessResponse::new(format!(
            "凭据 #{} Token 已强制刷新",
            id
        )))
        .into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/reset-stats
/// 重置所有凭据的 success_count
pub async fn reset_all_success_count(State(state): State<AdminState>) -> impl IntoResponse {
    match state.service.reset_success_count(None) {
        Ok(count) => Json(SuccessResponse::new(format!(
            "已重置 {} 个凭据的 success_count",
            count
        )))
        .into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/:id/reset-stats
/// 重置指定凭据的 success_count
pub async fn reset_success_count(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.reset_success_count(Some(id)) {
        Ok(_) => Json(SuccessResponse::new(format!(
            "凭据 #{} success_count 已重置",
            id
        )))
        .into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// GET /api/admin/config/load-balancing
/// 获取负载均衡模式
pub async fn get_load_balancing_mode(State(state): State<AdminState>) -> impl IntoResponse {
    let response = state.service.get_load_balancing_mode();
    Json(response)
}

/// PUT /api/admin/config/load-balancing
/// 设置负载均衡模式
pub async fn set_load_balancing_mode(
    State(state): State<AdminState>,
    Json(payload): Json<SetLoadBalancingModeRequest>,
) -> impl IntoResponse {
    match state.service.set_load_balancing_mode(payload) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// GET /api/admin/config/settings
/// 获取运行时设置
pub async fn get_runtime_settings(State(state): State<AdminState>) -> impl IntoResponse {
    let response = state.service.get_runtime_settings();
    Json(response)
}

/// PUT /api/admin/config/settings
/// 更新运行时设置
pub async fn update_runtime_settings(
    State(state): State<AdminState>,
    Json(payload): Json<UpdateRuntimeSettingsRequest>,
) -> impl IntoResponse {
    match state.service.update_runtime_settings(payload) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

// ============ 代理池 ============

/// GET /api/admin/proxy-pool
pub async fn get_proxy_pool(State(state): State<AdminState>) -> impl IntoResponse {
    Json(state.service.get_proxy_pool())
}

/// GET /api/admin/proxy-pool/settings
pub async fn get_proxy_pool_settings(State(state): State<AdminState>) -> impl IntoResponse {
    Json(state.service.get_proxy_pool_settings())
}

/// PUT /api/admin/proxy-pool/settings
pub async fn update_proxy_pool_settings(
    State(state): State<AdminState>,
    Json(payload): Json<UpdateProxyPoolSettingsRequest>,
) -> impl IntoResponse {
    match state.service.set_proxy_pool_settings(payload) {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/proxy-pool
pub async fn add_proxy(
    State(state): State<AdminState>,
    Json(payload): Json<AddProxyRequest>,
) -> impl IntoResponse {
    match state.service.add_proxy(payload) {
        Ok(entry) => Json(entry).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/proxy-pool/batch
pub async fn batch_add_proxy(
    State(state): State<AdminState>,
    Json(payload): Json<BatchAddProxyRequest>,
) -> impl IntoResponse {
    Json(state.service.batch_add_proxy(&payload.lines))
}

/// PUT /api/admin/proxy-pool/:id
pub async fn update_proxy(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<UpdateProxyRequest>,
) -> impl IntoResponse {
    match state.service.update_proxy(id, payload) {
        Ok(entry) => Json(entry).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// DELETE /api/admin/proxy-pool/:id
pub async fn delete_proxy(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.remove_proxy(id) {
        Ok(_) => Json(SuccessResponse::new(format!("代理 #{} 已删除", id))).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/proxy-pool/:id/disabled
pub async fn set_proxy_disabled(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetProxyDisabledRequest>,
) -> impl IntoResponse {
    match state.service.set_proxy_disabled(id, payload.disabled) {
        Ok(entry) => Json(entry).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/proxy-pool/:id/test
pub async fn test_proxy(State(state): State<AdminState>, Path(id): Path<u64>) -> impl IntoResponse {
    match state.service.test_proxy(id).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}
