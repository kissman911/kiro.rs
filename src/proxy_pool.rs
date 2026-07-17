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

use std::path::PathBuf;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::http_client::{ProxyConfig, build_client};
use crate::model::config::TlsBackend;

/// 默认探测 URL（返回出口 IP，海内外相对稳定）
pub fn default_probe_url() -> String {
    "https://api.ipify.org?format=json".to_string()
}

fn default_true() -> bool {
    true
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
    /// 出口 IP（探测 URL 返回时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    /// 失败信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// 探测时间（RFC3339）
    pub at: String,
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
}

impl ProxyEntry {
    pub fn usage_count(&self) -> usize {
        self.assignments.len()
    }
    pub fn is_free(&self) -> bool {
        !self.disabled && self.assignments.is_empty()
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

/// 分配结果
#[derive(Debug, Clone)]
pub struct AssignResult {
    pub proxy: ProxyEntry,
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

/// 代理池（线程安全）
pub struct ProxyPool {
    inner: Mutex<ProxyPoolData>,
    path: Option<PathBuf>,
    tls_backend: TlsBackend,
}

impl ProxyPool {
    /// 从文件加载（文件不存在则返回空池）
    pub fn load(path: Option<PathBuf>, tls_backend: TlsBackend) -> Self {
        let data = match &path {
            Some(p) if p.exists() => std::fs::read_to_string(p)
                .ok()
                .and_then(|c| serde_json::from_str::<ProxyPoolData>(&c).ok())
                .unwrap_or_default(),
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
        Self {
            inner: Mutex::new(data),
            path,
            tls_backend,
        }
    }

    fn persist(data: &ProxyPoolData, path: &Option<PathBuf>) {
        if let Some(p) = path {
            match serde_json::to_string_pretty(data) {
                Ok(content) => {
                    if let Err(e) = std::fs::write(p, content) {
                        tracing::warn!("保存代理池失败: {}", e);
                    }
                }
                Err(e) => tracing::warn!("序列化代理池失败: {}", e),
            }
        }
    }

    // ---- 查询 ----

    pub fn list(&self) -> Vec<ProxyEntry> {
        self.inner.lock().proxies.clone()
    }

    pub fn get(&self, id: u64) -> Option<ProxyEntry> {
        self.inner.lock().proxies.iter().find(|p| p.id == id).cloned()
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
    ) -> ProxyEntry {
        let mut d = self.inner.lock();
        let id = d.next_id;
        d.next_id += 1;
        let entry = ProxyEntry {
            id,
            url: url.trim().to_string(),
            username: username.filter(|s| !s.is_empty()),
            password: password.filter(|s| !s.is_empty()),
            label: label.unwrap_or_default(),
            disabled: false,
            assignments: Vec::new(),
            last_check: None,
        };
        d.proxies.push(entry.clone());
        Self::persist(&d, &self.path);
        entry
    }

    /// 批量添加：每行 `url [username] [password] [label...]`
    pub fn batch_add(&self, lines: &[String]) -> Vec<ProxyEntry> {
        let mut added = Vec::new();
        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            let url = parts[0].to_string();
            let username = parts.get(1).map(|s| s.to_string());
            let password = parts.get(2).map(|s| s.to_string());
            let label = if parts.len() > 3 {
                Some(parts[3..].join(" "))
            } else {
                None
            };
            added.push(self.add(url, username, password, label));
        }
        added
    }

    /// 更新代理基础字段（不含 assignments）
    pub fn update(
        &self,
        id: u64,
        url: Option<String>,
        username: Option<Option<String>>,
        password: Option<Option<String>>,
        label: Option<String>,
    ) -> Option<ProxyEntry> {
        let mut d = self.inner.lock();
        let entry = d.proxies.iter_mut().find(|p| p.id == id)?;
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
        Self::persist(&d, &self.path);
        Some(cloned)
    }

    /// 删除代理（占用中拒绝删除）
    pub fn remove(&self, id: u64) -> Result<ProxyEntry, String> {
        let mut d = self.inner.lock();
        let idx = d
            .proxies
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| "代理不存在".to_string())?;
        if d.proxies[idx].usage_count() > 0 {
            return Err("代理正在使用中，请先解绑".to_string());
        }
        let removed = d.proxies.remove(idx);
        Self::persist(&d, &self.path);
        Ok(removed)
    }

    pub fn set_disabled(&self, id: u64, disabled: bool) -> Option<ProxyEntry> {
        let mut d = self.inner.lock();
        let entry = d.proxies.iter_mut().find(|p| p.id == id)?;
        entry.disabled = disabled;
        let cloned = entry.clone();
        Self::persist(&d, &self.path);
        Some(cloned)
    }

    /// 解绑代理的全部挂载（仅清 assignments，不动凭据本身）
    pub fn release_all(&self, id: u64) -> Option<ProxyEntry> {
        let mut d = self.inner.lock();
        let entry = d.proxies.iter_mut().find(|p| p.id == id)?;
        entry.assignments.clear();
        let cloned = entry.clone();
        Self::persist(&d, &self.path);
        Some(cloned)
    }

    // ---- 设置 ----

    pub fn set_settings(&self, auto_assign: Option<bool>, probe_url: Option<String>) -> ProxyPoolSettings {
        let mut d = self.inner.lock();
        if let Some(a) = auto_assign {
            d.settings.auto_assign_enabled = a;
        }
        if let Some(u) = probe_url {
            let u = u.trim();
            if !u.is_empty() {
                d.settings.probe_url = u.to_string();
            }
        }
        let cloned = d.settings.clone();
        Self::persist(&d, &self.path);
        cloned
    }

    // ---- 分配 ----

    /// 自动分配：空闲优先（负载最低），无空闲则复用负载最低的在用代理。
    /// `skip_ids` 用于跳过探测失败的代理。
    pub fn auto_assign(&self, cred_id: Option<u64>, skip_ids: &[u64]) -> Option<AssignResult> {
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
        // 挑负载最低（先 free=0，其次最少），tie 用 id 稳定
        let chosen = candidates
            .into_iter()
            .min_by(|&a, &b| {
                d.proxies[a]
                    .usage_count()
                    .cmp(&d.proxies[b].usage_count())
                    .then(d.proxies[a].id.cmp(&d.proxies[b].id))
            })?;
        let reused = d.proxies[chosen].usage_count() > 0;
        if let Some(cid) = cred_id {
            if !d.proxies[chosen].assignments.contains(&cid) {
                d.proxies[chosen].assignments.push(cid);
            }
        }
        let proxy = d.proxies[chosen].clone();
        Self::persist(&d, &self.path);
        Some(AssignResult { proxy, reused })
    }

    /// 手动指定代理。默认只允许空闲；allow_reuse=true 允许复用在用代理。
    pub fn assign(
        &self,
        proxy_id: u64,
        cred_id: Option<u64>,
        allow_reuse: bool,
    ) -> Result<AssignResult, String> {
        let mut d = self.inner.lock();
        let entry = d
            .proxies
            .iter_mut()
            .find(|p| p.id == proxy_id)
            .ok_or_else(|| "代理不存在".to_string())?;
        if entry.disabled {
            return Err("代理已禁用".to_string());
        }
        if entry.usage_count() > 0 && !allow_reuse {
            return Err(format!(
                "代理正在使用中（{} 个凭据），如需共享请开启复用",
                entry.usage_count()
            ));
        }
        let reused = entry.usage_count() > 0;
        if let Some(cid) = cred_id {
            if !entry.assignments.contains(&cid) {
                entry.assignments.push(cid);
            }
        }
        let proxy = entry.clone();
        Self::persist(&d, &self.path);
        Ok(AssignResult { proxy, reused })
    }

    /// 分配后回填 credId（分配时 cred_id 未知的场景：先探测占位再回填）
    /// 这里用「把 placeholder 替换为真实 credId」的方式：找到最近一次 assign 但 cred 为占位的。
    /// 简化实现：直接把真实 credId 追加到指定 proxy。
    pub fn record_assignment(&self, proxy_id: u64, cred_id: u64) {
        let mut d = self.inner.lock();
        if let Some(entry) = d.proxies.iter_mut().find(|p| p.id == proxy_id) {
            if !entry.assignments.contains(&cred_id) {
                entry.assignments.push(cred_id);
            }
            Self::persist(&d, &self.path);
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
            Self::persist(&d, &self.path);
        }
        released
    }

    /// 记录探测结果
    pub fn record_probe(&self, id: u64, result: ProbeResult) {
        let mut d = self.inner.lock();
        if let Some(entry) = d.proxies.iter_mut().find(|p| p.id == id) {
            entry.last_check = Some(result);
            Self::persist(&d, &self.path);
        }
    }

    // ---- 探测 ----

    /// 探测某代理是否可用，返回结果（不写库，调用方决定是否记录）
    pub async fn probe(&self, entry: &ProxyEntry) -> ProbeResult {
        let probe_url = self.probe_url();
        probe_proxy(&entry.to_proxy_config(), &probe_url, self.tls_backend).await
    }
}

/// 通过指定代理探测 URL，测量连通性与延迟
pub async fn probe_proxy(
    proxy: &ProxyConfig,
    probe_url: &str,
    tls_backend: TlsBackend,
) -> ProbeResult {
    let now = chrono::Utc::now().to_rfc3339();
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
                // 尝试从常见返回体解析出口 IP
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

    #[test]
    fn test_add_and_list() {
        let p = pool();
        let e = p.add("http://a:1".into(), None, None, Some("lbl".into()));
        assert_eq!(e.id, 1);
        assert_eq!(p.list().len(), 1);
        assert!(e.is_free());
    }

    #[test]
    fn test_auto_assign_prefers_free_then_reuses() {
        let p = pool();
        p.add("http://a:1".into(), None, None, None); // id1
        p.add("http://b:2".into(), None, None, None); // id2
        // 第一次分配 → 空闲，选 id1（负载 0，id 最小）
        let r1 = p.auto_assign(Some(100), &[]).unwrap();
        assert_eq!(r1.proxy.id, 1);
        assert!(!r1.reused);
        // 第二次 → id2 仍空闲
        let r2 = p.auto_assign(Some(101), &[]).unwrap();
        assert_eq!(r2.proxy.id, 2);
        assert!(!r2.reused);
        // 第三次 → 无空闲，复用负载最低（都为1，选 id1）
        let r3 = p.auto_assign(Some(102), &[]).unwrap();
        assert!(r3.reused);
        assert_eq!(r3.proxy.id, 1);
        assert_eq!(p.get(1).unwrap().usage_count(), 2);
    }

    #[test]
    fn test_auto_assign_skip_ids() {
        let p = pool();
        p.add("http://a:1".into(), None, None, None); // id1
        p.add("http://b:2".into(), None, None, None); // id2
        let r = p.auto_assign(Some(1), &[1]).unwrap();
        assert_eq!(r.proxy.id, 2); // id1 被跳过
    }

    #[test]
    fn test_manual_assign_reuse_guard() {
        let p = pool();
        p.add("http://a:1".into(), None, None, None); // id1
        p.assign(1, Some(10), false).unwrap();
        // 再次分配同一代理，未开复用 → 报错
        assert!(p.assign(1, Some(11), false).is_err());
        // 开复用 → ok
        let r = p.assign(1, Some(11), true).unwrap();
        assert!(r.reused);
        assert_eq!(p.get(1).unwrap().usage_count(), 2);
    }

    #[test]
    fn test_release_by_cred() {
        let p = pool();
        p.add("http://a:1".into(), None, None, None);
        p.assign(1, Some(10), false).unwrap();
        p.assign(1, Some(11), true).unwrap();
        assert_eq!(p.release_by_cred(10), 1);
        assert_eq!(p.get(1).unwrap().usage_count(), 1);
    }

    #[test]
    fn test_remove_guard_when_in_use() {
        let p = pool();
        p.add("http://a:1".into(), None, None, None);
        p.assign(1, Some(10), false).unwrap();
        assert!(p.remove(1).is_err());
        p.release_all(1);
        assert!(p.remove(1).is_ok());
    }

    #[test]
    fn test_stats() {
        let p = pool();
        p.add("http://a:1".into(), None, None, None);
        p.add("http://b:2".into(), None, None, None);
        p.assign(1, Some(10), false).unwrap();
        p.assign(1, Some(11), true).unwrap();
        let s = p.stats();
        assert_eq!(s.total, 2);
        assert_eq!(s.available, 1);
        assert_eq!(s.assigned, 1);
        assert_eq!(s.shared, 1);
    }
}
