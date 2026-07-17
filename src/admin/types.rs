//! Admin API 类型定义

use serde::{Deserialize, Serialize};

use crate::kiro::token_manager::RequestEventSnapshot;
use crate::model::rate_limit::RateLimitRule;

// ============ 版本信息 ============

/// KissAPI 二次开发版本信息响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfoResponse {
    pub version: String,
    pub channel: String,
    pub codename: String,
    pub date: String,
    pub summary: String,
    pub package_version: String,
    pub git_sha: String,
    pub build_tag: String,
    pub changelog: String,
}

// ============ 凭据状态 ============

/// 所有凭据状态响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialsStatusResponse {
    /// 凭据总数
    pub total: usize,
    /// 可用凭据数量（未禁用）
    pub available: usize,
    /// 当前活跃凭据 ID
    pub current_id: u64,
    /// 各凭据状态列表
    pub credentials: Vec<CredentialStatusItem>,
}

/// 单个凭据的状态信息
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatusItem {
    /// 凭据唯一 ID
    pub id: u64,
    /// 优先级（数字越小优先级越高）
    pub priority: u32,
    /// 是否被禁用
    pub disabled: bool,
    /// 连续失败次数
    pub failure_count: u32,
    /// 是否为当前活跃凭据
    pub is_current: bool,
    /// Token 过期时间（RFC3339 格式）
    pub expires_at: Option<String>,
    /// 认证方式
    pub auth_method: Option<String>,
    /// 是否有 Profile ARN
    pub has_profile_arn: bool,
    /// refreshToken 的 SHA-256 哈希（仅 OAuth 凭据，用于前端去重）
    pub refresh_token_hash: Option<String>,
    /// kiroApiKey 的 SHA-256 哈希（仅 API Key 凭据，用于前端去重）
    pub api_key_hash: Option<String>,
    /// kiroApiKey 的脱敏展示（仅 API Key 凭据，用于前端显示）
    pub masked_api_key: Option<String>,
    /// 用户邮箱（用于前端显示）
    pub email: Option<String>,
    /// 自定义显示名称（Admin UI 展示用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// API 调用成功次数
    pub success_count: u64,
    /// 最后一次 API 调用时间（RFC3339 格式）
    pub last_used_at: Option<String>,
    /// 是否配置了凭据级代理
    pub has_proxy: bool,
    /// 代理 URL（用于前端展示）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(alias = "proxy_url")]
    pub proxy_url: Option<String>,
    /// Token 刷新连续失败次数
    pub refresh_failure_count: u32,
    /// 禁用原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    /// 端点名称（决定该凭据走哪套 Kiro API，已回退到默认端点）
    pub endpoint: String,
    /// 是否允许超额使用
    pub allow_overage: bool,
    /// 凭据级限流规则（未配置时为 None，运行时会回退到全局 defaultRateLimits）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, alias = "rate_limits")]
    pub rate_limits: Option<Vec<RateLimitRule>>,
    /// 运行时冷却截止时间（RFC3339，仅内存状态）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown_until: Option<String>,
    /// 运行时冷却剩余秒数（仅内存状态）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown_remaining_seconds: Option<u64>,
    /// 最近 100 次请求事件（旧 -> 新）
    pub request_history: Vec<RequestEventSnapshot>,
}

// ============ 操作请求 ============

/// 启用/禁用凭据请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDisabledRequest {
    /// 是否禁用
    pub disabled: bool,
}

/// 修改优先级请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPriorityRequest {
    /// 新优先级值
    pub priority: u32,
}

/// 修改凭据级限流规则请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRateLimitsRequest {
    /// 新限流规则；传 null 或空数组表示清空凭据级限流并回退到全局默认
    #[serde(default, alias = "rate_limits")]
    pub rate_limits: Option<Vec<RateLimitRule>>,
}

/// 设置凭据超额模式请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAllowOverageRequest {
    /// 是否允许超额使用
    pub allow_overage: bool,
}

/// 设置凭据显示名称请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDisplayNameRequest {
    /// 新显示名称；传 null 或空字符串表示清除
    #[serde(default, alias = "display_name")]
    pub display_name: Option<String>,
}

