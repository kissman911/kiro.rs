# KissAPI kiro-rs 二次开发版本记录

这个文件记录 KissAPI 分支每一次面向生产的二次开发版本。每次功能更新或生产镜像发布前，先新增一条记录，再提交代码。

## 2026.5.15-kiss.1 — overage-autodisable

- 当 Kiro 上游返回 `402 Payment Required` 且错误体包含 `OVERAGE_REQUEST...` 或 `You have reached the limit for overages` 时，自动判定该凭据超额额度已耗尽。
- 复用 `QuotaExceeded` 禁用路径：立即禁用触发请求的对应凭据，并切换到下一条可用凭据继续重试。
- 支持识别 NewAPI/上游包装后的错误文本，例如 `status_code=502 ... 402 Payment Required {...}`。
- 保留原 `MONTHLY_REQUEST_COUNT` 自动禁用逻辑。

## 2026.5.14-kiss.2 — version-badge

- 新增 `VERSION.json` 作为 KissAPI 二次开发分支当前版本源。
- 新增 `docs/KISSAPI_CHANGELOG.md` 记录每次生产迭代。
- 新增 Admin API：`GET /api/admin/version`。
- Admin UI 右上角显示当前二开版本号、代号，并在 hover 时展示摘要、Git SHA 与构建 tag。
- Docker/GitHub Actions 构建时注入 `KISSAPI_BUILD_TAG` 与 `KISSAPI_GIT_SHA`，便于追踪镜像来源。

## 2026.5.14-kiss.1 — profilearn-runtime

- 迁移 Kiro 旧 `q.<region>.amazonaws.com` 调用到新 `runtime.<region>.kiro.dev` / `management.<region>.kiro.dev`。
- 请求体自动注入 `profileArn`。
- IdC 凭据刷新 token 后若缺少 `profileArn`，自动调用 `ListAvailableProfiles` 解析并写回。
- 修复 Opus 4.7 重复测试名导致全量测试无法编译的问题。
- 已灰度到 `kirors-c`，再上线到 DMIT2 生产 `kirors-a`。

## 2026.5.9-kiss.1 — opus47-overage-baseline

- 基于上游 Opus 4.7 支持，保留 KissAPI 二开功能。
- 生产入口 `kirors-a` 曾运行镜像 `ghcr.io/kissman911/kiro-rs:beta-eec3a6`。
