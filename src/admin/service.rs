//! Admin API 业务逻辑服务

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::credit_ledger::{CreditEntry, CreditLedger, SampleInput, SampleReport};
use crate::kiro::model::credentials::KiroCredentials;
use crate::kiro::token_manager::MultiTokenManager;

use super::error::AdminServiceError;
use super::types::{
    AddCredentialRequest, AddCredentialResponse, ArchivedRoundSummary, BalanceResponse,
    CreditLedgerResponse, CredentialStatusItem, CredentialsStatusResponse,
    LoadBalancingModeResponse, RuntimeSettingsResponse, SetLoadBalancingModeRequest,
    UpdateRuntimeSettingsRequest,
};

/// 余额缓存过期时间（秒），5 分钟
const BALANCE_CACHE_TTL_SECS: i64 = 300;

/// 积分采样默认间隔（秒），与余额缓存 TTL 对齐，避免额外压上游
pub const CREDIT_SAMPLE_INTERVAL_SECS: u64 = 300;

/// 批量采样时每个凭据之间的间隔（毫秒），避免瞬间打满上游
const CREDIT_SAMPLE_STAGGER_MS: u64 = 400;

/// 缓存的余额条目（含时间戳）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedBalance {
    /// 缓存时间（Unix 秒）
    cached_at: f64,
    /// 缓存的余额数据
    data: BalanceResponse,
}

/// Admin 服务
///
/// 封装所有 Admin API 的业务逻辑
pub struct AdminService {
    token_manager: Arc<MultiTokenManager>,
    balance_cache: Mutex<HashMap<u64, CachedBalance>>,
    cache_path: Option<PathBuf>,
    /// 已注册的端点名称集合（用于 add_credential 校验）
    known_endpoints: HashSet<String>,
    /// IP 代理池
    proxy_pool: Arc<crate::proxy_pool::ProxyPool>,
    /// 拼车补号配置（面板可配，daemon 消费）
    carpool: Arc<crate::carpool::Carpool>,
    /// 积分消耗账本（kirors-b 专属）
    credit_ledger: Arc<CreditLedger>,
}

impl AdminService {
    pub fn new(
        token_manager: Arc<MultiTokenManager>,
        known_endpoints: impl IntoIterator<Item = String>,
        proxy_pool: Arc<crate::proxy_pool::ProxyPool>,
        carpool: Arc<crate::carpool::Carpool>,
        credit_ledger: Arc<CreditLedger>,
    ) -> Self {
        let cache_path = token_manager
            .cache_dir()
            .map(|d| d.join("kiro_balance_cache.json"));

        let balance_cache = Self::load_balance_cache_from(&cache_path);

        Self {
            token_manager,
            balance_cache: Mutex::new(balance_cache),
            cache_path,
            known_endpoints: known_endpoints.into_iter().collect(),
            proxy_pool,
            carpool,
            credit_ledger,
        }
    }

    // ============ 积分账本（kirors-b 专属） ============

    /// 凭据指纹：优先 API Key 哈希 → refreshToken 哈希 → 兜底 ID
    ///
    /// 用指纹而不是自增 ID 建账，凭据被删后重新导入同一个号也能续上账。
    fn fingerprint_of(entry: &crate::kiro::token_manager::CredentialEntrySnapshot) -> String {
        entry
            .api_key_hash
            .clone()
            .or_else(|| entry.refresh_token_hash.clone())
            .unwrap_or_else(|| format!("id:{}", entry.id))
    }

    /// 凭据展示标签：显示名 → 邮箱 → 脱敏 Key → #ID
    fn label_of(entry: &crate::kiro::token_manager::CredentialEntrySnapshot) -> String {
        entry
            .display_name
            .clone()
            .or_else(|| entry.email.clone())
            .or_else(|| entry.masked_api_key.clone())
            .unwrap_or_else(|| format!("#{}", entry.id))
    }

    /// 当前轮次来源标记：提号接口 host + token 尾 8 位（不落完整 token）
    fn carpool_source_tag(&self) -> Option<String> {
        let url = self.carpool.settings().get_url;
        let url = url.trim();
        if url.is_empty() {
            return None;
        }
        let host = url
            .split("://")
            .nth(1)
            .and_then(|rest| rest.split('/').next())
            .unwrap_or("unknown");
        let tail = url
            .split("token=")
            .nth(1)
            .map(|t| t.split('&').next().unwrap_or(t))
            .map(|t| {
                if t.len() > 8 {
                    format!("…{}", &t[t.len() - 8..])
                } else {
                    t.to_string()
                }
            });
        Some(match tail {
            Some(t) => format!("{} (token {})", host, t),
            None => host.to_string(),
        })
    }

