//! IP 代理池模块
//!
//! 管理一组可复用的代理 IP，供添加凭据时自动/手动分配。
//!
//! 设计要点（方案 A）：
//! - 分配时把选中代理的 url/username/password 写入凭据自身的 proxy_url 字段，
//!   运行时请求路径沿用现有 `effective_proxy` 机制，无需改动。
//! - 代理池只记录 `assignments`（每个代理挂了哪些 credId），用于空闲/复用判定与统计。
//! - 一个代理 IP 可挂多个凭据（支持"无空闲时复用在用 IP"）。
//! - 独立持久化到 `proxy_pool.json`（与 credentials.json 同目录）。
//!
//! 并发安全（reservation 机制）：
//! - 分配分两步：先原子 `reserve_*` 预占（占位 token，不写盘），凭据创建/探测成功后
//!   `commit_reservation` 转为真实 credId 并落盘；失败则 `cancel_reservation` 回滚。
//! - 预占计入负载，避免并发下多个请求选中同一个空闲代理。
//! - reservation 仅存内存（不持久化），进程重启后自动失效，不会造成幽灵占用。

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::http_client::{ProxyConfig, build_client};
use crate::model::config::TlsBackend;

/// 探测结果缓存 TTL：成功 5 分钟
const PROBE_OK_TTL_SECS: i64 = 300;
/// 探测结果缓存 TTL：失败冷却 60 秒（避免批量分配时对坏代理反复探测）
const PROBE_FAIL_TTL_SECS: i64 = 60;

/// 默认探测 URL（返回出口 IP，海内外相对稳定）
pub fn default_probe_url() -> String {
    "https://api.ipify.org?format=json".to_string()
}

fn default_true() -> bool {
    true
}

/// 代理池操作错误
#[derive(Debug, Clone)]
pub enum PoolError {
    /// 代理不存在
    NotFound,
    /// 非法输入（URL 校验失败、参数错误等）
    Invalid(String),
    /// 占用中/复用冲突等业务约束
    Conflict(String),
    /// 持久化失败（磁盘写入错误）——需向调用层传播，避免 API 假成功
    Persist(String),
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolError::NotFound => write!(f, "代理不存在"),
            PoolError::Invalid(m) => write!(f, "{}", m),
            PoolError::Conflict(m) => write!(f, "{}", m),
            PoolError::Persist(m) => write!(f, "代理池持久化失败: {}", m),
        }
    }
}

impl std::error::Error for PoolError {}

/// 批量导入单行解析结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedProxyLine {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub label: Option<String>,
}

/// 解析批量导入的一行，支持两种格式：
///
/// 1. 空格分隔（原格式）：`url [username] [password] [label...]`
///    例：`socks5://1.2.3.4:1080 user pass 美国静态`
/// 2. 冒号分隔（代理商常见导出格式）：`host:port:username:password`
///    例：`63.246.151.171:5502:kmkmhuyw:3d1it5o1kxnu`
///    无协议前缀时补 `socks5://`（实测该类代理 socks5 与 http 均可用，
///    socks5 支持 UDP 与远端 DNS 解析，作为默认更通用）。
///
/// 冒号格式也接受带协议前缀的写法，如 `http://host:port:user:pass`，
/// 此时保留原协议。仅 `host:port` 两段时视为无认证代理。
pub fn parse_proxy_line(raw: &str) -> Result<ParsedProxyLine, PoolError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(PoolError::Invalid("代理行不能为空".to_string()));
    }

    // 含空白字符 → 按原有空格分隔格式解析
    if trimmed.split_whitespace().count() > 1 {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        return Ok(ParsedProxyLine {
            url: parts[0].to_string(),
            username: parts.get(1).map(|s| s.to_string()),
            password: parts.get(2).map(|s| s.to_string()),
            label: if parts.len() > 3 {
                Some(parts[3..].join(" "))
            } else {
                None
            },
        });
    }

    // 拆出协议前缀（如有），剩余部分按冒号分段
    let (scheme, rest) = match trimmed.split_once("://") {
        Some((s, r)) => (Some(s.to_ascii_lowercase()), r),
        None => (None, trimmed),
    };

    let segments: Vec<&str> = rest.split(':').collect();
    match segments.len() {
        // host:port —— 无认证
        2 => Ok(ParsedProxyLine {
            url: format!(
                "{}://{}:{}",
                scheme.as_deref().unwrap_or("socks5"),
                segments[0],
                segments[1]
            ),
            username: None,
            password: None,
            label: None,
        }),
        // host:port:username:password
        4 => {
            if segments[2].is_empty() || segments[3].is_empty() {
                return Err(PoolError::Invalid(
                    "host:port:username:password 格式中用户名与密码不能为空".to_string(),
                ));
            }
            Ok(ParsedProxyLine {
                url: format!(
                    "{}://{}:{}",
                    scheme.as_deref().unwrap_or("socks5"),
                    segments[0],
                    segments[1]
                ),
                username: Some(segments[2].to_string()),
                password: Some(segments[3].to_string()),
                label: None,
            })
        }
        // 单段：可能是 http://host（依赖默认端口），交给 URL 校验判定
        1 => Ok(ParsedProxyLine {
            url: trimmed.to_string(),
            username: None,
            password: None,
            label: None,
        }),
        n => Err(PoolError::Invalid(format!(
            "无法识别的代理格式（冒号分段数 {}）。支持 host:port、host:port:user:pass，或空格分隔的 url user pass",
            n
        ))),
    }
}

