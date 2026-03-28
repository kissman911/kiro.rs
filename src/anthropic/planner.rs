//! 请求规划与执行模式骨架
//!
//! 当前仅提供单阶段兼容模式所需的数据结构，
//! 为后续 native-like two-phase flow 预留扩展点。

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionMode {
    /// 当前兼容模式：单阶段直接请求
    SinglePhase,
    /// 预留：原生化双阶段模式（simple-task -> main model）
    TwoPhaseNativeLike,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseKind {
    /// 预处理阶段（原生样本中对应 simple-task）
    PreflightSimpleTask,
    /// 主模型阶段
    MainModel,
}

#[derive(Debug, Clone)]
pub struct RequestIdentity {
    /// 会话 ID（Kiro conversationId）
    pub conversation_id: String,
    /// 请求模型（来自 Anthropic 请求映射后的 modelId）
    pub requested_model: String,
    /// 可选的上游提取会话标识
    pub extracted_session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PhasePlan {
    pub phase: PhaseKind,
    pub model_id: String,
    pub include_tools: bool,
    pub include_tool_results: bool,
    pub include_full_history: bool,
}

#[derive(Debug, Clone)]
pub struct RequestPlan {
    pub identity: RequestIdentity,
    pub mode: ExecutionMode,
    pub phases: Vec<PhasePlan>,
}

impl RequestPlan {
    /// 当前默认规划：保持现有单阶段行为不变
    pub fn single_phase(identity: RequestIdentity) -> Self {
        let requested_model = identity.requested_model.clone();
        Self {
            identity,
            mode: ExecutionMode::SinglePhase,
            phases: vec![PhasePlan {
                phase: PhaseKind::MainModel,
                model_id: requested_model,
                include_tools: true,
                include_tool_results: true,
                include_full_history: true,
            }],
        }
    }
}
