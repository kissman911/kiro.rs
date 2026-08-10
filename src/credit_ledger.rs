//! 积分消耗账本（kirors-b 专属）
//!
//! Kiro 上游只返回「当前累计用量」快照（`current_usage`），没有历史。
//! 号一删、一换、月度重置，数据就归零。本模块把快照差分成账本：
//!
//! - 每个凭据按**指纹**（apiKeyHash / refreshTokenHash）建账，凭据 ID 变了也能续上；
//! - 每次采样算 `delta = current_usage - last_usage`，累加成消耗；
//! - **我 / 他人拆分**：拼车号可能有别人在用。采样区间里我们自己的 `success_count`
//!   涨了就把 delta 记到「我」，没涨但用量涨了就记到「他人」；
//! - **轮次**（round）对应一轮车队。开新一轮时旧账归档，存活凭据以当前用量为新基线重开；
//! - 凭据删除前由上层补一次终值采样，然后标记 `alive=false` 留在本轮账上，避免换号丢账。
//!
//! 独立持久化到 `credit_ledger.json`（与 credentials.json 同目录），原子写 + `.bak` 兜底。

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// 归档保留的最大轮次数（防止文件无限增长）
const MAX_ARCHIVED_ROUNDS: usize = 20;

/// 用量回退判定容差：上游浮点抖动不算重置
const RESET_EPSILON: f64 = 0.5;

/// 单个凭据在**当前轮次**的积分账
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditEntry {
    /// 凭据指纹（apiKeyHash / refreshTokenHash，兜底 `id:<n>`）
    pub fingerprint: String,
    /// 所属轮次
    pub round_id: u64,
    /// 当前对应的凭据 ID（删除后保留最后已知值）
    pub cred_id: u64,
    /// 展示用标签（显示名 / 邮箱 / 脱敏 key）
    pub label: String,
    /// 订阅等级
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_title: Option<String>,
    /// 本轮起始用量基线
    pub baseline_usage: f64,
    /// 上次采样到的用量
    pub last_usage: f64,
    /// 上游额度上限（展示用）
    #[serde(default)]
    pub usage_limit: f64,
    /// 上次采样时我们自己的成功请求数
    pub last_success_count: u64,
    /// 本轮「我」消耗的积分
    pub my_credits: f64,
    /// 本轮「他人」消耗的积分（拼车同池其他人）
    pub others_credits: f64,
    /// 本轮我们自己的成功请求数增量
    #[serde(default)]
    pub my_requests: u64,
    /// 检测到用量重置的次数（月度重置 / 换号）
    #[serde(default)]
    pub resets: u32,
    /// 采样次数
    #[serde(default)]
    pub samples: u32,
    /// 首次入账时间（RFC3339）
    pub first_seen: String,
    /// 最后采样时间（RFC3339）
    pub last_seen: String,
    /// 凭据是否仍在 kiro-rs 中
    #[serde(default = "default_true")]
    pub alive: bool,
    /// 死号原因（禁用原因 / deleted）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dead_reason: Option<String>,
}

fn default_true() -> bool {
    true
}

impl CreditEntry {
    /// 本轮总消耗（我 + 他人）
    pub fn total_credits(&self) -> f64 {
        self.my_credits + self.others_credits
    }
}

/// 轮次元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundMeta {
    pub id: u64,
    /// 开始时间（RFC3339）
    pub started_at: String,
    /// 结束时间（仍在进行中为 None）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// 备注（可选，开轮时可写车队/来源说明）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// 提号接口来源标记（URL 主机 + token 尾 8 位，不存完整 token）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// 已归档轮次（汇总 + 明细）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedRound {
    pub meta: RoundMeta,
    pub entries: Vec<CreditEntry>,
}

impl ArchivedRound {
    pub fn total_my(&self) -> f64 {
        self.entries.iter().map(|e| e.my_credits).sum()
    }
    pub fn total_others(&self) -> f64 {
        self.entries.iter().map(|e| e.others_credits).sum()
    }
}

