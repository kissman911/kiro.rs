## 2026.7.3-kiss.2 — configurable-cooldown

- 新增 `config.suspiciousCooldownSeconds` 配置项，控制 suspicious activity 429 触发后的运行时冷却秒数。
- 默认冷却从硬编码 30 分钟改为 10 分钟（600 秒）。
- 配置值为 0 时回退到默认 600 秒，避免误配成完全不冷却。

## 2026.7.3-kiss.1 — manual-cooldown-clear

- Admin API 新增 `POST /api/admin/credentials/:id/clear-cooldown`，可手动清除凭据的运行时风控冷却状态。
- Admin UI 在“冷却中”的凭据卡片显示“退出冷却”按钮，操作后立即刷新凭据列表。
- “重置失败”现在也会同时清除运行时冷却状态，避免凭据仍被调度排除。

# KissAPI kiro-rs 二次开发版本记录

这个文件记录 KissAPI 分支每一次面向生产的二次开发版本。每次功能更新或生产镜像发布前，先新增一条记录，再提交代码。

## 2026.5.18-kiss.2 — cooldown-api-pass-through

- 修复 Admin Service 层漏传 `cooldownUntil` / `cooldownRemainingSeconds` 的问题。
- 顶部“可用凭据”已按冷却扣减，但凭据卡片此前无法显示冷却 badge；本版让前端能收到并展示冷却状态。

## 2026.5.18-kiss.1 — cooldown-visibility

- Admin API 凭据快照新增 `cooldownUntil` 与 `cooldownRemainingSeconds` 字段，用于展示 suspicious activity 429 的运行时冷却状态。
- Admin UI 凭据卡片新增“冷却中”徽标，并显示剩余时间与冷却截止时间。
- 冷却状态仍只保存在内存中，不写回 credentials.json，也不等同于永久禁用。

## 2026.5.16-kiss.1 — credential-card-layout

- Admin UI 凭据卡片标题区重新排版，启用开关独立成状态行，避免被长标签挤压遮挡。
- 凭据卡片底部操作按钮改为稳定 2 列布局；删除按钮独占整行，窄卡片下不再重叠或截断。
- 按钮文字居中并使用响应式字号，图标增加 `shrink-0` 防止挤压文案。
- 已先灰度到 `kirors-c`，确认正常后部署到 DMIT2 生产 `kirors-a`。

## 2026.5.15-kiss.2 — credential-owner-email

- Admin UI 凭据卡片新增“邮箱 / 归属”展示区，明确显示每条凭据归属。
- 没有邮箱时显示“未识别邮箱”，并提示可通过 KAM 导入带 email 的凭据或刷新 Token 尝试自动识别。
- Token 刷新后会从 refresh 响应与 Kiro `ListAvailableProfiles` 响应中递归提取 email / emailAddress / userEmail 等字段，并写回凭据文件。
- 保留导入时传入的 email 字段，避免自动解析覆盖人工标注。

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
