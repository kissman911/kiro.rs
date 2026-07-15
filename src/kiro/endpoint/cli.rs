//! Kiro CLI 端点
//!
//! 对应 Kiro CLI 使用的 AWS JSON 协议：
//! - API: `https://runtime.{api_region}.kiro.dev/`
//! - Content-Type: `application/x-amz-json-1.0`
//! - `x-amz-target: AmazonCodeWhispererStreamingService.GenerateAssistantResponse`
//! - 请求体中的客户端来源改为 `KIRO_CLI`

use reqwest::RequestBuilder;
use uuid::Uuid;

use super::{KiroEndpoint, RequestContext};

pub const CLI_ENDPOINT_NAME: &str = "cli";

pub struct CliEndpoint;

impl CliEndpoint {
    pub fn new() -> Self {
        Self
    }

    fn api_region<'a>(&self, ctx: &'a RequestContext<'_>) -> &'a str {
        ctx.credentials.effective_api_region(ctx.config)
    }

    fn host(&self, ctx: &RequestContext<'_>) -> String {
        format!("runtime.{}.kiro.dev", self.api_region(ctx))
    }

    fn user_agent(&self, ctx: &RequestContext<'_>) -> String {
        format!(
            "aws-sdk-rust/1.3.15 ua/2.1 api/codewhispererstreaming/0.1.16551 os/{} lang/rust/1.92.0 md/appVersion-{} app/AmazonQ-For-CLI",
            ctx.config.system_version, ctx.config.kiro_version
        )
    }

    fn x_amz_user_agent(&self, ctx: &RequestContext<'_>) -> String {
        format!(
            "aws-sdk-rust/1.3.15 ua/2.1 api/codewhispererstreaming/0.1.16551 os/{} lang/rust/1.92.0 m/F app/AmazonQ-For-CLI",
            ctx.config.system_version
        )
    }
}

impl Default for CliEndpoint {
    fn default() -> Self {
        Self::new()
    }
}

impl KiroEndpoint for CliEndpoint {
    fn name(&self) -> &'static str {
        CLI_ENDPOINT_NAME
    }

    fn content_type(&self) -> &'static str {
        "application/x-amz-json-1.0"
    }

    fn api_url(&self, ctx: &RequestContext<'_>) -> String {
        format!("https://runtime.{}.kiro.dev/", self.api_region(ctx))
    }

    fn mcp_url(&self, ctx: &RequestContext<'_>) -> String {
        format!("https://runtime.{}.kiro.dev/mcp", self.api_region(ctx))
    }

    fn decorate_api(&self, req: RequestBuilder, ctx: &RequestContext<'_>) -> RequestBuilder {
        let mut req = req
            .header(
                "x-amz-target",
                "AmazonCodeWhispererStreamingService.GenerateAssistantResponse",
            )
            .header("x-amzn-codewhisperer-optout", "false")
            .header("x-amz-user-agent", self.x_amz_user_agent(ctx))
            .header("user-agent", self.user_agent(ctx))
            .header("host", self.host(ctx))
            .header("amz-sdk-invocation-id", Uuid::new_v4().to_string())
            .header("amz-sdk-request", "attempt=1; max=3")
            .header("Authorization", format!("Bearer {}", ctx.token));

        if ctx.credentials.is_api_key_credential() {
            req = req.header("TokenType", "API_KEY");
        } else if ctx.credentials.is_external_idp_credential() {
            req = req.header("TokenType", "EXTERNAL_IDP");
        }
        req
    }

    fn decorate_mcp(&self, req: RequestBuilder, ctx: &RequestContext<'_>) -> RequestBuilder {
        let mut req = req
            .header("x-amz-user-agent", self.x_amz_user_agent(ctx))
            .header("user-agent", self.user_agent(ctx))
            .header("host", self.host(ctx))
            .header("amz-sdk-invocation-id", Uuid::new_v4().to_string())
            .header("amz-sdk-request", "attempt=1; max=3")
            .header("Authorization", format!("Bearer {}", ctx.token));

        if let Some(ref arn) = ctx.credentials.profile_arn {
            req = req.header("x-amzn-kiro-profile-arn", arn);
        }
        if ctx.credentials.is_api_key_credential() {
            req = req.header("TokenType", "API_KEY");
        } else if ctx.credentials.is_external_idp_credential() {
            req = req.header("TokenType", "EXTERNAL_IDP");
        }
        req
    }

    fn transform_api_body(&self, body: &str, ctx: &RequestContext<'_>) -> String {
        rewrite_cli_body(body, &ctx.credentials.profile_arn)
    }
}

fn rewrite_cli_body(body: &str, profile_arn: &Option<String>) -> String {
    let Ok(mut json) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.to_string();
    };

    if let Some(state) = json
        .get_mut("conversationState")
        .and_then(|value| value.as_object_mut())
    {
        if let Some(input) = state
            .get_mut("currentMessage")
            .and_then(|value| value.get_mut("userInputMessage"))
            .and_then(|value| value.as_object_mut())
        {
            input.insert("origin".into(), "KIRO_CLI".into());
        }
        if let Some(history) = state
            .get_mut("history")
            .and_then(|value| value.as_array_mut())
        {
            for message in history {
                if let Some(input) = message
                    .get_mut("userInputMessage")
                    .and_then(|value| value.as_object_mut())
                {
                    input.insert("origin".into(), "KIRO_CLI".into());
                }
            }
        }
    }

    if let Some(arn) = profile_arn {
        json["profileArn"] = serde_json::Value::String(arn.clone());
    }

    serde_json::to_string(&json).unwrap_or_else(|_| body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn rewrites_current_and_history_origin_and_injects_profile() {
        let body = r#"{"conversationState":{"currentMessage":{"userInputMessage":{"origin":"AI_EDITOR"}},"history":[{"userInputMessage":{"origin":"AI_EDITOR"}}]}}"#;
        let result = rewrite_cli_body(body, &Some("arn:test".into()));
        let json: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            json.pointer("/conversationState/currentMessage/userInputMessage/origin"),
            Some(&Value::String("KIRO_CLI".into()))
        );
        assert_eq!(
            json.pointer("/conversationState/history/0/userInputMessage/origin"),
            Some(&Value::String("KIRO_CLI".into()))
        );
        assert_eq!(json["profileArn"], "arn:test");
    }

    #[test]
    fn leaves_invalid_json_unchanged() {
        assert_eq!(rewrite_cli_body("invalid", &None), "invalid");
    }
}
