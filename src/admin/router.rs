//! Admin API 路由配置

use axum::{
    Router, middleware,
    routing::{delete, get, post, put},
};

use super::{
    handlers::{
        add_credential, clear_credential_cooldown, delete_credential, force_refresh_token,
        get_all_credentials, get_credential_balance, get_load_balancing_mode,
        get_runtime_settings, get_version_info, reset_all_success_count, reset_failure_count,
        reset_success_count, set_credential_allow_overage, set_credential_disabled,
        set_credential_display_name, set_credential_priority, set_credential_rate_limits,
        set_load_balancing_mode, update_runtime_settings,
    },
    middleware::{AdminState, admin_auth_middleware},
};

/// 创建 Admin API 路由
///
/// # 端点
/// - `GET /credentials` - 获取所有凭据状态
/// - `POST /credentials` - 添加新凭据
/// - `DELETE /credentials/:id` - 删除凭据
/// - `POST /credentials/:id/disabled` - 设置凭据禁用状态
/// - `POST /credentials/:id/priority` - 设置凭据优先级
/// - `PUT /credentials/:id/display-name` - 设置凭据显示名称
/// - `PUT /credentials/:id/allow-overage` - 设置凭据超额模式
/// - `PUT /credentials/:id/rate-limits` - 设置凭据级限流规则
/// - `POST /credentials/:id/reset` - 重置失败计数
/// - `POST /credentials/:id/clear-cooldown` - 清除运行时风控冷却
/// - `POST /credentials/:id/refresh` - 强制刷新 Token
/// - `GET /credentials/:id/balance` - 获取凭据余额
/// - `GET /config/load-balancing` - 获取负载均衡模式
/// - `PUT /config/load-balancing` - 设置负载均衡模式
/// - `GET /config/settings` - 获取运行时设置
/// - `PUT /config/settings` - 更新运行时设置
///
/// # 认证
/// 需要 Admin API Key 认证，支持：
/// - `x-api-key` header
/// - `Authorization: Bearer <token>` header
pub fn create_admin_router(state: AdminState) -> Router {
    Router::new()
        .route("/version", get(get_version_info))
        .route(
            "/credentials",
            get(get_all_credentials).post(add_credential),
        )
        .route("/credentials/{id}", delete(delete_credential))
        .route("/credentials/{id}/disabled", post(set_credential_disabled))
        .route("/credentials/{id}/priority", post(set_credential_priority))
        .route(
            "/credentials/{id}/display-name",
            put(set_credential_display_name),
        )
        .route(
            "/credentials/{id}/allow-overage",
            put(set_credential_allow_overage),
        )
        .route(
            "/credentials/{id}/rate-limits",
            put(set_credential_rate_limits),
        )
        .route("/credentials/{id}/reset", post(reset_failure_count))
        .route(
            "/credentials/{id}/clear-cooldown",
            post(clear_credential_cooldown),
        )
        .route("/credentials/{id}/reset-stats", post(reset_success_count))
        .route("/credentials/reset-stats", post(reset_all_success_count))
        .route("/credentials/{id}/refresh", post(force_refresh_token))
        .route("/credentials/{id}/balance", get(get_credential_balance))
        .route(
            "/config/load-balancing",
            get(get_load_balancing_mode).put(set_load_balancing_mode),
        )
        .route(
            "/config/settings",
            get(get_runtime_settings).put(update_runtime_settings),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_auth_middleware,
        ))
        .with_state(state)
}