/// 校验代理 URL：scheme 必须是 http/https/socks5/socks5h，且含 host + port。
pub fn validate_proxy_url(raw: &str) -> Result<(), PoolError> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(PoolError::Invalid("代理 URL 不能为空".to_string()));
    }
    let url = reqwest::Url::parse(s)
        .map_err(|e| PoolError::Invalid(format!("代理 URL 格式非法: {}", e)))?;
    let scheme = url.scheme().to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https" | "socks5" | "socks5h") {
        return Err(PoolError::Invalid(format!(
            "不支持的代理协议 \"{}\"，仅支持 http/https/socks5/socks5h",
            scheme
        )));
    }
    match url.host_str() {
        Some(h) if !h.is_empty() => {}
        _ => return Err(PoolError::Invalid("代理 URL 缺少主机名".to_string())),
    }
    // 要求有可用端口：http/https 有已知默认端口（80/443），socks5/socks5h 无默认，必须显式指定。
    if url.port_or_known_default().is_none() {
        return Err(PoolError::Invalid(
            "代理 URL 必须指定端口，如 socks5://1.2.3.4:1080".to_string(),
        ));
    }
    Ok(())
}

/// 判断 IP 是否属于禁止访问的范围（SSRF 防护）：
/// 回环 / 未指定 / 私网 / 链路本地 / 唯一本地地址等。
fn is_forbidden_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_forbidden_v4(v4),
        IpAddr::V6(v6) => is_forbidden_v6(v6),
    }
}

fn is_forbidden_v4(ip: &Ipv4Addr) -> bool {
    ip.is_loopback()          // 127.0.0.0/8
        || ip.is_private()    // 10/8, 172.16/12, 192.168/16
        || ip.is_link_local() // 169.254/16
        || ip.is_unspecified()// 0.0.0.0
        || ip.is_broadcast()  // 255.255.255.255
        || ip.is_documentation()
        || {
            // CGNAT 100.64.0.0/10
            let o = ip.octets();
            o[0] == 100 && (o[1] & 0xc0) == 64
        }
}

fn is_forbidden_v6(ip: &Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    let seg = ip.segments();
    // 唯一本地地址 fc00::/7
    if (seg[0] & 0xfe00) == 0xfc00 {
        return true;
    }
    // 链路本地 fe80::/10
    if (seg[0] & 0xffc0) == 0xfe80 {
        return true;
    }
    // IPv4-mapped ::ffff:0:0/96 → 按内嵌 v4 判定
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_forbidden_v4(&v4);
    }
    false
}