    /// 读取积分账本（当前轮明细 + 汇总 + 历史轮摘要）
    pub fn get_credit_ledger(&self) -> CreditLedgerResponse {
        let data = self.credit_ledger.snapshot();

        let mut entries: Vec<CreditEntry> = data.entries.into_values().collect();
        // 存活优先，其次按本轮总消耗降序
        entries.sort_by(|a, b| {
            b.alive
                .cmp(&a.alive)
                .then(b.total_credits().total_cmp(&a.total_credits()))
        });

        let my_total: f64 = entries.iter().map(|e| e.my_credits).sum();
        let others_total: f64 = entries.iter().map(|e| e.others_credits).sum();
        let my_requests: u64 = entries.iter().map(|e| e.my_requests).sum();
        let alive_count = entries.iter().filter(|e| e.alive).count();
        let dead_count = entries.len() - alive_count;

        let archived = data
            .archived
            .iter()
            .rev()
            .map(|r| ArchivedRoundSummary {
                id: r.meta.id,
                started_at: r.meta.started_at.clone(),
                ended_at: r.meta.ended_at.clone(),
                note: r.meta.note.clone(),
                source: r.meta.source.clone(),
                my_total: r.total_my(),
                others_total: r.total_others(),
                total: r.total_my() + r.total_others(),
                credential_count: r.entries.len(),
            })
            .collect();

        CreditLedgerResponse {
            current_round: data.current_round,
            entries,
            my_total,
            others_total,
            total: my_total + others_total,
            my_requests,
            alive_count,
            dead_count,
            last_sample_at: data.last_sample_at,
            sample_interval_seconds: CREDIT_SAMPLE_INTERVAL_SECS,
            archived,
        }
    }

    /// 组装单个凭据的采样输入（查上游额度）
    async fn build_sample_input(
        &self,
        entry: &crate::kiro::token_manager::CredentialEntrySnapshot,
    ) -> Result<SampleInput, String> {
        let usage = self
            .token_manager
            .get_usage_limits_for(entry.id)
            .await
            .map_err(|e| format!("凭据 #{} 额度查询失败: {}", entry.id, e))?;

        Ok(SampleInput {
            fingerprint: Self::fingerprint_of(entry),
            cred_id: entry.id,
            label: Self::label_of(entry),
            subscription_title: usage.subscription_title().map(|s| s.to_string()),
            current_usage: usage.current_usage(),
            usage_limit: usage.usage_limit(),
            success_count: entry.success_count,
        })
    }

    /// 采样全部凭据用量并写账本
    ///
    /// 单个凭据查询失败不影响其他凭据；失败原因回报给调用方。
    /// 同时把管理器里已禁用的凭据在账本上标记为死号（保留消耗记录）。
    pub async fn sample_credits(&self) -> SampleReport {
        let snapshot = self.token_manager.snapshot();
        let mut inputs = Vec::with_capacity(snapshot.entries.len());
        let mut errors = Vec::new();

        let total = snapshot.entries.len();
        for (idx, entry) in snapshot.entries.iter().enumerate() {
            match self.build_sample_input(entry).await {
                Ok(input) => inputs.push(input),
                Err(msg) => {
                    tracing::debug!("积分采样跳过: {}", msg);
                    errors.push(msg);
                }
            }
            if idx + 1 < total {
                tokio::time::sleep(std::time::Duration::from_millis(CREDIT_SAMPLE_STAGGER_MS))
                    .await;
            }
        }

        let mut report = self.credit_ledger.apply_samples(inputs);
        report.failed = errors.len();
        report.errors = errors;

        // 已禁用凭据标死号：账留在本轮，换号后仍能看到它烧了多少
        for entry in &snapshot.entries {
            if entry.disabled {
                let reason = entry
                    .disabled_reason
                    .clone()
                    .unwrap_or_else(|| "disabled".to_string());
                self.credit_ledger
                    .mark_dead(&Self::fingerprint_of(entry), reason);
            }
        }

        // 轮次来源跟随当前拼车接口
        self.credit_ledger.set_current_source(self.carpool_source_tag());

        report
    }

    /// 删除/禁用前补一次终值采样，避免换号时最后一段消耗丢账
    async fn final_sample_for(&self, id: u64) {
        let snapshot = self.token_manager.snapshot();
        let entry = match snapshot.entries.iter().find(|e| e.id == id) {
            Some(e) => e,
            None => return,
        };
        match self.build_sample_input(entry).await {
            Ok(input) => {
                self.credit_ledger.apply_samples(vec![input]);
            }
            Err(msg) => tracing::warn!("终值采样失败（消耗记录停留在上次采样值）: {}", msg),
        }
    }

    /// 开启新一轮车队统计（旧轮归档，存活凭据以当前用量为新基线）
    pub async fn start_credit_round(
        &self,
        note: Option<String>,
    ) -> Result<crate::credit_ledger::RoundMeta, AdminServiceError> {
        // 先把旧轮的账记全，再切轮，否则最后一段消耗会落到新轮
        self.sample_credits().await;
        self.credit_ledger
            .start_new_round(note, self.carpool_source_tag())
            .map_err(|e| AdminServiceError::InternalError(e.to_string()))
    }

    /// 清空积分账本（含历史轮次）
    pub fn reset_credit_ledger(&self) -> Result<(), AdminServiceError> {
        self.credit_ledger
            .reset_all()
            .map_err(|e| AdminServiceError::InternalError(e.to_string()))
    }

    /// 获取拼车补号配置
    pub fn get_carpool_settings(&self) -> crate::carpool::CarpoolSettings {
        self.carpool.settings()
    }

    /// 更新拼车补号配置
    pub fn set_carpool_settings(
        &self,
        req: crate::carpool::CarpoolSettingsPatch,
    ) -> Result<crate::carpool::CarpoolSettings, AdminServiceError> {
        self.carpool.patch(req).map_err(|e| {
            AdminServiceError::InvalidRequest(e.to_string())
        })
    }