/// 账本落盘结构
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerData {
    /// 当前轮次
    pub current_round: RoundMeta,
    /// 当前轮次的凭据账（key = fingerprint）
    #[serde(default)]
    pub entries: HashMap<String, CreditEntry>,
    /// 历史轮次
    #[serde(default)]
    pub archived: Vec<ArchivedRound>,
    /// 下一个轮次 ID
    #[serde(default = "default_next_round")]
    pub next_round_id: u64,
    /// 最后一次采样时间（RFC3339）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sample_at: Option<String>,
}

fn default_next_round() -> u64 {
    2
}

impl Default for LedgerData {
    fn default() -> Self {
        Self {
            current_round: RoundMeta {
                id: 1,
                started_at: Utc::now().to_rfc3339(),
                ended_at: None,
                note: None,
                source: None,
            },
            entries: HashMap::new(),
            archived: Vec::new(),
            next_round_id: 2,
            last_sample_at: None,
        }
    }
}

/// 一次采样的输入（由上层从 token_manager + 上游额度组装）
#[derive(Debug, Clone)]
pub struct SampleInput {
    pub fingerprint: String,
    pub cred_id: u64,
    pub label: String,
    pub subscription_title: Option<String>,
    pub current_usage: f64,
    pub usage_limit: f64,
    pub success_count: u64,
}

/// 单条采样结果（便于日志与调试）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleOutcome {
    pub fingerprint: String,
    pub cred_id: u64,
    /// 本次增量归属：new / mine / others / reset / idle
    pub attribution: &'static str,
    pub delta: f64,
}

/// 采样汇总（Admin API 返回）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleReport {
    pub sampled: usize,
    pub failed: usize,
    pub my_delta: f64,
    pub others_delta: f64,
    pub outcomes: Vec<SampleOutcome>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

/// 账本错误
#[derive(Debug)]
pub enum LedgerError {
    Persist(String),
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::Persist(m) => write!(f, "账本持久化失败: {}", m),
        }
    }
}

impl std::error::Error for LedgerError {}

/// 积分账本（线程安全）
pub struct CreditLedger {
    inner: Mutex<LedgerData>,
    path: Option<PathBuf>,
}

impl CreditLedger {
    /// 从文件加载。解析失败先试 `.bak`，仍失败用默认（不阻断启动）。
    pub fn load(path: Option<PathBuf>) -> Self {
        let data = match &path {
            Some(p) if p.exists() => Self::load_from_disk(p),
            _ => LedgerData::default(),
        };
        Self {
            inner: Mutex::new(data),
            path,
        }
    }