/// 校验探测 URL（SSRF 防护）：
/// - 仅允许 https
/// - 禁止 localhost / 明显私网主机名
/// - 若 host 为 IP 字面量，禁止私网/回环/链路本地
///
/// 注意：主机名解析出的 IP 在实际探测前 (`probe_proxy`) 再做一次校验，
/// 以防止通过 DNS 指向内网。
pub fn validate_probe_url(raw: &str) -> Result<(), PoolError> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(PoolError::Invalid("探测 URL 不能为空".to_string()));
    }
    let url = reqwest::Url::parse(s)
        .map_err(|e| PoolError::Invalid(format!("探测 URL 格式非法: {}", e)))?;
    if !url.scheme().eq_ignore_ascii_case("https") {
        return Err(PoolError::Invalid("探测 URL 必须使用 https".to_string()));
    }
    let host = url
        .host_str()
        .ok_or_else(|| PoolError::Invalid("探测 URL 缺少主机名".to_string()))?;
    let host_l = host.to_ascii_lowercase();
    if host_l == "localhost" || host_l.ends_with(".localhost") || host_l == "ip6-localhost" {
        return Err(PoolError::Invalid(
            "探测 URL 不能指向 localhost".to_string(),
        ));
    }
    // IP 字面量直接校验
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_forbidden_ip(&ip) {
            return Err(PoolError::Invalid(
                "探测 URL 不能指向私网/回环/链路本地地址".to_string(),
            ));
        }
    } else if let Some(stripped) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        // IPv6 带方括号
        if let Ok(ip) = stripped.parse::<IpAddr>() {
            if is_forbidden_ip(&ip) {
                return Err(PoolError::Invalid(
                    "探测 URL 不能指向私网/回环/链路本地地址".to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// 代理探测结果（记录最近一次校验）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    /// 是否连通
    pub ok: bool,
    /// 延迟（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    /// 出口 IP（探测 URL 返回时，且为合法 IP 才记录）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    /// 失败信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// 探测时间（RFC3339）
    pub at: String,
}

impl ProbeResult {
    /// 探测结果是否仍在 TTL 内（成功 5min / 失败 60s）
    fn is_fresh(&self) -> bool {
        let parsed = chrono::DateTime::parse_from_rfc3339(&self.at);
        let at = match parsed {
            Ok(t) => t.timestamp(),
            Err(_) => return false,
        };
        let ttl = if self.ok {
            PROBE_OK_TTL_SECS
        } else {
            PROBE_FAIL_TTL_SECS
        };
        let now = chrono::Utc::now().timestamp();
        (now - at) < ttl
    }
}

/// 单个代理条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyEntry {
    pub id: u64,
    /// 代理地址（http/https/socks5://host:port）
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub disabled: bool,
    /// 挂载的凭据 ID 列表（空 = 空闲）
    #[serde(default)]
    pub assignments: Vec<u64>,
    /// 最近一次探测结果
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_check: Option<ProbeResult>,
    /// 未落盘的预占 token（仅内存，进程重启失效）
    #[serde(skip)]
    reserved: Vec<u64>,
}

impl ProxyEntry {
    /// 已挂载凭据数（实际绑定，不含预占）
    pub fn usage_count(&self) -> usize {
        self.assignments.len()
    }
    /// 负载 = 实际挂载 + 预占（用于分配时的负载均衡与空闲判定）
    fn load(&self) -> usize {
        self.assignments.len() + self.reserved.len()
    }
    /// 空闲：未禁用、无挂载、无预占
    pub fn is_free(&self) -> bool {
        !self.disabled && self.assignments.is_empty() && self.reserved.is_empty()
    }
    /// 转换为可用于构建请求的 ProxyConfig
    pub fn to_proxy_config(&self) -> ProxyConfig {
        let mut c = ProxyConfig::new(self.url.clone());
        if let (Some(u), Some(p)) = (&self.username, &self.password) {
            c = c.with_auth(u.clone(), p.clone());
        }
        c
    }
}

/// 代理池设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyPoolSettings {
    /// 添加/批量导入凭据时是否默认从池自动分配
    #[serde(default = "default_true")]
    pub auto_assign_enabled: bool,
    /// 探测 URL（可配置）
    #[serde(default = "default_probe_url")]
    pub probe_url: String,
}

impl Default for ProxyPoolSettings {
    fn default() -> Self {
        Self {
            auto_assign_enabled: true,
            probe_url: default_probe_url(),
        }
    }
}

/// 磁盘持久化结构
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ProxyPoolData {
    #[serde(default)]
    settings: ProxyPoolSettings,
    #[serde(default)]
    proxies: Vec<ProxyEntry>,
    #[serde(default)]
    next_id: u64,
}

/// 预占句柄：分配成功但尚未提交
#[derive(Debug, Clone)]
pub struct Reservation {
    /// 预占的代理快照（含 url/账号密码，供构建凭据）
    pub proxy: ProxyEntry,
    /// 预占 token（commit/cancel 时使用）
    pub token: u64,
    /// 是否复用了在用代理（多凭据共享）
    pub reused: bool,
}

/// 统计
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyPoolStats {
    pub total: usize,
    pub available: usize,
    pub assigned: usize,
    pub shared: usize,
    pub disabled: usize,
}

/// 批量添加结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAddResult {
    pub added: Vec<ProxyEntry>,
    /// 逐行错误：(行号从 1 起, 原始内容, 错误信息)
    pub errors: Vec<BatchAddError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAddError {
    pub line: usize,
    pub content: String,
    pub error: String,
}

/// 代理池（线程安全）
pub struct ProxyPool {
    inner: Mutex<ProxyPoolData>,
    /// 预占 token 自增计数（仅内存）
    reservation_seq: Mutex<u64>,
    path: Option<PathBuf>,
    tls_backend: TlsBackend,
}

impl ProxyPool {
    /// 从文件加载。解析失败时告警并尝试 `.bak`，仍失败则返回空池。
    pub fn load(path: Option<PathBuf>, tls_backend: TlsBackend) -> Self {
        let data = match &path {
            Some(p) if p.exists() => Self::load_from_disk(p),
            _ => ProxyPoolData::default(),
        };
        let mut data = data;
        // 修正 next_id，防止旧文件缺失或过小
        let max_id = data.proxies.iter().map(|p| p.id).max().unwrap_or(0);
        if data.next_id <= max_id {
            data.next_id = max_id + 1;
        }
        if data.next_id == 0 {
            data.next_id = 1;
        }
        // 清空可能残留的预占（内存字段，反序列化后本应为空，双保险）
        for p in data.proxies.iter_mut() {
            p.reserved.clear();
        }
        Self {
            inner: Mutex::new(data),
            reservation_seq: Mutex::new(1),
            path,
            tls_backend,
        }
    }

    /// 读取并解析磁盘文件，失败时尝试 `.bak`。
    fn load_from_disk(p: &PathBuf) -> ProxyPoolData {
        match std::fs::read_to_string(p) {
            Ok(content) => match serde_json::from_str::<ProxyPoolData>(&content) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("解析代理池文件失败 ({:?}): {}，尝试从 .bak 恢复", p, e);
                    Self::load_from_bak(p)
                }
            },
            Err(e) => {
                tracing::error!("读取代理池文件失败 ({:?}): {}，尝试从 .bak 恢复", p, e);
                Self::load_from_bak(p)
            }
        }
    }

    fn load_from_bak(p: &PathBuf) -> ProxyPoolData {
        let bak = Self::bak_path(p);
        match std::fs::read_to_string(&bak) {
            Ok(content) => match serde_json::from_str::<ProxyPoolData>(&content) {
                Ok(d) => {
                    tracing::warn!("已从 .bak 恢复代理池: {:?}", bak);
                    d
                }
                Err(e) => {
                    tracing::error!("解析代理池 .bak 也失败 ({:?}): {}，使用空池", bak, e);
                    ProxyPoolData::default()
                }
            },
            Err(_) => {
                tracing::warn!("代理池 .bak 不存在，使用空池");
                ProxyPoolData::default()
            }
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

    /// 原子持久化：先备份旧文件到 .bak，写临时文件，再 rename 覆盖。
    /// 任一步失败均返回 `PoolError::Persist`，供调用层传播。
    fn persist(data: &ProxyPoolData, path: &Option<PathBuf>) -> Result<(), PoolError> {
        let p = match path {
            Some(p) => p,
            None => return Ok(()), // 无持久化路径（如测试），直接成功
        };
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| PoolError::Persist(format!("序列化失败: {}", e)))?;

        // 备份现有文件（best-effort，失败仅告警不阻断）
        if p.exists() {
            let bak = Self::bak_path(p);
            if let Err(e) = std::fs::copy(p, &bak) {
                tracing::warn!("备份代理池到 .bak 失败（忽略）: {}", e);
            }
        }

        // 写临时文件 + 原子 rename
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
            .map_err(|e| PoolError::Persist(format!("写临时文件失败: {}", e)))?;
        std::fs::rename(&tmp, p).map_err(|e| {
            // rename 失败时清理临时文件
            let _ = std::fs::remove_file(&tmp);
            PoolError::Persist(format!("原子替换失败: {}", e))
        })?;
        Ok(())
    }

    fn next_reservation_token(&self) -> u64 {
        let mut seq = self.reservation_seq.lock();
        let t = *seq;
        *seq += 1;
        t
    }

    // ---- 查询 ----

    pub fn list(&self) -> Vec<ProxyEntry> {
        self.inner.lock().proxies.clone()
    }

    pub fn get(&self, id: u64) -> Option<ProxyEntry> {
        self.inner
            .lock()
            .proxies
            .iter()
            .find(|p| p.id == id)
            .cloned()
    }

    pub fn settings(&self) -> ProxyPoolSettings {
        self.inner.lock().settings.clone()
    }

    pub fn is_auto_assign_enabled(&self) -> bool {
        self.inner.lock().settings.auto_assign_enabled
    }

    pub fn probe_url(&self) -> String {
        self.inner.lock().settings.probe_url.clone()
    }

    pub fn stats(&self) -> ProxyPoolStats {
        let d = self.inner.lock();
        ProxyPoolStats {
            total: d.proxies.len(),
            available: d.proxies.iter().filter(|p| p.is_free()).count(),
            assigned: d.proxies.iter().filter(|p| p.usage_count() > 0).count(),
            shared: d.proxies.iter().filter(|p| p.usage_count() > 1).count(),
            disabled: d.proxies.iter().filter(|p| p.disabled).count(),
        }
    }

    // ---- 增删改 ----

    pub fn add(
        &self,
        url: String,
        username: Option<String>,
        password: Option<String>,
        label: Option<String>,
    ) -> Result<ProxyEntry, PoolError> {
        let url = url.trim().to_string();
        validate_proxy_url(&url)?;
        let mut d = self.inner.lock();
        let id = d.next_id;
        d.next_id += 1;
        let entry = ProxyEntry {
            id,
            url,
            username: username.filter(|s| !s.is_empty()),
            password: password.filter(|s| !s.is_empty()),
            label: label.unwrap_or_default(),
            disabled: false,
            assignments: Vec::new(),
            last_check: None,
            reserved: Vec::new(),
        };
        d.proxies.push(entry.clone());
        Self::persist(&d, &self.path)?;
        Ok(entry)
    }

    /// 批量添加，每行支持两种格式：
    /// - `url [username] [password] [label...]`（空格分隔）
    /// - `host:port:username:password` 或 `host:port`（代理商常见导出格式，缺协议时补 socks5）
    ///
    /// 逐行校验，非法行进入 errors，不阻断其它行。
    pub fn batch_add(&self, lines: &[String]) -> BatchAddResult {
        let mut added = Vec::new();
        let mut errors = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            let lineno = idx + 1;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parsed = match parse_proxy_line(trimmed) {
                Ok(p) => p,
                Err(e) => {
                    errors.push(BatchAddError {
                        line: lineno,
                        content: trimmed.to_string(),
                        error: e.to_string(),
                    });
                    continue;
                }
            };
            match self.add(parsed.url, parsed.username, parsed.password, parsed.label) {
                Ok(e) => added.push(e),
                Err(e) => errors.push(BatchAddError {
                    line: lineno,
                    content: trimmed.to_string(),
                    error: e.to_string(),
                }),
            }
        }
        BatchAddResult { added, errors }
    }

    /// 更新代理基础字段（不含 assignments）
    pub fn update(
        &self,
        id: u64,
        url: Option<String>,
        username: Option<Option<String>>,
        password: Option<Option<String>>,
        label: Option<String>,
    ) -> Result<ProxyEntry, PoolError> {
        // 先校验 URL（在锁外）
        if let Some(u) = url.as_ref() {
            validate_proxy_url(u)?;
        }
        let mut d = self.inner.lock();
        let entry = d
            .proxies
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or(PoolError::NotFound)?;
        if let Some(u) = url {
            entry.url = u.trim().to_string();
        }
        if let Some(u) = username {
            entry.username = u.filter(|s| !s.is_empty());
        }
        if let Some(p) = password {
            entry.password = p.filter(|s| !s.is_empty());
        }
        if let Some(l) = label {
            entry.label = l;
        }
        let cloned = entry.clone();
        Self::persist(&d, &self.path)?;
        Ok(cloned)
    }

    /// 删除代理（占用或预占中拒绝删除）
    pub fn remove(&self, id: u64) -> Result<ProxyEntry, PoolError> {
        let mut d = self.inner.lock();
        let idx = d
            .proxies
            .iter()
            .position(|p| p.id == id)
            .ok_or(PoolError::NotFound)?;
        if d.proxies[idx].usage_count() > 0 {
            return Err(PoolError::Conflict(
                "代理正在使用中，请先删除/改绑对应凭据".to_string(),
            ));
        }
        if !d.proxies[idx].reserved.is_empty() {
            return Err(PoolError::Conflict(
                "代理有正在进行的分配，请稍后再试".to_string(),
            ));
        }
        let removed = d.proxies.remove(idx);
        Self::persist(&d, &self.path)?;
        Ok(removed)
    }

    pub fn set_disabled(&self, id: u64, disabled: bool) -> Result<ProxyEntry, PoolError> {
        let mut d = self.inner.lock();
        let entry = d
            .proxies
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or(PoolError::NotFound)?;
        entry.disabled = disabled;
        let cloned = entry.clone();
        Self::persist(&d, &self.path)?;
        Ok(cloned)
    }

    // ---- 设置 ----

    pub fn set_settings(
        &self,
        auto_assign: Option<bool>,
        probe_url: Option<String>,
    ) -> Result<ProxyPoolSettings, PoolError> {
        // 校验 probe_url（锁外）
        let probe_url = match probe_url {
            Some(u) => {
                let u = u.trim().to_string();
                if u.is_empty() {
                    None
                } else {
                    validate_probe_url(&u)?;
                    Some(u)
                }
            }
            None => None,
        };
        let mut d = self.inner.lock();
        if let Some(a) = auto_assign {
            d.settings.auto_assign_enabled = a;
        }
        if let Some(u) = probe_url {
            d.settings.probe_url = u;
        }
        let cloned = d.settings.clone();
        Self::persist(&d, &self.path)?;
        Ok(cloned)
    }

    // ---- 分配（reservation 两阶段） ----

    /// 自动预占：空闲优先（负载最低），无空闲则复用负载最低的在用代理。
    /// `skip_ids` 用于跳过探测失败的代理。返回预占句柄（未落盘）。
    pub fn reserve_auto(&self, skip_ids: &[u64]) -> Option<Reservation> {
        let mut d = self.inner.lock();
        let candidates: Vec<usize> = d
            .proxies
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.disabled && !skip_ids.contains(&p.id))
            .map(|(i, _)| i)
            .collect();
        if candidates.is_empty() {
            return None;
        }
        // 挑负载最低（含预占），tie 用 id 稳定
        let chosen = candidates.into_iter().min_by(|&a, &b| {
            d.proxies[a]
                .load()
                .cmp(&d.proxies[b].load())
                .then(d.proxies[a].id.cmp(&d.proxies[b].id))
        })?;
        let reused = d.proxies[chosen].load() > 0;
        let token = self.next_reservation_token();
        d.proxies[chosen].reserved.push(token);
        let proxy = d.proxies[chosen].clone();
        // 预占不落盘
        Some(Reservation {
            proxy,
            token,
            reused,
        })
    }

    /// 手动预占指定代理。默认只允许空闲；allow_reuse=true 允许复用在用代理。
    pub fn reserve_manual(
        &self,
        proxy_id: u64,
        allow_reuse: bool,
    ) -> Result<Reservation, PoolError> {
        let mut d = self.inner.lock();
        let token = self.next_reservation_token();
        let entry = d
            .proxies
            .iter_mut()
            .find(|p| p.id == proxy_id)
            .ok_or(PoolError::NotFound)?;
        if entry.disabled {
            return Err(PoolError::Conflict("代理已禁用".to_string()));
        }
        if entry.load() > 0 && !allow_reuse {
            return Err(PoolError::Conflict(format!(
                "代理正在使用中（{} 个占用），如需共享请开启复用",
                entry.load()
            )));
        }
        let reused = entry.load() > 0;
        entry.reserved.push(token);
        let proxy = entry.clone();
        Ok(Reservation {
            proxy,
            token,
            reused,
        })
    }

    /// 提交预占：把预占 token 转为真实 credId 挂载并落盘。
    pub fn commit_reservation(&self, token: u64, cred_id: u64) -> Result<(), PoolError> {
        let mut d = self.inner.lock();
        let entry = d.proxies.iter_mut().find(|p| p.reserved.contains(&token));
        let entry = match entry {
            Some(e) => e,
            None => {
                // token 已失效（可能进程重启或被取消）——记录告警但不报错
                tracing::warn!("提交代理预占失败：token {} 不存在", token);
                return Ok(());
            }
        };
        entry.reserved.retain(|&t| t != token);
        if !entry.assignments.contains(&cred_id) {
            entry.assignments.push(cred_id);
        }
        Self::persist(&d, &self.path)
    }

    /// 取消预占（凭据创建/探测失败时回滚）。不落盘。
    pub fn cancel_reservation(&self, token: u64) {
        let mut d = self.inner.lock();
        for p in d.proxies.iter_mut() {
            p.reserved.retain(|&t| t != token);
        }
    }

    /// 按 credId 释放（删除凭据时调用）
    pub fn release_by_cred(&self, cred_id: u64) -> usize {
        let mut d = self.inner.lock();
        let mut released = 0;
        for p in d.proxies.iter_mut() {
            let before = p.assignments.len();
            p.assignments.retain(|&c| c != cred_id);
            released += before - p.assignments.len();
        }
        if released > 0 {
            if let Err(e) = Self::persist(&d, &self.path) {
                tracing::error!("释放代理占用后持久化失败: {}", e);
            }
        }
        released
    }

    /// 记录探测结果
    pub fn record_probe(&self, id: u64, result: ProbeResult) {
        let mut d = self.inner.lock();
        if let Some(entry) = d.proxies.iter_mut().find(|p| p.id == id) {
            entry.last_check = Some(result);
            if let Err(e) = Self::persist(&d, &self.path) {
                tracing::warn!("记录探测结果后持久化失败（忽略）: {}", e);
            }
        }
    }

    // ---- 探测 ----

    /// 探测某代理是否可用，返回结果（不写库，调用方决定是否记录）
    pub async fn probe(&self, entry: &ProxyEntry) -> ProbeResult {
        let probe_url = self.probe_url();
        probe_proxy(&entry.to_proxy_config(), &probe_url, self.tls_backend).await
    }

    /// 探测（带 TTL 缓存）：若最近一次探测仍在 TTL 内直接复用，否则真正探测并记录。
    /// 用于批量分配时避免对同一代理反复探测。
    pub async fn probe_or_cached(&self, id: u64) -> Option<ProbeResult> {
        let entry = self.get(id)?;
        if let Some(last) = &entry.last_check {
            if last.is_fresh() {
                return Some(last.clone());
            }
        }
        let result = self.probe(&entry).await;
        self.record_probe(id, result.clone());
        Some(result)
    }
}