/// 添加凭据请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCredentialRequest {
    /// 刷新令牌（OAuth 凭据必填，API Key 凭据不需要）
    #[serde(alias = "refresh_token")]
    pub refresh_token: Option<String>,

    /// 访问令牌（导入外部缓存时可保留）
    #[serde(alias = "access_token")]
    pub access_token: Option<String>,

    /// Profile ARN（Builder-ID / IdC / External IdP 可选但建议保留）
    #[serde(alias = "profile_arn")]
    pub profile_arn: Option<String>,

    /// 认证方式（可选，默认 social）
    #[serde(default = "default_auth_method")]
    #[serde(alias = "auth_method")]
    pub auth_method: String,

    /// OIDC Client ID（IdC 认证需要）
    #[serde(alias = "client_id")]
    pub client_id: Option<String>,

    /// OIDC Client Secret（IdC 认证需要）
    #[serde(alias = "client_secret")]
    pub client_secret: Option<String>,

    /// External IdP token endpoint（M365 / Entra ID 企业 SSO 需要）
    #[serde(alias = "token_endpoint")]
    pub token_endpoint: Option<String>,

    /// External IdP issuer URL（可选）
    #[serde(alias = "issuer_url")]
    pub issuer_url: Option<String>,

    /// External IdP OAuth scopes（可选）
    pub scopes: Option<String>,

    /// 身份提供商（可选，如 ExternalIdp / AzureAD）
    pub provider: Option<String>,

    /// 优先级（可选，默认 0）
    #[serde(default)]
    pub priority: u32,

    /// 凭据级 Region 配置（用于 OIDC token 刷新）
    /// 未配置时回退到 config.json 的全局 region
    pub region: Option<String>,

    /// 凭据级 Auth Region（用于 Token 刷新）
    #[serde(alias = "auth_region")]
    pub auth_region: Option<String>,

    /// 凭据级 API Region（用于 API 请求）
    #[serde(alias = "api_region")]
    pub api_region: Option<String>,

    /// 凭据级 Machine ID（可选，64 位字符串）
    /// 未配置时回退到 config.json 的 machineId
    #[serde(alias = "machine_id")]
    pub machine_id: Option<String>,

    /// 用户邮箱（可选，用于前端显示）
    pub email: Option<String>,

    /// 自定义显示名称（可选，仅用于 Admin UI 展示）
    #[serde(alias = "display_name")]
    pub display_name: Option<String>,

    /// 凭据级代理 URL（可选，特殊值 "direct" 表示不使用代理）
    #[serde(alias = "proxy_url")]
    pub proxy_url: Option<String>,

    /// 凭据级代理认证用户名（可选）
    #[serde(alias = "proxy_username")]
    pub proxy_username: Option<String>,

    /// 凭据级代理认证密码（可选）
    #[serde(alias = "proxy_password")]
    pub proxy_password: Option<String>,

    /// 手动指定代理池中的代理 ID（可选）。指定后从池分配该 IP。
    #[serde(default, alias = "proxy_id")]
    pub proxy_id: Option<u64>,

    /// 手动指定代理时，是否允许复用已在用的 IP（默认 false）
    #[serde(default, alias = "proxy_allow_reuse")]
    pub proxy_allow_reuse: Option<bool>,

    /// 是否从代理池自动分配空闲 IP（可选）。
    /// None = 沿用池设置的默认开关；true/false 显式覆盖。
    #[serde(default, alias = "use_pool")]
    pub use_pool: Option<bool>,

    /// Kiro API Key（API Key 凭据必填，格式: ksk_xxxxxxxx）
    /// 设置后直接作为 Bearer Token 使用，无需 refreshToken
    #[serde(skip_serializing_if = "Option::is_none", alias = "kiro_api_key")]
    pub kiro_api_key: Option<String>,

    /// 端点名称（可选，未配置时使用 config.defaultEndpoint）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,

    /// 是否允许超额使用（可选，默认 false）
    #[serde(default, alias = "allow_overage")]
    pub allow_overage: Option<bool>,

    /// 凭据级限流规则（可选）
    #[serde(default, alias = "rate_limits")]
    pub rate_limits: Option<Vec<RateLimitRule>>,
}

fn default_auth_method() -> String {
    "social".to_string()
}

/// 添加凭据成功响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCredentialResponse {
    pub success: bool,
    pub message: String,
    /// 新添加的凭据 ID
    pub credential_id: u64,
    /// 用户邮箱（如果获取成功）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

// ============ 余额查询 ============

/// 余额查询响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceResponse {
    /// 凭据 ID
    pub id: u64,
    /// 订阅类型
    pub subscription_title: Option<String>,
    /// 当前使用量
    pub current_usage: f64,
    /// 原始使用限额
    pub usage_limit: f64,
    /// 有效限额（含本地超额额度）
    pub effective_limit: f64,
    /// 本地超额额度
    pub overage_allowance: f64,
    /// 剩余额度（基于有效限额）
    pub remaining: f64,
    /// 使用百分比（基于有效限额）
    pub usage_percentage: f64,
    /// 是否允许超额使用
    pub allow_overage: bool,
    /// 是否正在使用超额部分
    pub overage_active: bool,
    /// 下次重置时间（Unix 时间戳）
    pub next_reset_at: Option<f64>,
}

