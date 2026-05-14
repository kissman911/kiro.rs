use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=VERSION.json");
    println!("cargo:rerun-if-changed=docs/KISSAPI_CHANGELOG.md");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
    let version_json_path = Path::new(&manifest_dir).join("VERSION.json");
    let changelog_path = Path::new(&manifest_dir).join("docs/KISSAPI_CHANGELOG.md");

    let version_json = fs::read_to_string(&version_json_path).unwrap_or_else(|_| "{}".to_string());
    let changelog = fs::read_to_string(&changelog_path).unwrap_or_default();

    println!(
        "cargo:rustc-env=KISSAPI_VERSION_JSON={}",
        escape_env(&version_json)
    );
    println!(
        "cargo:rustc-env=KISSAPI_CHANGELOG={}",
        escape_env(&changelog)
    );
    println!(
        "cargo:rustc-env=KISSAPI_GIT_SHA={}",
        env::var("KISSAPI_GIT_SHA")
            .unwrap_or_else(|_| short_git_sha().unwrap_or_else(|| "unknown".to_string()))
    );
    println!(
        "cargo:rustc-env=KISSAPI_BUILD_TAG={}",
        env::var("KISSAPI_BUILD_TAG").unwrap_or_else(|_| "local".to_string())
    );
}

fn escape_env(value: &str) -> String {
    value.replace('\n', "\\n").replace('\r', "")
}

fn short_git_sha() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
