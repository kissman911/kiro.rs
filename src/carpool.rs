//! 拼车补号配置模块（kirors-b 专属）
//!
//! 只负责「持久化存储 + 线程安全读写」拼车补号的配置。
//! 实际的补号动作由外部 daemon（kiro-carpool-feeder）执行，
//! daemon 每轮通过 Admin API 拉取本配置 → 面板改配置即时热更新。
//!
//! 独立持久化到 `carpool.json`（与 credentials.json 同目录）。
//! URL 含 token：Admin API 已鉴权，daemon 需要完整 URL 才能工作，故明文返回。

use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

fn default_target_active() -> u32 {
    3
}
fn default_poll_interval() -> u32 {
    30
}
fn default_recent_window() -> u32 {
    20
}
fn default_min_sample() -> u32 {
    10
}
fn default_disable_err_ratio() -> f64 {
    0.4
}
fn default_healthy_err_ratio() -> f64 {
    0.2
}

/// 拼车补号配置（面板可配，daemon 消费）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CarpoolSettings {
    /// 总开关：false 时 daemon 不补号（只读状态，不动 kiro 凭据）
    #[serde(default)]
    pub enabled: bool,
    /// 自动提 JSON 接口完整 URL（含 token）
    #[serde(default)]
    pub get_url: String,
    /// 维持的活号数
    #[serde(default = "default_target_active")]
    pub target_active: u32,
    /// daemon 轮询秒数
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u32,
    /// 只读演练：true 时 daemon 只打印不改动
    #[serde(default)]
    pub dry_run: bool,
    /// 健康度判定：看最近 N 次请求
    #[serde(default = "default_recent_window")]
    pub recent_window: u32,
    /// 样本不足不判（避免误杀新号）
    #[serde(default = "default_min_sample")]
    pub min_sample: u32,
    /// 错误率 >= 此值 → 报错多（候选禁用）
    #[serde(default = "default_disable_err_ratio")]
    pub disable_err_ratio: f64,
    /// 错误率 < 此值 → 健康号（永不禁）
    #[serde(default = "default_healthy_err_ratio")]
    pub healthy_err_ratio: f64,
}

impl Default for CarpoolSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            get_url: String::new(),
            target_active: default_target_active(),
            poll_interval: default_poll_interval(),
            dry_run: false,
            recent_window: default_recent_window(),
            min_sample: default_min_sample(),
            disable_err_ratio: default_disable_err_ratio(),
            healthy_err_ratio: default_healthy_err_ratio(),
        }
    }
}

/// 部分更新请求：所有字段可选，只覆盖传入项
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CarpoolSettingsPatch {
    pub enabled: Option<bool>,
    #[serde(alias = "getUrl", alias = "carpoolGetUrl", alias = "url")]
    pub get_url: Option<String>,
    pub target_active: Option<u32>,
    pub poll_interval: Option<u32>,
    pub dry_run: Option<bool>,
    pub recent_window: Option<u32>,
    pub min_sample: Option<u32>,
    pub disable_err_ratio: Option<f64>,
    pub healthy_err_ratio: Option<f64>,
}

/// 校验错误
#[derive(Debug)]
pub enum CarpoolError {
    Invalid(String),
    Persist(String),
}

impl std::fmt::Display for CarpoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CarpoolError::Invalid(m) => write!(f, "配置非法: {}", m),
            CarpoolError::Persist(m) => write!(f, "持久化失败: {}", m),
        }
    }
}

impl std::error::Error for CarpoolError {}

/// 拼车配置（线程安全）
pub struct Carpool {
    inner: Mutex<CarpoolSettings>,
    path: Option<PathBuf>,
}

impl Carpool {
    /// 从文件加载。解析失败时告警并尝试 `.bak`，仍失败则用默认。
    pub fn load(path: Option<PathBuf>) -> Self {
        let settings = match &path {
            Some(p) if p.exists() => Self::load_from_disk(p),
            _ => CarpoolSettings::default(),
        };
        Self {
            inner: Mutex::new(settings),
            path,
        }
    }