// ============ 负载均衡配置 ============

/// 负载均衡模式响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadBalancingModeResponse {
    /// 当前模式（"priority" 或 "balanced"）
    pub mode: String,
}

/// 设置负载均衡模式请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetLoadBalancingModeRequest {
    /// 模式（"priority" 或 "balanced"）
    pub mode: String,
}

// ============ 运行时设置（Settings） ============

/// 运行时设置响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSettingsResponse {
    /// suspicious activity 429 冷却时长（分钟）
    pub suspicious_cooldown_minutes: f64,
    /// 底层冷却秒数（便于前端校验/展示）
    pub suspicious_cooldown_seconds: u64,
    /// 是否提取非流式响应的 thinking 块
    pub extract_thinking: bool,
    /// 是否启用原生化双阶段执行模式（实验）
    pub native_like_two_phase_flow: bool,
}

/// 更新运行时设置请求（字段均可选，只更新传入的字段）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRuntimeSettingsRequest {
    /// suspicious 冷却时长（分钟）
    #[serde(default)]
    pub suspicious_cooldown_minutes: Option<f64>,
    /// 是否提取 thinking 块
    #[serde(default)]
    pub extract_thinking: Option<bool>,
    /// 是否启用双阶段执行
    #[serde(default)]
    pub native_like_two_phase_flow: Option<bool>,
}

// ============ 代理池 ============

use crate::proxy_pool::{ProxyEntry, ProxyPoolSettings, ProxyPoolStats, ProbeResult};

/// 代理池列表响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyPoolResponse {
    #[serde(flatten)]
    pub stats: ProxyPoolStats,
    pub auto_assign_enabled: bool,
    pub probe_url: String,
    pub proxies: Vec<ProxyEntryView>,
}

/// 单个代理展示（附带派生字段）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyEntryView {
    pub id: u64,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    pub label: String,
    pub disabled: bool,
    pub assignments: Vec<u64>,
    pub usage_count: usize,
    pub free: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_check: Option<ProbeResult>,
}

impl From<ProxyEntry> for ProxyEntryView {
    fn from(e: ProxyEntry) -> Self {
        let usage_count = e.usage_count();
        let free = e.is_free();
        Self {
            id: e.id,
            url: e.url,
            username: e.username,
            label: e.label,
            disabled: e.disabled,
            assignments: e.assignments,
            usage_count,
            free,
            last_check: e.last_check,
        }
    }
}

/// 添加代理请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProxyRequest {
    pub url: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

/// 批量添加代理请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAddProxyRequest {
    pub lines: Vec<String>,
}

/// 更新代理请求（字段均可选）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProxyRequest {
    #[serde(default)]
    pub url: Option<String>,
    /// 包裹一层：Some(None) 表示显式清空，None 表示不改
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub username: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub password: Option<Option<String>>,
    #[serde(default)]
    pub label: Option<String>,
}

/// 设置禁用状态请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetProxyDisabledRequest {
    pub disabled: bool,
}

/// 更新代理池设置请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProxyPoolSettingsRequest {
    #[serde(default, alias = "auto_assign_enabled")]
    pub auto_assign_enabled: Option<bool>,
    #[serde(default, alias = "probe_url")]
    pub probe_url: Option<String>,
}

/// 代理池设置响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyPoolSettingsResponse {
    pub auto_assign_enabled: bool,
    pub probe_url: String,
}

impl From<ProxyPoolSettings> for ProxyPoolSettingsResponse {
    fn from(s: ProxyPoolSettings) -> Self {
        Self {
            auto_assign_enabled: s.auto_assign_enabled,
            probe_url: s.probe_url,
        }
    }
}

/// 代理探测响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
}

/// 自定义反序列：区分“字段缺失”与“显式 null”
fn deserialize_optional_field<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<String>::deserialize(deserializer)?))
}

// ============ 通用响应 ============

/// 操作成功响应
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

impl SuccessResponse {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }
}

/// 错误响应
#[derive(Debug, Serialize)]
pub struct AdminErrorResponse {
    pub error: AdminError,
}

#[derive(Debug, Serialize)]
pub struct AdminError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

impl AdminErrorResponse {
    pub fn new(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: AdminError {
                error_type: error_type.into(),
                message: message.into(),
            },
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new("invalid_request", message)
    }

    pub fn authentication_error() -> Self {
        Self::new("authentication_error", "Invalid or missing admin API key")
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("not_found", message)
    }

    pub fn api_error(message: impl Into<String>) -> Self {
        Self::new("api_error", message)
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new("internal_error", message)
    }
}
