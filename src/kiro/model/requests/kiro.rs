//! Kiro 请求类型定义
//!
//! 定义 Kiro API 的主请求结构

use serde::{Deserialize, Serialize};

use super::conversation::ConversationState;

/// Kiro API 请求
///
/// 用于构建发送给 Kiro API 的请求
///
/// # 示例
///
/// ```rust
/// use kiro_rs::kiro::model::requests::{
///     KiroRequest, ConversationState, CurrentMessage, UserInputMessage, Tool
/// };
///
/// // 创建简单请求
/// let state = ConversationState::new("conv-123")
///     .with_agent_task_type("vibe")
///     .with_current_message(CurrentMessage::new(
///         UserInputMessage::new("Hello", "claude-3-5-sonnet")
///     ));
///
/// let request = KiroRequest::new(state);
/// let json = request.to_json().unwrap();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroRequest {
    /// 对话状态
    pub conversation_state: ConversationState,
    /// Profile ARN（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_arn: Option<String>,
    /// 真实 Kiro CLI wire 字段，携带 output_config.effort 等控制开关。
    ///
    /// 真实拓包样例（抓自 Kiro CLI 流量）：
    /// ```json
    /// "additionalModelRequestFields": {
    ///     "output_config": { "effort": "max" }
    /// }
    /// ```
    /// effort 档位与模型相关：老 4.5/4.6 接受 low/medium/high/max；
    /// 新型号可能额外接受 xhigh。与 XML 伪协议不同，这是真正生效的协议字段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_model_request_fields: Option<AdditionalModelRequestFields>,
}

/// AWS Q CodeWhisperer `additionalModelRequestFields` 顶层容器。
///
/// 注意：真实 wire 格式中，内层 `output_config` 是 snake_case，
/// 与外层 `additionalModelRequestFields`（camelCase）不同，
/// 所以这个结构 **不能** 继承 `rename_all = "camelCase"`。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdditionalModelRequestFields {
    /// 输出配置（含 reasoning effort）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<KiroOutputConfig>,
}

/// AWS Q 后端识别的 effort 控制字段。
///
/// 档位与模型相关：老 4.5/4.6 接受 low/medium/high/max；新型号可能接受 xhigh。
/// 阶梯实验：同一 prompt 从 low 到 max，响应时间与输出长度相差约 5 倍，
/// 证明这是真正生效的协议字段，而非往 system prompt 塞 `<thinking_effort>` 的伪协议。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KiroOutputConfig {
    pub effort: String,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_kiro_request_deserialize() {
        let json = r#"{
            "conversationState": {
                "conversationId": "conv-456",
                "currentMessage": {
                    "userInputMessage": {
                        "content": "Test message",
                        "modelId": "claude-3-5-sonnet",
                        "userInputMessageContext": {}
                    }
                }
            }
        }"#;

        let request: KiroRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.conversation_state.conversation_id, "conv-456");
        assert_eq!(
            request
                .conversation_state
                .current_message
                .user_input_message
                .content,
            "Test message"
        );
    }
}