    fn load_from_disk(p: &PathBuf) -> LedgerData {
        match std::fs::read_to_string(p) {
            Ok(content) => match serde_json::from_str::<LedgerData>(&content) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("解析积分账本失败 ({:?}): {}，尝试 .bak", p, e);
                    Self::load_from_bak(p)
                }
            },
            Err(e) => {
                tracing::error!("读取积分账本失败 ({:?}): {}，尝试 .bak", p, e);
                Self::load_from_bak(p)
            }
        }
    }

    fn load_from_bak(p: &PathBuf) -> LedgerData {
        let bak = Self::bak_path(p);
        match std::fs::read_to_string(&bak) {
            Ok(content) => serde_json::from_str::<LedgerData>(&content).unwrap_or_else(|e| {
                tracing::error!("解析积分账本 .bak 也失败: {}，用默认", e);
                LedgerData::default()
            }),
            Err(_) => LedgerData::default(),
        }
    }

    fn bak_path(p: &PathBuf) -> PathBuf {
        let mut b = p.clone();
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!("{}.bak", e))
            .unwrap_or_else(|| "bak".to_string());
        b.set_extension(ext);
        b
    }

    /// 原子持久化：备份旧文件 → 写临时文件 → rename 覆盖
    fn persist(data: &LedgerData, path: &Option<PathBuf>) -> Result<(), LedgerError> {
        let p = match path {
            Some(p) => p,
            None => return Ok(()),
        };
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| LedgerError::Persist(format!("序列化失败: {}", e)))?;
        if p.exists() {
            let bak = Self::bak_path(p);
            if let Err(e) = std::fs::copy(p, &bak) {
                tracing::warn!("备份积分账本到 .bak 失败（忽略）: {}", e);
            }
        }
        let tmp = {
            let mut t = p.clone();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!("{}.tmp", e))
                .unwrap_or_else(|| "tmp".to_string());
            t.set_extension(ext);
            t
        };
        std::fs::write(&tmp, &content)
            .map_err(|e| LedgerError::Persist(format!("写临时文件失败: {}", e)))?;
        std::fs::rename(&tmp, p).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            LedgerError::Persist(format!("原子替换失败: {}", e))
        })?;
        Ok(())
    }

    fn save_locked(data: &LedgerData, path: &Option<PathBuf>) {
        if let Err(e) = Self::persist(data, path) {
            tracing::error!("{}", e);
        }
    }

    /// 读取账本快照
    pub fn snapshot(&self) -> LedgerData {
        self.inner.lock().clone()
    }

    /// 应用一批采样，返回汇总。落盘一次。
    pub fn apply_samples(&self, inputs: Vec<SampleInput>) -> SampleReport {
        let now = Utc::now().to_rfc3339();
        let mut report = SampleReport {
            sampled: inputs.len(),
            failed: 0,
            my_delta: 0.0,
            others_delta: 0.0,
            outcomes: Vec::with_capacity(inputs.len()),
            errors: Vec::new(),
        };

        let mut d = self.inner.lock();
        let round_id = d.current_round.id;

        for input in inputs {
            let existing = d.entries.get_mut(&input.fingerprint);
            match existing {
                None => {
                    // 首次入账：当前用量作为基线，本次不计消耗
                    d.entries.insert(
                        input.fingerprint.clone(),
                        CreditEntry {
                            fingerprint: input.fingerprint.clone(),
                            round_id,
                            cred_id: input.cred_id,
                            label: input.label,
                            subscription_title: input.subscription_title,
                            baseline_usage: input.current_usage,
                            last_usage: input.current_usage,
                            usage_limit: input.usage_limit,
                            last_success_count: input.success_count,
                            my_credits: 0.0,
                            others_credits: 0.0,
                            my_requests: 0,
                            resets: 0,
                            samples: 1,
                            first_seen: now.clone(),
                            last_seen: now.clone(),
                            alive: true,
                            dead_reason: None,
                        },
                    );
                    report.outcomes.push(SampleOutcome {
                        fingerprint: input.fingerprint,
                        cred_id: input.cred_id,
                        attribution: "new",
                        delta: 0.0,
                    });
                }
                Some(e) => {
                    e.cred_id = input.cred_id;
                    e.label = input.label;
                    if input.subscription_title.is_some() {
                        e.subscription_title = input.subscription_title;
                    }
                    e.usage_limit = input.usage_limit;
                    e.last_seen = now.clone();
                    e.samples = e.samples.saturating_add(1);
                    e.alive = true;
                    e.dead_reason = None;

                    let prev_usage = e.last_usage;
                    let delta = input.current_usage - prev_usage;
                    let success_delta = input.success_count.saturating_sub(e.last_success_count);

                    let attribution = if delta < -RESET_EPSILON {
                        // 用量回退：月度重置或号被换掉。重设基线，不倒扣已记账的消耗。
                        e.resets = e.resets.saturating_add(1);
                        e.baseline_usage = input.current_usage;
                        e.last_usage = input.current_usage;
                        e.last_success_count = input.success_count;
                        e.my_requests = e.my_requests.saturating_add(success_delta);
                        tracing::info!(
                            "凭据 #{} 用量回退（{:.2} → {:.2}），判定重置，已重设基线",
                            input.cred_id,
                            prev_usage,
                            input.current_usage
                        );
                        report.outcomes.push(SampleOutcome {
                            fingerprint: input.fingerprint,
                            cred_id: input.cred_id,
                            attribution: "reset",
                            delta,
                        });
                        continue;
                    } else if delta <= 0.0 {
                        // 无增长（含上游浮点抖动的微小回退）
                        "idle"
                    } else if success_delta > 0 {
                        // 本区间我们自己有成功请求 → 记到「我」
                        e.my_credits += delta;
                        report.my_delta += delta;
                        "mine"
                    } else {
                        // 我们没打请求但用量涨了 → 拼车同池其他人消耗
                        e.others_credits += delta;
                        report.others_delta += delta;
                        "others"
                    };

                    e.last_usage = input.current_usage;
                    e.last_success_count = input.success_count;
                    e.my_requests = e.my_requests.saturating_add(success_delta);

                    report.outcomes.push(SampleOutcome {
                        fingerprint: input.fingerprint,
                        cred_id: input.cred_id,
                        attribution,
                        delta: delta.max(0.0),
                    });
                }
            }
        }

        d.last_sample_at = Some(now);
        Self::save_locked(&d, &self.path);
        report
    }

    /// 标记凭据已死（禁用原因 / 删除），保留在本轮账上
    pub fn mark_dead(&self, fingerprint: &str, reason: impl Into<String>) {
        let mut d = self.inner.lock();
        if let Some(e) = d.entries.get_mut(fingerprint) {
            e.alive = false;
            e.dead_reason = Some(reason.into());
            Self::save_locked(&d, &self.path);
        }
    }

    /// 开启新一轮：当前轮归档，存活凭据以当前用量为新基线续开
    pub fn start_new_round(
        &self,
        note: Option<String>,
        source: Option<String>,
    ) -> Result<RoundMeta, LedgerError> {
        let now = Utc::now().to_rfc3339();
        let mut d = self.inner.lock();

        // 归档当前轮
        let mut meta = d.current_round.clone();
        meta.ended_at = Some(now.clone());
        let entries: Vec<CreditEntry> = d.entries.values().cloned().collect();
        if !entries.is_empty() {
            d.archived.push(ArchivedRound { meta, entries });
            if d.archived.len() > MAX_ARCHIVED_ROUNDS {
                let drop_n = d.archived.len() - MAX_ARCHIVED_ROUNDS;
                d.archived.drain(0..drop_n);
            }
        }

        // 新轮：存活凭据延续（基线 = 当前用量），死号不带进新轮
        let new_id = d.next_round_id;
        let carried: HashMap<String, CreditEntry> = d
            .entries
            .iter()
            .filter(|(_, e)| e.alive)
            .map(|(k, e)| {
                let mut n = e.clone();
                n.round_id = new_id;
                n.baseline_usage = e.last_usage;
                n.my_credits = 0.0;
                n.others_credits = 0.0;
                n.my_requests = 0;
                n.resets = 0;
                n.samples = 0;
                n.first_seen = now.clone();
                n.last_seen = now.clone();
                (k.clone(), n)
            })
            .collect();

        d.entries = carried;
        d.current_round = RoundMeta {
            id: new_id,
            started_at: now,
            ended_at: None,
            note,
            source,
        };
        d.next_round_id = new_id + 1;

        let meta = d.current_round.clone();
        Self::persist(&d, &self.path)?;
        Ok(meta)
    }

    /// 更新当前轮的来源标记（提号接口 host + token 尾号）
    pub fn set_current_source(&self, source: Option<String>) {
        let mut d = self.inner.lock();
        if d.current_round.source != source {
            d.current_round.source = source;
            Self::save_locked(&d, &self.path);
        }
    }

    /// 清空全部账本（含历史），慎用
    pub fn reset_all(&self) -> Result<(), LedgerError> {
        let mut d = self.inner.lock();
        *d = LedgerData::default();
        Self::persist(&d, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(fp: &str, usage: f64, success: u64) -> SampleInput {
        SampleInput {
            fingerprint: fp.to_string(),
            cred_id: 1,
            label: "test".to_string(),
            subscription_title: Some("KIRO PRO+".to_string()),
            current_usage: usage,
            usage_limit: 1000.0,
            success_count: success,
        }
    }

    #[test]
    fn first_sample_sets_baseline_without_charging() {
        let l = CreditLedger::load(None);
        let r = l.apply_samples(vec![input("fp1", 120.0, 5)]);
        assert_eq!(r.my_delta, 0.0);
        assert_eq!(r.others_delta, 0.0);
        let e = &l.snapshot().entries["fp1"];
        assert_eq!(e.baseline_usage, 120.0);
        assert_eq!(e.total_credits(), 0.0);
    }

    #[test]
    fn delta_with_own_requests_counts_as_mine() {
        let l = CreditLedger::load(None);
        l.apply_samples(vec![input("fp1", 100.0, 0)]);
        let r = l.apply_samples(vec![input("fp1", 130.0, 4)]);
        assert_eq!(r.my_delta, 30.0);
        assert_eq!(r.others_delta, 0.0);
        let e = &l.snapshot().entries["fp1"];
        assert_eq!(e.my_credits, 30.0);
        assert_eq!(e.my_requests, 4);
    }

    #[test]
    fn delta_without_own_requests_counts_as_others() {
        let l = CreditLedger::load(None);
        l.apply_samples(vec![input("fp1", 100.0, 7)]);
        let r = l.apply_samples(vec![input("fp1", 155.0, 7)]);
        assert_eq!(r.others_delta, 55.0);
        assert_eq!(r.my_delta, 0.0);
        let e = &l.snapshot().entries["fp1"];
        assert_eq!(e.others_credits, 55.0);
    }

    #[test]
    fn usage_rollback_resets_baseline_without_deducting() {
        let l = CreditLedger::load(None);
        l.apply_samples(vec![input("fp1", 100.0, 0)]);
        l.apply_samples(vec![input("fp1", 180.0, 3)]);
        let r = l.apply_samples(vec![input("fp1", 10.0, 3)]);
        assert_eq!(r.my_delta, 0.0);
        let e = &l.snapshot().entries["fp1"];
        assert_eq!(e.my_credits, 80.0, "已记账消耗不倒扣");
        assert_eq!(e.baseline_usage, 10.0);
        assert_eq!(e.resets, 1);
    }

    #[test]
    fn dead_credential_stays_on_current_round() {
        let l = CreditLedger::load(None);
        l.apply_samples(vec![input("fp1", 100.0, 0)]);
        l.apply_samples(vec![input("fp1", 150.0, 2)]);
        l.mark_dead("fp1", "deleted");
        let e = &l.snapshot().entries["fp1"];
        assert!(!e.alive);
        assert_eq!(e.my_credits, 50.0);
    }

    #[test]
    fn new_round_archives_and_carries_alive_entries() {
        let l = CreditLedger::load(None);
        l.apply_samples(vec![input("alive", 100.0, 0), input("dead", 200.0, 0)]);
        l.apply_samples(vec![input("alive", 140.0, 3), input("dead", 260.0, 5)]);
        l.mark_dead("dead", "deleted");

        l.start_new_round(Some("round2".into()), None).unwrap();
        let s = l.snapshot();
        assert_eq!(s.current_round.id, 2);
        assert_eq!(s.archived.len(), 1);
        assert_eq!(s.archived[0].total_my(), 100.0, "旧轮汇总 40 + 60");
        assert!(s.entries.contains_key("alive"));
        assert!(!s.entries.contains_key("dead"), "死号不带进新轮");
        let carried = &s.entries["alive"];
        assert_eq!(carried.baseline_usage, 140.0);
        assert_eq!(carried.my_credits, 0.0);
    }

    #[test]
    fn new_round_then_sampling_charges_from_new_baseline() {
        let l = CreditLedger::load(None);
        l.apply_samples(vec![input("fp1", 100.0, 0)]);
        l.apply_samples(vec![input("fp1", 150.0, 2)]);
        l.start_new_round(None, None).unwrap();
        let r = l.apply_samples(vec![input("fp1", 175.0, 5)]);
        assert_eq!(r.my_delta, 25.0);
        assert_eq!(l.snapshot().entries["fp1"].my_credits, 25.0);
    }
}
