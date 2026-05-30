//! KissAPI 二次开发版本信息

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionInfo {
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

pub fn app_version_info() -> AppVersionInfo {
    let raw = option_env!("KISSAPI_VERSION_JSON")
        .unwrap_or("{}")
        .replace("\\n", "\n");
    let value = serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null);

    let build_tag = option_env!("KISSAPI_BUILD_TAG")
        .unwrap_or("local")
        .to_string();
    let git_sha = option_env!("KISSAPI_GIT_SHA")
        .unwrap_or("unknown")
        .to_string();
    let version_from_file = read_string(&value, "version", env!("CARGO_PKG_VERSION"));
    let codename_from_file = read_string(&value, "codename", "local");

    AppVersionInfo {
        // Prefer the immutable Docker/image build tag for beta/CI images so the Admin UI
        // automatically changes on every container update. Release images can still show
        // VERSION.json when no build tag is injected.
        version: if build_tag == "local" {
            version_from_file
        } else {
            build_tag.clone()
        },
        channel: read_string(&value, "channel", "kissapi"),
        codename: if build_tag == "local" {
            codename_from_file
        } else {
            let short_sha: String = git_sha.chars().take(7).collect();
            if short_sha.is_empty() || short_sha == "unknown" {
                "container-build".to_string()
            } else {
                format!("container-build {}", short_sha)
            }
        },
        date: read_string(&value, "date", "unknown"),
        summary: read_string(&value, "summary", "KissAPI kiro-rs custom build"),
        package_version: env!("CARGO_PKG_VERSION").to_string(),
        git_sha,
        build_tag,
        changelog: option_env!("KISSAPI_CHANGELOG")
            .unwrap_or("")
            .replace("\\n", "\n"),
    }
}

fn read_string(value: &Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::app_version_info;

    #[test]
    fn app_version_has_kissapi_metadata() {
        let info = app_version_info();
        assert!(!info.version.is_empty());
        assert_eq!(info.channel, "kissapi");
        assert!(!info.changelog.is_empty());
    }
}