    fn load_from_disk(p: &PathBuf) -> CarpoolSettings {
        match std::fs::read_to_string(p) {
            Ok(content) => match serde_json::from_str::<CarpoolSettings>(&content) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("解析拼车配置失败 ({:?}): {}，尝试 .bak", p, e);
                    Self::load_from_bak(p)
                }
            },
            Err(e) => {
                tracing::error!("读取拼车配置失败 ({:?}): {}，尝试 .bak", p, e);
                Self::load_from_bak(p)
            }
        }
    }

    fn load_from_bak(p: &PathBuf) -> CarpoolSettings {
        let bak = Self::bak_path(p);
        match std::fs::read_to_string(&bak) {
            Ok(content) => serde_json::from_str::<CarpoolSettings>(&content).unwrap_or_else(|e| {
                tracing::error!("解析拼车 .bak 也失败: {}，用默认", e);
                CarpoolSettings::default()
            }),
            Err(_) => CarpoolSettings::default(),
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

    /// 原子持久化：备份旧文件 → 写临时文件 → rename 覆盖。
    fn persist(settings: &CarpoolSettings, path: &Option<PathBuf>) -> Result<(), CarpoolError> {
        let p = match path {
            Some(p) => p,
            None => return Ok(()),
        };
        let content = serde_json::to_string_pretty(settings)
            .map_err(|e| CarpoolError::Persist(format!("序列化失败: {}", e)))?;
        if p.exists() {
            let bak = Self::bak_path(p);
            if let Err(e) = std::fs::copy(p, &bak) {
                tracing::warn!("备份拼车配置到 .bak 失败（忽略）: {}", e);
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
            .map_err(|e| CarpoolError::Persist(format!("写临时文件失败: {}", e)))?;
        std::fs::rename(&tmp, p).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            CarpoolError::Persist(format!("原子替换失败: {}", e))
        })?;
        Ok(())
    }

    /// 读取当前配置
    pub fn settings(&self) -> CarpoolSettings {
        self.inner.lock().clone()
    }

    /// 应用部分更新并持久化，返回更新后的完整配置
    pub fn patch(&self, req: CarpoolSettingsPatch) -> Result<CarpoolSettings, CarpoolError> {
        // 锁外校验 URL
        let get_url = match req.get_url {
            Some(u) => {
                let u = u.trim().to_string();
                if !u.is_empty() && !(u.starts_with("http://") || u.starts_with("https://")) {
                    return Err(CarpoolError::Invalid(
                        "get_url 必须以 http:// 或 https:// 开头".to_string(),
                    ));
                }
                Some(u)
            }
            None => None,
        };
        if let Some(r) = req.disable_err_ratio {
            if !(0.0..=1.0).contains(&r) {
                return Err(CarpoolError::Invalid("disable_err_ratio 需在 0..=1".to_string()));
            }
        }
        if let Some(r) = req.healthy_err_ratio {
            if !(0.0..=1.0).contains(&r) {
                return Err(CarpoolError::Invalid("healthy_err_ratio 需在 0..=1".to_string()));
            }
        }
        if let Some(p) = req.poll_interval {
            if p < 5 {
                return Err(CarpoolError::Invalid("poll_interval 最小 5 秒".to_string()));
            }
        }

        let mut s = self.inner.lock();
        if let Some(v) = req.enabled {
            s.enabled = v;
        }
        if let Some(v) = get_url {
            s.get_url = v;
        }
        if let Some(v) = req.target_active {
            s.target_active = v;
        }
        if let Some(v) = req.poll_interval {
            s.poll_interval = v;
        }
        if let Some(v) = req.dry_run {
            s.dry_run = v;
        }
        if let Some(v) = req.recent_window {
            s.recent_window = v;
        }
        if let Some(v) = req.min_sample {
            s.min_sample = v;
        }
        if let Some(v) = req.disable_err_ratio {
            s.disable_err_ratio = v;
        }
        if let Some(v) = req.healthy_err_ratio {
            s.healthy_err_ratio = v;
        }
        let cloned = s.clone();
        Self::persist(&s, &self.path)?;
        Ok(cloned)
    }
}