    /// 获取所有凭据状态
    pub fn get_all_credentials(&self) -> CredentialsStatusResponse {
        let snapshot = self.token_manager.snapshot();
        let default_endpoint = self.token_manager.config().default_endpoint.clone();

        let mut credentials: Vec<CredentialStatusItem> = snapshot
            .entries
            .into_iter()
            .map(|entry| CredentialStatusItem {
                id: entry.id,
                priority: entry.priority,
                disabled: entry.disabled,
                failure_count: entry.failure_count,
                is_current: entry.id == snapshot.current_id,
                expires_at: entry.expires_at,
                auth_method: entry.auth_method,
                has_profile_arn: entry.has_profile_arn,
                refresh_token_hash: entry.refresh_token_hash,
                api_key_hash: entry.api_key_hash,
                masked_api_key: entry.masked_api_key,
                email: entry.email,
                display_name: entry.display_name,
                success_count: entry.success_count,
                last_used_at: entry.last_used_at.clone(),
                has_proxy: entry.has_proxy,
                proxy_url: entry.proxy_url,
                refresh_failure_count: entry.refresh_failure_count,
                disabled_reason: entry.disabled_reason,
                endpoint: entry.endpoint.unwrap_or_else(|| default_endpoint.clone()),
                allow_overage: entry.allow_overage,
                rate_limits: entry.rate_limits,
                cooldown_until: entry.cooldown_until,
                cooldown_remaining_seconds: entry.cooldown_remaining_seconds,
                request_history: entry.request_history,
            })
            .collect();

        // 按优先级排序（数字越小优先级越高）
        credentials.sort_by_key(|c| c.priority);

        CredentialsStatusResponse {
            total: snapshot.total,
            available: snapshot.available,
            current_id: snapshot.current_id,
            credentials,
        }
    }

    /// 设置凭据禁用状态
    pub fn set_disabled(&self, id: u64, disabled: bool) -> Result<(), AdminServiceError> {
        // 先获取当前凭据 ID，用于判断是否需要切换
        let snapshot = self.token_manager.snapshot();
        let current_id = snapshot.current_id;

        self.token_manager
            .set_disabled(id, disabled)
            .map_err(|e| self.classify_error(e, id))?;

        // 只有禁用的是当前凭据时才尝试切换到下一个
        if disabled && id == current_id {
            let _ = self.token_manager.switch_to_next();
        }
        Ok(())
    }

