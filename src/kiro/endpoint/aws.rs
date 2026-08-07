//! AWS 旧版端点（kirors-b 专属）
//!
//! 对应上游 hank9999/kiro.rs 原版使用的 AWS CodeWhisperer 端点：
//! - API: `https://q.{api_region}.amazonaws.com/generateAssistantResponse`
//! - MCP: `https://q.{api_region}.amazonaws.com/mcp`
//! - 额度: `https://q.{api_region}.amazonaws.com/getUsageLimits`
//! - Profile: `https://q.{api_region}.amazonaws.com/ListAvailableProfiles`
//!
//! 与 `ide` 端点的差异仅在域名与配套的额度/Profile 主机；请求头、请求体
//! （根对象注入 `profileArn`）保持一致，便于同一凭据在两套环境间来回切换对比。
//!
//! 该端点对请求体的校验比 `runtime.*.kiro.dev` 宽松：`profileArn` 可缺省，
//! 且鉴权失败先于业务参数校验返回。

use reqwest::RequestBuilder;
use uuid::Uuid;

use super::ide::inject_profile_arn;
use super::{KiroEndpoint, RequestContext};

/// AWS 旧版端点名称
pub const AWS_ENDPOINT_NAME: &str = "aws";

/// AWS 旧版端点主机（`q.{region}.amazonaws.com`）
pub fn aws_host(region: &str) -> String {
    format!("q.{}.amazonaws.com", region)
}

/// AWS 旧版端点
pub struct AwsEndpoint;

impl AwsEndpoint {
    pub fn new() -> Self {
        Self
    }

    fn api_region<'a>(&self, ctx: &'a RequestContext<'_>) -> &'a str {
        ctx.credentials.effective_api_region(ctx.config)
    }

    fn host(&self, ctx: &RequestContext<'_>) -> String {
        aws_host(self.api_region(ctx))
    }

    fn x_amz_user_agent(&self, ctx: &RequestContext<'_>) -> String {
        format!(
            "aws-sdk-js/1.0.34 KiroIDE-{}-{}",
            ctx.config.kiro_version, ctx.machine_id
        )
    }

    fn user_agent(&self, ctx: &RequestContext<'_>) -> String {
        format!(
            "aws-sdk-js/1.0.34 ua/2.1 os/{} lang/js md/nodejs#{} api/codewhispererstreaming#1.0.34 m/E KiroIDE-{}-{}",
            ctx.config.system_version,
            ctx.config.node_version,
            ctx.config.kiro_version,
            ctx.machine_id
        )
    }
}

impl Default for AwsEndpoint {
    fn default() -> Self {
        Self::new()
    }
}

impl KiroEndpoint for AwsEndpoint {
    fn name(&self) -> &'static str {
        AWS_ENDPOINT_NAME
    }

    fn api_url(&self, ctx: &RequestContext<'_>) -> String {
        format!(
            "https://{}/generateAssistantResponse",
            aws_host(self.api_region(ctx))
        )
    }

    fn mcp_url(&self, ctx: &RequestContext<'_>) -> String {
        format!("https://{}/mcp", aws_host(self.api_region(ctx)))
    }

    fn decorate_api(&self, req: RequestBuilder, ctx: &RequestContext<'_>) -> RequestBuilder {
        let mut req = req
            .header("x-amzn-codewhisperer-optout", "true")
            .header("x-amzn-kiro-agent-mode", "vibe")
            .header("x-amz-user-agent", self.x_amz_user_agent(ctx))
            .header("user-agent", self.user_agent(ctx))
            .header("host", self.host(ctx))
            .header("amz-sdk-invocation-id", Uuid::new_v4().to_string())
            .header("amz-sdk-request", "attempt=1; max=3")
            .header("Authorization", format!("Bearer {}", ctx.token));

        if ctx.credentials.is_api_key_credential() {
            req = req.header("tokentype", "API_KEY");
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
            req = req.header("tokentype", "API_KEY");
        } else if ctx.credentials.is_external_idp_credential() {
            req = req.header("TokenType", "EXTERNAL_IDP");
        }
        req
    }

    fn transform_api_body(&self, body: &str, ctx: &RequestContext<'_>) -> String {
        inject_profile_arn(body, &ctx.credentials.profile_arn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_host_uses_region() {
        assert_eq!(aws_host("us-east-1"), "q.us-east-1.amazonaws.com");
        assert_eq!(aws_host("eu-central-1"), "q.eu-central-1.amazonaws.com");
    }

    #[test]
    fn test_endpoint_name() {
        assert_eq!(AwsEndpoint::new().name(), "aws");
    }
}