/// 通过指定代理探测 URL，测量连通性与延迟
pub async fn probe_proxy(
    proxy: &ProxyConfig,
    probe_url: &str,
    tls_backend: TlsBackend,
) -> ProbeResult {
    let now = chrono::Utc::now().to_rfc3339();

    // SSRF 二次校验：探测前再校验 URL（含 IP 字面量私网判定）
    if let Err(e) = validate_probe_url(probe_url) {
        return ProbeResult {
            ok: false,
            latency_ms: None,
            ip: None,
            message: Some(format!("探测 URL 非法: {}", e)),
            at: now,
        };
    }

    let client = match build_client(Some(proxy), 12, tls_backend) {
        Ok(c) => c,
        Err(e) => {
            return ProbeResult {
                ok: false,
                latency_ms: None,
                ip: None,
                message: Some(format!("构建客户端失败: {}", e)),
                at: now,
            };
        }
    };
    let start = Instant::now();
    match tokio::time::timeout(Duration::from_secs(13), client.get(probe_url).send()).await {
        Ok(Ok(resp)) => {
            let status = resp.status();
            let latency = start.elapsed().as_millis() as u64;
            let body = resp.text().await.unwrap_or_default();
            if status.is_success() {
                // 尝试从常见返回体解析出口 IP，仅当结果是合法 IP 才记录
                let ip = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| {
                        v.get("ip")
                            .or_else(|| v.get("origin"))
                            .and_then(|x| x.as_str().map(|s| s.to_string()))
                    })
                    .or_else(|| {
                        let t = body.trim();
                        if !t.is_empty() && t.len() <= 45 && !t.contains('<') {
                            Some(t.to_string())
                        } else {
                            None
                        }
                    })
                    .and_then(|s| {
                        // origin 可能是 "1.2.3.4, 5.6.7.8"，取第一个
                        let first = s.split(',').next().unwrap_or("").trim().to_string();
                        if first.parse::<IpAddr>().is_ok() {
                            Some(first)
                        } else {
                            None
                        }
                    });
                ProbeResult {
                    ok: true,
                    latency_ms: Some(latency),
                    ip,
                    message: None,
                    at: now,
                }
            } else {
                ProbeResult {
                    ok: false,
                    latency_ms: Some(latency),
                    ip: None,
                    message: Some(format!("HTTP {}", status.as_u16())),
                    at: now,
                }
            }
        }
        Ok(Err(e)) => ProbeResult {
            ok: false,
            latency_ms: None,
            ip: None,
            message: Some(format!("请求失败: {}", e)),
            at: now,
        },
        Err(_) => ProbeResult {
            ok: false,
            latency_ms: None,
            ip: None,
            message: Some("超时".to_string()),
            at: now,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool() -> ProxyPool {
        ProxyPool::load(None, TlsBackend::Rustls)
    }

    fn add(p: &ProxyPool, url: &str) -> ProxyEntry {
        p.add(url.into(), None, None, None).expect("add ok")
    }

    #[test]
    fn test_parse_proxy_line_colon_host_port_user_pass() {
        let parsed = parse_proxy_line("63.246.151.171:5502:kmkmhuyw:3d1it5o1kxnu").unwrap();
        assert_eq!(parsed.url, "socks5://63.246.151.171:5502");
        assert_eq!(parsed.username.as_deref(), Some("kmkmhuyw"));
        assert_eq!(parsed.password.as_deref(), Some("3d1it5o1kxnu"));
        assert!(parsed.label.is_none());
        assert!(validate_proxy_url(&parsed.url).is_ok());
    }

    #[test]
    fn test_parse_proxy_line_colon_host_port_only() {
        let parsed = parse_proxy_line("107.180.180.233:5282").unwrap();
        assert_eq!(parsed.url, "socks5://107.180.180.233:5282");
        assert!(parsed.username.is_none());
        assert!(parsed.password.is_none());
    }

    /// 冒号格式带显式协议前缀时应保留原协议，不被 socks5 覆盖。
    #[test]
    fn test_parse_proxy_line_colon_keeps_explicit_scheme() {
        let parsed = parse_proxy_line("http://1.2.3.4:8080:user:pass").unwrap();
        assert_eq!(parsed.url, "http://1.2.3.4:8080");
        assert_eq!(parsed.username.as_deref(), Some("user"));
        assert_eq!(parsed.password.as_deref(), Some("pass"));
    }

    /// 空格分隔的原格式不能因新增冒号格式而回退。
    #[test]
    fn test_parse_proxy_line_whitespace_format_still_works() {
        let parsed = parse_proxy_line("socks5://1.2.3.4:1080 user pass 美国 静态").unwrap();
        assert_eq!(parsed.url, "socks5://1.2.3.4:1080");
        assert_eq!(parsed.username.as_deref(), Some("user"));
        assert_eq!(parsed.password.as_deref(), Some("pass"));
        assert_eq!(parsed.label.as_deref(), Some("美国 静态"));
    }

    #[test]
    fn test_parse_proxy_line_rejects_bad_shapes() {
        // 用户名/密码为空
        assert!(parse_proxy_line("1.2.3.4:1080::pass").is_err());
        assert!(parse_proxy_line("1.2.3.4:1080:user:").is_err());
        // 分段数不可识别
        assert!(parse_proxy_line("1.2.3.4:1080:user").is_err());
        assert!(parse_proxy_line("a:b:c:d:e").is_err());
        assert!(parse_proxy_line("   ").is_err());
    }

    /// 真实代理商导出格式的批量导入。
    #[test]
    fn test_batch_add_colon_format() {
        let p = pool();
        let lines: Vec<String> = vec![
            "63.246.151.171:5502:kmkmhuyw:3d1it5o1kxnu".to_string(),
            "107.180.180.233:5282:kmkmhuyw:3d1it5o1kxnu".to_string(),
            "".to_string(),
            "9.142.33.139:7310:kmkmhuyw:3d1it5o1kxnu".to_string(),
        ];
        let result = p.batch_add(&lines);
        assert_eq!(result.added.len(), 3, "errors: {:?}", result.errors);
        assert!(result.errors.is_empty());
        assert_eq!(result.added[0].url, "socks5://63.246.151.171:5502");
        assert_eq!(result.added[0].username.as_deref(), Some("kmkmhuyw"));
        assert!(result.added.iter().all(|e| e.is_free()));
    }

    /// 两种格式混在同一次导入里，并且非法行不阻断其它行。
    #[test]
    fn test_batch_add_mixed_formats_and_partial_failure() {
        let p = pool();
        let lines: Vec<String> = vec![
            "63.246.151.171:5502:kmkmhuyw:3d1it5o1kxnu".to_string(),
            "socks5://5.6.7.8:1080 u pw 备注".to_string(),
            "1.2.3.4:1080:user".to_string(),
            "192.168.1.1:8080:user:pass".to_string(),
        ];
        let result = p.batch_add(&lines);
        assert_eq!(result.added.len(), 3, "errors: {:?}", result.errors);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line, 3);
    }

    #[test]
    fn test_add_and_list() {
        let p = pool();
        let e = p
            .add(
                "http://a.example:1080".into(),
                None,
                None,
                Some("lbl".into()),
            )
            .unwrap();
        assert_eq!(e.id, 1);
        assert_eq!(p.list().len(), 1);
        assert!(e.is_free());
    }

    #[test]
    fn test_add_rejects_invalid_url() {
        let p = pool();
        assert!(p.add("not-a-url".into(), None, None, None).is_err());
        assert!(p.add("ftp://host:21".into(), None, None, None).is_err());
        // socks5 缺端口（无已知默认）
        assert!(p.add("socks5://host".into(), None, None, None).is_err());
    }

    #[test]
    fn test_validate_proxy_url_schemes() {
        assert!(validate_proxy_url("http://1.2.3.4:8080").is_ok());
        assert!(validate_proxy_url("https://h.example:443").is_ok());
        assert!(validate_proxy_url("socks5://1.2.3.4:1080").is_ok());
        assert!(validate_proxy_url("socks5h://1.2.3.4:1080").is_ok());
        assert!(validate_proxy_url("socks4://1.2.3.4:1080").is_err());
        // http 默认端口 80 可用，允许
        assert!(validate_proxy_url("http://1.2.3.4").is_ok());
        // socks5 无已知默认端口，必须显式指定
        assert!(validate_proxy_url("socks5://1.2.3.4").is_err());
        assert!(validate_proxy_url("").is_err());
    }

    #[test]
    fn test_validate_probe_url_ssrf() {
        assert!(validate_probe_url("https://api.ipify.org?format=json").is_ok());
        assert!(validate_probe_url("http://api.ipify.org").is_err()); // 非 https
        assert!(validate_probe_url("https://localhost/x").is_err());
        assert!(validate_probe_url("https://127.0.0.1/x").is_err());
        assert!(validate_probe_url("https://10.0.0.5/x").is_err());
        assert!(validate_probe_url("https://192.168.1.1/x").is_err());
        assert!(validate_probe_url("https://169.254.169.254/latest/meta-data").is_err());
        assert!(validate_probe_url("https://[::1]/x").is_err());
        assert!(validate_probe_url("https://172.16.0.1/x").is_err());
    }

    #[test]
    fn test_reserve_auto_prefers_free_then_reuses() {
        let p = pool();
        add(&p, "http://a.example:1"); // id1
        add(&p, "http://b.example:2"); // id2
        // 预占 1 → 空闲，选 id1
        let r1 = p.reserve_auto(&[]).unwrap();
        assert_eq!(r1.proxy.id, 1);
        assert!(!r1.reused);
        p.commit_reservation(r1.token, 100).unwrap();
        // 预占 2 → id2 仍空闲
        let r2 = p.reserve_auto(&[]).unwrap();
        assert_eq!(r2.proxy.id, 2);
        assert!(!r2.reused);
        p.commit_reservation(r2.token, 101).unwrap();
        // 预占 3 → 无空闲，复用负载最低（都为1，选 id1）
        let r3 = p.reserve_auto(&[]).unwrap();
        assert!(r3.reused);
        assert_eq!(r3.proxy.id, 1);
        p.commit_reservation(r3.token, 102).unwrap();
        assert_eq!(p.get(1).unwrap().usage_count(), 2);
    }

    #[test]
    fn test_reserve_auto_concurrent_pick_distinct() {
        // 两个并发预占（未提交）应选中不同的空闲代理
        let p = pool();
        add(&p, "http://a.example:1"); // id1
        add(&p, "http://b.example:2"); // id2
        let r1 = p.reserve_auto(&[]).unwrap();
        let r2 = p.reserve_auto(&[]).unwrap();
        assert_ne!(r1.proxy.id, r2.proxy.id);
        assert!(!r1.reused);
        assert!(!r2.reused);
    }

    #[test]
    fn test_reserve_cancel_frees_slot() {
        let p = pool();
        add(&p, "http://a.example:1"); // id1
        let r1 = p.reserve_auto(&[]).unwrap();
        assert!(!p.get(1).unwrap().is_free()); // 预占中
        p.cancel_reservation(r1.token);
        assert!(p.get(1).unwrap().is_free()); // 取消后恢复空闲
        assert_eq!(p.get(1).unwrap().usage_count(), 0);
    }

    #[test]
    fn test_reserve_auto_skip_ids() {
        let p = pool();
        add(&p, "http://a.example:1"); // id1
        add(&p, "http://b.example:2"); // id2
        let r = p.reserve_auto(&[1]).unwrap();
        assert_eq!(r.proxy.id, 2); // id1 被跳过
    }

    #[test]
    fn test_manual_reserve_reuse_guard() {
        let p = pool();
        add(&p, "http://a.example:1"); // id1
        let r1 = p.reserve_manual(1, false).unwrap();
        p.commit_reservation(r1.token, 10).unwrap();
        // 再次分配同一代理，未开复用 → 报错
        assert!(p.reserve_manual(1, false).is_err());
        // 开复用 → ok
        let r = p.reserve_manual(1, true).unwrap();
        assert!(r.reused);
        p.commit_reservation(r.token, 11).unwrap();
        assert_eq!(p.get(1).unwrap().usage_count(), 2);
    }

    #[test]
    fn test_release_by_cred() {
        let p = pool();
        add(&p, "http://a.example:1");
        let r1 = p.reserve_manual(1, false).unwrap();
        p.commit_reservation(r1.token, 10).unwrap();
        let r2 = p.reserve_manual(1, true).unwrap();
        p.commit_reservation(r2.token, 11).unwrap();
        assert_eq!(p.release_by_cred(10), 1);
        assert_eq!(p.get(1).unwrap().usage_count(), 1);
    }

    #[test]
    fn test_remove_guard_when_in_use_or_reserved() {
        let p = pool();
        add(&p, "http://a.example:1");
        let r = p.reserve_manual(1, false).unwrap();
        // 预占中拒删
        assert!(p.remove(1).is_err());
        p.commit_reservation(r.token, 10).unwrap();
        // 在用拒删
        assert!(p.remove(1).is_err());
        p.release_by_cred(10);
        assert!(p.remove(1).is_ok());
    }

    #[test]
    fn test_batch_add_reports_errors() {
        let p = pool();
        let lines = vec![
            "http://1.2.3.4:8080 user pass 美国".to_string(),
            "bad-line-no-scheme".to_string(),
            "".to_string(),
            "socks5://5.6.7.8:1080".to_string(),
        ];
        let res = p.batch_add(&lines);
        assert_eq!(res.added.len(), 2);
        assert_eq!(res.errors.len(), 1);
        assert_eq!(res.errors[0].line, 2);
    }

    #[test]
    fn test_update_rejects_invalid_url() {
        let p = pool();
        add(&p, "http://a.example:1080");
        assert!(p.update(1, Some("bad".into()), None, None, None).is_err());
        assert!(
            p.update(1, Some("https://ok.example:443".into()), None, None, None)
                .is_ok()
        );
    }

    #[test]
    fn test_stats() {
        let p = pool();
        add(&p, "http://a.example:1");
        add(&p, "http://b.example:2");
        let r1 = p.reserve_manual(1, false).unwrap();
        p.commit_reservation(r1.token, 10).unwrap();
        let r2 = p.reserve_manual(1, true).unwrap();
        p.commit_reservation(r2.token, 11).unwrap();
        let s = p.stats();
        assert_eq!(s.total, 2);
        assert_eq!(s.available, 1);
        assert_eq!(s.assigned, 1);
        assert_eq!(s.shared, 1);
    }

    #[test]
    fn test_persist_and_reload_roundtrip() {
        let dir = std::env::temp_dir().join(format!("proxypool_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("proxy_pool.json");
        let _ = std::fs::remove_file(&path);
        {
            let p = ProxyPool::load(Some(path.clone()), TlsBackend::Rustls);
            let e = p
                .add("http://1.2.3.4:8080".into(), None, None, None)
                .unwrap();
            let r = p.reserve_manual(e.id, false).unwrap();
            p.commit_reservation(r.token, 42).unwrap();
        }
        // reload
        let p2 = ProxyPool::load(Some(path.clone()), TlsBackend::Rustls);
        assert_eq!(p2.list().len(), 1);
        assert_eq!(p2.get(1).unwrap().assignments, vec![42]);
        // reserved 不持久化
        assert!(p2.get(1).unwrap().reserved.is_empty());
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(ProxyPool::bak_path(&path));
    }

    #[test]
    fn test_load_falls_back_to_bak_on_corruption() {
        let dir = std::env::temp_dir().join(format!("proxypool_bak_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("proxy_pool.json");
        let bak = ProxyPool::bak_path(&path);
        // 写入一个合法 .bak
        let good = r#"{"settings":{"autoAssignEnabled":true,"probeUrl":"https://api.ipify.org?format=json"},"proxies":[{"id":7,"url":"http://9.9.9.9:8080","label":"","disabled":false,"assignments":[1]}],"nextId":8}"#;
        std::fs::write(&bak, good).unwrap();
        // 主文件损坏
        std::fs::write(&path, "{ this is not json").unwrap();
        let p = ProxyPool::load(Some(path.clone()), TlsBackend::Rustls);
        assert_eq!(p.list().len(), 1);
        assert_eq!(p.get(7).unwrap().url, "http://9.9.9.9:8080");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&bak);
    }
}