    /// 设置凭据优先级
    pub fn set_priority(&self, id: u64, priority: u32) -> Result<(), AdminServiceError> {
        self.token_manager
            .set_priority(id, priority)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 重置失败计数并重新启用
    pub fn reset_and_enable(&self, id: u64) -> Result<(), AdminServiceError> {
        self.token_manager
            .reset_and_enable(id)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 清除运行时风控冷却
    pub fn clear_cooldown(&self, id: u64) -> Result<(), AdminServiceError> {
        self.token_manager
            .clear_cooldown(id)
            .map_err(|e| self.classify_error(e, id))
    }

    pub fn reset_success_count(&self, id: Option<u64>) -> Result<u32, AdminServiceError> {
        self.token_manager
            .reset_success_count(id)
            .map_err(|e| self.classify_error(e, id.unwrap_or(0)))
    }

    /// 获取凭据余额（带缓存）
    pub async fn get_balance(&self, id: u64) -> Result<BalanceResponse, AdminServiceError> {
        // 先查缓存
        {
            let cache = self.balance_cache.lock();
            if let Some(cached) = cache.get(&id) {
                let now = Utc::now().timestamp() as f64;
                if (now - cached.cached_at) < BALANCE_CACHE_TTL_SECS as f64 {
                    tracing::debug!("凭据 #{} 余额命中缓存", id);
                    return Ok(cached.data.clone());
                }
            }
        }

        // 缓存未命中或已过期，从上游获取
        let balance = self.fetch_balance(id).await?;

        // 更新缓存
        {
            let mut cache = self.balance_cache.lock();
            cache.insert(
                id,
                CachedBalance {
                    cached_at: Utc::now().timestamp() as f64,
                    data: balance.clone(),
                },
            );
        }
        self.save_balance_cache();

        Ok(balance)
    }

    /// 从上游获取余额（无缓存）
    async fn fetch_balance(&self, id: u64) -> Result<BalanceResponse, AdminServiceError> {
        let usage = self
            .token_manager
            .get_usage_limits_for(id)
            .await
            .map_err(|e| self.classify_balance_error(e, id))?;

        let current_usage = usage.current_usage();
        let usage_limit = usage.usage_limit();
        let allow_overage = self.token_manager.get_allow_overage(id);
        let overage_allowance = if allow_overage { 10000.0 } else { 0.0 };
        let effective_limit = usage_limit + overage_allowance;
        let remaining = (effective_limit - current_usage).max(0.0);
        let usage_percentage = if effective_limit > 0.0 {
            (current_usage / effective_limit * 100.0).min(100.0)
        } else {
            0.0
        };
        let overage_active = allow_overage && current_usage > usage_limit;

        Ok(BalanceResponse {
            id,
            subscription_title: usage.subscription_title().map(|s| s.to_string()),
            current_usage,
            usage_limit,
            effective_limit,
            overage_allowance,
            remaining,
            usage_percentage,
            allow_overage,
            overage_active,
            next_reset_at: usage.next_date_reset,
        })
    }

    /// 添加新凭据
    pub async fn add_credential(
        &self,
        req: AddCredentialRequest,
    ) -> Result<AddCredentialResponse, AdminServiceError> {
        // 校验端点名：未指定则默认合法，指定则必须已注册
        if let Some(ref name) = req.endpoint {
            if !self.known_endpoints.contains(name) {
                let mut known: Vec<&str> =
                    self.known_endpoints.iter().map(|s| s.as_str()).collect();
                known.sort();
                return Err(AdminServiceError::InvalidCredential(format!(
                    "未知端点 \"{}\"，已注册端点: {:?}",
                    name, known
                )));
            }
        }

        // 代理池分配：仅当凭据未手填 proxy_url 时才介入
        // 手动指定 proxy_id > 自动分配（受 use_pool / 池默认开关控制）
        //
        // 采用 reservation 两阶段：先原子预占（占位，不落盘、计入负载防止并发选中同一代理），
        // 凭据创建成功后 commit 转真实 credId 并落盘；任一步失败则 cancel 回滚，避免幽灵占用。
        let mut req = req;
        // 预占句柄：(proxy_id, token)，成功后 commit，失败/提前返回时 cancel
        let mut reservation_token: Option<u64> = None;
        let mut reserved_proxy_id: Option<u64> = None;
        let manual_proxy_url_present = req
            .proxy_url
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if !manual_proxy_url_present {
            let pool = &self.proxy_pool;
            let allow_reuse = req.proxy_allow_reuse.unwrap_or(false);
            let allow_probe_failure = req.proxy_allow_probe_failure.unwrap_or(false);
            let want_auto = req
                .use_pool
                .unwrap_or_else(|| pool.is_auto_assign_enabled());
            if let Some(pid) = req.proxy_id {
                // 手动指定：先预占（占位），再探测。默认探测失败拒绝创建，除非显式允许。
                let reservation = pool.reserve_manual(pid, allow_reuse).map_err(|e| {
                    AdminServiceError::InvalidCredential(format!("指定代理分配失败: {}", e))
                })?;
                let probe = pool.probe(&reservation.proxy).await;
                pool.record_probe(pid, probe.clone());
                if !probe.ok && !allow_probe_failure {
                    pool.cancel_reservation(reservation.token);
                    return Err(AdminServiceError::InvalidCredential(format!(
                        "指定代理 #{} 探测失败（{}），已拒绝创建。如确认可用可强制允许后重试",
                        pid,
                        probe.message.as_deref().unwrap_or("未知")
                    )));
                }
                if !probe.ok {
                    tracing::warn!(
                        "手动指定代理 #{} 探测失败({})，用户强制分配",
                        pid,
                        probe.message.as_deref().unwrap_or("未知")
                    );
                }
                req.proxy_url = Some(reservation.proxy.url.clone());
                req.proxy_username = reservation.proxy.username.clone();
                req.proxy_password = reservation.proxy.password.clone();
                reservation_token = Some(reservation.token);
                reserved_proxy_id = Some(reservation.proxy.id);
                tracing::info!(
                    "手动指定代理 #{} ({}){} 预占给新凭据",
                    reservation.proxy.id,
                    if reservation.proxy.label.is_empty() {
                        &reservation.proxy.url
                    } else {
                        &reservation.proxy.label
                    },
                    if reservation.reused {
                        " ♻️复用在用IP"
                    } else {
                        ""
                    }
                );
            } else if want_auto {
                // 自动分配：优先空闲，逐个预占并探测（带 TTL 缓存，避免反复探测坏代理），
                // 跳过不通的；无空闲时复用在用 IP。
                let mut skip: Vec<u64> = Vec::new();
                loop {
                    match pool.reserve_auto(&skip) {
                        Some(reservation) => {
                            let pid = reservation.proxy.id;
                            // 使用 TTL 缓存的探测：新预占的代理若近期探测过（成功/失败）直接复用结果
                            let probe = match pool.probe_or_cached(pid).await {
                                Some(r) => r,
                                None => {
                                    // 代理在探测前消失（并发删除），取消预占并跳过
                                    pool.cancel_reservation(reservation.token);
                                    skip.push(pid);
                                    continue;
                                }
                            };
                            if probe.ok {
                                req.proxy_url = Some(reservation.proxy.url.clone());
                                req.proxy_username = reservation.proxy.username.clone();
                                req.proxy_password = reservation.proxy.password.clone();
                                reservation_token = Some(reservation.token);
                                reserved_proxy_id = Some(pid);
                                tracing::info!(
                                    "自动分配代理 #{} ({}){} 预占给新凭据",
                                    pid,
                                    if reservation.proxy.label.is_empty() {
                                        &reservation.proxy.url
                                    } else {
                                        &reservation.proxy.label
                                    },
                                    if reservation.reused {
                                        " ♻️复用在用IP"
                                    } else {
                                        ""
                                    }
                                );
                                break;
                            } else {
                                // 探测失败：取消预占并跳过该代理重试
                                pool.cancel_reservation(reservation.token);
                                tracing::warn!(
                                    "代理 #{} 探测失败({})，跳过",
                                    pid,
                                    probe.message.as_deref().unwrap_or("未知")
                                );
                                skip.push(pid);
                            }
                        }
                        None => {
                            tracing::warn!(
                                "代理池无可用 IP（全部探测失败或池为空），新凭据不分配代理"
                            );
                            break;
                        }
                    }
                }
            }
        }

        // 构建凭据对象
        let email = req.email.clone();
        let new_cred = KiroCredentials {
            id: None,
            access_token: req.access_token,
            refresh_token: req.refresh_token,
            profile_arn: req.profile_arn,
            expires_at: None,
            auth_method: Some(req.auth_method),
            provider: req.provider,
            client_id: req.client_id,
            client_secret: req.client_secret,
            token_endpoint: req.token_endpoint,
            issuer_url: req.issuer_url,
            scopes: req.scopes,
            priority: req.priority,
            region: req.region,
            auth_region: req.auth_region,
            api_region: req.api_region,
            machine_id: req.machine_id,
            email: req.email,
            display_name: req.display_name,
            subscription_title: None, // 将在首次获取使用额度时自动更新
            proxy_url: req.proxy_url,
            proxy_username: req.proxy_username,
            proxy_password: req.proxy_password,
            disabled: false, // 新添加的凭据默认启用
            kiro_api_key: req.kiro_api_key,
            endpoint: req.endpoint,
            allow_overage: req.allow_overage.unwrap_or(false),
            rate_limits: req.rate_limits,
        };

        // 调用 token_manager 添加凭据
        let credential_id = match self.token_manager.add_credential(new_cred).await {
            Ok(id) => id,
            Err(e) => {
                // 凭据创建失败：回滚代理预占，避免幽灵占用
                if let Some(token) = reservation_token {
                    self.proxy_pool.cancel_reservation(token);
                    tracing::info!("凭据创建失败，已回滚代理 #{:?} 预占", reserved_proxy_id);
                }
                return Err(self.classify_add_error(e));
            }
        };

        // 提交代理预占（用真实 credId）。
        // 分配阶段先预占占位，添加成功后才 commit 为真实挂载并落盘。
        // 若 commit 落盘失败，凭据已创建且 proxy_url 已写入凭据，代理确实在用；
        // 仅代理池统计可能与凭据不一致，告警但不影响凭据可用性。
        if let Some(token) = reservation_token {
            if let Err(e) = self.proxy_pool.commit_reservation(token, credential_id) {
                tracing::error!(
                    "凭据 #{} 已创建，但代理池提交落盘失败（代理仍已写入凭据，运行无影响）: {}",
                    credential_id,
                    e
                );
            }
        }

        // 主动获取订阅等级，避免首次请求时 Free 账号绕过 Opus 模型过滤
        if let Err(e) = self.token_manager.get_usage_limits_for(credential_id).await {
            tracing::warn!("添加凭据后获取订阅等级失败（不影响凭据添加）: {}", e);
        }

        Ok(AddCredentialResponse {
            success: true,
            message: format!("凭据添加成功，ID: {}", credential_id),
            credential_id,
            email,
        })
    }

    /// 设置凭据超额模式
    pub fn set_allow_overage(&self, id: u64, allow_overage: bool) -> Result<(), AdminServiceError> {
        self.token_manager
            .set_allow_overage(id, allow_overage)
            .map_err(|e| self.classify_error(e, id))?;

        {
            let mut cache = self.balance_cache.lock();
            cache.remove(&id);
        }
        self.save_balance_cache();
        Ok(())
    }

    /// 设置凭据端点（kirors-b 专属：ide / cli / aws 环境隔离切换）
    pub fn set_endpoint(
        &self,
        id: u64,
        endpoint: Option<String>,
    ) -> Result<(), AdminServiceError> {
        let normalized = endpoint
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if let Some(name) = normalized.as_deref() {
            if !crate::kiro::endpoint::is_known_endpoint(name) {
                return Err(AdminServiceError::InvalidRequest(format!(
                    "未知端点 \"{}\"，可用: {}",
                    name,
                    crate::kiro::endpoint::KNOWN_ENDPOINTS.join(" / ")
                )));
            }
        }

        self.token_manager
            .set_endpoint(id, normalized)
            .map_err(|e| self.classify_error(e, id))?;

        // 端点切换会改变额度查询主机（q.* vs management.*），旧余额缓存不再可比
        {
            let mut cache = self.balance_cache.lock();
            cache.remove(&id);
        }
        self.save_balance_cache();
        Ok(())
    }

    /// 设置凭据自定义显示名称
    pub fn set_display_name(
        &self,
        id: u64,
        display_name: Option<String>,
    ) -> Result<(), AdminServiceError> {
        self.token_manager
            .set_display_name(id, display_name)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 设置凭据级限流规则
    pub fn set_credential_rate_limits(
        &self,
        id: u64,
        rate_limits: Option<Vec<crate::model::rate_limit::RateLimitRule>>,
    ) -> Result<(), AdminServiceError> {
        self.token_manager
            .set_rate_limits(id, rate_limits)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 删除凭据
    pub async fn delete_credential(&self, id: u64) -> Result<(), AdminServiceError> {
        // 删号前补一次终值采样 + 标死号，保住本轮已产生的消耗记录
        let fingerprint = self
            .token_manager
            .snapshot()
            .entries
            .iter()
            .find(|e| e.id == id)
            .map(Self::fingerprint_of);
        self.final_sample_for(id).await;

        self.token_manager
            .delete_credential(id)
            .map_err(|e| self.classify_delete_error(e, id))?;

        if let Some(fp) = fingerprint {
            self.credit_ledger.mark_dead(&fp, "deleted");
        }

        // 清理已删除凭据的余额缓存
        {
            let mut cache = self.balance_cache.lock();
            cache.remove(&id);
        }
        self.save_balance_cache();

        // 释放代理池中该凭据的占用
        let released = self.proxy_pool.release_by_cred(id);
        if released > 0 {
            tracing::info!("删除凭据 #{}，释放 {} 个代理分配", id, released);
        }

        Ok(())
    }

    // ============ 代理池 ============

    /// 获取代理池列表 + 统计
    pub fn get_proxy_pool(&self) -> super::types::ProxyPoolResponse {
        let settings = self.proxy_pool.settings();
        super::types::ProxyPoolResponse {
            stats: self.proxy_pool.stats(),
            auto_assign_enabled: settings.auto_assign_enabled,
            probe_url: settings.probe_url,
            proxies: self.proxy_pool.list().into_iter().map(Into::into).collect(),
        }
    }

    /// 获取代理池设置
    pub fn get_proxy_pool_settings(&self) -> super::types::ProxyPoolSettingsResponse {
        self.proxy_pool.settings().into()
    }

    /// 更新代理池设置
    pub fn set_proxy_pool_settings(
        &self,
        req: super::types::UpdateProxyPoolSettingsRequest,
    ) -> Result<super::types::ProxyPoolSettingsResponse, AdminServiceError> {
        self.proxy_pool
            .set_settings(req.auto_assign_enabled, req.probe_url)
            .map(Into::into)
            .map_err(Self::map_pool_error)
    }

    /// 添加代理
    pub fn add_proxy(
        &self,
        req: super::types::AddProxyRequest,
    ) -> Result<super::types::ProxyEntryView, AdminServiceError> {
        self.proxy_pool
            .add(req.url, req.username, req.password, req.label)
            .map(Into::into)
            .map_err(Self::map_pool_error)
    }

    /// 批量添加代理（返回成功条目 + 逐行错误）
    pub fn batch_add_proxy(&self, lines: &[String]) -> super::types::BatchAddProxyResponse {
        let result = self.proxy_pool.batch_add(lines);
        super::types::BatchAddProxyResponse {
            added: result.added.len(),
            proxies: result.added.into_iter().map(Into::into).collect(),
            errors: result.errors,
        }
    }

    /// 更新代理
    pub fn update_proxy(
        &self,
        id: u64,
        req: super::types::UpdateProxyRequest,
    ) -> Result<super::types::ProxyEntryView, AdminServiceError> {
        self.proxy_pool
            .update(id, req.url, req.username, req.password, req.label)
            .map(Into::into)
            .map_err(Self::map_pool_error)
    }

    /// 删除代理
    pub fn remove_proxy(&self, id: u64) -> Result<(), AdminServiceError> {
        self.proxy_pool
            .remove(id)
            .map(|_| ())
            .map_err(Self::map_pool_error)
    }

    /// 启用/禁用代理
    pub fn set_proxy_disabled(
        &self,
        id: u64,
        disabled: bool,
    ) -> Result<super::types::ProxyEntryView, AdminServiceError> {
        self.proxy_pool
            .set_disabled(id, disabled)
            .map(Into::into)
            .map_err(Self::map_pool_error)
    }

    /// 将 PoolError 映射为 AdminServiceError
    fn map_pool_error(e: crate::proxy_pool::PoolError) -> AdminServiceError {
        use crate::proxy_pool::PoolError;
        match e {
            PoolError::NotFound => AdminServiceError::NotFound { id: 0 },
            PoolError::Invalid(m) => AdminServiceError::InvalidCredential(m),
            PoolError::Conflict(m) => AdminServiceError::InvalidCredential(m),
            PoolError::Persist(m) => AdminServiceError::InternalError(m),
        }
    }

    /// 探测代理可用性
    pub async fn test_proxy(
        &self,
        id: u64,
    ) -> Result<super::types::ProxyTestResponse, AdminServiceError> {
        let entry = self
            .proxy_pool
            .get(id)
            .ok_or(AdminServiceError::NotFound { id })?;
        let result = self.proxy_pool.probe(&entry).await;
        self.proxy_pool.record_probe(id, result.clone());
        let message = if result.ok {
            match result.latency_ms {
                Some(ms) => format!("连接成功，延迟 {}ms", ms),
                None => "连接成功".to_string(),
            }
        } else {
            format!("连接失败: {}", result.message.as_deref().unwrap_or("未知"))
        };
        Ok(super::types::ProxyTestResponse {
            success: result.ok,
            message,
            latency_ms: result.latency_ms,
            ip: result.ip,
        })
    }

    /// 获取负载均衡模式
    pub fn get_load_balancing_mode(&self) -> LoadBalancingModeResponse {
        LoadBalancingModeResponse {
            mode: self.token_manager.get_load_balancing_mode(),
        }
    }

    /// 设置负载均衡模式
    pub fn set_load_balancing_mode(
        &self,
        req: SetLoadBalancingModeRequest,
    ) -> Result<LoadBalancingModeResponse, AdminServiceError> {
        // 验证模式值
        if req.mode != "priority" && req.mode != "balanced" {
            return Err(AdminServiceError::InvalidCredential(
                "mode 必须是 'priority' 或 'balanced'".to_string(),
            ));
        }

        self.token_manager
            .set_load_balancing_mode(req.mode.clone())
            .map_err(|e| AdminServiceError::InternalError(e.to_string()))?;

        Ok(LoadBalancingModeResponse { mode: req.mode })
    }

    /// 获取运行时设置
    pub fn get_runtime_settings(&self) -> RuntimeSettingsResponse {
        let seconds = self.token_manager.get_suspicious_cooldown_seconds();
        RuntimeSettingsResponse {
            suspicious_cooldown_minutes: seconds as f64 / 60.0,
            suspicious_cooldown_seconds: seconds,
            extract_thinking: self.token_manager.get_extract_thinking(),
            native_like_two_phase_flow: self.token_manager.get_native_like_two_phase_flow(),
        }
    }

    /// 更新运行时设置（只更新传入的字段）
    pub fn update_runtime_settings(
        &self,
        req: UpdateRuntimeSettingsRequest,
    ) -> Result<RuntimeSettingsResponse, AdminServiceError> {
        if let Some(minutes) = req.suspicious_cooldown_minutes {
            if !minutes.is_finite() || minutes < 0.0 {
                return Err(AdminServiceError::InvalidCredential(
                    "suspiciousCooldownMinutes 必须是非负数".to_string(),
                ));
            }
            // 上限 24 小时，防止误输导致凭据长时间不可用
            if minutes > 24.0 * 60.0 {
                return Err(AdminServiceError::InvalidCredential(
                    "suspiciousCooldownMinutes 不能超过 1440（24 小时）".to_string(),
                ));
            }
            let seconds = (minutes * 60.0).round() as u64;
            self.token_manager
                .set_suspicious_cooldown_seconds(seconds)
                .map_err(|e| AdminServiceError::InternalError(e.to_string()))?;
        }

        if let Some(enabled) = req.extract_thinking {
            self.token_manager
                .set_extract_thinking(enabled)
                .map_err(|e| AdminServiceError::InternalError(e.to_string()))?;
        }

        if let Some(enabled) = req.native_like_two_phase_flow {
            self.token_manager
                .set_native_like_two_phase_flow(enabled)
                .map_err(|e| AdminServiceError::InternalError(e.to_string()))?;
        }

        Ok(self.get_runtime_settings())
    }

    /// 强制刷新指定凭据的 Token
    pub async fn force_refresh_token(&self, id: u64) -> Result<(), AdminServiceError> {
        self.token_manager
            .force_refresh_token_for(id)
            .await
            .map_err(|e| self.classify_balance_error(e, id))
    }

    // ============ 余额缓存持久化 ============

    fn load_balance_cache_from(cache_path: &Option<PathBuf>) -> HashMap<u64, CachedBalance> {
        let path = match cache_path {
            Some(p) => p,
            None => return HashMap::new(),
        };

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return HashMap::new(),
        };

        // 文件中使用字符串 key 以兼容 JSON 格式
        let map: HashMap<String, CachedBalance> = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("解析余额缓存失败，将忽略: {}", e);
                return HashMap::new();
            }
        };

        let now = Utc::now().timestamp() as f64;
        map.into_iter()
            .filter_map(|(k, v)| {
                let id = k.parse::<u64>().ok()?;
                // 丢弃超过 TTL 的条目
                if (now - v.cached_at) < BALANCE_CACHE_TTL_SECS as f64 {
                    Some((id, v))
                } else {
                    None
                }
            })
            .collect()
    }

    fn save_balance_cache(&self) {
        let path = match &self.cache_path {
            Some(p) => p,
            None => return,
        };

        // 持有锁期间完成序列化和写入，防止并发损坏
        let cache = self.balance_cache.lock();
        let map: HashMap<String, &CachedBalance> =
            cache.iter().map(|(k, v)| (k.to_string(), v)).collect();

        match serde_json::to_string_pretty(&map) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    tracing::warn!("保存余额缓存失败: {}", e);
                }
            }
            Err(e) => tracing::warn!("序列化余额缓存失败: {}", e),
        }
    }

    // ============ 错误分类 ============

    /// 分类简单操作错误（set_disabled, set_priority, reset_and_enable）
    fn classify_error(&self, e: anyhow::Error, id: u64) -> AdminServiceError {
        let msg = e.to_string();
        if msg.contains("不存在") {
            AdminServiceError::NotFound { id }
        } else {
            AdminServiceError::InternalError(msg)
        }
    }

    /// 分类余额查询错误（可能涉及上游 API 调用）
    fn classify_balance_error(&self, e: anyhow::Error, id: u64) -> AdminServiceError {
        let msg = e.to_string();

        // 1. 凭据不存在
        if msg.contains("不存在") {
            return AdminServiceError::NotFound { id };
        }

        // 2. 本地凭据元数据缺失：客户端请求错误，映射为 400
        if msg.contains("缺少 profileArn") || msg.contains("profileArn") {
            return AdminServiceError::InvalidCredential(msg);
        }

        // 3. API Key 凭据不支持刷新：客户端请求错误，映射为 400
        if msg.contains("API Key 凭据不支持刷新") {
            return AdminServiceError::InvalidCredential(msg);
        }

        // 4. 上游服务错误特征：HTTP 响应错误或网络错误
        let is_upstream_error =
            // HTTP 响应错误（来自 refresh_*_token 的错误消息）
            msg.contains("凭证已过期或无效") ||
            msg.contains("权限不足") ||
            msg.contains("已被限流") ||
            msg.contains("服务器错误") ||
            msg.contains("Token 刷新失败") ||
            msg.contains("暂时不可用") ||
            // 网络错误（reqwest 错误）
            msg.contains("error trying to connect") ||
            msg.contains("connection") ||
            msg.contains("timeout") ||
            msg.contains("timed out");

        if is_upstream_error {
            AdminServiceError::UpstreamError(msg)
        } else {
            // 4. 默认归类为内部错误（本地验证失败、配置错误等）
            // 包括：缺少 refreshToken、refreshToken 已被截断、无法生成 machineId 等
            AdminServiceError::InternalError(msg)
        }
    }

    /// 分类添加凭据错误
    fn classify_add_error(&self, e: anyhow::Error) -> AdminServiceError {
        let msg = e.to_string();

        // 凭据验证失败（refreshToken 无效、格式错误等）
        let is_invalid_credential = msg.contains("缺少 refreshToken")
            || msg.contains("refreshToken 为空")
            || msg.contains("refreshToken 已被截断")
            || msg.contains("凭据已存在")
            || msg.contains("refreshToken 重复")
            || msg.contains("kiroApiKey 重复")
            || msg.contains("缺少 kiroApiKey")
            || msg.contains("kiroApiKey 为空")
            || msg.contains("缺少 profileArn")
            || msg.contains("profileArn")
            || msg.contains("凭证已过期或无效")
            || msg.contains("权限不足")
            || msg.contains("已被限流");

        if is_invalid_credential {
            AdminServiceError::InvalidCredential(msg)
        } else if msg.contains("error trying to connect")
            || msg.contains("connection")
            || msg.contains("timeout")
        {
            AdminServiceError::UpstreamError(msg)
        } else {
            AdminServiceError::InternalError(msg)
        }
    }

    /// 分类删除凭据错误
    fn classify_delete_error(&self, e: anyhow::Error, id: u64) -> AdminServiceError {
        let msg = e.to_string();
        if msg.contains("不存在") {
            AdminServiceError::NotFound { id }
        } else if msg.contains("只能删除已禁用的凭据") || msg.contains("请先禁用凭据")
        {
            AdminServiceError::InvalidCredential(msg)
        } else {
            AdminServiceError::InternalError(msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::{Config, TlsBackend};
    use crate::proxy_pool::ProxyPool;

    fn build_service() -> AdminService {
        let config = Config::default();
        let tm = Arc::new(MultiTokenManager::new(config, vec![], None, None, false).expect("tm"));
        let pool = Arc::new(ProxyPool::load(None, TlsBackend::Rustls));
        let carpool = Arc::new(crate::carpool::Carpool::load(None));
        let ledger = Arc::new(crate::credit_ledger::CreditLedger::load(None));
        AdminService::new(tm, Vec::<String>::new(), pool, carpool, ledger)
    }

    #[test]
    fn test_add_proxy_rejects_invalid_url() {
        let svc = build_service();
        let err = svc
            .add_proxy(super::super::types::AddProxyRequest {
                url: "not-a-url".to_string(),
                username: None,
                password: None,
                label: None,
            })
            .unwrap_err();
        // 非法 URL 应映射为 400
        assert_eq!(err.status_code(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_add_proxy_ok_and_list() {
        let svc = build_service();
        let view = svc
            .add_proxy(super::super::types::AddProxyRequest {
                url: "http://1.2.3.4:8080".to_string(),
                username: Some("u".to_string()),
                password: Some("p".to_string()),
                label: Some("test".to_string()),
            })
            .unwrap();
        assert_eq!(view.id, 1);
        assert!(view.free);
        let pool = svc.get_proxy_pool();
        assert_eq!(pool.stats.total, 1);
        assert_eq!(pool.stats.available, 1);
    }

    #[test]
    fn test_batch_add_proxy_reports_errors() {
        let svc = build_service();
        let resp = svc.batch_add_proxy(&[
            "http://1.2.3.4:8080".to_string(),
            "bad".to_string(),
            "socks5://5.6.7.8:1080 user pass label".to_string(),
        ]);
        assert_eq!(resp.added, 2);
        assert_eq!(resp.errors.len(), 1);
        assert_eq!(resp.errors[0].line, 2);
    }

    #[test]
    fn test_update_proxy_settings_rejects_ssrf_probe_url() {
        let svc = build_service();
        let err = svc
            .set_proxy_pool_settings(super::super::types::UpdateProxyPoolSettingsRequest {
                auto_assign_enabled: None,
                probe_url: Some("https://127.0.0.1/x".to_string()),
            })
            .unwrap_err();
        assert_eq!(err.status_code(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_update_proxy_settings_accepts_valid_probe_url() {
        let svc = build_service();
        let resp = svc
            .set_proxy_pool_settings(super::super::types::UpdateProxyPoolSettingsRequest {
                auto_assign_enabled: Some(false),
                probe_url: Some("https://api.ipify.org?format=json".to_string()),
            })
            .unwrap();
        assert!(!resp.auto_assign_enabled);
    }

    #[test]
    fn test_remove_proxy_in_use_rejected() {
        let svc = build_service();
        svc.add_proxy(super::super::types::AddProxyRequest {
            url: "http://1.2.3.4:8080".to_string(),
            username: None,
            password: None,
            label: None,
        })
        .unwrap();
        // 直接通过池预占+提交模拟在用（不走真实 add_credential 避免网络）
        let r = svc.proxy_pool.reserve_manual(1, false).unwrap();
        svc.proxy_pool.commit_reservation(r.token, 999).unwrap();
        let err = svc.remove_proxy(1).unwrap_err();
        assert_eq!(err.status_code(), axum::http::StatusCode::BAD_REQUEST);
        // 删除凭据后释放，可删
        assert_eq!(svc.proxy_pool.release_by_cred(999), 1);
        assert!(svc.remove_proxy(1).is_ok());
    }
}
